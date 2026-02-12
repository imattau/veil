use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use rand::RngCore;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api::{
    CacheStatus, ContactBundle, EventEnvelope, LaneDetail, LaneHealth, LaneStatus, PublishRequest,
    QueueStatus, StatusResponse,
};
use crate::discovery::{DiscoveryStateHandle, DiscoveryTable};
use crate::state_store::{IdentityRecord, QueueItem, StateStore, StoreSnapshot};
use veil_crypto::signing::{NostrSigner, Signer};
use veil_node::policy::{
    parse_endorsement_payload, EndorsementIngestResult, LocalWotPolicy, WotConfig, WotSummary,
};
use veil_schema_feed::FeedBundle;

#[derive(Debug, Clone)]
pub struct NodeState {
    inner: Arc<Mutex<StateInner>>,
}

#[derive(Debug)]
struct StateInner {
    node_id: String,
    version: String,
    queue_pending: u64,
    queue_inflight: u64,
    queue_failed: u64,
    cache_entries: u64,
    cache_bytes: u64,
    quic: LaneHealth,
    websocket: LaneHealth,
    tor: LaneHealth,
    lane_details: Vec<LaneDetail>,
    subscriptions: HashSet<String>,
    events: broadcast::Sender<EventEnvelope>,
    event_seq: u64,
    event_buffer: VecDeque<EventEnvelope>,
    store: Option<StateStore>,
    queue: VecDeque<QueueItem>,
    queue_attempts: HashMap<Uuid, u32>,
    queue_next_attempt: HashMap<Uuid, u64>,
    identity: NodeIdentity,
    wot_policy: LocalWotPolicy,
    contacts: Vec<ContactBundle>,
    discovery: DiscoveryStateHandle,
}

#[derive(Debug, Clone)]
pub struct NodeIdentity {
    pub public_key: [u8; 32],
    pub secret_key: [u8; 32],
}

impl NodeIdentity {
    pub fn signer(&self) -> NostrSigner {
        NostrSigner::from_secret(self.secret_key).expect("stored identity secret must be valid")
    }

    pub fn public_key_hex(&self) -> String {
        hex_encode(&self.public_key)
    }

    pub fn to_record(&self) -> IdentityRecord {
        IdentityRecord {
            public_key_hex: self.public_key_hex(),
            secret_key_hex: hex_encode(&self.secret_key),
        }
    }
}

impl NodeState {
    pub fn new(version: impl Into<String>) -> Self {
        Self::new_with_store(version, None)
    }

    pub fn new_with_store(version: impl Into<String>, store_path: Option<PathBuf>) -> Self {
        let (events, _) = broadcast::channel(128);
        let store = store_path.map(StateStore::new);
        let snapshot = store.as_ref().map(StateStore::load).unwrap_or_default();
        let identity = snapshot
            .identity
            .as_ref()
            .and_then(parse_identity)
            .unwrap_or_else(generate_identity);
        let queue_pending = snapshot.queue.len() as u64;
        let wot_policy = snapshot
            .policy_json
            .as_deref()
            .and_then(|json| LocalWotPolicy::import_json(json).ok())
            .unwrap_or_default();
        let contacts = snapshot.contacts.clone();
        let subscriptions: HashSet<String> = snapshot.subscriptions.iter().cloned().collect();
        let event_buffer: VecDeque<EventEnvelope> = VecDeque::from(snapshot.feed_history.clone());
        let event_seq = event_buffer.iter().map(|e| e.seq).max().unwrap_or(0);

        let discovery_table = Arc::new(Mutex::new(DiscoveryTable::default()));
        {
            let mut table = discovery_table.lock().expect("discovery lock");
            for contact in &contacts {
                table.upsert(contact.clone());
            }
        }
        if let Some(store) = &store {
            if snapshot.identity.is_none() {
                store.persist(&StoreSnapshot {
                    queue: snapshot.queue.clone(),
                    identity: Some(identity.to_record()),
                    policy_json: wot_policy.export_json().ok(),
                    contacts: contacts.clone(),
                    feed_history: event_buffer.iter().cloned().collect(),
                    subscriptions: subscriptions.iter().cloned().collect(),
                });
            }
        }
        Self {
            inner: Arc::new(Mutex::new(StateInner {
                node_id: Uuid::new_v4().to_string(),
                version: version.into(),
                queue_pending,
                queue_inflight: 0,
                queue_failed: 0,
                cache_entries: 0,
                cache_bytes: 0,
                quic: LaneHealth::default(),
                websocket: LaneHealth::default(),
                tor: LaneHealth::default(),
                lane_details: Vec::new(),
                subscriptions,
                events,
                event_seq,
                event_buffer,
                store,
                queue: VecDeque::from(snapshot.queue),
                queue_attempts: HashMap::new(),
                queue_next_attempt: HashMap::new(),
                identity,
                wot_policy,
                contacts,
                discovery: DiscoveryStateHandle::new(discovery_table),
            })),
        }
    }

