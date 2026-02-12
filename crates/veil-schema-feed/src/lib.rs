//! Non-normative feed-oriented app schema for VEIL clients.
//!
//! This crate is intentionally separate from core protocol crates. It offers
//! reusable social/feed data structures without making them protocol-required.

use serde::{Deserialize, Serialize};
use veil_core::ObjectRoot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleMeta {
    pub version: u16,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_media_root: Option<ObjectRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub mime_type: String,
    pub url: String,
    pub bytes_hint: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub text: String,
    pub media_roots: Vec<ObjectRoot>,
    pub reply_to_root: Option<ObjectRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReactionBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub target_root: ObjectRoot,
    pub action_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectMessageBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub recipient_pubkey_hex: String,
    pub ciphertext_root: ObjectRoot,
    pub reply_to_root: Option<ObjectRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMessageBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub group_id: String,
    pub ciphertext_root: ObjectRoot,
    pub reply_to_root: Option<ObjectRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelDirectoryBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub title: String,
    pub about: String,
    pub profile_roots: Vec<ObjectRoot>,
    pub post_roots: Vec<ObjectRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndorsementBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub endorser_pubkey_hex: String,
    pub publisher_pubkey_hex: String,
    pub at_step: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub follower_pubkey_hex: String,
    pub followee_pubkey_hex: String,
    pub at_step: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuteBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub muter_pubkey_hex: String,
    pub muted_pubkey_hex: String,
    pub reason: Option<String>,
    pub at_step: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub blocker_pubkey_hex: String,
    pub blocked_pubkey_hex: String,
    pub reason: Option<String>,
    pub at_step: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceSignaturePolicyBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub namespace: u16,
    pub require_signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub title: String,
    pub kind: String,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ListItem {
    Pubkey(String),
    Object(ObjectRoot),
    Tag([u8; 32]),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMetadataBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub group_id: String,
    pub name: String,
    pub about: String,
    pub avatar_root: Option<ObjectRoot>,
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZapBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub amount: u64,
    pub unit: String,
    pub target_root: ObjectRoot,
    pub receipt_proof: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPreferencesBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub app_id: String,
    pub settings_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletionBundle {
    pub meta: BundleMeta,
    pub author_pubkey_hex: String,
    pub target_roots: Vec<ObjectRoot>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepostBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub target_root: ObjectRoot,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub question: String,
    pub options: Vec<String>,
    pub ends_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollVoteBundle {
    pub meta: BundleMeta,
    pub channel_id: String,
    pub author_pubkey_hex: String,
    pub poll_root: ObjectRoot,
    pub option_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveStatusBundle {
    pub meta: BundleMeta,
    pub author_pubkey_hex: String,
    pub status_text: String,
    pub emoji: Option<String>,
    pub expiry: Option<u64>,
    pub external_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FeedBundle {
    #[serde(rename = "profile")]
    Profile(ProfileBundle),
    #[serde(rename = "media")]
    Media(MediaBundle),
    #[serde(rename = "post")]
    Post(PostBundle),
    #[serde(rename = "reaction")]
    Reaction(ReactionBundle),
    #[serde(rename = "direct_message")]
    DirectMessage(DirectMessageBundle),
    #[serde(rename = "group_message")]
    GroupMessage(GroupMessageBundle),
    #[serde(rename = "channel_directory")]
    ChannelDirectory(ChannelDirectoryBundle),
    #[serde(rename = "endorsement")]
    Endorsement(EndorsementBundle),
    #[serde(rename = "follow")]
    Follow(FollowBundle),
    #[serde(rename = "mute")]
    Mute(MuteBundle),
    #[serde(rename = "block")]
    Block(BlockBundle),
    #[serde(rename = "namespace_signature_policy")]
    NamespaceSignaturePolicy(NamespaceSignaturePolicyBundle),
    #[serde(rename = "list")]
    List(ListBundle),
    #[serde(rename = "group_metadata")]
    GroupMetadata(GroupMetadataBundle),
    #[serde(rename = "zap")]
    Zap(ZapBundle),
    #[serde(rename = "app_preferences")]
    AppPreferences(AppPreferencesBundle),
    #[serde(rename = "deletion")]
    Deletion(DeletionBundle),
    #[serde(rename = "repost")]
    Repost(RepostBundle),
    #[serde(rename = "poll")]
    Poll(PollBundle),
    #[serde(rename = "poll_vote")]
    PollVote(PollVoteBundle),
    #[serde(rename = "live_status")]
    LiveStatus(LiveStatusBundle),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_bundle_round_trips_through_json() {
        let bundle = FeedBundle::ChannelDirectory(ChannelDirectoryBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_000,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: "11".repeat(32),
            title: "#general".to_string(),
            about: "Directory head".to_string(),
            profile_roots: vec![[0xAA; 32]],
            post_roots: vec![[0xBB; 32]],
        });

        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }
}
