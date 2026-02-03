use veil_core::{Epoch, Namespace};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier};
use veil_node::batch::FeedBatcher;
use veil_node::config::NodeRuntimeConfig;
use veil_node::publish::{
    publish_service_tick_multi_lane, PublishQueueTickParams, PublishServiceTickParams,
};
use veil_node::runtime::{
    pump_multi_lane_tick_with_config, ConfigMultiLanePumpParams, RuntimeStats,
};
use veil_node::state::NodeState;
use veil_transport::adapter::InMemoryAdapter;

fn route_outbound(
    from_fast: &mut InMemoryAdapter,
    from_fallback: &mut InMemoryAdapter,
    to_fast: &mut InMemoryAdapter,
    from_peer: &str,
) -> usize {
    let mut moved = 0usize;
    for (_, bytes) in from_fast.take_outbound() {
        to_fast.enqueue_inbound(from_peer.to_string(), bytes);
        moved += 1;
    }
    for (_, bytes) in from_fallback.take_outbound() {
        to_fast.enqueue_inbound(from_peer.to_string(), bytes);
        moved += 1;
    }
    moved
}

#[test]
fn e2e_auto_emit_ack_clears_pending_ack_state() {
    let mut publisher = NodeState::default();
    let mut subscriber = NodeState::default();

    let tag = [0x44_u8; 32];
    publisher.subscriptions.insert(tag);
    subscriber.subscriptions.insert(tag);

    let mut pub_fast = InMemoryAdapter::default();
    let mut pub_fallback = InMemoryAdapter::default();
    let mut sub_fast = InMemoryAdapter::default();
    let mut sub_fallback = InMemoryAdapter::default();

    let pub_peers = vec!["sub".to_string()];
    let sub_peers = vec!["pub".to_string()];

    let mut batcher = FeedBatcher::default();
    batcher.enqueue(b"payload that requests ack".to_vec());

    let mut cfg = NodeRuntimeConfig::default();
    cfg.ack_initial_timeout_steps = 10_000;
    let cipher = XChaCha20Poly1305Cipher;
    let verifier = Ed25519Verifier;
    let signer = Ed25519Signer::from_secret([0x22_u8; 32]);
    let key = [0xAA_u8; 32];

    let publish_out = publish_service_tick_multi_lane(
        &mut publisher,
        &mut pub_fast,
        &mut pub_fallback,
        PublishServiceTickParams {
            batcher: &mut batcher,
            publish: PublishQueueTickParams {
                namespace: Namespace(7),
                epoch: Epoch(11),
                tag,
                encrypt_key: &key,
                now_step: 1,
                flags: veil_codec::object::OBJECT_FLAG_SIGNED
                    | veil_codec::object::OBJECT_FLAG_ACK_REQUESTED,
                interactive_flush: true,
                fast_peers: &pub_peers,
                fallback_peers: &pub_peers,
            },
        },
        &cfg,
        &cipher,
        Some(&signer),
    )
    .expect("publish tick should succeed");

    let published = publish_out
        .published
        .expect("one object should be published");
    assert!(
        published.ack_tracked,
        "ack_requested must register pending state"
    );
    assert!(
        publisher.pending_acks.contains_key(&published.object_root),
        "publisher should track pending ack after publish",
    );

    let moved_to_subscriber =
        route_outbound(&mut pub_fast, &mut pub_fallback, &mut sub_fast, "pub");
    assert!(
        moved_to_subscriber > 0,
        "subscriber should receive object shards"
    );

    let mut sub_stats = RuntimeStats::default();
    for step in 2..40 {
        let _ = pump_multi_lane_tick_with_config(
            &mut subscriber,
            &mut sub_fast,
            &mut sub_fallback,
            ConfigMultiLanePumpParams {
                fast_peers: &sub_peers,
                fallback_peers: &sub_peers,
                now_step: step,
                decrypt_key: &key,
                config: &cfg,
                stats: &mut sub_stats,
            },
            &cipher,
            &verifier,
        )
        .expect("subscriber tick should succeed");
    }

    let moved_to_publisher = route_outbound(&mut sub_fast, &mut sub_fallback, &mut pub_fast, "sub");
    assert!(
        moved_to_publisher > 0,
        "subscriber should auto-emit ack shards back to publisher",
    );

    let mut pub_stats = RuntimeStats::default();
    for step in 40..120 {
        let _ = pump_multi_lane_tick_with_config(
            &mut publisher,
            &mut pub_fast,
            &mut pub_fallback,
            ConfigMultiLanePumpParams {
                fast_peers: &pub_peers,
                fallback_peers: &pub_peers,
                now_step: step,
                decrypt_key: &key,
                config: &cfg,
                stats: &mut pub_stats,
            },
            &cipher,
            &verifier,
        )
        .expect("publisher tick should succeed");
    }

    assert!(
        !publisher.pending_acks.contains_key(&published.object_root),
        "pending ack should clear once ack object is received",
    );
    assert!(
        pub_stats.ack_messages >= 1,
        "publisher should decode at least one ack payload object",
    );
}
