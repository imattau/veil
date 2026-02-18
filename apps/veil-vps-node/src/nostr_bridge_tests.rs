use super::{
    connect_with_timeout, current_unix, is_event_already_seen, map_event_to_payload,
    note_seen_duplicate_checkpoint, parse_nostr_event_message, reconnect_backoff,
    reconnect_backoff_raw_ms, reserve_and_send_bridged_item, reserve_bridge_slot,
    verify_nostr_event_authenticity, BridgeEnqueueError, ConnectRelayError, NextFrameError,
    NostrBridgeState, NostrEvent, NOSTR_EVENT_MAX_FUTURE_SKEW_SECS,
};
use sha2::{Digest, Sha256};
use veil_crypto::signing::{NostrSigner, Signer};
use veil_schema_feed::FeedBundle;

#[test]
fn reconnect_backoff_raw_increases_exponentially_and_caps() {
    assert_eq!(reconnect_backoff_raw_ms(1, 2_000, 120_000), 2_000);
    assert_eq!(reconnect_backoff_raw_ms(2, 2_000, 120_000), 4_000);
    assert_eq!(reconnect_backoff_raw_ms(3, 2_000, 120_000), 8_000);
    assert_eq!(reconnect_backoff_raw_ms(4, 2_000, 120_000), 16_000);
    assert_eq!(reconnect_backoff_raw_ms(10, 2_000, 120_000), 120_000);
    assert_eq!(reconnect_backoff_raw_ms(20, 2_000, 120_000), 120_000);
}

#[test]
fn reconnect_backoff_adds_bounded_jitter() {
    use std::collections::HashSet;

    let mut observed = HashSet::new();
    for _ in 0..64 {
        let delay = reconnect_backoff(3, 2_000, 120_000).as_millis() as u64;
        assert!((6_400..=9_600).contains(&delay));
        observed.insert(delay);
    }
    assert!(observed.len() > 1);
}

#[test]
fn reconnect_backoff_cap_stays_within_bounds() {
    for _ in 0..32 {
        let delay = reconnect_backoff(20, 2_000, 120_000).as_millis() as u64;
        assert!((96_000..=120_000).contains(&delay));
    }
}

#[test]
fn parses_event_message() {
    let msg = r#"["EVENT","sub-a",{"id":"abc","pubkey":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","kind":1,"created_at":1700000000,"content":"hello veil"}]"#;
    let ev = parse_nostr_event_message(msg).expect("event should parse");
    assert_eq!(ev.id, "abc");
    assert_eq!(ev.kind, 1);
}

#[test]
fn parse_nostr_event_message_rejects_oversized_frame() {
    let oversized_content = "a".repeat(super::NOSTR_MAX_EVENT_FRAME_BYTES + 1);
    let frame = format!(
            "[\"EVENT\",\"sub-a\",{{\"id\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"pubkey\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"kind\":1,\"created_at\":1700000000,\"content\":\"{}\"}}]",
            oversized_content
        );
    assert!(parse_nostr_event_message(&frame).is_none());
}

#[test]
fn maps_kind1_to_feed_post_payload() {
    let msg = r#"["EVENT","sub-a",{"id":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","pubkey":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","kind":1,"created_at":1700000000,"content":"hello veil"}]"#;
    let ev = parse_nostr_event_message(msg).expect("event should parse");
    let payload =
        map_event_to_payload(&ev, "nostr", 32).expect("payload should map for kind1 event");
    let bundle: FeedBundle = serde_json::from_slice(&payload).expect("bundle should decode");
    match bundle {
        FeedBundle::Post(post) => {
            assert_eq!(post.text, "hello veil");
            assert_eq!(post.channel_id, "nostr");
        }
        _ => panic!("expected post bundle"),
    }
}

#[test]
fn state_dedup_and_capacity_trim_work() {
    let mut state = NostrBridgeState::load(None, 2, 1);
    assert!(state.should_accept_event("r", "a", 10));
    assert!(state.should_accept_event("r", "b", 11));
    assert!(!state.should_accept_event("r", "a", 12));
    assert!(state.should_accept_event("r", "c", 13));
    assert_eq!(state.seen_order.len(), 2);
    assert!(!state.seen_set.contains("a"));
    assert_eq!(state.relay_last_created_at.get("r").copied(), Some(13));
}

#[test]
fn state_rejects_future_timestamp_without_advancing_checkpoint() {
    let mut state = NostrBridgeState::load(None, 8, 1);
    let future = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS + 1);
    assert!(!state.should_accept_event("relay-a", "future-id", future));
    assert!(state.relay_last_created_at.get("relay-a").is_none());
    assert!(state.seen_set.is_empty());
    assert!(state.seen_order.is_empty());
    assert!(state.should_accept_event("relay-a", "ok-id", current_unix()));
}

