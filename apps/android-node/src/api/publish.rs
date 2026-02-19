use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
pub struct DirectMessageTextPublishRequest {
    pub namespace: u16,
    pub channel_id: String,
    pub recipient_pubkey_hex: String,
    pub text: String,
    pub reply_to_root: Option<[u8; 32]>,
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
pub struct GroupMessageTextPublishRequest {
    pub namespace: u16,
    pub channel_id: String,
    pub group_id: String,
    pub text: String,
    pub reply_to_root: Option<[u8; 32]>,
    #[serde(default)]
    pub member_pubkeys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupKeyShareRequest {
    pub namespace: u16,
    pub channel_id: String,
    pub group_id: String,
    pub member_pubkeys: Vec<String>,
    #[serde(default)]
    pub rotate_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupKeyShareResponse {
    pub queued: bool,
    pub key_id: String,
    pub shares: usize,
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
pub struct ListPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::ListBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMetadataPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::GroupMetadataBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMetadataPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZapPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::ZapBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZapPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPreferencesPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::AppPreferencesBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPreferencesPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::DeletionBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepostPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::RepostBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepostPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::PollBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollVotePublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::PollVoteBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollVotePublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveStatusPublishRequest {
    pub namespace: u16,
    pub bundle: veil_schema_feed::LiveStatusBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveStatusPublishResponse {
    pub message_id: Uuid,
    pub queued: bool,
    pub author_pubkey_hex: String,
}
