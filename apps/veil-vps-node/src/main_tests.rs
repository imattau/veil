use super::{
    encode_fallback_peers, merge_peers, normalize_settings_key, parse_fallback_peer_strings, Cli,
    Commands, FallbackPeer, SettingsCommands,
};

#[test]
fn parse_fallback_peer_strings_supports_websocket_server_prefix() {
    let parsed = parse_fallback_peer_strings(&[
        "wssrv:127.0.0.1:8080".to_string(),
        "ws:relay-a".to_string(),
        "tor:peer.onion:5000".to_string(),
    ]);
    assert!(parsed.contains(&FallbackPeer::WebSocketServer("127.0.0.1:8080".to_string())));
    assert!(parsed.contains(&FallbackPeer::WebSocket("relay-a".to_string())));
    assert!(parsed.contains(&FallbackPeer::Tor("peer.onion:5000".to_string())));
}

#[test]
fn merge_peers_deduplicates_and_caps() {
    let configured = vec!["a".to_string(), "b".to_string(), "a".to_string()];
    let discovered = vec![
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ];
    let merged = merge_peers(&configured, &discovered, 4);
    assert_eq!(merged, vec!["a", "b", "c", "d"]);
}

#[test]
fn encode_and_parse_roundtrip_keeps_websocket_server_peers() {
    let peers = vec![
        FallbackPeer::WebSocket("relay-a".to_string()),
        FallbackPeer::WebSocketServer("192.168.1.10:8080".to_string()),
        FallbackPeer::Tor("peer.onion:5000".to_string()),
    ];
    let encoded = encode_fallback_peers(&peers);
    let decoded = parse_fallback_peer_strings(&encoded);
    assert_eq!(decoded, peers);
}