#[test]
fn map_event_to_payload_rejects_invalid_pubkey_and_oversized_content() {
    let invalid_pubkey_msg = r#"["EVENT","sub-a",{"id":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","pubkey":"not-hex","kind":1,"created_at":1700000000,"content":"hello"}]"#;
    let invalid_pubkey_event =
        parse_nostr_event_message(invalid_pubkey_msg).expect("event should parse");
    assert!(map_event_to_payload(&invalid_pubkey_event, "nostr", 32).is_none());

    let oversized = "a".repeat(16 * 1024 + 1);
    let oversized_msg = format!(
            "[\"EVENT\",\"sub-a\",{{\"id\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"pubkey\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"kind\":1,\"created_at\":1700000000,\"content\":\"{}\"}}]",
            oversized
        );
    let oversized_event = parse_nostr_event_message(&oversized_msg).expect("event should parse");
    assert!(map_event_to_payload(&oversized_event, "nostr", 32).is_none());
}

#[test]
fn verify_nostr_event_authenticity_accepts_valid_signed_event() {
    let msg = signed_event_frame([0x33; 32], 1_700_000_123, "hello signed");
    let event = parse_nostr_event_message(&msg).expect("event should parse");
    assert!(verify_nostr_event_authenticity(&event));
}

#[test]
fn verify_nostr_event_authenticity_rejects_tampered_event() {
    let msg = signed_event_frame([0x34; 32], 1_700_000_123, "hello signed");
    let mut event = parse_nostr_event_message(&msg).expect("event should parse");
    event.content = "tampered".to_string();
    assert!(!verify_nostr_event_authenticity(&event));
}

#[test]
fn note_bridge_event_seen_accepts_once_and_tracks_checkpoint() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let event = NostrEvent {
        id: "11".repeat(32),
        pubkey: "not-a-hex-pubkey".to_string(),
        kind: 1,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };

    assert!(super::note_bridge_event_seen(&state, "relay-a", &event));
    assert!(!super::note_bridge_event_seen(&state, "relay-a", &event));
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(
        guard.relay_last_created_at.get("relay-a").copied(),
        Some(event.created_at)
    );
}

#[test]
fn note_bridge_event_seen_rejects_invalid_event_id_without_state_change() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let event = NostrEvent {
        id: "bad-id".to_string(),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };

    assert!(!super::note_bridge_event_seen(&state, "relay-a", &event));
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert!(guard.relay_last_created_at.get("relay-a").is_none());
    assert!(guard.seen_set.is_empty());
}

#[test]
fn note_bridge_event_seen_rejects_non_kind1_event_without_state_change() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let event = NostrEvent {
        id: "22".repeat(32),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 42,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "not kind1".to_string(),
        sig: None,
    };

    assert!(!super::note_bridge_event_seen(&state, "relay-a", &event));
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert!(guard.relay_last_created_at.get("relay-a").is_none());
    assert!(guard.seen_set.is_empty());
    assert!(guard.seen_order.is_empty());
}

#[test]
fn note_bridge_event_seen_dedupes_case_variant_event_ids() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let lower_id = "ab".repeat(32);
    let upper_id = lower_id.to_ascii_uppercase();
    let base_event = NostrEvent {
        id: lower_id,
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };
    let upper_event = NostrEvent {
        id: upper_id,
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: 1_700_000_124,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };

    assert!(super::note_bridge_event_seen(
        &state,
        "relay-a",
        &base_event
    ));
    assert!(!super::note_bridge_event_seen(
        &state,
        "relay-a",
        &upper_event
    ));
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(guard.seen_set.len(), 1);
    assert!(guard.seen_set.contains(&"ab".repeat(32)));
}

#[test]
fn is_event_already_seen_detects_case_variant_ids() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let base_event = NostrEvent {
        id: "cd".repeat(32),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };
    assert!(!is_event_already_seen(&state, &base_event));
    assert!(super::note_bridge_event_seen(
        &state,
        "relay-a",
        &base_event
    ));

    let mut variant = base_event;
    variant.id = "CD".repeat(32);
    assert!(is_event_already_seen(&state, &variant));
}

#[test]
fn note_seen_duplicate_checkpoint_updates_checkpoint_for_other_relays() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let event = NostrEvent {
        id: "ef".repeat(32),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };
    assert!(super::note_bridge_event_seen(&state, "relay-a", &event));
    note_seen_duplicate_checkpoint(&state, "relay-b", &event);
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(
        guard.relay_last_created_at.get("relay-b").copied(),
        Some(event.created_at)
    );
}

#[test]
fn note_seen_duplicate_checkpoint_does_not_checkpoint_future_timestamp() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let future = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS + 1);
    let event = NostrEvent {
        id: "aa".repeat(32),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: current_unix(),
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };
    assert!(super::note_bridge_event_seen(&state, "relay-a", &event));

    let future_variant = NostrEvent {
        id: "AA".repeat(32),
        created_at: future,
        ..event
    };
    note_seen_duplicate_checkpoint(&state, "relay-b", &future_variant);
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert!(guard.relay_last_created_at.get("relay-b").is_none());
}

