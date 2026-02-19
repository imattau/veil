use super::{
    decode_hex_32, default_protocol_config, derive_server_name, erasure_mode_from_shards,
    is_ws_url, ProtocolEngine,
};
use crate::api::ContactBundle;
use veil_core::types::NAMESPACE_PUBLIC_FEED;
use veil_core::{Epoch, Namespace, ObjectRoot};
use veil_crypto::signing::{NostrSigner, Signer};
use veil_fec::profile::ErasureCodingMode;
use veil_fec::sharder::{derive_object_root, object_to_shards_with_mode};

#[test]
fn default_protocol_config_enables_network_efficiency_policies() {
    let cfg = default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "peer-a".to_string(),
        32,
        [0x11; 32],
        [0xAA; 32],
        NostrSigner::from_secret([0x22; 32]).expect("valid nostr test key"),
    );

    assert!(cfg.runtime_config.probabilistic_forwarding.enabled);
    assert!(cfg.runtime_config.bloom_exchange.enabled);
    assert_eq!(
        cfg.runtime_config
            .erasure_mode_for_namespace(NAMESPACE_PUBLIC_FEED),
        ErasureCodingMode::Systematic
    );
}

#[test]
fn reconstruct_mode_prefers_wire_header_mode() {
    let object = b"public-feed-systematic".to_vec();
    let root = derive_object_root(&object);
    let shards = object_to_shards_with_mode(
        &object,
        Namespace(32),
        Epoch(1),
        [0x44; 32],
        root,
        ErasureCodingMode::Systematic,
    )
    .expect("systematic shards");

    let mode = erasure_mode_from_shards(&shards, ErasureCodingMode::HardenedNonSystematic);
    assert_eq!(mode, ErasureCodingMode::Systematic);
}

#[test]
fn derive_server_name_parses_urls_and_host_port_fallback() {
    assert_eq!(
        derive_server_name("quic://node.example:9443"),
        Some("node.example".to_string())
    );
    assert_eq!(
        derive_server_name("wss://relay.example/ws"),
        Some("relay.example".to_string())
    );
    assert_eq!(
        derive_server_name("127.0.0.1:9443"),
        Some("127.0.0.1".to_string())
    );
    assert_eq!(derive_server_name("[::1]:9443"), Some("::1".to_string()));
    assert_eq!(derive_server_name(""), None);
    assert_eq!(derive_server_name(":"), None);
}

#[test]
fn is_ws_url_accepts_only_ws_and_wss() {
    assert!(is_ws_url("ws://relay.example/ws"));
    assert!(is_ws_url("wss://relay.example/ws"));
    assert!(!is_ws_url("https://relay.example/ws"));
    assert!(!is_ws_url("ws://"));
    assert!(!is_ws_url("relay.example"));
}

#[test]
fn decode_hex_32_requires_exact_length_and_hex() {
    assert_eq!(decode_hex_32(&"aa".repeat(32)), Some([0xAA; 32]));
    assert!(decode_hex_32("aa").is_none());
    assert!(decode_hex_32(&"zz".repeat(32)).is_none());
}

#[tokio::test]
async fn add_contact_normalizes_and_deduplicates_dynamic_endpoints() {
    let signer = NostrSigner::from_secret([0x33; 32]).expect("valid secret");
    let pubkey = signer.public_key();
    let protocol = ProtocolEngine::new(default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "node-a".to_string(),
        32,
        pubkey,
        [0xAA; 32],
        signer,
    ))
    .expect("protocol init");

    let contact = ContactBundle {
        peer_id: " peer-a ".to_string(),
        ws_url: Some(" ws://example.com/ws ".to_string()),
        quic_addr: Some(" 127.0.0.1:9444 ".to_string()),
        pubkey_hex: "11".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    };
    protocol.add_contact(&contact).await;

    let duplicate = ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://example.com/ws".to_string()),
        quic_addr: Some("127.0.0.1:9444".to_string()),
        pubkey_hex: "11".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    };
    protocol.add_contact(&duplicate).await;

    let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
    let peer_map = protocol.dynamic_peer_map_snapshot().await;

    assert_eq!(fast, vec!["127.0.0.1:9444".to_string()]);
    assert_eq!(fallback, vec!["ws://example.com/ws".to_string()]);
    assert_eq!(peer_map.len(), 1);
    assert!(peer_map.contains_key("peer-a"));
}