    pub fn status(&self) -> StatusResponse {
        let inner = self.inner.lock().expect("state lock");
        status_from_inner(&inner)
    }

    pub fn identity(&self) -> NodeIdentity {
        let inner = self.inner.lock().expect("state lock");
        inner.identity.clone()
    }

    pub fn rotate_identity(&self) -> NodeIdentity {
        let mut inner = self.inner.lock().expect("state lock");
        let identity = generate_identity();
        inner.identity = identity.clone();
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
        identity
    }

    pub fn enqueue_publish(&self, request: PublishRequest) -> Uuid {
        let mut inner = self.inner.lock().expect("state lock");
        let message_id = Uuid::new_v4();
        inner.queue.push_back(QueueItem {
            id: message_id,
            namespace: request.namespace,
            payload: request.payload,
        });
        update_queue_counts(&mut inner);
        let pending = inner.queue_pending;
        emit_event_locked(
            &mut inner,
            "publish_queued",
            serde_json::json!({
                "message_id": message_id,
                "pending": pending,
            }),
        );
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
        message_id
    }

    pub fn take_next_queued(&self, now_ms: u64) -> Option<QueueItem> {
        let mut inner = self.inner.lock().expect("state lock");
        let index = inner.queue.iter().position(|item| {
            inner
                .queue_next_attempt
                .get(&item.id)
                .copied()
                .map(|next| now_ms >= next)
                .unwrap_or(true)
        })?;
        let item = inner.queue.remove(index)?;
        inner.queue_inflight = inner.queue_inflight.saturating_add(1);
        let attempts = inner.queue_attempts.entry(item.id).or_insert(0);
        *attempts += 1;
        update_queue_counts(&mut inner);
        Some(item)
    }

    pub fn take_next_queued_batch(
        &self,
        now_ms: u64,
        max_items: usize,
        target_batch_bytes: usize,
        max_item_bytes: usize,
    ) -> Vec<QueueItem> {
        if max_items == 0 {
            return Vec::new();
        }
        let mut inner = self.inner.lock().expect("state lock");
        let first_index = match inner.queue.iter().position(|item| {
            inner
                .queue_next_attempt
                .get(&item.id)
                .copied()
                .map(|next| now_ms >= next)
                .unwrap_or(true)
        }) {
            Some(index) => index,
            None => return Vec::new(),
        };

        let mut batch = Vec::with_capacity(max_items);
        let first = match inner.queue.remove(first_index) {
            Some(item) => item,
            None => return Vec::new(),
        };
        let namespace = first.namespace;
        let mut total_bytes = first.payload.len();
        inner.queue_inflight = inner.queue_inflight.saturating_add(1);
        let attempts = inner.queue_attempts.entry(first.id).or_insert(0);
        *attempts += 1;
        batch.push(first);

        // Only aggregate additional small items. Large payloads still flow as
        // single-item publishes to avoid starving queue order.
        if total_bytes <= max_item_bytes && target_batch_bytes > total_bytes {
            let mut index = 0usize;
            while index < inner.queue.len()
                && batch.len() < max_items
                && total_bytes < target_batch_bytes
            {
                let due = inner
                    .queue_next_attempt
                    .get(&inner.queue[index].id)
                    .copied()
                    .map(|next| now_ms >= next)
                    .unwrap_or(true);
                if !due {
                    index += 1;
                    continue;
                }
                let candidate = &inner.queue[index];
                if candidate.namespace != namespace {
                    index += 1;
                    continue;
                }
                let item_len = candidate.payload.len();
                if item_len > max_item_bytes || total_bytes + item_len > target_batch_bytes {
                    index += 1;
                    continue;
                }
                let item = match inner.queue.remove(index) {
                    Some(item) => item,
                    None => continue,
                };
                total_bytes += item.payload.len();
                inner.queue_inflight = inner.queue_inflight.saturating_add(1);
                let attempts = inner.queue_attempts.entry(item.id).or_insert(0);
                *attempts += 1;
                batch.push(item);
            }
        }

        update_queue_counts(&mut inner);
        batch
    }

