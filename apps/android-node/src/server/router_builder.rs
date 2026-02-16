use axum::{
    routing::{get, post},
    Router,
};

use super::contact_routes::{contact_delete, contact_import, contact_list, contact_self};
use super::discovery_routes::{discovery_announce, discovery_gossip, discovery_lookup};
use super::ingest_routes::{publish, publish_object};
use super::messaging::{
    publish_direct_message, publish_direct_message_text, publish_group_message,
    publish_group_message_text, share_group_key,
};
use super::node_routes::{
    export_identity, feed, health, identity, import_identity, rotate_identity, status, subscribe,
    subscriptions, unsubscribe,
};
use super::policy_routes::{
    policy_block, policy_explain, policy_lists, policy_mute, policy_summary, policy_trust,
    policy_unblock, policy_unmute, policy_untrust, update_policy_config,
};
use super::publish_routes::{
    publish_app_preferences, publish_block, publish_deletion, publish_follow,
    publish_group_metadata, publish_list, publish_live_status, publish_media, publish_mute,
    publish_poll, publish_poll_vote, publish_post, publish_profile, publish_reaction,
    publish_repost, publish_zap,
};
use super::read_routes::{events_ws, fetch_object, fetch_shard};
use super::AppState;

pub(super) fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/feed", get(feed))
        .route("/subscriptions", get(subscriptions))
        .route("/identity", get(identity))
        .route("/identity/rotate", post(rotate_identity))
        .route("/identity/export", get(export_identity))
        .route("/identity/import", post(import_identity))
        .route("/publish", post(publish))
        .route("/publish_object", post(publish_object))
        .route("/profile", post(publish_profile))
        .route("/post", post(publish_post))
        .route("/reaction", post(publish_reaction))
        .route("/direct_message_text", post(publish_direct_message_text))
        .route("/direct_message", post(publish_direct_message))
        .route("/group_message_text", post(publish_group_message_text))
        .route("/group_key/share", post(share_group_key))
        .route("/group_message", post(publish_group_message))
        .route("/media", post(publish_media))
        .route("/follow", post(publish_follow))
        .route("/mute", post(publish_mute))
        .route("/block", post(publish_block))
        .route("/list", post(publish_list))
        .route("/group_metadata", post(publish_group_metadata))
        .route("/zap", post(publish_zap))
        .route("/app_preferences", post(publish_app_preferences))
        .route("/deletion", post(publish_deletion))
        .route("/repost", post(publish_repost))
        .route("/poll", post(publish_poll))
        .route("/poll_vote", post(publish_poll_vote))
        .route("/live_status", post(publish_live_status))
        .route("/subscribe", post(subscribe))
        .route("/unsubscribe", post(unsubscribe))
        .route("/policy", get(policy_summary))
        .route("/policy/lists", get(policy_lists))
        .route("/policy/explain", post(policy_explain))
        .route("/policy/config", post(update_policy_config))
        .route("/policy/trust", post(policy_trust))
        .route("/policy/untrust", post(policy_untrust))
        .route("/policy/mute", post(policy_mute))
        .route("/policy/unmute", post(policy_unmute))
        .route("/policy/block", post(policy_block))
        .route("/policy/unblock", post(policy_unblock))
        .route("/contact", get(contact_list).post(contact_import))
        .route("/contact/delete", post(contact_delete))
        .route("/contact/self", get(contact_self))
        .route("/discovery/announce", post(discovery_announce))
        .route("/discovery/lookup", post(discovery_lookup))
        .route("/discovery/gossip", post(discovery_gossip))
        .route("/shard/:id", get(fetch_shard))
        .route("/object/:root", get(fetch_object))
        .route("/events", get(events_ws))
        .with_state(state)
}
