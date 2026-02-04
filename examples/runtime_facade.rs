use veil_core::{Epoch, Namespace};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier};
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::{
    NodeRuntime, NodeRuntimeCallbacks, PublisherRuntime, PublisherTickOptionsInput,
};
use veil_transport::adapter::{route_in_memory_outbound, InMemoryAdapter};

fn main() {
    let tag = [0x11_u8; 32];
    let peers = vec!["peer-a".to_string(), "peer-b".to_string()];
    let key = [0xA5_u8; 32];

    let cfg = NodeRuntimeConfig::builder()
        .base_fast_fanout(2)
        .base_fallback_fanout(1)
        .ttl_steps(10_000)
        .build();

    let mut publisher = PublisherRuntime::new(
        veil_node::state::NodeState::default(),
        veil_node::batch::FeedBatcher::default(),
        InMemoryAdapter::default(),
        InMemoryAdapter::default(),
        cfg.clone(),
        key,
        Some(Ed25519Signer::from_secret([0x42; 32])),
        XChaCha20Poly1305Cipher,
    );
    publisher.enqueue(b"hello from publisher runtime facade".to_vec());
    publisher
        .tick_with_options(PublisherTickOptionsInput {
            namespace: Namespace(7),
            epoch: Epoch(8),
            tag,
            now_step: 1,
            options: veil_node::publish::PublishOptions::signed().with_ack_requested(true),
            interactive_flush: true,
            fast_peers: &peers,
            fallback_peers: &peers,
        })
        .expect("publisher tick should succeed");

    let mut subscriber_state = veil_node::state::NodeState::default();
    subscriber_state.subscriptions.insert(tag);
    let mut subscriber = NodeRuntime::new(
        subscriber_state,
        InMemoryAdapter::default(),
        InMemoryAdapter::default(),
        cfg,
        key,
        XChaCha20Poly1305Cipher,
        Ed25519Verifier,
    );

    let moved_a = route_in_memory_outbound(
        &mut publisher.fast_adapter,
        &mut subscriber.fast_adapter,
        "publisher",
    );
    let moved_b = route_in_memory_outbound(
        &mut publisher.fallback_adapter,
        &mut subscriber.fallback_adapter,
        "publisher",
    );

    let mut delivered = 0usize;
    let mut ack_clears = 0usize;
    for step in 2..40 {
        let _ = subscriber
            .tick_with_callbacks(
                step,
                &peers,
                &peers,
                NodeRuntimeCallbacks {
                    on_delivered: Some(&mut |_root, _payload| delivered += 1),
                    on_ack_cleared: Some(&mut |count| ack_clears += count),
                    ..NodeRuntimeCallbacks::default()
                },
            )
            .expect("subscriber tick should succeed");
    }

    println!(
        "runtime facade demo: moved={} delivered={} ack_clears={}",
        moved_a + moved_b,
        delivered,
        ack_clears
    );
}
