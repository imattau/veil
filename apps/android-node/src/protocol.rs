use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;

use veil_core::tags::derive_feed_tag;
use veil_core::{Epoch, Namespace};
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
use veil_crypto::signing::NostrSigner;
use veil_crypto::signing::NostrVerifier;
use veil_fec::profile::ErasureCodingMode;
use veil_node::batch::FeedBatcher;
use veil_node::config::{BloomExchangeConfig, NodeRuntimeConfig, ProbabilisticForwardingConfig};
use veil_node::policy::LocalWotPolicy;
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::{
    pump_multi_lane_tick_with_config_resolvers_split, ConfigMultiLanePumpParams, RuntimeStats,
};
use veil_node::service::{PublisherRuntime, PublisherTickInput};

use crate::adapters::{FallbackAdapter, FastAdapter, LaneAdapter, LaneSnapshot, MultiLaneAdapter};
use crate::api::{LaneDetail, LaneStats};
use crate::discovery::discovery_tag;
use veil_codec::object::decode_object_cbor_prefix;
use veil_codec::shard::decode_shard_cbor;
use veil_core::ObjectRoot;
use veil_fec::sharder::{derive_object_root, reconstruct_object_padded_with_mode};
use veil_node::persistence::load_state_or_default;

mod dynamic_peers;
use self::dynamic_peers::DynamicPeerStore;

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
    inner: Arc<
        Mutex<PublisherRuntime<FastAdapter, FallbackAdapter, XChaCha20Poly1305Cipher, NostrSigner>>,
    >,
    config: ProtocolConfig,
    steps: Arc<AtomicU64>,
    runtime_stats: Arc<Mutex<RuntimeStats>>,
    verifier: NostrVerifier,
    identity_pubkey: Arc<Mutex<[u8; 32]>>,
    dynamic_peers: Arc<Mutex<DynamicPeerStore>>,
}

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
        let mut runtime = self.inner.lock().await;
        runtime.enqueue(payload);
        let step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;
        let (fast_peers, fallback_peers) = self.publish_peer_lists().await?;
        runtime
            .tick(PublisherTickInput {
                namespace,
                epoch: current_epoch(),
                tag,
                now_step: step,
                flags: 0,
                interactive_flush: true,
                fast_peers: &fast_peers,
                fallback_peers: &fallback_peers,
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
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
        let mut runtime = self.inner.lock().await;
        for payload in payloads {
            runtime.enqueue(payload);
        }
        let step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;
        let (fast_peers, fallback_peers) = self.publish_peer_lists().await?;
        runtime
            .tick(PublisherTickInput {
                namespace,
                epoch: current_epoch(),
                tag,
                now_step: step,
                flags: 0,
                interactive_flush: false,
                fast_peers: &fast_peers,
                fallback_peers: &fallback_peers,
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub async fn publish_encoded_object(&self, encoded_object: Vec<u8>) -> Result<(), String> {
        let step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;
        let (fast_peers, fallback_peers) = self.publish_peer_lists().await?;
        let mut runtime = self.inner.lock().await;
        let PublisherRuntime {
            state,
            fast_adapter,
            fallback_adapter,
            config,
            ..
        } = &mut *runtime;

        veil_node::publish::publish_encoded_object_multi_lane(
            state,
            fast_adapter,
            fallback_adapter,
            &encoded_object,
            &fast_peers,
            &fallback_peers,
            step,
            config,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    pub async fn inject_object(&self, encoded_object: Vec<u8>) -> Result<ObjectRoot, String> {
        let mut runtime = self.inner.lock().await;
        let wire_root = veil_fec::sharder::derive_object_root(&encoded_object);
        let object =
            veil_codec::object::decode_object_cbor(&encoded_object).map_err(|e| e.to_string())?;

        let erasure_mode = runtime.config.erasure_mode_for_namespace(object.namespace);
        let shards = veil_fec::sharder::object_to_shards_with_mode_and_padding(
            &encoded_object,
            object.namespace,
            object.epoch,
            object.tag,
            wire_root,
            erasure_mode,
            runtime.config.bucket_jitter_extra_levels,
        )
        .map_err(|e| e.to_string())?;

        for shard in shards {
            let sid = veil_fec::sharder::shard_id(&shard).map_err(|e| e.to_string())?;
            let bytes = veil_codec::shard::encode_shard_cbor(&shard).map_err(|e| e.to_string())?;
            veil_node::cache::cache_put(&mut runtime.state, sid, bytes, 0, 1000);
        }

        Ok(wire_root)
    }

    pub async fn publish_discovery(
        &self,
        msg: crate::discovery::DiscoveryMessage,
    ) -> Result<(), String> {
        let payload = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
        let namespace = self.config.discovery_namespace;
        let tag = discovery_tag(namespace);
        self.publish_with_tag(payload, namespace, tag).await
    }

    pub async fn subscribe_pubkey(&self, pubkey: [u8; 32], namespace: Namespace) {
        let tag = derive_feed_tag(&pubkey, namespace);
        let mut runtime = self.inner.lock().await;
        runtime.state.subscriptions.insert(tag);
    }

    pub async fn subscribe_tag(&self, tag: [u8; 32]) {
        let mut runtime = self.inner.lock().await;
        runtime.state.subscriptions.insert(tag);
    }

    pub async fn has_subscription(&self, tag: [u8; 32]) -> bool {
        let runtime = self.inner.lock().await;
        runtime.state.subscriptions.contains(&tag)
    }

    pub async fn sync_subscriptions(
        &self,
        channels: &[String],
        contacts: &[crate::api::ContactBundle],
    ) {
        let mut tags = Vec::new();
        let my_pubkey = *self.identity_pubkey.lock().await;

        // 1. Add discovery tag
        tags.push(discovery_tag(self.config.discovery_namespace));

        // 2. For each channel name, derive tags for self and all contacts
        for channel in channels {
            // If it looks like a hex tag, add it directly
            if let Some(tag) = decode_hex_32(channel) {
                tags.push(tag);
                continue;
            }

            // Otherwise treat as channel name
            tags.push(veil_core::tags::derive_channel_feed_tag(
                &my_pubkey,
                self.config.namespace,
                channel,
            ));
            for contact in contacts {
                if let Some(pubkey) = decode_hex_32(&contact.pubkey_hex) {
                    tags.push(veil_core::tags::derive_channel_feed_tag(
                        &pubkey,
                        self.config.namespace,
                        channel,
                    ));
                }
            }
        }

        let mut runtime = self.inner.lock().await;
        runtime.state.subscriptions.clear();
        for tag in tags {
            runtime.state.subscriptions.insert(tag);
        }
    }

    pub async fn update_identity(&self, pubkey: [u8; 32], signer: NostrSigner) {
        let mut runtime = self.inner.lock().await;
        runtime.signer = Some(signer);
        let mut guard = self.identity_pubkey.lock().await;
        *guard = pubkey;
    }

    pub async fn update_wot_policy(&self, policy: LocalWotPolicy) {
        let mut runtime = self.inner.lock().await;
        runtime.config.wot_policy = policy;
    }

    pub async fn build_object(
        &self,
        item: Vec<u8>,
        namespace: u16,
        flags: u16,
    ) -> Result<(Vec<u8>, ObjectRoot), String> {
        let runtime = self.inner.lock().await;
        let items = vec![item];
        let mut payload = Vec::new();
        ciborium::ser::into_writer(&items, &mut payload).map_err(|e| e.to_string())?;

        let now_step = self.steps.load(Ordering::Relaxed);
        let epoch = current_epoch();
        let pubkey = *self.identity_pubkey.lock().await;
        let tag = derive_feed_tag(&pubkey, Namespace(namespace));

        let encoded_object = veil_node::publish::build_encoded_object(
            &payload,
            Namespace(namespace),
            epoch,
            tag,
            &runtime.encrypt_key,
            now_step,
            flags | veil_codec::object::OBJECT_FLAG_BATCHED,
            &XChaCha20Poly1305Cipher,
            runtime.signer.as_ref(),
        )
        .map_err(|e| e.to_string())?;

        let wire_root = veil_fec::sharder::derive_object_root(&encoded_object);
        Ok((encoded_object, wire_root))
    }

    pub async fn add_contact(&self, contact: &crate::api::ContactBundle) {
        self.dynamic_peers.lock().await.add_contact(contact);
    }

    pub async fn sync_contacts(&self, contacts: &[crate::api::ContactBundle]) {
        self.dynamic_peers
            .lock()
            .await
            .replace_from_contacts(contacts);
    }

    pub async fn get_cached_shard(&self, shard_id: [u8; 32]) -> Option<Vec<u8>> {
        let runtime = self.inner.lock().await;
        runtime
            .state
            .cache
            .get(&shard_id)
            .map(|cached| cached.bytes.clone())
    }

    pub async fn reconstruct_object(&self, root: [u8; 32]) -> Option<Vec<u8>> {
        let runtime = self.inner.lock().await;
        let mut shards = Vec::new();
        if let Some(sids) = runtime.state.shard_index.get(&root) {
            for sid in sids {
                if let Some(cached) = runtime.state.cache.get(sid) {
                    if let Ok(shard) = decode_shard_cbor(&cached.bytes) {
                        shards.push(shard);
                    }
                }
            }
        }
        if shards.is_empty() {
            return None;
        }
        let mode = erasure_mode_from_shards(&shards, runtime.config.erasure_coding_mode);
        reconstruct_object_padded_with_mode(&shards, root, mode).ok()
    }

    pub async fn reconstruct_payload(&self, root: [u8; 32]) -> Option<Vec<u8>> {
        let runtime = self.inner.lock().await;
        let cipher = XChaCha20Poly1305Cipher;

        // 1. Try direct lookup by root (assume it's a wire_root or indexed content_root)
        let target_wire_roots = {
            let mut set = std::collections::HashSet::new();
            set.insert(root);
            if let Some(wire_root) = runtime.state.content_index.get(&root) {
                set.insert(*wire_root);
            }
            set
        };

        for target_root in target_wire_roots {
            if let Some(sids) = runtime.state.shard_index.get(&target_root) {
                let mut shards = Vec::new();
                for sid in sids {
                    if let Some(cached) = runtime.state.cache.get(sid) {
                        if let Ok(shard) = decode_shard_cbor(&cached.bytes) {
                            shards.push(shard);
                        }
                    }
                }
                if !shards.is_empty() {
                    let mode =
                        erasure_mode_from_shards(&shards, runtime.config.erasure_coding_mode);
                    if let Ok(reconstructed) =
                        reconstruct_object_padded_with_mode(&shards, target_root, mode)
                    {
                        if let Ok((object, _)) = decode_object_cbor_prefix(&reconstructed) {
                            let aad = build_veil_aad(object.tag, object.namespace, object.epoch);

                            // Try primary encrypt_key, fallback to public zero-key
                            let decrypted_payload = cipher
                                .decrypt(
                                    &runtime.encrypt_key,
                                    object.nonce,
                                    &aad,
                                    &object.ciphertext,
                                )
                                .or_else(|_| {
                                    cipher.decrypt(
                                        &[0u8; 32],
                                        object.nonce,
                                        &aad,
                                        &object.ciphertext,
                                    )
                                })
                                .ok();

                            if let Some(decrypted_payload) = decrypted_payload {
                                // Direct match on wire_root?
                                if target_root == root {
                                    return Some(decrypted_payload);
                                }

                                // Match on ObjectV1.object_root (the payload hash)
                                if object.object_root == root {
                                    if let Ok(batch) = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(
                                        decrypted_payload.as_slice(),
                                    ) {
                                        for item in batch {
                                            if derive_object_root(&item) == root {
                                                return Some(item);
                                            }
                                        }
                                    }
                                    return Some(decrypted_payload);
                                }

                                // Match on individual item inside a batch
                                if let Ok(batch) = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(
                                    decrypted_payload.as_slice(),
                                ) {
                                    for item in batch {
                                        if derive_object_root(&item) == root {
                                            return Some(item);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Fallback scan for content hash match (searching inside objects)
        let roots: Vec<_> = runtime.state.shard_index.keys().cloned().collect();
        for wire_root in roots {
            if wire_root == root {
                continue; // Already tried direct lookup
            }

            let mut shards = Vec::new();
            if let Some(sids) = runtime.state.shard_index.get(&wire_root) {
                for sid in sids {
                    if let Some(cached) = runtime.state.cache.get(sid) {
                        if let Ok(shard) = decode_shard_cbor(&cached.bytes) {
                            shards.push(shard);
                        }
                    }
                }
            }

            if shards.is_empty() {
                continue;
            }

            let mode = erasure_mode_from_shards(&shards, runtime.config.erasure_coding_mode);
            let reconstructed = match reconstruct_object_padded_with_mode(&shards, wire_root, mode)
            {
                Ok(value) => value,
                Err(_) => continue,
            };
            let (object, _) = match decode_object_cbor_prefix(&reconstructed) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let aad = build_veil_aad(object.tag, object.namespace, object.epoch);

            // Try primary encrypt_key, fallback to public zero-key
            let decrypted_payload = cipher
                .decrypt(&runtime.encrypt_key, object.nonce, &aad, &object.ciphertext)
                .or_else(|_| cipher.decrypt(&[0u8; 32], object.nonce, &aad, &object.ciphertext))
                .ok();

            if let Some(decrypted_payload) = decrypted_payload {
                // 2a. Match on ObjectV1.object_root (the payload hash)
                if object.object_root == root {
                    // It might still be a batch! If so, we need to check items.
                    if let Ok(batch) =
                        ciborium::de::from_reader::<Vec<Vec<u8>>, _>(decrypted_payload.as_slice())
                    {
                        for item in batch {
                            if derive_object_root(&item) == root {
                                return Some(item);
                            }
                        }
                    }
                    return Some(decrypted_payload);
                }

                // 2b. Fallback: match on individual item inside a batch
                if let Ok(batch) =
                    ciborium::de::from_reader::<Vec<Vec<u8>>, _>(decrypted_payload.as_slice())
                {
                    for item in batch {
                        if derive_object_root(&item) == root {
                            return Some(item);
                        }
                    }
                }
            }
        }

        None
    }

    pub async fn persist_cache_state(&self) {
        let path = match &self.config.cache_state_path {
            Some(path) => path.clone(),
            None => return,
        };
        let mut runtime = self.inner.lock().await;
        let _ = veil_node::persistence::save_state_to_path(path, &mut runtime.state);
    }

    pub async fn persist_state(&self) {
        // NodeState has its own persist method internally.
    }

    pub fn peer_id(&self) -> String {
        self.config.peer_id.clone()
    }

    pub fn ws_url(&self) -> Option<String> {
        self.config.ws_url.clone()
    }

    pub fn discovery_namespace(&self) -> Namespace {
        self.config.discovery_namespace
    }

    pub fn quic_bind_addr(&self) -> String {
        self.config.quic_bind_addr.clone()
    }

    pub async fn pump_inbound(&self) -> Result<Option<ReceiveEvent>, String> {
        let mut runtime = self.inner.lock().await;
        let mut stats = self.runtime_stats.lock().await;
        let (fast_peers, fallback_peers) = self.publish_peer_lists().await?;
        let cfg = self.config.runtime_config.clone();
        let PublisherRuntime {
            state,
            fast_adapter,
            fallback_adapter,
            encrypt_key,
            ..
        } = &mut *runtime;
        let dynamic = self.dynamic_peers.lock().await.peer_map().clone();
        let resolver = |peer: &String| {
            dynamic
                .get(peer)
                .copied()
                .or_else(|| cfg.publisher_for_peer(peer))
        };
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

    pub async fn lane_details(&self) -> Vec<LaneDetail> {
        let runtime = self.inner.lock().await;
        let mut details = Vec::new();
        details.extend(build_lane_details(
            "fast",
            runtime.fast_adapter.lane_snapshots(),
        ));
        details.extend(build_lane_details(
            "fallback",
            runtime.fallback_adapter.lane_snapshots(),
        ));
        details
    }

    async fn fast_peers(&self) -> Vec<String> {
        let mut peers = self.config.fast_peers.clone();
        let dynamic = self.dynamic_peers.lock().await;
        for peer in dynamic.fast_peers() {
            if !peers.contains(peer) {
                peers.push(peer.clone());
            }
        }
        peers
    }

    async fn fallback_peers(&self) -> Vec<String> {
        let mut peers = self.config.fallback_peers.clone();
        let dynamic = self.dynamic_peers.lock().await;
        for peer in dynamic.fallback_peers() {
            if !peers.contains(peer) {
                peers.push(peer.clone());
            }
        }
        peers
    }

    async fn publish_peer_lists(&self) -> Result<(Vec<String>, Vec<String>), String> {
        let fast = self.fast_peers().await;
        let mut fallback = self.fallback_peers().await;
        if fallback.is_empty() {
            if let Some(ws_url) = &self.config.ws_url {
                if !ws_url.trim().is_empty() {
                    fallback.push(ws_url.clone());
                }
            }
        }
        if fast.is_empty() && fallback.is_empty() {
            return Err("no peers configured for publish".to_string());
        }
        Ok((fast, fallback))
    }

    #[cfg(test)]
    pub(crate) async fn dynamic_peer_snapshot(&self) -> (Vec<String>, Vec<String>) {
        let dynamic = self.dynamic_peers.lock().await;
        let (fast, fallback, _) = dynamic.snapshots();
        (fast, fallback)
    }

    #[cfg(test)]
    pub(crate) async fn dynamic_peer_map_snapshot(
        &self,
    ) -> std::collections::HashMap<String, [u8; 32]> {
        let dynamic = self.dynamic_peers.lock().await;
        let (_, _, peer_map) = dynamic.snapshots();
        peer_map
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
    let mut cfg = NodeRuntimeConfig::default();
    cfg.probabilistic_forwarding = ProbabilisticForwardingConfig {
        enabled: true,
        min_probability: 0.20,
        replica_divisor: 8,
    };
    cfg.bloom_exchange = BloomExchangeConfig {
        enabled: true,
        interval_steps: 192,
        false_positive_rate: 0.05,
    };
    ProtocolConfig {
        ws_url: Some(ws_url),
        quic_bind_addr: "0.0.0.0:0".to_string(),
        quic_server_name: None,
        quic_trusted_certs: Vec::new(),
        tor_socks: None,
        peer_id,
        namespace: Namespace(namespace),
        discovery_namespace: Namespace(4096),
        encrypt_key,
        identity_pubkey,
        signer,
        fast_peers: Vec::new(),
        fallback_peers: Vec::new(),
        runtime_config: cfg,
        cache_state_path: None,
    }
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Epoch((now / 86_400) as u32)
}

fn derive_server_name(peer: &str) -> Option<String> {
    let trimmed = peer.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(url) = reqwest::Url::parse(trimmed) {
        return url.host_str().map(ToString::to_string);
    }
    if let Ok(addr) = trimmed.parse::<std::net::SocketAddr>() {
        return Some(addr.ip().to_string());
    }
    let host = trimmed.split(':').next().unwrap_or(trimmed);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    hex::decode_to_slice(value, &mut out).ok()?;
    Some(out)
}

fn is_ws_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value.trim()) else {
        return false;
    };
    matches!(url.scheme(), "ws" | "wss")
}

fn build_ws_fast(config: &ProtocolConfig) -> Result<LaneAdapter, String> {
    let ws_url = config
        .ws_url
        .clone()
        .ok_or_else(|| "missing WS url".to_string())?;
    let ws = crate::adapters::build_ws_adapter(ws_url, config.peer_id.clone())
        .map_err(|e| e.to_string())?;
    Ok(LaneAdapter::WebSocket(ws))
}

fn build_lane_details(role: &str, snapshots: Vec<LaneSnapshot>) -> Vec<LaneDetail> {
    snapshots
        .into_iter()
        .map(|snapshot| LaneDetail {
            role: role.to_string(),
            lane: snapshot.label.to_string(),
            connected: snapshot.connected,
            last_error: snapshot.health.last_error,
            last_error_code: snapshot.health.last_error_code,
            stats: LaneStats {
                outbound_queued: snapshot.health.outbound_queued,
                outbound_send_ok: snapshot.health.outbound_send_ok,
                outbound_send_err: snapshot.health.outbound_send_err,
                inbound_received: snapshot.health.inbound_received,
                inbound_dropped: snapshot.health.inbound_dropped,
                reconnect_attempts: snapshot.health.reconnect_attempts,
            },
        })
        .collect()
}

fn build_fast_adapter(config: &ProtocolConfig) -> Result<FastAdapter, String> {
    let mut lanes: Vec<LaneAdapter> = Vec::new();

    let server_name = config.quic_server_name.clone().or_else(|| {
        config
            .fast_peers
            .first()
            .and_then(|peer| derive_server_name(peer))
    });
    if let Some(name) = server_name {
        let bind_addr = config
            .quic_bind_addr
            .parse()
            .map_err(|_| "invalid QUIC bind addr")?;
        let quic =
            crate::adapters::build_quic_adapter(bind_addr, name, config.quic_trusted_certs.clone())
                .map_err(|e| e.to_string())?;
        lanes.push(LaneAdapter::Quic(quic));
    }

    // Fast lane only uses WebSocket if QUIC is not available
    if lanes.is_empty() && config.ws_url.is_some() {
        lanes.push(build_ws_fast(config)?);
    }

    if lanes.is_empty() {
        return Err("no fast lanes available".to_string());
    }

    Ok(MultiLaneAdapter::new(lanes))
}

fn build_fallback_adapter(config: &ProtocolConfig) -> Result<FallbackAdapter, String> {
    let mut lanes: Vec<LaneAdapter> = Vec::new();
    let mut seen_ws = std::collections::HashSet::new();

    // Fallback only uses WebSocket if it's NOT already in the fast lane
    let has_ws_in_fast = config.quic_server_name.is_none()
        && config.fast_peers.is_empty()
        && config.ws_url.is_some();

    if !has_ws_in_fast {
        // 1. Add explicitly configured primary ws_url
        if let Some(ws_url) = &config.ws_url {
            let ws_url = ws_url.trim();
            if !ws_url.is_empty() && is_ws_url(ws_url) {
                let ws =
                    crate::adapters::build_ws_adapter(ws_url.to_string(), config.peer_id.clone())
                        .map_err(|e| e.to_string())?;
                lanes.push(LaneAdapter::WebSocket(ws));
                seen_ws.insert(ws_url.to_string());
            }
        }

        // 2. Add any WebSocket URLs from fallback_peers (from WS_PEERS env var)
        for peer in &config.fallback_peers {
            let peer = peer.trim();
            if is_ws_url(peer) && !seen_ws.contains(peer) {
                let ws =
                    crate::adapters::build_ws_adapter(peer.to_string(), config.peer_id.clone())
                        .map_err(|e| e.to_string())?;
                lanes.push(LaneAdapter::WebSocket(ws));
                seen_ws.insert(peer.to_string());
            }
        }
    }

    if let Some(socks) = &config.tor_socks {
        let tor = crate::adapters::build_tor_adapter(socks.clone()).map_err(|e| e.to_string())?;
        lanes.push(LaneAdapter::Tor(tor));
    }

    if lanes.is_empty() {
        lanes.push(LaneAdapter::InMemory(
            veil_transport::adapter::InMemoryAdapter::default(),
        ));
    }

    Ok(MultiLaneAdapter::new(lanes))
}

#[cfg(test)]
mod tests {
    use super::{
        decode_hex_32, default_protocol_config, derive_server_name, erasure_mode_from_shards,
        is_ws_url, ProtocolEngine,
    };
    use crate::api::ContactBundle;
    use veil_core::types::NAMESPACE_PUBLIC_FEED;
    use veil_core::{Epoch, Namespace, ObjectRoot};
    use veil_crypto::signing::{NostrSigner, Signer};
    use veil_fec::profile::ErasureCodingMode;
    use veil_fec::sharder::{derive_object_root, object_to_shards_with_mode};

    #[test]
    fn default_protocol_config_enables_network_efficiency_policies() {
        let cfg = default_protocol_config(
            "ws://127.0.0.1:1/ws".to_string(),
            "peer-a".to_string(),
            32,
            [0x11; 32],
            [0xAA; 32],
            NostrSigner::from_secret([0x22; 32]).expect("valid nostr test key"),
        );

        assert!(cfg.runtime_config.probabilistic_forwarding.enabled);
        assert!(cfg.runtime_config.bloom_exchange.enabled);
        assert_eq!(
            cfg.runtime_config
                .erasure_mode_for_namespace(NAMESPACE_PUBLIC_FEED),
            ErasureCodingMode::Systematic
        );
    }

    #[test]
    fn reconstruct_mode_prefers_wire_header_mode() {
        let object = b"public-feed-systematic".to_vec();
        let root = derive_object_root(&object);
        let shards = object_to_shards_with_mode(
            &object,
            Namespace(32),
            Epoch(1),
            [0x44; 32],
            root,
            ErasureCodingMode::Systematic,
        )
        .expect("systematic shards");

        let mode = erasure_mode_from_shards(&shards, ErasureCodingMode::HardenedNonSystematic);
        assert_eq!(mode, ErasureCodingMode::Systematic);
    }

    #[test]
    fn derive_server_name_parses_urls_and_host_port_fallback() {
        assert_eq!(
            derive_server_name("quic://node.example:9443"),
            Some("node.example".to_string())
        );
        assert_eq!(
            derive_server_name("wss://relay.example/ws"),
            Some("relay.example".to_string())
        );
        assert_eq!(
            derive_server_name("127.0.0.1:9443"),
            Some("127.0.0.1".to_string())
        );
        assert_eq!(derive_server_name(""), None);
        assert_eq!(derive_server_name(":"), None);
    }

    #[test]
    fn is_ws_url_accepts_only_ws_and_wss() {
        assert!(is_ws_url("ws://relay.example/ws"));
        assert!(is_ws_url("wss://relay.example/ws"));
        assert!(!is_ws_url("https://relay.example/ws"));
        assert!(!is_ws_url("ws://"));
        assert!(!is_ws_url("relay.example"));
    }

    #[test]
    fn decode_hex_32_requires_exact_length_and_hex() {
        assert_eq!(decode_hex_32(&"aa".repeat(32)), Some([0xAA; 32]));
        assert!(decode_hex_32("aa").is_none());
        assert!(decode_hex_32(&"zz".repeat(32)).is_none());
    }

    #[tokio::test]
    async fn add_contact_normalizes_and_deduplicates_dynamic_endpoints() {
        let signer = NostrSigner::from_secret([0x33; 32]).expect("valid secret");
        let pubkey = signer.public_key();
        let protocol = ProtocolEngine::new(default_protocol_config(
            "ws://127.0.0.1:1/ws".to_string(),
            "node-a".to_string(),
            32,
            pubkey,
            [0xAA; 32],
            signer,
        ))
        .expect("protocol init");

        let contact = ContactBundle {
            peer_id: " peer-a ".to_string(),
            ws_url: Some(" ws://example.com/ws ".to_string()),
            quic_addr: Some(" 127.0.0.1:9444 ".to_string()),
            pubkey_hex: "11".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        };
        protocol.add_contact(&contact).await;

        let duplicate = ContactBundle {
            peer_id: "peer-a".to_string(),
            ws_url: Some("ws://example.com/ws".to_string()),
            quic_addr: Some("127.0.0.1:9444".to_string()),
            pubkey_hex: "11".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        };
        protocol.add_contact(&duplicate).await;

        let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
        let peer_map = protocol.dynamic_peer_map_snapshot().await;

        assert_eq!(fast, vec!["127.0.0.1:9444".to_string()]);
        assert_eq!(fallback, vec!["ws://example.com/ws".to_string()]);
        assert_eq!(peer_map.len(), 1);
        assert!(peer_map.contains_key("peer-a"));
    }

    #[tokio::test]
    async fn sync_contacts_filters_invalid_dynamic_entries() {
        let signer = NostrSigner::from_secret([0x44; 32]).expect("valid secret");
        let pubkey = signer.public_key();
        let protocol = ProtocolEngine::new(default_protocol_config(
            "ws://127.0.0.1:1/ws".to_string(),
            "node-b".to_string(),
            32,
            pubkey,
            [0xBB; 32],
            signer,
        ))
        .expect("protocol init");

        let too_long_endpoint = "x".repeat(1024 + 1);
        let contacts = vec![
            ContactBundle {
                peer_id: " peer-a ".to_string(),
                ws_url: Some(" ws://peer-a/ws ".to_string()),
                quic_addr: Some(" 127.0.0.1:9555 ".to_string()),
                pubkey_hex: "22".repeat(32),
                rpc_url: None,
                lan_addrs: Vec::new(),
            },
            ContactBundle {
                peer_id: " ".to_string(),
                ws_url: Some(too_long_endpoint.clone()),
                quic_addr: Some(too_long_endpoint),
                pubkey_hex: "33".repeat(32),
                rpc_url: None,
                lan_addrs: Vec::new(),
            },
        ];

        protocol.sync_contacts(&contacts).await;

        let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
        let peer_map = protocol.dynamic_peer_map_snapshot().await;

        assert_eq!(fast, vec!["127.0.0.1:9555".to_string()]);
        assert_eq!(fallback, vec!["ws://peer-a/ws".to_string()]);
        assert_eq!(peer_map.len(), 1);
        assert!(peer_map.contains_key("peer-a"));
    }

    #[tokio::test]
    async fn add_contact_caps_dynamic_peer_storage() {
        let signer = NostrSigner::from_secret([0x51; 32]).expect("valid secret");
        let pubkey = signer.public_key();
        let protocol = ProtocolEngine::new(default_protocol_config(
            "ws://127.0.0.1:1/ws".to_string(),
            "node-c".to_string(),
            32,
            pubkey,
            [0xCC; 32],
            signer,
        ))
        .expect("protocol init");

        for index in 0..(super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS + 8) {
            let contact = ContactBundle {
                peer_id: format!("peer-{index}"),
                ws_url: Some(format!("ws://peer-{index}.example/ws")),
                quic_addr: Some(format!("127.0.0.1:{}", 20_000 + index)),
                pubkey_hex: format!("{:064x}", index + 1),
                rpc_url: None,
                lan_addrs: Vec::new(),
            };
            protocol.add_contact(&contact).await;
        }

        let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
        let peer_map = protocol.dynamic_peer_map_snapshot().await;

        assert_eq!(fast.len(), super::dynamic_peers::MAX_DYNAMIC_FAST_PEERS);
        assert_eq!(
            fallback.len(),
            super::dynamic_peers::MAX_DYNAMIC_FALLBACK_PEERS
        );
        assert_eq!(
            peer_map.len(),
            super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS
        );
        assert!(!fast.contains(&format!(
            "127.0.0.1:{}",
            20_000 + super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS + 7
        )));
        assert!(!peer_map.contains_key(&format!(
            "peer-{}",
            super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS + 7
        )));
    }

    #[tokio::test]
    async fn media_round_trip_integration() {
        let signer = NostrSigner::from_secret([0x42; 32]).unwrap();
        let pubkey = signer.public_key();
        let key = [0xAA; 32];

        let cfg = default_protocol_config(
            "ws://127.0.0.1:1/ws".to_string(),
            "node-a".to_string(),
            32,
            pubkey,
            key,
            signer,
        );

        let protocol = ProtocolEngine::new(cfg).expect("protocol init");

        // 1. Build a mock media object (e.g. image bytes)
        let media_payload = b"this is a mock image payload".to_vec();
        let (encoded_object, wire_root): (Vec<u8>, ObjectRoot) = protocol
            .build_object(media_payload.clone(), 32, 0)
            .await
            .expect("build object");

        // 2. Inject it into local cache (simulating it being available after publish)
        let injected_root: ObjectRoot = protocol
            .inject_object(encoded_object)
            .await
            .expect("inject object");
        assert_eq!(injected_root, wire_root);

        // 3. Try to reconstruct by wire_root (fast path)
        let reconstructed_wire: Vec<u8> = protocol
            .reconstruct_payload(wire_root)
            .await
            .expect("reconstruct by wire_root");
        // Note: when matched by wire_root, it returns the whole payload (CBOR batch)
        assert!(reconstructed_wire.len() > media_payload.len());

        // 4. Try to reconstruct by media_payload hash (slow path / content match)
        let content_hash = veil_fec::sharder::derive_object_root(&media_payload);
        let reconstructed_content: Vec<u8> = protocol
            .reconstruct_payload(content_hash)
            .await
            .expect("reconstruct by content_hash");

        // Should return the UNWRAPPED media bytes
        assert_eq!(reconstructed_content, media_payload);
    }
}
