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
}

#[cfg(test)]
mod tests {
    use super::{
        BlockBundle, BundleMeta, ChannelDirectoryBundle, DirectMessageBundle, EndorsementBundle,
        FeedBundle, FollowBundle, GroupMessageBundle, MuteBundle,
        NamespaceSignaturePolicyBundle, PostBundle, ReactionBundle,
    };

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

    #[test]
    fn post_bundle_keeps_media_refs() {
        let bundle = FeedBundle::Post(PostBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_001,
            },
            channel_id: "dev".to_string(),
            author_pubkey_hex: "22".repeat(32),
            text: "hello".to_string(),
            media_roots: vec![[0xCC; 32], [0xDD; 32]],
            reply_to_root: None,
        });

        match bundle {
            FeedBundle::Post(post) => assert_eq!(post.media_roots.len(), 2),
            _ => panic!("expected post bundle"),
        }
    }

    #[test]
    fn endorsement_bundle_round_trip_through_json() {
        let bundle = FeedBundle::Endorsement(EndorsementBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_002,
            },
            channel_id: "general".to_string(),
            endorser_pubkey_hex: "aa".repeat(32),
            publisher_pubkey_hex: "bb".repeat(32),
            at_step: 1234,
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn namespace_signature_policy_bundle_round_trip() {
        let bundle = FeedBundle::NamespaceSignaturePolicy(NamespaceSignaturePolicyBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_003,
            },
            channel_id: "general".to_string(),
            namespace: 7,
            require_signed: true,
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn follow_bundle_round_trip_through_json() {
        let bundle = FeedBundle::Follow(FollowBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_010,
            },
            channel_id: "general".to_string(),
            follower_pubkey_hex: "aa".repeat(32),
            followee_pubkey_hex: "bb".repeat(32),
            at_step: 42,
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn reaction_bundle_round_trip_through_json() {
        let bundle = FeedBundle::Reaction(ReactionBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_020,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: "aa".repeat(32),
            target_root: [0x11; 32],
            action_code: "like".to_string(),
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn direct_message_bundle_round_trip_through_json() {
        let bundle = FeedBundle::DirectMessage(DirectMessageBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_021,
            },
            channel_id: "dm".to_string(),
            author_pubkey_hex: "bb".repeat(32),
            recipient_pubkey_hex: "cc".repeat(32),
            ciphertext_root: [0x22; 32],
            reply_to_root: None,
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn group_message_bundle_round_trip_through_json() {
        let bundle = FeedBundle::GroupMessage(GroupMessageBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_022,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: "dd".repeat(32),
            group_id: "group-alpha".to_string(),
            ciphertext_root: [0x33; 32],
            reply_to_root: Some([0x44; 32]),
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn mute_bundle_round_trip_through_json() {
        let bundle = FeedBundle::Mute(MuteBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_011,
            },
            channel_id: "general".to_string(),
            muter_pubkey_hex: "cc".repeat(32),
            muted_pubkey_hex: "dd".repeat(32),
            reason: Some("spam".to_string()),
            at_step: 84,
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn block_bundle_round_trip_through_json() {
        let bundle = FeedBundle::Block(BlockBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_012,
            },
            channel_id: "general".to_string(),
            blocker_pubkey_hex: "ee".repeat(32),
            blocked_pubkey_hex: "ff".repeat(32),
            reason: None,
            at_step: 128,
        });
        let json = serde_json::to_string(&bundle).expect("serialize should work");
        let decoded: FeedBundle = serde_json::from_str(&json).expect("decode should work");
        assert_eq!(decoded, bundle);
    }
}
