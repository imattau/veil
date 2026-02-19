use super::*;

#[test]
fn emits_feed_bundle_event() {
    let state = NodeState::new("0.1-test");
    let mut rx = state.subscribe_events();
    let bundle = veil_schema_feed::FeedBundle::Post(veil_schema_feed::PostBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_040,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: "aa".repeat(32),
        text: "hello".to_string(),
        media_roots: vec![],
        reply_to_root: None,
    });
    let payload = serde_json::to_vec(&bundle).expect("encode");
    state.emit_payload(&[0x11; 32], &payload, 32, 1, &[0x22; 32], 0);
    let first = rx.try_recv().expect("event");
    assert_eq!(first.event, "payload");
    let second = rx.try_recv().expect("event");
    assert_eq!(second.event, "feed_bundle");
}

#[test]
fn injects_local_feed_bundle() {
    let state = NodeState::new("0.1-test");
    let mut rx = state.subscribe_events();

    let bundle = serde_json::json!({
        "kind": "post",
        "text": "local post"
    });
    let root = [0x99; 32];

    state.inject_local_feed_bundle(bundle, root);

    let event = rx.try_recv().expect("should receive event");
    assert_eq!(event.event, "feed_bundle");
    assert_eq!(event.data["text"], "local post");
    assert_eq!(event.data["object_root"], hex::encode(root));
}

#[test]
fn emits_decrypted_payload_for_direct_message_envelope() {
    let state = NodeState::new("0.1-test");
    let mut rx = state.subscribe_events();
    let identity = state.identity();
    let recipient_pubkey_hex = hex::encode(identity.public_key);
    let sender_secret = [7u8; 32];
    let sender_pubkey_hex = hex::encode(
        veil_crypto::signing::NostrSigner::from_secret(sender_secret)
            .expect("sender")
            .public_key(),
    );
    let encrypted = encrypt_direct_message_payload(
        sender_secret,
        &sender_pubkey_hex,
        &recipient_pubkey_hex,
        b"hello secret",
    )
    .expect("encrypt");
    state.emit_payload(&[0x44; 32], &encrypted, 32, 1, &[0x22; 32], 0);
    let first = rx.try_recv().expect("payload event");
    assert_eq!(first.event, "payload");
    let payload_b64 = first
        .data
        .get("payload_b64")
        .and_then(|v| v.as_str())
        .expect("payload b64");
    let plaintext = base64::engine::general_purpose::STANDARD
        .decode(payload_b64)
        .expect("decode");
    assert_eq!(plaintext, b"hello secret");
}

#[test]
fn emits_decrypted_payload_for_group_message_envelope() {
    let state = NodeState::new("0.1-test");
    let mut rx = state.subscribe_events();
    let identity = state.identity();
    let local_pubkey_hex = hex::encode(identity.public_key);
    let sender_secret = [6u8; 32];
    let sender_pubkey_hex = hex::encode(
        veil_crypto::signing::NostrSigner::from_secret(sender_secret)
            .expect("sender")
            .public_key(),
    );
    let share = encrypt_group_key_share_payload(
        sender_secret,
        &sender_pubkey_hex,
        &local_pubkey_hex,
        "group-alpha",
        "k1",
        [2u8; 32],
    )
    .expect("share");
    state.emit_payload(&[0x46; 32], &share, 32, 1, &[0x22; 32], 0);
    let encrypted =
        encrypt_group_message_payload("group-alpha", "k1", [2u8; 32], b"hello group secret")
            .expect("encrypt");
    state.emit_payload(&[0x45; 32], &encrypted, 32, 1, &[0x22; 32], 0);
    let mut found = false;
    for _ in 0..6 {
        let event = rx.try_recv().expect("event");
        if event.event != "payload" {
            continue;
        }
        let payload_b64 = event
            .data
            .get("payload_b64")
            .and_then(|v| v.as_str())
            .expect("payload b64");
        let plaintext = base64::engine::general_purpose::STANDARD
            .decode(payload_b64)
            .expect("decode");
        if plaintext == b"hello group secret" {
            found = true;
            break;
        }
    }
    assert!(found);
}
