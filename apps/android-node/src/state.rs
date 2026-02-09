use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api::{
    CacheStatus, EventEnvelope, LaneHealth, LaneStatus, PublishRequest, QueueStatus, StatusResponse,
};
use crate::state_store::{QueueItem, StateStore, StoreSnapshot};

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
    subscriptions: HashSet<String>,
    events: broadcast::Sender<EventEnvelope>,
    store: Option<StateStore>,
    queue: Vec<QueueItem>,
    queue_attempts: HashMap<Uuid, u32>,
}

impl NodeState {
    pub fn new(version: impl Into<String>) -> Self {
        Self::new_with_store(version, None)
    }

    pub fn new_with_store(version: impl Into<String>, store_path: Option<PathBuf>) -> Self {
        let (events, _) = broadcast::channel(128);
        let store = store_path.map(StateStore::new);
        let snapshot = store
            .as_ref()
            .map(StateStore::load)
            .unwrap_or_default();
        let queue_pending = snapshot.queue.len() as u64;
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
                subscriptions: HashSet::new(),
                events,
                store,
                queue: snapshot.queue,
                queue_attempts: HashMap::new(),
            })),
        }
    }

    pub fn status(&self) -> StatusResponse {
        let inner = self.inner.lock().expect("state lock");
        StatusResponse {
            node_id: inner.node_id.clone(),
            version: inner.version.clone(),
            lanes: LaneStatus {
                quic: inner.quic.clone(),
                websocket: inner.websocket.clone(),
                tor: inner.tor.clone(),
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

    pub fn enqueue_publish(&self, request: PublishRequest) -> Uuid {
        let mut inner = self.inner.lock().expect("state lock");
        let message_id = Uuid::new_v4();
        inner.queue.push(QueueItem {
            id: message_id,
            namespace: request.namespace,
            payload: request.payload,
        });
        inner.queue_pending = inner.queue_pending.saturating_add(1);
        let _ = inner.events.send(EventEnvelope {
            event: "publish_queued".to_string(),
            data: serde_json::json!({
                "message_id": message_id,
                "pending": inner.queue_pending,
            }),
        });
        if let Some(store) = &inner.store {
            store.persist(&StoreSnapshot {
                queue: inner.queue.clone(),
            });
        }
        message_id
    }

    pub fn take_next_queued(&self) -> Option<QueueItem> {
        let mut inner = self.inner.lock().expect("state lock");
        let item = inner.queue.first().cloned();
        if let Some(item) = &item {
            inner.queue_inflight = inner.queue_inflight.saturating_add(1);
            inner.queue_pending = inner.queue_pending.saturating_sub(1);
            let attempts = inner.queue_attempts.entry(item.id).or_insert(0);
            *attempts += 1;
        }
        item
    }

    pub fn complete_item(&self, item: &QueueItem, success: bool) {
        let mut inner = self.inner.lock().expect("state lock");
        if let Some(pos) = inner.queue.iter().position(|entry| entry.id == item.id) {
            if success {
                inner.queue.remove(pos);
                inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
                inner.queue_attempts.remove(&item.id);
                let _ = inner.events.send(EventEnvelope {
                    event: "publish_sent".to_string(),
                    data: serde_json::json!({ "message_id": item.id }),
                });
            } else {
                inner.queue_failed = inner.queue_failed.saturating_add(1);
                inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
                let attempts = inner.queue_attempts.get(&item.id).copied().unwrap_or(0);
                let _ = inner.events.send(EventEnvelope {
                    event: "publish_failed".to_string(),
                    data: serde_json::json!({
                        "message_id": item.id,
                        "attempts": attempts,
                    }),
                });
            }
        }
        if let Some(store) = &inner.store {
            store.persist(&StoreSnapshot {
                queue: inner.queue.clone(),
            });
        }
    }

    pub fn attempts_for(&self, item: &QueueItem) -> u32 {
        let inner = self.inner.lock().expect("state lock");
        inner.queue_attempts.get(&item.id).copied().unwrap_or(0)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
}