#[tokio::test]
async fn sync_contacts_filters_invalid_dynamic_entries() {
    let signer = NostrSigner::from_secret([0x44; 32]).expect("valid secret");
    let pubkey = signer.public_key();
    let protocol = ProtocolEngine::new(default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "node-b".to_string(),
        32,
        pubkey,
        [0xBB; 32],
        signer,
    ))
    .expect("protocol init");

    let too_long_endpoint = "x".repeat(1024 + 1);
    let contacts = vec![
        ContactBundle {
            peer_id: " peer-a ".to_string(),
            ws_url: Some(" ws://peer-a/ws ".to_string()),
            quic_addr: Some(" 127.0.0.1:9555 ".to_string()),
            pubkey_hex: "22".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        },
        ContactBundle {
            peer_id: " ".to_string(),
            ws_url: Some(too_long_endpoint.clone()),
            quic_addr: Some(too_long_endpoint),
            pubkey_hex: "33".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        },
    ];

    protocol.sync_contacts(&contacts).await;

    let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
    let peer_map = protocol.dynamic_peer_map_snapshot().await;

    assert_eq!(fast, vec!["127.0.0.1:9555".to_string()]);
    assert_eq!(fallback, vec!["ws://peer-a/ws".to_string()]);
    assert_eq!(peer_map.len(), 1);
    assert!(peer_map.contains_key("peer-a"));
}

#[tokio::test]
async fn add_contact_caps_dynamic_peer_storage() {
    let signer = NostrSigner::from_secret([0x51; 32]).expect("valid secret");
    let pubkey = signer.public_key();
    let protocol = ProtocolEngine::new(default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "node-c".to_string(),
        32,
        pubkey,
        [0xCC; 32],
        signer,
    ))
    .expect("protocol init");

    for index in 0..(super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS + 8) {
        let contact = ContactBundle {
            peer_id: format!("peer-{index}"),
            ws_url: Some(format!("ws://peer-{index}.example/ws")),
            quic_addr: Some(format!("127.0.0.1:{}", 20_000 + index)),
            pubkey_hex: format!("{:064x}", index + 1),
            rpc_url: None,
            lan_addrs: Vec::new(),
        };
        protocol.add_contact(&contact).await;
    }

    let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
    let peer_map = protocol.dynamic_peer_map_snapshot().await;

    assert_eq!(fast.len(), super::dynamic_peers::MAX_DYNAMIC_FAST_PEERS);
    assert_eq!(
        fallback.len(),
        super::dynamic_peers::MAX_DYNAMIC_FALLBACK_PEERS
    );
    assert_eq!(
        peer_map.len(),
        super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS
    );
    assert!(!fast.contains(&format!(
        "127.0.0.1:{}",
        20_000 + super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS + 7
    )));
    assert!(!peer_map.contains_key(&format!(
        "peer-{}",
        super::dynamic_peers::MAX_DYNAMIC_PEER_BINDINGS + 7
    )));
}

#[tokio::test]
async fn media_round_trip_integration() {
    let signer = NostrSigner::from_secret([0x42; 32]).unwrap();
    let pubkey = signer.public_key();
    let key = [0xAA; 32];

    let cfg = default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "node-a".to_string(),
        32,
        pubkey,
        key,
        signer,
    );

    let protocol = ProtocolEngine::new(cfg).expect("protocol init");

    // 1. Build a mock media object (e.g. image bytes)
    let media_payload = b"this is a mock image payload".to_vec();
    let (encoded_object, wire_root): (Vec<u8>, ObjectRoot) = protocol
        .build_object(media_payload.clone(), 32, 0)
        .await
        .expect("build object");

    // 2. Inject it into local cache (simulating it being available after publish)
    let injected_root: ObjectRoot = protocol
        .inject_object(encoded_object)
        .await
        .expect("inject object");
    assert_eq!(injected_root, wire_root);

    // 3. Try to reconstruct by wire_root (fast path)
    let reconstructed_wire: Vec<u8> = protocol
        .reconstruct_payload(wire_root)
        .await
        .expect("reconstruct by wire_root");
    // Note: when matched by wire_root, it returns the whole payload (CBOR batch)
    assert!(reconstructed_wire.len() > media_payload.len());

    // 4. Try to reconstruct by media_payload hash (slow path / content match)
    let content_hash = veil_fec::sharder::derive_object_root(&media_payload);
    let reconstructed_content: Vec<u8> = protocol
        .reconstruct_payload(content_hash)
        .await
        .expect("reconstruct by content_hash");

    // Should return the UNWRAPPED media bytes
    assert_eq!(reconstructed_content, media_payload);
}