    pub fn complete_success(&self, item: &QueueItem) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
        inner.queue_attempts.remove(&item.id);
        inner.queue_next_attempt.remove(&item.id);
        emit_event_locked(
            &mut inner,
            "publish_sent",
            serde_json::json!({ "message_id": item.id }),
        );
        update_queue_counts(&mut inner);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn complete_failure(&self, item: QueueItem, retry_after_ms: u64) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.queue_failed = inner.queue_failed.saturating_add(1);
        inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
        let message_id = item.id;
        let attempts = inner.queue_attempts.get(&message_id).copied().unwrap_or(0);
        let next_attempt = now_millis().saturating_add(retry_after_ms);
        inner.queue_next_attempt.insert(message_id, next_attempt);
        inner.queue.push_back(item);
        emit_event_locked(
            &mut inner,
            "publish_failed",
            serde_json::json!({
                "message_id": message_id,
                "attempts": attempts,
                "retry_after_ms": retry_after_ms,
            }),
        );
        update_queue_counts(&mut inner);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn drop_item(&self, item: &QueueItem) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
        inner.queue_attempts.remove(&item.id);
        inner.queue_next_attempt.remove(&item.id);
        emit_event_locked(
            &mut inner,
            "publish_failed",
            serde_json::json!({
                "message_id": item.id,
                "dropped": true,
            }),
        );
        update_queue_counts(&mut inner);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn attempts_for(&self, item: &QueueItem) -> u32 {
        let inner = self.inner.lock().expect("state lock");
        inner.queue_attempts.get(&item.id).copied().unwrap_or(0)
    }

    pub fn get_feed(&self, limit: usize) -> Vec<EventEnvelope> {
        let inner = self.inner.lock().expect("state lock");
        inner
            .event_buffer
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn get_subscriptions(&self) -> Vec<String> {
        let inner = self.inner.lock().expect("state lock");
        inner.subscriptions.iter().cloned().collect()
    }

    pub fn export_identity(&self) -> (String, String) {
        let inner = self.inner.lock().expect("state lock");
        (
            inner.identity.public_key_hex(),
            hex_encode(&inner.identity.secret_key),
        )
    }

    pub fn import_identity(&self, secret_key_hex: String) -> Result<NodeIdentity, String> {
        let sec_bytes = hex::decode(&secret_key_hex).map_err(|e| e.to_string())?;
        if sec_bytes.len() != 32 {
            return Err("secret key must be 32 bytes".to_string());
        }
        let mut secret_key = [0u8; 32];
        secret_key.copy_from_slice(&sec_bytes);
        let signer = NostrSigner::from_secret(secret_key)
            .map_err(|_| "secret key is not a valid Nostr secp256k1 secret".to_string())?;
        let public_key = signer.public_key();
        let identity = NodeIdentity {
            public_key,
            secret_key,
        };

        let mut inner = self.inner.lock().expect("state lock");
        inner.identity = identity.clone();
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
        Ok(identity)
    }

    pub fn policy_summary(&self) -> WotSummary {
        let inner = self.inner.lock().expect("state lock");
        inner.wot_policy.summary()
    }

    pub fn policy_config(&self) -> WotConfig {
        let inner = self.inner.lock().expect("state lock");
        inner.wot_policy.config
    }

    pub fn update_policy_config(&self, config: WotConfig) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.wot_policy.update_config(config);
        self.persist_policy_locked(&mut inner);
    }

