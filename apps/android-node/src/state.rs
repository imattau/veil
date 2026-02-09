use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api::{CacheStatus, EventEnvelope, LaneHealth, LaneStatus, QueueStatus, StatusResponse};

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
}

impl NodeState {
    pub fn new(version: impl Into<String>) -> Self {
        let (events, _) = broadcast::channel(128);
        Self {
            inner: Arc::new(Mutex::new(StateInner {
                node_id: Uuid::new_v4().to_string(),
                version: version.into(),
                queue_pending: 0,
                queue_inflight: 0,
                queue_failed: 0,
                cache_entries: 0,
                cache_bytes: 0,
                quic: LaneHealth::default(),
                websocket: LaneHealth::default(),
                tor: LaneHealth::default(),
                subscriptions: HashSet::new(),
                events,
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

    pub fn enqueue_publish(&self) -> Uuid {
        let mut inner = self.inner.lock().expect("state lock");
        inner.queue_pending = inner.queue_pending.saturating_add(1);
        let message_id = Uuid::new_v4();
        let _ = inner.events.send(EventEnvelope {
            event: "publish_queued".to_string(),
            data: serde_json::json!({
                "message_id": message_id,
                "pending": inner.queue_pending,
            }),
        });
        message_id
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
