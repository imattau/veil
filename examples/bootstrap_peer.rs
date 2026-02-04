use veil_core::Tag;
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::NodeRuntime;
use veil_transport::adapter::InMemoryAdapter;

#[derive(Debug, Clone)]
struct BootstrapAnnouncement {
    endpoint: String,
    namespace: u16,
    bootstrap_tag: Tag,
}

fn publish_bootstrap_announcement(announcement: &BootstrapAnnouncement) {
    // Placeholder for real publication (NIP-11/NIP-65 style, DNS record, etc).
    println!(
        "bootstrap endpoint announced: endpoint={} namespace={} tag={:02x}{:02x}..",
        announcement.endpoint,
        announcement.namespace,
        announcement.bootstrap_tag[0],
        announcement.bootstrap_tag[1]
    );
}

fn main() {
    let cfg = NodeRuntimeConfig::bootstrap_peer_defaults();
    let key = [0xA5_u8; 32];
    let bootstrap_tag = [0x42_u8; 32];

    let mut state = veil_node::state::NodeState::default();
    // Bootstrap peer can subscribe to a shared bootstrap/discovery tag to help
    // initial connectivity, while keeping quotas conservative.
    state.subscriptions.insert(bootstrap_tag);

    let mut bootstrap = NodeRuntime::new(
        state,
        InMemoryAdapter::default(),
        InMemoryAdapter::default(),
        cfg,
        key,
        XChaCha20Poly1305Cipher,
        Ed25519Verifier,
    );

    let peers = vec!["new-node-a".to_string(), "new-node-b".to_string()];
    let announcement = BootstrapAnnouncement {
        endpoint: "quic://203.0.113.10:4433".to_string(),
        namespace: 1,
        bootstrap_tag,
    };
    publish_bootstrap_announcement(&announcement);

    let _ = bootstrap
        .tick(1, &peers, &peers)
        .expect("bootstrap tick should succeed");

    println!("bootstrap-peer profile");
    println!("fast_fanout: {}", bootstrap.config.base_fast_fanout);
    println!("cache_cap: {}", bootstrap.config.max_cache_shards);
}