    pub fn trust_pubkey(&self, pubkey: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.wot_policy.trust(pubkey);
        self.persist_policy_locked(&mut inner);
    }

    pub fn untrust_pubkey(&self, pubkey: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.wot_policy.untrust(pubkey);
        self.persist_policy_locked(&mut inner);
    }

    pub fn mute_pubkey(&self, pubkey: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.wot_policy.mute(pubkey);
        self.persist_policy_locked(&mut inner);
    }

    pub fn unmute_pubkey(&self, pubkey: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.wot_policy.unmute(pubkey);
        self.persist_policy_locked(&mut inner);
    }

    pub fn block_pubkey(&self, pubkey: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.wot_policy.block(pubkey);
        self.persist_policy_locked(&mut inner);
    }

    pub fn unblock_pubkey(&self, pubkey: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.wot_policy.unblock(pubkey);
        self.persist_policy_locked(&mut inner);
    }

    pub fn wot_policy(&self) -> LocalWotPolicy {
        let inner = self.inner.lock().expect("state lock");
        inner.wot_policy.clone()
    }

    pub fn policy_lists(&self) -> crate::api::PolicyListsResponse {
        let inner = self.inner.lock().expect("state lock");
        let json = inner.wot_policy.export_json().ok();
        let Some(json) = json else {
            return crate::api::PolicyListsResponse::default();
        };
        let parsed = serde_json::from_str::<PolicyJsonLists>(&json).ok();
        let Some(parsed) = parsed else {
            return crate::api::PolicyListsResponse::default();
        };
        crate::api::PolicyListsResponse {
            trusted_pubkeys: parsed
                .trusted
                .iter()
                .map(|value| hex_encode(value))
                .collect(),
            muted_pubkeys: parsed.muted.iter().map(|value| hex_encode(value)).collect(),
            blocked_pubkeys: parsed
                .blocked
                .iter()
                .map(|value| hex_encode(value))
                .collect(),
        }
    }