#[tokio::test]
async fn connect_with_timeout_times_out_on_stalled_handshake() {
    use std::time::Duration;
    use tokio::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind should succeed");
    let addr = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
        let (_sock, _) = listener.accept().await.expect("accept should succeed");
        tokio::time::sleep(Duration::from_millis(250)).await;
    });

    let relay = format!("ws://{addr}");
    let result = connect_with_timeout(&relay, Duration::from_millis(50)).await;
    assert!(matches!(result, Err(ConnectRelayError::Timeout)));
    let _ = server.await;
}

#[tokio::test]
async fn reserve_bridge_slot_times_out_when_channel_is_full() {
    use std::time::Duration;
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    tx.send(super::BridgedItem {
        payload: vec![1, 2, 3],
        source_relay: "relay-a".to_string(),
        source_event_id: "11".repeat(32),
    })
    .await
    .expect("seed queue");
    let result = reserve_bridge_slot(&tx, Duration::from_millis(30)).await;
    assert!(matches!(result, Err(BridgeEnqueueError::Timeout)));
    let _ = rx.recv().await;
}

#[tokio::test]
async fn reserve_bridge_slot_reports_closed_when_receiver_is_gone() {
    use std::time::Duration;
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    drop(rx);
    let result = reserve_bridge_slot(&tx, Duration::from_millis(30)).await;
    assert!(matches!(result, Err(BridgeEnqueueError::Closed)));
}

#[tokio::test]
async fn reserve_and_send_does_not_checkpoint_when_queue_is_full() {
    use std::time::Duration;
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    tx.send(super::BridgedItem {
        payload: vec![9, 9, 9],
        source_relay: "relay-a".to_string(),
        source_event_id: "11".repeat(32),
    })
    .await
    .expect("seed full queue");

    let event = NostrEvent {
        id: "22".repeat(32),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };
    let result = reserve_and_send_bridged_item(
        &tx,
        &state,
        "relay-a",
        &event,
        vec![1, 2, 3],
        Duration::from_millis(30),
    )
    .await;
    assert!(matches!(result, Err(BridgeEnqueueError::Timeout)));
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert!(guard.relay_last_created_at.get("relay-a").is_none());
    assert!(guard.seen_set.is_empty());
    assert!(guard.seen_order.is_empty());
    drop(guard);
    let _ = rx.recv().await;
}

#[tokio::test]
async fn reserve_and_send_emits_canonical_source_event_id() {
    use std::time::Duration;
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let event = NostrEvent {
        id: "EF".repeat(32),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: 1_700_000_123,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };
    let result = reserve_and_send_bridged_item(
        &tx,
        &state,
        "relay-a",
        &event,
        vec![7, 8, 9],
        Duration::from_millis(30),
    )
    .await;
    assert!(matches!(result, Ok(true)));

    let item = rx.recv().await.expect("expected bridged item");
    assert_eq!(item.source_event_id, "ef".repeat(32));
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert!(guard.seen_set.contains(&"ef".repeat(32)));
}

#[test]
fn relay_checkpoint_needs_update_tracks_created_at_watermark() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
    let ts = 1_700_000_123;
    assert!(super::relay_checkpoint_needs_update(&state, "relay-a", ts));
    let event = NostrEvent {
        id: "11".repeat(32),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        kind: 1,
        created_at: ts,
        tags: serde_json::json!([]),
        content: "hello".to_string(),
        sig: None,
    };
    assert!(super::note_bridge_event_seen(&state, "relay-a", &event));
    assert!(!super::relay_checkpoint_needs_update(&state, "relay-a", ts));
    assert!(!super::relay_checkpoint_needs_update(
        &state,
        "relay-a",
        ts.saturating_sub(1)
    ));
    assert!(super::relay_checkpoint_needs_update(
        &state,
        "relay-a",
        ts.saturating_add(1)
    ));

    let future = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS + 1);
    assert!(!super::relay_checkpoint_needs_update(
        &state, "relay-a", future
    ));
}

#[tokio::test]
async fn next_text_frame_with_timeout_detects_stalled_relay() {
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind should succeed");
    let addr = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept should succeed");
        let _ws = accept_async(stream)
            .await
            .expect("ws accept should succeed");
        tokio::time::sleep(Duration::from_millis(250)).await;
    });

    let relay = format!("ws://{addr}");
    let (mut ws, _) = connect_with_timeout(&relay, Duration::from_secs(1))
        .await
        .expect("connect should succeed");
    let result = super::next_text_frame_with_timeout(&mut ws, Duration::from_millis(50)).await;
    assert!(matches!(result, Err(NextFrameError::Timeout)));
    let _ = server.await;
}

