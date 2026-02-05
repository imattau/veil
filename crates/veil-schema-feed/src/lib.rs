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
#[serde(tag = "kind")]
pub enum FeedBundle {
    #[serde(rename = "profile")]
    Profile(ProfileBundle),
    #[serde(rename = "media")]
    Media(MediaBundle),
    #[serde(rename = "post")]
    Post(PostBundle),
    #[serde(rename = "channel_directory")]
    ChannelDirectory(ChannelDirectoryBundle),
    #[serde(rename = "endorsement")]
    Endorsement(EndorsementBundle),
}

#[cfg(test)]
mod tests {
    use super::{BundleMeta, ChannelDirectoryBundle, EndorsementBundle, FeedBundle, PostBundle};

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
}
