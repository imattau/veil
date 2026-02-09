use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub node_id: String,
    pub version: String,
    pub lanes: LaneStatus,
    pub queue: QueueStatus,
    pub cache: CacheStatus,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LaneStatus {
    pub quic: LaneHealth,
    pub websocket: LaneHealth,
    pub tor: LaneHealth,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LaneHealth {
    pub connected: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct QueueStatus {
    pub pending: u64,
    pub inflight: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CacheStatus {
    pub entries: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    pub namespace: u16,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub tag: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscribeResponse {
    pub subscribed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeRequest {
    pub tag: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnsubscribeResponse {
    pub unsubscribed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventEnvelope {
    pub event: String,
    pub data: serde_json::Value,
}
