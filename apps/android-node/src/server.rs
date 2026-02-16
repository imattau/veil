use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use base64::Engine;

use crate::api::{
    AppPreferencesPublishRequest, AppPreferencesPublishResponse, BlockPublishRequest,
    BlockPublishResponse, ContactDeleteRequest, ContactDeleteResponse, ContactImportRequest,
    ContactImportResponse, ContactListResponse, DeletionPublishRequest, DeletionPublishResponse,
    DirectMessagePublishRequest, DirectMessagePublishResponse, DirectMessageTextPublishRequest,
    DiscoveryAnnounceRequest, DiscoveryGossipRequest, DiscoveryGossipResponse,
    DiscoveryLookupRequest, DiscoveryLookupResponse, ErrorResponse, FeedResponse,
    FollowPublishRequest, FollowPublishResponse, GroupKeyShareRequest, GroupKeyShareResponse,
    GroupMessagePublishRequest, GroupMessagePublishResponse, GroupMessageTextPublishRequest,
    GroupMetadataPublishRequest, GroupMetadataPublishResponse, HealthResponse,
    IdentityExportResponse, IdentityImportRequest, IdentityResponse, IdentityRotateResponse,
    ListPublishRequest, ListPublishResponse, LiveStatusPublishRequest, LiveStatusPublishResponse,
    MediaPublishRequest, MediaPublishResponse, MutePublishRequest, MutePublishResponse,
    ObjectFetchResponse, ObjectPublishRequest, ObjectPublishResponse, PolicyConfigRequest,
    PolicyConfigResponse, PolicyListsResponse, PolicySetRequest, PolicySetResponse,
    PolicySummaryResponse, PollPublishRequest, PollPublishResponse, PollVotePublishRequest,
    PollVotePublishResponse, PostPublishRequest, PostPublishResponse, ProfilePublishRequest,
    ProfilePublishResponse, PublishRequest, PublishResponse, ReactionPublishRequest,
    ReactionPublishResponse, RepostPublishRequest, RepostPublishResponse, ShardFetchResponse,
    StatusResponse, SubscribeRequest, SubscribeResponse, SubscriptionListResponse,
    UnsubscribeRequest, UnsubscribeResponse, ZapPublishRequest, ZapPublishResponse,
};
use crate::discovery::build_self_contact;
use crate::secure_message::{
    encrypt_direct_message_payload, encrypt_group_key_share_payload, encrypt_group_message_payload,
};
use crate::state::NodeState;
use crate::ProtocolEngine;
use veil_schema_feed::{BundleMeta, DirectMessageBundle, FeedBundle, GroupMessageBundle};

mod app_state;
mod contact_routes;
mod discovery_routes;
mod ingest_routes;
mod limits;
mod messaging;
mod node_routes;
mod policy_routes;
mod publish_routes;
mod read_routes;
mod route_utils;
mod router_builder;
mod runtime;

pub use self::app_state::AppState;
pub(in crate::server) use self::limits::{
    MAX_ACTION_LEN, MAX_BIO_LEN, MAX_BUNDLE_JSON_BYTES, MAX_CHANNEL_LEN, MAX_GROUP_ID_LEN,
    MAX_MIME_LEN, MAX_NAME_LEN, MAX_RAW_PAYLOAD_BYTES, MAX_REASON_LEN, MAX_TEXT_LEN,
    MAX_UPLOAD_PAYLOAD_BYTES, MAX_URL_LEN,
};
use self::route_utils::{
    authorized, bad_request, current_unix_seconds, hex_to_pubkey, valid_channel, valid_pubkey_hex,
};
pub use self::runtime::serve;

pub fn build_router(state: AppState) -> Router {
    self::router_builder::build_router(state)
}

#[cfg(test)]
mod tests;
