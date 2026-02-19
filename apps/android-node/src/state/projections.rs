use crate::api::{CacheStatus, LaneStatus, QueueStatus, StatusResponse};
use crate::state_store::StoreSnapshot;

use super::identity_group_keys::flatten_group_keys;
use super::StateInner;

pub(super) fn status_from_inner(inner: &StateInner) -> StatusResponse {
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

pub(super) fn snapshot_from_inner(inner: &StateInner) -> StoreSnapshot {
    StoreSnapshot {
        queue: inner.queue.iter().cloned().collect(),
        identity: Some(inner.identity.to_record()),
        policy_json: inner.wot_policy.export_json().ok(),
        contacts: inner.contact_book.contacts(),
        feed_history: inner.event_buffer.iter().cloned().collect(),
        subscriptions: inner.subscriptions.iter().cloned().collect(),
        group_keys: flatten_group_keys(&inner.group_keys),
    }
}

pub(super) fn update_queue_counts(inner: &mut StateInner) {
    inner.queue_pending = inner.queue.len() as u64;
}
