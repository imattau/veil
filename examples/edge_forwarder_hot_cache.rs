use veil_core::Tag;
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::{NodeRuntime, NodeRuntimeCallbacks};
use veil_transport::adapter::{route_in_memory_outbound, InMemoryAdapter};

fn main() {
    // Transport note:
    // - fast lane: QUIC/UDP in production
    // - fallback lane: Tor/WebRTC in production
    // This example uses in-memory adapters to demonstrate runtime wiring.
    let cfg = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();
    let key = [0xA5_u8; 32];
    let mut state = veil_node::state::NodeState::default();

    // Forwarder subscribes to feed/rendezvous tags it should accelerate.
    let subscribed_tag: Tag = [0x11_u8; 32];
    state.subscriptions.insert(subscribed_tag);

    let mut forwarder = NodeRuntime::new(
        state,
        InMemoryAdapter::default(),
        InMemoryAdapter::default(),
        cfg,
        key,
        XChaCha20Poly1305Cipher,
        Ed25519Verifier,
    );

    // In production these peers are real endpoints.
    let peers = vec![
        "sender".to_string(),
        "peer-a".to_string(),
        "peer-b".to_string(),
        "peer-c".to_string(),
    ];

    // Simulate inbound traffic on fast lane.
    forwarder
        .fast_adapter
        .enqueue_inbound("sender", vec![0xDE, 0xAD, 0xBE, 0xEF]);

    let mut delivered = 0usize;
    let mut send_failures = 0usize;
    let _ = forwarder
        .tick_with_callbacks(
            1,
            &peers,
            &peers,
            NodeRuntimeCallbacks {
                on_delivered: Some(&mut |_root, _payload| delivered += 1),
                on_send_failure: Some(&mut |count| send_failures += count),
                ..NodeRuntimeCallbacks::default()
            },
        )
        .expect("edge-forwarder tick should succeed");

    // Optional local bridge to another node in simulation.
    let mut downstream_fast = InMemoryAdapter::default();
    let bridged = route_in_memory_outbound(
        &mut forwarder.fast_adapter,
        &mut downstream_fast,
        "forwarder",
    );

    println!("edge-forwarder+hot-cache profile");
    println!("fast_fanout: {}", forwarder.config.base_fast_fanout);
    println!("cache_cap: {}", forwarder.config.max_cache_shards);
    println!("bridged_messages: {bridged}");
    println!("delivered_events: {delivered}");
    println!("send_failures: {send_failures}");
}
