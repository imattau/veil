use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api::{ContactBundle, EventEnvelope, LaneDetail, LaneHealth, PublishRequest};
use crate::state_store::{IdentityRecord, QueueItem, StateStore, StoreSnapshot};
use veil_crypto::signing::{NostrSigner, Signer};
use veil_node::policy::{EndorsementIngestResult, LocalWotPolicy};
mod contact_book;
mod contact_mutations;
mod event_stream;
mod group_keys;
mod identity_group_keys;
mod lane_health;
mod node_state_identity;
mod node_state_policy_contacts;
mod node_state_queue;
mod node_state_runtime;
mod payload_events;
mod payload_parsing;
mod policy_lists;
mod policy_mutations;
mod projections;
mod queue_ops;
mod queue_retry;
use self::contact_book::ContactBook;
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