    pub fn contacts(&self) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contacts.clone()
    }

    pub fn add_contact(&self, contact: ContactBundle) {
        let mut inner = self.inner.lock().expect("state lock");
        let upsert_contact: Option<ContactBundle>;
        if let Some(existing) = inner
            .contacts
            .iter_mut()
            .find(|existing| existing.peer_id == contact.peer_id)
        {
            if existing.ws_url.is_none() {
                existing.ws_url = contact.ws_url.clone();
            }
            if existing.quic_addr.is_none() {
                existing.quic_addr = contact.quic_addr.clone();
            }
            if existing.rpc_url.is_none() {
                existing.rpc_url = contact.rpc_url.clone();
            }
            if existing.pubkey_hex.is_empty() {
                existing.pubkey_hex = contact.pubkey_hex.clone();
            }
            for addr in &contact.lan_addrs {
                if !existing.lan_addrs.contains(addr) {
                    existing.lan_addrs.push(addr.clone());
                }
            }
            upsert_contact = Some(existing.clone());
        } else {
            inner.contacts.push(contact.clone());
            upsert_contact = Some(contact);
        }
        if let Some(contact) = upsert_contact {
            inner.discovery.upsert(contact);
        }
        self.persist_policy_locked(&mut inner);
    }

    pub fn set_contact(&self, contact: ContactBundle) {
        let mut inner = self.inner.lock().expect("state lock");
        if let Some(existing) = inner
            .contacts
            .iter_mut()
            .find(|existing| existing.peer_id == contact.peer_id)
        {
            *existing = contact.clone();
        } else {
            inner.contacts.push(contact.clone());
        }
        inner.discovery.upsert(contact);
        self.persist_policy_locked(&mut inner);
    }

    pub fn remove_contact(&self, peer_id: &str) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        let before = inner.contacts.len();
        inner.contacts.retain(|contact| contact.peer_id != peer_id);
        let removed = inner.contacts.len() != before;
        if removed {
            // Rebuild discovery table from remaining contacts to ensure removed peers
            // are no longer served by local lookup.
            let table = std::sync::Arc::new(std::sync::Mutex::new(DiscoveryTable::default()));
            {
                let mut guard = table.lock().expect("discovery lock");
                for contact in &inner.contacts {
                    guard.upsert(contact.clone());
                }
            }
            inner.discovery = DiscoveryStateHandle::new(table);
            self.persist_policy_locked(&mut inner);
        }
        removed
    }

    pub fn discovery_lookup_peer(&self, peer_id: &str, limit: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.discovery.lookup_peer(peer_id, limit)
    }

    pub fn discovery_lookup_pubkey(&self, pubkey_hex: &str, limit: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.discovery.lookup_pubkey(pubkey_hex, limit)
    }

    pub fn discovery_lookup_contact(
        &self,
        contact: &ContactBundle,
        limit: usize,
    ) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.discovery.lookup_contact(contact, limit)
    }

    pub fn discovery_sample(&self, max: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.discovery.sample(max)
    }

    pub fn mark_lane_health(&self, lane: &str, connected: bool, last_error: Option<String>) {
        let mut inner = self.inner.lock().expect("state lock");
        let target = match lane {
            "quic" => &mut inner.quic,
            "tor" => &mut inner.tor,
            _ => &mut inner.websocket,
        };
        target.connected = connected;
        target.last_error = last_error.clone();
        emit_event_locked(
            &mut inner,
            "lane_health",
            serde_json::json!({
                "lane": lane,
                "connected": connected,
                "last_error": last_error,
            }),
        );
    }

    pub fn mark_lane_details(&self, details: Vec<LaneDetail>) {
        let mut inner = self.inner.lock().expect("state lock");
        inner.lane_details = details.clone();
        inner.quic = LaneHealth::default();
        inner.websocket = LaneHealth::default();
        inner.tor = LaneHealth::default();

        for detail in details {
            let target = if detail.lane.contains("quic") {
                &mut inner.quic
            } else if detail.lane.contains("tor") {
                &mut inner.tor
            } else {
                &mut inner.websocket
            };
            target.connected |= detail.connected;
            if target.last_error.is_none() {
                target.last_error = detail.last_error.clone();
            }
        }
    }

    pub fn emit_payload(
        &self,
        object_root: &[u8; 32],
        payload: &[u8],
        namespace: u16,
        epoch: u32,
        tag: &[u8; 32],
        flags: u16,
    ) {
        let mut inner = self.inner.lock().expect("state lock");
        let mut feed_bundles = Vec::new();
        if let Ok(bundle) = serde_json::from_slice::<FeedBundle>(payload) {
            feed_bundles.push(bundle);
        } else if let Ok(batch) = serde_cbor::from_slice::<Vec<Vec<u8>>>(payload) {
            for item in batch {
                if let Ok(bundle) = serde_json::from_slice::<FeedBundle>(&item) {
                    feed_bundles.push(bundle);
                }
            }
        }
        emit_event_locked(
            &mut inner,
            "payload",
            serde_json::json!({
                "object_root": hex_encode(object_root),
                "payload_b64": base64::engine::general_purpose::STANDARD.encode(payload),
                "namespace": namespace,
                "epoch": epoch,
                "tag": hex_encode(tag),
                "flags": flags,
            }),
        );
        for bundle in feed_bundles {
            let mut value = serde_json::to_value(bundle).unwrap_or_default();
            if let Some(obj) = value.as_object_mut() {
                obj.insert(
                    "object_root".to_string(),
                    serde_json::json!(hex_encode(object_root)),
                );
            }
            emit_event_locked(&mut inner, "feed_bundle", value);
        }
    }

    pub fn ingest_endorsement_payload(&self, payload: &[u8], now_step: u64) -> bool {
        let mut endorsements = Vec::new();
        if let Some(parsed) = parse_endorsement_payload(payload) {
            endorsements.push(parsed);
        } else if let Ok(batch) = serde_cbor::from_slice::<Vec<Vec<u8>>>(payload) {
            for item in batch {
                if let Some(parsed) = parse_endorsement_payload(&item) {
                    endorsements.push(parsed);
                }
            }
        }
        if endorsements.is_empty() {
            return false;
        }

        let mut inner = self.inner.lock().expect("state lock");
        let mut changed = false;
        for endorsement in endorsements {
            let result = inner.wot_policy.ingest_endorsement(
                endorsement.endorser,
                endorsement.publisher,
                endorsement.at_step,
                now_step,
            );
            if matches!(result, EndorsementIngestResult::Applied) {
                changed = true;
            }
        }
        if changed {
            let summary = inner.wot_policy.summary();
            emit_event_locked(
                &mut inner,
                "policy_updated",
                serde_json::json!({
                    "trusted": summary.trusted,
                    "muted": summary.muted,
                    "blocked": summary.blocked,
                    "endorsements": summary.endorsements,
                }),
            );
            self.persist_policy_locked(&mut inner);
        }
        changed
    }

    pub fn subscribe(&self, tag: &str) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        inner.subscriptions.insert(tag.to_string())
    }

    pub fn unsubscribe(&self, tag: &str) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        inner.subscriptions.remove(tag)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<EventEnvelope> {
        let inner = self.inner.lock().expect("state lock");
        inner.events.subscribe()
    }

    pub fn subscribe_events_since(
        &self,
        since: Option<u64>,
    ) -> (Vec<EventEnvelope>, broadcast::Receiver<EventEnvelope>) {
        let inner = self.inner.lock().expect("state lock");
        let backlog = match since {
            Some(seq) => inner
                .event_buffer
                .iter()
                .filter(|event| event.seq > seq)
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        (backlog, inner.events.subscribe())
    }

    pub fn emit_status_event(&self) -> EventEnvelope {
        let mut inner = self.inner.lock().expect("state lock");
        let status = status_from_inner(&inner);
        emit_event_locked(
            &mut inner,
            "node_status",
            serde_json::to_value(status).unwrap_or_default(),
        )
    }

    pub fn persist(&self) {
        let mut inner = self.inner.lock().expect("state lock");
        self.persist_policy_locked(&mut inner);
    }

    pub fn inject_local_feed_bundle(&self, bundle: serde_json::Value, object_root: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        let mut value = bundle;
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "object_root".to_string(),
                serde_json::json!(hex_encode(&object_root)),
            );
        }
        emit_event_locked(&mut inner, "feed_bundle", value);
    }

    fn persist_policy_locked(&self, inner: &mut StateInner) {
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(inner));
        }
    }
}

