use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api::{
    ContactBundle, EventEnvelope, LaneDetail, LaneHealth, PublishRequest, StatusResponse,
};
use crate::state_store::{IdentityRecord, QueueItem, StateStore, StoreSnapshot};
use veil_crypto::signing::{NostrSigner, Signer};
use veil_node::policy::{EndorsementIngestResult, LocalWotPolicy, WotConfig, WotSummary};
mod contact_book;
mod contact_mutations;
mod event_stream;
mod group_keys;
mod identity_group_keys;
mod lane_health;
mod payload_events;
mod payload_parsing;
mod policy_lists;
mod policy_mutations;
mod projections;
mod queue_ops;
mod queue_retry;
use self::contact_book::ContactBook;
use self::contact_mutations::{execute_contact_mutation, ContactMutation};
use self::event_stream::{
    emit_status_event, feed_snapshot, subscribe_events_receiver, subscribe_events_since,
    subscribe_tag, subscriptions_snapshot, unsubscribe_tag,
};
use self::group_keys::{
    ensure_group_key as ensure_group_key_inner, rotate_group_key as rotate_group_key_inner,
};
use self::identity_group_keys::{
    decode_hex_32, generate_identity, parse_group_keys, parse_identity,
};
use self::lane_health::{
    mark_lane_details as mark_lane_details_inner, mark_lane_health as mark_lane_health_inner,
};
use self::payload_events::{emit_local_feed_bundle_event, emit_payload_events};
use self::payload_parsing::parse_endorsements;
use self::policy_lists::export_policy_lists;
use self::policy_mutations::{apply_pubkey_mutation, PolicyPubkeyMutation};
use self::projections::{snapshot_from_inner, status_from_inner};
use self::queue_retry::RetrySchedule;
#[cfg(test)]
const MAX_CONTACTS_TOTAL: usize = contact_book::MAX_CONTACTS_TOTAL;

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
    retry_schedule: RetrySchedule,
    identity: NodeIdentity,
    wot_policy: LocalWotPolicy,
    contact_book: ContactBook,
    group_keys: HashMap<String, HashMap<String, [u8; 32]>>,
}

#[derive(Debug, Clone)]
pub struct NodeIdentity {
    pub public_key: [u8; 32],
    pub secret_key: [u8; 32],
    pub encrypt_key: [u8; 32],
}

impl NodeIdentity {
    pub fn signer(&self) -> NostrSigner {
        NostrSigner::from_secret(self.secret_key).expect("stored identity secret must be valid")
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }

    pub fn to_record(&self) -> IdentityRecord {
        IdentityRecord {
            public_key_hex: self.public_key_hex(),
            secret_key_hex: hex::encode(self.secret_key),
            secret_key_enc_nonce_b64: None,
            secret_key_enc_b64: None,
            encrypt_key_hex: hex::encode(self.encrypt_key),
            encrypt_key_enc_nonce_b64: None,
            encrypt_key_enc_b64: None,
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
        let contact_book = ContactBook::from_contacts(snapshot.contacts.clone());
        let group_keys = parse_group_keys(&snapshot.group_keys);
        let subscriptions: HashSet<String> = snapshot.subscriptions.iter().cloned().collect();
        let event_buffer: VecDeque<EventEnvelope> = VecDeque::from(snapshot.feed_history.clone());
        let event_seq = event_buffer.iter().map(|e| e.seq).max().unwrap_or(0);

        if let Some(store) = &store {
            if snapshot.identity.is_none() {
                store.persist(&StoreSnapshot {
                    queue: snapshot.queue.clone(),
                    identity: Some(identity.to_record()),
                    policy_json: wot_policy.export_json().ok(),
                    contacts: contact_book.contacts(),
                    feed_history: event_buffer.iter().cloned().collect(),
                    subscriptions: subscriptions.iter().cloned().collect(),
                    group_keys: snapshot.group_keys.clone(),
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
                retry_schedule: RetrySchedule::default(),
                identity,
                wot_policy,
                contact_book,
                group_keys,
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
        let message_id = queue_ops::enqueue_request(&mut inner, request);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
        message_id
    }

    pub fn take_next_queued(&self, now_ms: u64) -> Option<QueueItem> {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::take_due_item(&mut inner, now_ms)
    }

    pub fn take_next_queued_batch(
        &self,
        now_ms: u64,
        max_items: usize,
        target_batch_bytes: usize,
        max_item_bytes: usize,
    ) -> Vec<QueueItem> {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::take_due_batch(
            &mut inner,
            now_ms,
            max_items,
            target_batch_bytes,
            max_item_bytes,
        )
    }

    pub fn complete_success(&self, item: &QueueItem) {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::mark_success(&mut inner, item);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn complete_failure(&self, item: QueueItem, retry_after_ms: u64) {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::mark_failure(&mut inner, item, retry_after_ms, now_millis());
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn drop_item(&self, item: &QueueItem) {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::mark_dropped(&mut inner, item);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn attempts_for(&self, item: &QueueItem) -> u32 {
        let inner = self.inner.lock().expect("state lock");
        queue_ops::attempts_for(&inner, item)
    }

    pub fn get_feed(&self, limit: usize) -> Vec<EventEnvelope> {
        let inner = self.inner.lock().expect("state lock");
        feed_snapshot(&inner, limit)
    }

    pub fn get_subscriptions(&self) -> Vec<String> {
        let inner = self.inner.lock().expect("state lock");
        subscriptions_snapshot(&inner)
    }

    pub fn export_identity(&self) -> (String, String) {
        let inner = self.inner.lock().expect("state lock");
        (
            inner.identity.public_key_hex(),
            hex::encode(inner.identity.secret_key),
        )
    }

    pub fn import_identity(&self, secret_key_hex: String) -> Result<NodeIdentity, String> {
        let secret_key = decode_hex_32(&secret_key_hex)
            .ok_or_else(|| "secret key must be 32 bytes".to_string())?;
        let signer = NostrSigner::from_secret(secret_key)
            .map_err(|_| "secret key is not a valid Nostr secp256k1 secret".to_string())?;
        let public_key = signer.public_key();
        let encrypt_key = veil_crypto::keys::derive_encrypt_key(&secret_key);
        let identity = NodeIdentity {
            public_key,
            secret_key,
            encrypt_key,
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
        self.mutate_policy(|policy| policy.update_config(config));
    }

    pub fn trust_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Trust, pubkey);
    }

    pub fn untrust_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Untrust, pubkey);
    }

    pub fn mute_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Mute, pubkey);
    }

    pub fn unmute_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Unmute, pubkey);
    }

    pub fn block_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Block, pubkey);
    }

    pub fn unblock_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Unblock, pubkey);
    }

    pub fn wot_policy(&self) -> LocalWotPolicy {
        let inner = self.inner.lock().expect("state lock");
        inner.wot_policy.clone()
    }

    pub fn policy_lists(&self) -> crate::api::PolicyListsResponse {
        let inner = self.inner.lock().expect("state lock");
        export_policy_lists(&inner.wot_policy)
    }

    pub fn contacts(&self) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.contacts()
    }

    pub fn add_contact(&self, contact: ContactBundle) {
        let _ = self.apply_contact_mutation(ContactMutation::AddOrMerge(contact));
    }

    pub fn set_contact(&self, contact: ContactBundle) {
        let _ = self.apply_contact_mutation(ContactMutation::Set(contact));
    }

    pub fn remove_contact(&self, peer_id: &str) -> bool {
        self.apply_contact_mutation(ContactMutation::RemoveByPeerId(peer_id.to_string()))
    }

    pub fn discovery_lookup_peer(&self, peer_id: &str, limit: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.lookup_peer(peer_id, limit)
    }

    pub fn discovery_lookup_pubkey(&self, pubkey_hex: &str, limit: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.lookup_pubkey(pubkey_hex, limit)
    }

    pub fn discovery_lookup_contact(
        &self,
        contact: &ContactBundle,
        limit: usize,
    ) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.lookup_contact(contact, limit)
    }

    pub fn discovery_sample(&self, max: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.sample(max)
    }

    pub fn mark_lane_health(&self, lane: &str, connected: bool, last_error: Option<String>) {
        let mut inner = self.inner.lock().expect("state lock");
        mark_lane_health_inner(&mut inner, lane, connected, last_error);
    }

    pub fn mark_lane_details(&self, details: Vec<LaneDetail>) {
        let mut inner = self.inner.lock().expect("state lock");
        mark_lane_details_inner(&mut inner, details);
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
        let group_key_updated = emit_payload_events(
            &mut inner,
            object_root,
            payload,
            namespace,
            epoch,
            tag,
            flags,
        );
        if group_key_updated {
            if let Some(store) = &inner.store {
                store.persist(&snapshot_from_inner(&inner));
            }
        }
    }

    pub fn ingest_endorsement_payload(&self, payload: &[u8], now_step: u64) -> bool {
        let endorsements = parse_endorsements(payload);
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
        subscribe_tag(&mut inner, tag)
    }

    pub fn unsubscribe(&self, tag: &str) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        unsubscribe_tag(&mut inner, tag)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<EventEnvelope> {
        let inner = self.inner.lock().expect("state lock");
        subscribe_events_receiver(&inner)
    }

    pub fn subscribe_events_since(
        &self,
        since: Option<u64>,
    ) -> (Vec<EventEnvelope>, broadcast::Receiver<EventEnvelope>) {
        let inner = self.inner.lock().expect("state lock");
        subscribe_events_since(&inner, since)
    }

    pub fn emit_status_event(&self) -> EventEnvelope {
        let mut inner = self.inner.lock().expect("state lock");
        emit_status_event(&mut inner)
    }

    pub fn persist(&self) {
        let mut inner = self.inner.lock().expect("state lock");
        self.persist_policy_locked(&mut inner);
    }

    pub fn ensure_group_key(&self, group_id: &str) -> (String, [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        let material = ensure_group_key_inner(&mut inner, group_id);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
        material
    }

    pub fn rotate_group_key(&self, group_id: &str) -> (String, [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        let material = rotate_group_key_inner(&mut inner, group_id);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
        material
    }

    pub fn inject_local_feed_bundle(&self, bundle: serde_json::Value, object_root: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        emit_local_feed_bundle_event(&mut inner, bundle, object_root);
    }

    fn mutate_policy(&self, mutate: impl FnOnce(&mut LocalWotPolicy)) {
        let mut inner = self.inner.lock().expect("state lock");
        mutate(&mut inner.wot_policy);
        self.persist_policy_locked(&mut inner);
    }

    fn apply_policy_pubkey_mutation(&self, mutation: PolicyPubkeyMutation, pubkey: [u8; 32]) {
        self.mutate_policy(|policy| apply_pubkey_mutation(policy, mutation, pubkey));
    }

    fn mutate_contacts(&self, mutate: impl FnOnce(&mut ContactBook) -> bool) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        let changed = mutate(&mut inner.contact_book);
        if changed {
            self.persist_policy_locked(&mut inner);
        }
        changed
    }

    fn apply_contact_mutation(&self, mutation: ContactMutation) -> bool {
        self.mutate_contacts(|book| execute_contact_mutation(book, mutation))
    }

    fn persist_policy_locked(&self, inner: &mut StateInner) {
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(inner));
        }
    }
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

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
