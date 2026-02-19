use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

use veil_core::tags::derive_feed_tag;
use veil_core::{Epoch, Namespace};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::NostrSigner;
use veil_crypto::signing::NostrVerifier;
use veil_fec::profile::ErasureCodingMode;
use veil_node::batch::FeedBatcher;
use veil_node::config::NodeRuntimeConfig;
use veil_node::policy::LocalWotPolicy;
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::RuntimeStats;
use veil_node::service::PublisherRuntime;

use crate::adapters::{FallbackAdapter, FastAdapter};
use crate::api::LaneDetail;
use crate::discovery::discovery_tag;
use veil_core::ObjectRoot;
use veil_node::persistence::load_state_or_default;

mod cache_reconstruction;
mod config_helpers;
mod dynamic_peers;
mod engine_methods;
mod inbound_pump;
mod lane_builders;
mod object_ops;
mod payload_reconstruction;
mod peer_lists;
mod peer_state;
mod publish_tick;
mod subscription_tags;
use self::cache_reconstruction::{cached_shard_bytes, reconstruct_cached_object};
use self::dynamic_peers::DynamicPeerStore;
use self::inbound_pump::pump_inbound_once;
use self::lane_builders::{build_fallback_adapter, build_fast_adapter, build_lane_details};
use self::object_ops::{
    build_batched_object, inject_encoded_object, publish_encoded_object_multi_lane,
};
use self::payload_reconstruction::reconstruct_payload_for_root;
use self::peer_lists::{finalize_publish_peer_lists, finalize_runtime_peer_lists};
use self::peer_state::{
    add_dynamic_contact, merged_dynamic_publish_peers, replace_dynamic_contacts,
};
#[cfg(test)]
use self::peer_state::{dynamic_peer_map_snapshot_from_store, dynamic_peer_snapshot_from_store};
use self::publish_tick::{enqueue_payload, enqueue_payload_batch, tick_publish};
use self::subscription_tags::build_subscription_tags;

#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    pub ws_url: Option<String>,
    pub quic_bind_addr: String,
    pub quic_server_name: Option<String>,
    pub quic_trusted_certs: Vec<Vec<u8>>,
    pub tor_socks: Option<String>,
    pub peer_id: String,
    pub namespace: Namespace,
    pub discovery_namespace: Namespace,
    pub encrypt_key: [u8; 32],
    pub identity_pubkey: [u8; 32],
    pub signer: NostrSigner,
    pub fast_peers: Vec<String>,
    pub fallback_peers: Vec<String>,
    pub runtime_config: NodeRuntimeConfig,
    pub cache_state_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct ProtocolEngine {
    inner: Arc<Mutex<ProtocolRuntime>>,
    config: ProtocolConfig,
    steps: Arc<AtomicU64>,
    runtime_stats: Arc<Mutex<RuntimeStats>>,
    verifier: NostrVerifier,
    identity_pubkey: Arc<Mutex<[u8; 32]>>,
    dynamic_peers: Arc<Mutex<DynamicPeerStore>>,
}

type ProtocolRuntime =
    PublisherRuntime<FastAdapter, FallbackAdapter, XChaCha20Poly1305Cipher, NostrSigner>;

impl ProtocolEngine {
    pub fn new(config: ProtocolConfig) -> Result<Self, String> {
        let identity_pubkey = config.identity_pubkey;
        let fast_adapter = build_fast_adapter(&config)?;
        let fallback_adapter = build_fallback_adapter(&config)?;
        let state = if let Some(path) = &config.cache_state_path {
            load_state_or_default(path).unwrap_or_default()
        } else {
            veil_node::state::NodeState::default()
        };
        let runtime = PublisherRuntime::new(
            state,
            FeedBatcher::default(),
            fast_adapter,
            fallback_adapter,
            config.runtime_config.clone(),
            config.encrypt_key,
            Some(config.signer.clone()),
            XChaCha20Poly1305Cipher,
        );
        let mut runtime = runtime;
        let tag = discovery_tag(config.discovery_namespace);
        runtime.state.subscriptions.insert(tag);
        Ok(Self {
            inner: Arc::new(Mutex::new(runtime)),
            config,
            steps: Arc::new(AtomicU64::new(0)),
            runtime_stats: Arc::new(Mutex::new(RuntimeStats::default())),
            verifier: NostrVerifier,
            identity_pubkey: Arc::new(Mutex::new(identity_pubkey)),
            dynamic_peers: Arc::new(Mutex::new(DynamicPeerStore::default())),
        })
    }

