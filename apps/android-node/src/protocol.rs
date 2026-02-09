use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::RngCore;
use tokio::sync::Mutex;

use veil_core::tags::derive_feed_tag;
use veil_core::{Epoch, Namespace, Tag};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_crypto::signing::Ed25519Signer;
use veil_node::batch::FeedBatcher;
use veil_node::config::NodeRuntimeConfig;
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::{
    pump_multi_lane_tick_with_config_resolvers_split, ConfigMultiLanePumpParams, RuntimeStats,
};
use veil_node::service::{PublisherRuntime, PublisherTickInput};
use veil_transport::adapter::{InMemoryAdapter, TransportAdapter, TransportHealthSnapshot};
use veil_transport_websocket::{WebSocketAdapter, WebSocketAdapterConfig};

#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    pub ws_url: String,
    pub peer_id: String,
    pub namespace: Namespace,
    pub tag: Tag,
    pub encrypt_key: [u8; 32],
    pub fast_peers: Vec<String>,
    pub fallback_peers: Vec<String>,
    pub runtime_config: NodeRuntimeConfig,
}

#[derive(Clone)]
pub struct ProtocolEngine {
    inner: Arc<
        Mutex<
            PublisherRuntime<
                WebSocketAdapter,
                InMemoryAdapter,
                XChaCha20Poly1305Cipher,
                Ed25519Signer,
            >,
        >,
    >,
    config: ProtocolConfig,
    steps: Arc<AtomicU64>,
    runtime_stats: Arc<Mutex<RuntimeStats>>,
    verifier: Ed25519Verifier,
}

impl ProtocolEngine {
    pub fn new(config: ProtocolConfig) -> Result<Self, String> {
        let ws_config = WebSocketAdapterConfig::new(config.ws_url.clone(), config.peer_id.clone());
        let fast_adapter = WebSocketAdapter::connect(ws_config).map_err(|e| e.to_string())?;
        let fallback_adapter = InMemoryAdapter::default();
        let runtime = PublisherRuntime::new(
            veil_node::state::NodeState::default(),
            FeedBatcher::default(),
            fast_adapter,
            fallback_adapter,
            config.runtime_config.clone(),
            config.encrypt_key,
            None,
            XChaCha20Poly1305Cipher,
        );
        Ok(Self {
            inner: Arc::new(Mutex::new(runtime)),
            config,
            steps: Arc::new(AtomicU64::new(0)),
            runtime_stats: Arc::new(Mutex::new(RuntimeStats::default())),
            verifier: Ed25519Verifier::default(),
        })
    }

    pub async fn publish(&self, payload: Vec<u8>) -> Result<(), String> {
        let mut runtime = self.inner.lock().await;
        runtime.enqueue(payload);
        let step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;
        let fast_peers = if self.config.fast_peers.is_empty() {
            vec!["peer".to_string()]
        } else {
            self.config.fast_peers.clone()
        };
        let fallback_peers = if self.config.fallback_peers.is_empty() {
            fast_peers.clone()
        } else {
            self.config.fallback_peers.clone()
        };
        runtime
            .tick(PublisherTickInput {
                namespace: self.config.namespace,
                epoch: current_epoch(),
                tag: self.config.tag,
                now_step: step,
                flags: 0,
                interactive_flush: true,
                fast_peers: &fast_peers,
                fallback_peers: &fallback_peers,
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub async fn pump_inbound(&self) -> Result<Option<ReceiveEvent>, String> {
        let mut runtime = self.inner.lock().await;
        let mut stats = self.runtime_stats.lock().await;
        let fast_peers = if self.config.fast_peers.is_empty() {
            vec!["peer".to_string()]
        } else {
            self.config.fast_peers.clone()
        };
        let fallback_peers = if self.config.fallback_peers.is_empty() {
            fast_peers.clone()
        } else {
            self.config.fallback_peers.clone()
        };
        let cfg = self.config.runtime_config.clone();
        let PublisherRuntime {
            state,
            fast_adapter,
            fallback_adapter,
            encrypt_key,
            ..
        } = &mut *runtime;
        let resolver = |peer: &String| cfg.publisher_for_peer(peer);
        let event = pump_multi_lane_tick_with_config_resolvers_split(
            state,
            fast_adapter,
            fallback_adapter,
            ConfigMultiLanePumpParams {
                fast_peers: &fast_peers,
                fallback_peers: &fallback_peers,
                now_step: self.steps.fetch_add(1, Ordering::Relaxed) + 1,
                decrypt_key: encrypt_key,
                config: &cfg,
                stats: &mut stats,
            },
            &resolver,
            &resolver,
            &XChaCha20Poly1305Cipher,
            &self.verifier,
        )
        .map_err(|e| e.to_string())?;
        Ok(event)
    }

    pub async fn health_snapshot(&self) -> (TransportHealthSnapshot, TransportHealthSnapshot) {
        let runtime = self.inner.lock().await;
        let fast = runtime.fast_adapter.health_snapshot();
        let fallback = runtime.fallback_adapter.health_snapshot();
        (fast, fallback)
    }
}

pub fn default_protocol_config(ws_url: String, peer_id: String, namespace: u16) -> ProtocolConfig {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    let mut pubkey = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut pubkey);
    let cfg = NodeRuntimeConfig::default();
    ProtocolConfig {
        ws_url,
        peer_id,
        namespace: Namespace(namespace),
        tag: derive_feed_tag(&pubkey, Namespace(namespace)),
        encrypt_key: key,
        fast_peers: Vec::new(),
        fallback_peers: Vec::new(),
        runtime_config: cfg,
    }
}

fn current_epoch() -> Epoch {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Epoch((now / 86_400) as u32)
}
