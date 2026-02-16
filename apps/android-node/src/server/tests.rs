use super::*;
use crate::api::ContactBundle;
use crate::api::DiscoveryAnnounceResponse;
use axum::body::{Body, Bytes};
use base64::Engine;
use http::{Request, StatusCode};
use tower::ServiceExt;
use veil_crypto::signing::Signer;
use veil_schema_feed::{
    BlockBundle, BundleMeta, DirectMessageBundle, FollowBundle, GroupMessageBundle, MediaBundle,
    MuteBundle, PostBundle, ProfileBundle, ReactionBundle,
};

fn test_state() -> AppState {
    let node = NodeState::new("0.1-test");
    let identity = node.identity();
    let protocol_config = crate::default_protocol_config(
        "ws://127.0.0.1:9001/ws".to_string(),
        "test-node".to_string(),
        32,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    );
    let protocol =
        std::sync::Arc::new(ProtocolEngine::new(protocol_config).expect("protocol init"));
    AppState {
        node,
        protocol,
        auth_token: "secret".to_string(),
        allow_identity_export: false,
        version: "0.1-test".to_string(),
    }
}

fn test_state_with_export_enabled() -> AppState {
    let mut state = test_state();
    state.allow_identity_export = true;
    state
}

mod contact_tests;
mod core_tests;
mod direct_message_tests;
mod discovery_tests;
mod feed_publish_tests;
mod group_message_tests;
mod policy_tests;
mod social_publish_tests;