fn status_from_inner(inner: &StateInner) -> StatusResponse {
    StatusResponse {
        node_id: inner.node_id.clone(),
        version: inner.version.clone(),
        lanes: LaneStatus {
            quic: inner.quic.clone(),
            websocket: inner.websocket.clone(),
            tor: inner.tor.clone(),
            details: inner.lane_details.clone(),
        },
        queue: QueueStatus {
            pending: inner.queue_pending,
            inflight: inner.queue_inflight,
            failed: inner.queue_failed,
        },
        cache: CacheStatus {
            entries: inner.cache_entries,
            bytes: inner.cache_bytes,
        },
    }
}

fn snapshot_from_inner(inner: &StateInner) -> StoreSnapshot {
    StoreSnapshot {
        queue: inner.queue.iter().cloned().collect(),
        identity: Some(inner.identity.to_record()),
        policy_json: inner.wot_policy.export_json().ok(),
        contacts: inner.contacts.clone(),
        feed_history: inner.event_buffer.iter().cloned().collect(),
        subscriptions: inner.subscriptions.iter().cloned().collect(),
    }
}

fn update_queue_counts(inner: &mut StateInner) {
    inner.queue_pending = inner.queue.len() as u64;
}

const EVENT_VERSION: u16 = 1;
const EVENT_BUFFER_MAX: usize = 256;