#[tokio::test]
async fn next_text_frame_with_timeout_detects_closed_relay() {
    use futures_util::SinkExt;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind should succeed");
    let addr = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept should succeed");
        let mut ws = accept_async(stream)
            .await
            .expect("ws accept should succeed");
        ws.send(Message::Close(None))
            .await
            .expect("close frame should send");
    });

    let relay = format!("ws://{addr}");
    let (mut ws, _) = connect_with_timeout(&relay, Duration::from_secs(1))
        .await
        .expect("connect should succeed");
    let result = super::next_text_frame_with_timeout(&mut ws, Duration::from_secs(1)).await;
    assert!(matches!(result, Err(NextFrameError::Closed)));
    let _ = server.await;
}

fn signed_event_frame(secret: [u8; 32], created_at: u64, content: &str) -> String {
    let signer = NostrSigner::from_secret(secret).expect("valid signer");
    let pubkey_hex = hex::encode(signer.public_key());
    let tags = serde_json::json!([]);
    let canonical = serde_json::json!([0, pubkey_hex, created_at, 1, tags, content]);
    let encoded = serde_json::to_vec(&canonical).expect("canonical event bytes");
    let id_bytes: [u8; 32] = Sha256::digest(encoded).into();
    let id_hex = hex::encode(id_bytes);
    let sig_hex = hex::encode(signer.sign(&id_bytes).expect("sign event id"));
    serde_json::json!([
        "EVENT",
        "sub-a",
        {
            "id": id_hex,
            "pubkey": pubkey_hex,
            "kind": 1,
            "created_at": created_at,
            "tags": [],
            "content": content,
            "sig": sig_hex
        }
    ])
    .to_string()
}

#[tokio::test]
#[ignore = "live network test; run explicitly"]
async fn live_relays_emit_bridge_item() {
    let relays = vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.snort.social".to_string(),
    ];
    let mut rx = super::start_nostr_bridge(super::NostrBridgeConfig {
        relays: relays.clone(),
        channel_id: "nostr-bridge".to_string(),
        namespace: 32,
        since: std::time::Duration::from_secs(3_600),
        state_path: None,
        max_seen_ids: 1_000,
        persist_every_updates: 1,
    });
    let item = tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv())
        .await
        .expect("timeout waiting for bridge item")
        .expect("expected at least one bridged nostr event from live relays");
    assert!(
        relays.contains(&item.source_relay),
        "unexpected relay source: {}",
        item.source_relay
    );
    assert_eq!(item.source_event_id.len(), 64);
    let bundle: FeedBundle =
        serde_json::from_slice(&item.payload).expect("bridge payload should decode");
    match bundle {
        FeedBundle::Post(post) => {
            assert!(!post.text.trim().is_empty());
            assert_eq!(post.channel_id, "nostr-bridge");
        }
        _ => panic!("expected post bundle"),
    }
}

#[tokio::test]
#[ignore = "live network test; run explicitly"]
async fn live_relays_restart_uses_persisted_dedupe_state() {
    let relays = vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.snort.social".to_string(),
    ];
    let temp = tempfile::tempdir().expect("tempdir");
    let state_path = temp.path().join("nostr-bridge-state.json");

    let first_id = {
        let mut rx = super::start_nostr_bridge(super::NostrBridgeConfig {
            relays: relays.clone(),
            channel_id: "nostr-bridge".to_string(),
            namespace: 32,
            since: std::time::Duration::from_secs(3_600),
            state_path: Some(state_path.clone()),
            max_seen_ids: 2_000,
            persist_every_updates: 1,
        });
        let item = tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv())
            .await
            .expect("timeout")
            .expect("expected bridged event on first run");
        item.source_event_id
    };

    let mut persisted_contains_first = false;
    for _ in 0..20 {
        if let Ok(bytes) = std::fs::read(&state_path) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let ids = value
                    .get("seen_event_ids")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                if ids.iter().any(|v| v.as_str() == Some(first_id.as_str())) {
                    persisted_contains_first = true;
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    assert!(
        persisted_contains_first,
        "expected first bridged event id to be persisted"
    );

    let mut rx2 = super::start_nostr_bridge(super::NostrBridgeConfig {
        relays: relays.clone(),
        channel_id: "nostr-bridge".to_string(),
        namespace: 32,
        since: std::time::Duration::from_secs(3_600),
        state_path: Some(state_path.clone()),
        max_seen_ids: 2_000,
        persist_every_updates: 1,
    });
    let item2 = tokio::time::timeout(std::time::Duration::from_secs(30), rx2.recv())
        .await
        .expect("timeout")
        .expect("expected bridged event on second run");
    assert_ne!(
        item2.source_event_id, first_id,
        "persisted dedupe should prevent replaying same first event id on restart"
    );
}
