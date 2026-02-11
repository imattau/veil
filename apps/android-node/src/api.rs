use serde::{Deserialize, Serialize};
use uuid::Uuid;
use veil_node::policy::WotConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub node_id: String,
    pub version: String,
    pub lanes: LaneStatus,
    pub queue: QueueStatus,
    pub cache: CacheStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityResponse {
    pub public_key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRotateResponse {
    pub public_key_hex: String,
    pub rotated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaneStatus {
    pub quic: LaneHealth,
    pub websocket: LaneHealth,
    pub tor: LaneHealth,
    pub details: Vec<LaneDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaneHealth {
    pub connected: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaneStats {
    pub outbound_queued: u64,
    pub outbound_send_ok: u64,
    pub outbound_send_err: u64,
    pub inbound_received: u64,
    pub inbound_dropped: u64,
    pub reconnect_attempts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaneDetail {
    pub role: String,
    pub lane: String,
    pub connected: bool,
    pub last_error: Option<String>,
    pub stats: LaneStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueStatus {
    pub pending: u64,
    pub inflight: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStatus {
    pub entries: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    pub namespace: u16,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilePublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::ProfileBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilePublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::PostBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::ReactionBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessagePublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::DirectMessageBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessagePublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessagePublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::GroupMessageBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessagePublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::MediaBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::FollowBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub follower_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutePublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::MuteBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutePublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub muter_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::BlockBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub blocker_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeResponse {
    pub subscribed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeRequest {
    pub tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeResponse {
    pub unsubscribed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: u16,
    pub seq: u64,
    pub event: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySummaryResponse {
    pub trusted: usize,
    pub muted: usize,
    pub blocked: usize,
    pub endorsements: usize,
    pub config: WotConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfigRequest {
    pub config: WotConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfigResponse {
    pub updated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySetRequest {
    pub pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySetResponse {
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactBundle {
    pub peer_id: String,
    pub ws_url: Option<String>,
    pub quic_addr: Option<String>,
    pub pubkey_hex: String,
    #[serde(default)]
    pub rpc_url: Option<String>,
    #[serde(default)]
    pub lan_addrs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactImportRequest {
    pub contact: ContactBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactImportResponse {
    pub imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactListResponse {
    pub contacts: Vec<ContactBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryAnnounceRequest {
    pub contact: ContactBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryAnnounceResponse {
    pub accepted: bool,
    pub neighbors: Vec<ContactBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryLookupRequest {
    pub peer_id: Option<String>,
    pub pubkey_hex: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryLookupResponse {
    pub contacts: Vec<ContactBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryGossipRequest {
    pub contacts: Vec<ContactBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryGossipResponse {
    pub contacts: Vec<ContactBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardFetchResponse {
    pub shard_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectFetchResponse {
    pub object_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueWorkerConfig {
    pub tick_ms: u64,
    pub max_attempts: u32,
    pub backoff_base_ms: u64,
    pub backoff_max_ms: u64,
}