fn emit_event_locked(
    inner: &mut StateInner,
    event: &str,
    data: serde_json::Value,
) -> EventEnvelope {
    inner.event_seq = inner.event_seq.saturating_add(1);
    let envelope = EventEnvelope {
        version: EVENT_VERSION,
        seq: inner.event_seq,
        event: event.to_string(),
        data,
    };
    if inner.event_buffer.len() >= EVENT_BUFFER_MAX {
        inner.event_buffer.pop_front();
    }
    inner.event_buffer.push_back(envelope.clone());
    let _ = inner.events.send(envelope.clone());
    envelope
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

#[derive(Debug, serde::Deserialize)]
struct PolicyJsonLists {
    #[serde(default)]
    trusted: Vec<[u8; 32]>,
    #[serde(default)]
    muted: Vec<[u8; 32]>,
    #[serde(default)]
    blocked: Vec<[u8; 32]>,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn parse_identity(record: &IdentityRecord) -> Option<NodeIdentity> {
    if record.public_key_hex.len() != 64 || record.secret_key_hex.len() != 64 {
        return None;
    }
    let pub_bytes = hex::decode(&record.public_key_hex).ok()?;
    let sec_bytes = hex::decode(&record.secret_key_hex).ok()?;
    if pub_bytes.len() != 32 || sec_bytes.len() != 32 {
        return None;
    }
    let mut public_key = [0u8; 32];
    let mut secret_key = [0u8; 32];
    public_key.copy_from_slice(&pub_bytes);
    secret_key.copy_from_slice(&sec_bytes);
    let derived = NostrSigner::from_secret(secret_key).ok()?.public_key();
    if derived != public_key {
        return None;
    }
    Some(NodeIdentity {
        public_key,
        secret_key,
    })
}

fn generate_identity() -> NodeIdentity {
    let (secret_key, signer) = loop {
        let mut secret_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret_key);
        if let Ok(signer) = NostrSigner::from_secret(secret_key) {
            break (secret_key, signer);
        }
    };
    let public_key = signer.public_key();
    NodeIdentity {
        public_key,
        secret_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use veil_schema_feed::BundleMeta;

    #[test]
    fn persists_queue_to_disk() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("node_state.json");
        let state = NodeState::new_with_store("0.1-test", Some(path.clone()));
        let _ = state.enqueue_publish(PublishRequest {
            namespace: 32,
            payload: "hello".to_string(),
        });

        let restored = NodeState::new_with_store("0.1-test", Some(path));
        let status = restored.status();
        assert_eq!(status.queue.pending, 1);
    }

    #[test]
    fn lane_details_update_summary() {
        let state = NodeState::new("0.1-test");
        state.mark_lane_details(vec![
            LaneDetail {
                role: "fast".to_string(),
                lane: "quic".to_string(),
                connected: true,
                last_error: None,
                stats: crate::api::LaneStats::default(),
            },
            LaneDetail {
                role: "fallback".to_string(),
                lane: "websocket".to_string(),
                connected: false,
                last_error: Some("send_error".to_string()),
                stats: crate::api::LaneStats::default(),
            },
        ]);

        let status = state.status();
        assert_eq!(status.lanes.details.len(), 2);
        assert!(status.lanes.quic.connected);
        assert!(!status.lanes.websocket.connected);
        assert_eq!(
            status.lanes.websocket.last_error.as_deref(),
            Some("send_error")
        );
    }

    #[test]
    fn identity_persists_across_restart() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("node_state.json");
        let state = NodeState::new_with_store("0.1-test", Some(path.clone()));
        let first = state.identity();
        let restored = NodeState::new_with_store("0.1-test", Some(path));
        let second = restored.identity();
        assert_eq!(first.public_key, second.public_key);
        assert_eq!(first.secret_key, second.secret_key);
    }

    #[test]
    fn policy_persists_across_restart() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("node_state.json");
        let state = NodeState::new_with_store("0.1-test", Some(path.clone()));
        state.trust_pubkey([0x11; 32]);

        let restored = NodeState::new_with_store("0.1-test", Some(path));
        let summary = restored.policy_summary();
        assert_eq!(summary.trusted, 1);
    }

    #[test]
    fn endorsement_payload_updates_policy() {
        let state = NodeState::new("0.1-test");
        let endorsement =
            veil_schema_feed::FeedBundle::Endorsement(veil_schema_feed::EndorsementBundle {
                meta: BundleMeta {
                    version: 1,
                    created_at: 1_700_000_060,
                },
                channel_id: "general".to_string(),
                endorser_pubkey_hex: "aa".repeat(32),
                publisher_pubkey_hex: "bb".repeat(32),
                at_step: 10,
            });
        let payload = serde_json::to_vec(&endorsement).expect("encode");
        assert!(state.ingest_endorsement_payload(&payload, 10));
        let summary = state.policy_summary();
        assert_eq!(summary.endorsements, 1);
    }

    #[test]
    fn queue_retries_with_backoff() {
        let state = NodeState::new("0.1-test");
        let _ = state.enqueue_publish(PublishRequest {
            namespace: 32,
            payload: "hello".to_string(),
        });
        let item = state.take_next_queued(now_millis()).expect("item");
        let status = state.status();
        assert_eq!(status.queue.pending, 0);
        assert_eq!(status.queue.inflight, 1);

        state.complete_failure(item, 0);
        let status = state.status();
        assert_eq!(status.queue.pending, 1);
        assert_eq!(status.queue.inflight, 0);
        assert_eq!(status.queue.failed, 1);

        let item = state.take_next_queued(now_millis()).expect("item");
        state.complete_success(&item);
        let status = state.status();
        assert_eq!(status.queue.pending, 0);
        assert_eq!(status.queue.inflight, 0);
    }

    #[test]
    fn take_next_queued_batch_groups_small_same_namespace_items() {
        let state = NodeState::new("0.1-test");
        let _ = state.enqueue_publish(PublishRequest {
            namespace: 32,
            payload: "a".repeat(32),
        });
        let _ = state.enqueue_publish(PublishRequest {
            namespace: 32,
            payload: "b".repeat(24),
        });
        // Different namespace should not be coalesced into the same batch.
        let _ = state.enqueue_publish(PublishRequest {
            namespace: 64,
            payload: "c".repeat(24),
        });

        let batch = state.take_next_queued_batch(now_millis(), 16, 96 * 1024, 4 * 1024);
        assert_eq!(batch.len(), 2);
        assert!(batch.iter().all(|item| item.namespace == 32));

        for item in &batch {
            state.complete_success(item);
        }

        let next = state
            .take_next_queued(now_millis())
            .expect("remaining item");
        assert_eq!(next.namespace, 64);
    }

    #[test]
    fn take_next_queued_batch_leaves_large_payload_as_single_item() {
        let state = NodeState::new("0.1-test");
        let _ = state.enqueue_publish(PublishRequest {
            namespace: 32,
            payload: "x".repeat(8 * 1024),
        });
        let _ = state.enqueue_publish(PublishRequest {
            namespace: 32,
            payload: "y".repeat(32),
        });

        let batch = state.take_next_queued_batch(now_millis(), 16, 96 * 1024, 4 * 1024);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].payload.len(), 8 * 1024);

        state.complete_success(&batch[0]);
        let next = state
            .take_next_queued(now_millis())
            .expect("small item remains");
        assert_eq!(next.payload.len(), 32);
    }

    #[test]
    fn emits_feed_bundle_event() {
        let state = NodeState::new("0.1-test");
        let mut rx = state.subscribe_events();
        let bundle = veil_schema_feed::FeedBundle::Post(veil_schema_feed::PostBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_040,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: "aa".repeat(32),
            text: "hello".to_string(),
            media_roots: vec![],
            reply_to_root: None,
        });
        let payload = serde_json::to_vec(&bundle).expect("encode");
        state.emit_payload(&[0x11; 32], &payload, 32, 1, &[0x22; 32], 0);
        let first = rx.try_recv().expect("event");
        assert_eq!(first.event, "payload");
        let second = rx.try_recv().expect("event");
        assert_eq!(second.event, "feed_bundle");
    }

    #[test]
    fn injects_local_feed_bundle() {
        let state = NodeState::new("0.1-test");
        let mut rx = state.subscribe_events();

        let bundle = serde_json::json!({
            "kind": "post",
            "text": "local post"
        });
        let root = [0x99; 32];

        state.inject_local_feed_bundle(bundle, root);

        let event = rx.try_recv().expect("should receive event");
        assert_eq!(event.event, "feed_bundle");
        assert_eq!(event.data["text"], "local post");
        assert_eq!(event.data["object_root"], hex_encode(&root));
    }
}