#[test]
fn normalize_settings_key_supports_legacy_nostr_toggle_name() {
    assert_eq!(
        normalize_settings_key("VEIL_VPS_NOSTR_BRIDGE_ENABLE"),
        Some("VEIL_VPS_NOSTR_BRIDGE_ENABLED")
    );
    assert_eq!(
        normalize_settings_key("VEIL_VPS_NOSTR_BRIDGE_ENABLED"),
        Some("VEIL_VPS_NOSTR_BRIDGE_ENABLED")
    );
    assert_eq!(normalize_settings_key("VEIL_VPS_UNKNOWN"), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nostr_bridge_payload_publishes_and_android_receives_feed_bundle() {
    use sha2::{Digest, Sha256};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;
    use tokio::time::timeout;
    use tokio_tungstenite::tungstenite::{accept, Message};
    use veil_android_node::NodeState as AndroidNodeState;
    use veil_codec::object::OBJECT_FLAG_SIGNED;
    use veil_core::tags::derive_channel_feed_tag;
    use veil_core::{Epoch, Namespace};
    use veil_crypto::aead::XChaCha20Poly1305Cipher;
    use veil_crypto::signing::{NostrSigner, NostrVerifier, Signer};
    use veil_node::batch::FeedBatcher;
    use veil_node::config::NodeRuntimeConfig;
    use veil_node::publish::{publish_queue_tick_multi_lane, PublishQueueTickParams};
    use veil_node::runtime::{
        pump_multi_lane_tick_with_config, ConfigMultiLanePumpParams, RuntimeStats,
    };
    use veil_transport::adapter::{route_in_memory_outbound, InMemoryAdapter};

    let relay_listener = TcpListener::bind("127.0.0.1:0").expect("bind relay");
    let relay_addr = relay_listener.local_addr().expect("relay addr");
    let relay_url = format!("ws://{relay_addr}");
    let relay_signer = NostrSigner::from_secret([0x21; 32]).expect("valid relay signer");
    let relay_pubkey = hex::encode(relay_signer.public_key());
    let relay_created_at = 1_700_000_123u64;
    let relay_content = "bridge e2e hello";
    let relay_canonical = serde_json::json!([
        0,
        relay_pubkey,
        relay_created_at,
        1,
        serde_json::json!([]),
        relay_content
    ]);
    let relay_event_id = hex::encode(Sha256::digest(relay_canonical.to_string().as_bytes()));
    let relay_event_id_bytes = hex::decode(&relay_event_id).expect("relay event id hex");
    let relay_event_id_bytes =
        <[u8; 32]>::try_from(relay_event_id_bytes.as_slice()).expect("relay event id bytes");
    let relay_sig = hex::encode(
        relay_signer
            .sign(&relay_event_id_bytes)
            .expect("relay event should sign"),
    );
    let event_message = serde_json::json!([
        "EVENT",
        "veil-bridge",
        {
            "id": relay_event_id,
            "pubkey": relay_pubkey,
            "kind": 1,
            "created_at": relay_created_at,
            "tags": [],
            "content": relay_content,
            "sig": relay_sig
        }
    ])
    .to_string();

    let relay_thread = thread::spawn(move || {
        let (stream, _) = relay_listener.accept().expect("accept relay connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("set write timeout");
        let mut ws = accept(stream).expect("ws handshake");
        let req = ws.read().expect("read nostr req");
        let req_text = match req {
            Message::Text(text) => text,
            other => panic!("expected text req, got {other:?}"),
        };
        assert!(req_text.contains("\"REQ\""), "expected nostr REQ frame");
        ws.send(Message::Text(event_message))
            .expect("write nostr event");
    });

    let mut bridge_rx =
        crate::nostr_bridge::start_nostr_bridge(crate::nostr_bridge::NostrBridgeConfig {
            relays: vec![relay_url.clone()],
            channel_id: "nostr-bridge".to_string(),
            namespace: 32,
            since: Duration::from_secs(600),
            state_path: None,
            max_seen_ids: 128,
            persist_every_updates: 1,
        });

    let bridged = timeout(Duration::from_secs(10), bridge_rx.recv())
        .await
        .expect("bridge recv timeout")
        .expect("bridge should emit item");
    assert_eq!(bridged.source_relay, relay_url);

    relay_thread.join().expect("relay thread join");

    let signer = NostrSigner::from_secret([0x33; 32]).expect("valid signer");
    let publisher_pubkey = signer.public_key();
    let decrypt_key = [0x42; 32];
    let namespace = Namespace(32);
    let tag = derive_channel_feed_tag(&publisher_pubkey, namespace, "nostr-bridge");
    let cfg = NodeRuntimeConfig::builder()
        .base_fast_fanout(1)
        .base_fallback_fanout(1)
        .fallback_redundancy_fanout(1)
        .build();

    let mut sender_state = veil_node::state::NodeState::default();
    let mut sender_fast = InMemoryAdapter::default();
    let mut sender_fallback = InMemoryAdapter::default();
    let mut sender_batcher = FeedBatcher::default();
    let mut receiver_state = veil_node::state::NodeState::default();
    receiver_state.subscriptions.insert(tag);
    let mut receiver_fast = InMemoryAdapter::default();
    let mut receiver_fallback = InMemoryAdapter::default();
    let mut receiver_stats = RuntimeStats::default();
    let peers = vec!["receiver".to_string()];

    sender_batcher.enqueue(bridged.payload.clone());
    let _ = publish_queue_tick_multi_lane(
        &mut sender_state,
        &mut sender_fast,
        &mut sender_fallback,
        &mut sender_batcher,
        PublishQueueTickParams {
            namespace,
            epoch: Epoch(1),
            tag,
            encrypt_key: &decrypt_key,
            now_step: 1,
            flags: OBJECT_FLAG_SIGNED,
            interactive_flush: false,
            fast_peers: &peers,
            fallback_peers: &peers,
        },
        &cfg,
        &XChaCha20Poly1305Cipher,
        Some(&signer),
    );

    route_in_memory_outbound(&mut sender_fast, &mut receiver_fast, "vps");
    route_in_memory_outbound(&mut sender_fallback, &mut receiver_fallback, "vps");

    let mut delivered_payload = None;
    for step in 1..=12 {
        let event = pump_multi_lane_tick_with_config(
            &mut receiver_state,
            &mut receiver_fast,
            &mut receiver_fallback,
            ConfigMultiLanePumpParams {
                fast_peers: &peers,
                fallback_peers: &peers,
                now_step: step,
                decrypt_key: &decrypt_key,
                config: &cfg,
                stats: &mut receiver_stats,
            },
            &XChaCha20Poly1305Cipher,
            &NostrVerifier,
        )
        .expect("pump ok");
        if let Some(veil_node::receive::ReceiveEvent::Delivered {
            payload,
            tag: delivered_tag,
            ..
        }) = event
        {
            if delivered_tag == tag {
                delivered_payload = Some(payload);
                break;
            }
        }
    }
    let delivered_payload = delivered_payload.expect("expected delivered bridged payload");

    let android_state = AndroidNodeState::new("0.1-test");
    android_state.emit_payload(&[0xAB; 32], &delivered_payload, 32, 1, &tag, 0);
    let (events, _) = android_state.subscribe_events_since(Some(0));

    assert!(
        events.iter().any(|event| event.event == "payload"),
        "android node should emit payload event"
    );
    let feed_event = events
        .iter()
        .find(|event| event.event == "feed_bundle")
        .expect("android node should emit feed_bundle event");
    assert!(
        feed_event.data.to_string().contains("bridge e2e hello"),
        "feed bundle should carry bridged nostr text"
    );
}

#[test]
fn test_cli_parsing() {
    use clap::Parser;

    // Test 'run' (implicit)
    let cli = Cli::try_parse_from(["veil-vps-node"]).unwrap();
    assert!(cli.command.is_none());

    // Test 'run' (explicit)
    let cli = Cli::try_parse_from(["veil-vps-node", "run"]).unwrap();
    match cli.command {
        Some(Commands::Run) => {}
        _ => panic!("expected Run command"),
    }

    // Test 'settings'
    let cli = Cli::try_parse_from(["veil-vps-node", "settings", "list"]).unwrap();
    match cli.command {
        Some(Commands::Settings {
            action: SettingsCommands::List,
            ..
        }) => {}
        _ => panic!("expected Settings List command"),
    }

    // Test 'settings' with custom DB
    let cli = Cli::try_parse_from([
        "veil-vps-node",
        "settings",
        "--db",
        "custom.db",
        "get",
        "key",
    ])
    .unwrap();
    match cli.command {
        Some(Commands::Settings {
            ref db,
            action: SettingsCommands::Get { ref key },
        }) => {
            assert_eq!(db, &std::path::PathBuf::from("custom.db"));
            assert_eq!(key, "key");
        }
        _ => panic!("expected Settings Get command"),
    }
}