    pub async fn publish(&self, payload: Vec<u8>, namespace: Option<u16>) -> Result<(), String> {
        let namespace = Namespace(namespace.unwrap_or(self.config.namespace.0));
        let pubkey = *self.identity_pubkey.lock().await;
        let tag = derive_feed_tag(&pubkey, namespace);
        self.publish_with_tag(payload, namespace, tag).await
    }

    pub async fn publish_batch(
        &self,
        payloads: Vec<Vec<u8>>,
        namespace: Option<u16>,
    ) -> Result<(), String> {
        let namespace = Namespace(namespace.unwrap_or(self.config.namespace.0));
        let pubkey = *self.identity_pubkey.lock().await;
        let tag = derive_feed_tag(&pubkey, namespace);
        self.publish_batch_with_tag(payloads, namespace, tag).await
    }

    pub async fn publish_with_tag(
        &self,
        payload: Vec<u8>,
        namespace: Namespace,
        tag: [u8; 32],
    ) -> Result<(), String> {
        let (fast_peers, fallback_peers) = self.publish_peer_lists().await?;
        let mut runtime = self.inner.lock().await;
        enqueue_payload(&mut runtime, payload);
        let step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;
        tick_publish(
            &mut runtime,
            namespace,
            current_epoch(),
            tag,
            step,
            &fast_peers,
            &fallback_peers,
            true,
        )
    }

    pub async fn publish_batch_with_tag(
        &self,
        payloads: Vec<Vec<u8>>,
        namespace: Namespace,
        tag: [u8; 32],
    ) -> Result<(), String> {
        if payloads.is_empty() {
            return Ok(());
        }
        let (fast_peers, fallback_peers) = self.publish_peer_lists().await?;
        let mut runtime = self.inner.lock().await;
        enqueue_payload_batch(&mut runtime, payloads);
        let step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;
        tick_publish(
            &mut runtime,
            namespace,
            current_epoch(),
            tag,
            step,
            &fast_peers,
            &fallback_peers,
            false,
        )
    }

    pub async fn publish_encoded_object(&self, encoded_object: Vec<u8>) -> Result<(), String> {
        let step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;
        let (fast_peers, fallback_peers) = self.publish_peer_lists().await?;
        let mut runtime = self.inner.lock().await;
        publish_encoded_object_multi_lane(
            &mut runtime,
            &encoded_object,
            &fast_peers,
            &fallback_peers,
            step,
        )
    }

    pub async fn inject_object(&self, encoded_object: Vec<u8>) -> Result<ObjectRoot, String> {
        let mut runtime = self.inner.lock().await;
        inject_encoded_object(&mut runtime, &encoded_object)
    }
}

pub fn default_protocol_config(
    ws_url: String,
    peer_id: String,
    namespace: u16,
    identity_pubkey: [u8; 32],
    encrypt_key: [u8; 32],
    signer: NostrSigner,
) -> ProtocolConfig {
    config_helpers::build_default_protocol_config(
        ws_url,
        peer_id,
        namespace,
        identity_pubkey,
        encrypt_key,
        signer,
    )
}

fn erasure_mode_from_shards(
    shards: &[veil_codec::shard::ShardV1],
    fallback: ErasureCodingMode,
) -> ErasureCodingMode {
    shards
        .first()
        .map(|shard| match shard.header.erasure_mode {
            veil_codec::shard::ShardErasureMode::Systematic => ErasureCodingMode::Systematic,
            veil_codec::shard::ShardErasureMode::HardenedNonSystematic => {
                ErasureCodingMode::HardenedNonSystematic
            }
        })
        .unwrap_or(fallback)
}

fn current_epoch() -> Epoch {
    config_helpers::current_epoch()
}

fn derive_server_name(peer: &str) -> Option<String> {
    lane_builders::derive_server_name(peer)
}

fn is_ws_url(value: &str) -> bool {
    lane_builders::is_ws_url(value)
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    config_helpers::decode_hex_32(value)
}

#[cfg(test)]
mod tests;
