use super::*;

fn make_contact(peer_id: &str, pubkey_hex: &str) -> ContactBundle {
    ContactBundle {
        peer_id: peer_id.to_string(),
        ws_url: None,
        quic_addr: None,
        pubkey_hex: pubkey_hex.to_string(),
        rpc_url: None,
        lan_addrs: Vec::new(),
    }
}

#[test]
fn discovery_message_roundtrip() {
    let contact = make_contact(
        "peer-x",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    );
    let msg = DiscoveryMessage::announce(contact);
    let encoded = serde_json::to_vec(&msg).unwrap();
    let decoded: DiscoveryMessage = serde_json::from_slice(&encoded).unwrap();
    assert!(matches!(decoded.kind, DiscoveryKind::Announce));
    assert!(decoded.contact.is_some());
}

#[test]
fn sanitize_contact_rejects_invalid_pubkey() {
    let contact = make_contact("peer-x", "zzzz");
    assert!(sanitize_contact(contact).is_none());
}

#[test]
fn discovery_announce_rejects_invalid_contact() {
    let state = NodeState::new("test");
    let request = DiscoveryAnnounceRequest {
        contact: ContactBundle {
            peer_id: "peer-x".to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: "abcd".to_string(),
            rpc_url: None,
            lan_addrs: Vec::new(),
        },
    };
    let response = handle_discovery_announce(&state, request, 16);
    assert!(!response.accepted);
    assert!(response.neighbors.is_empty());
    assert!(state.contacts().is_empty());
}

#[test]
fn discovery_gossip_limits_and_filters_contacts() {
    let state = NodeState::new("test");
    let mut contacts = vec![ContactBundle {
        peer_id: String::new(),
        ws_url: Some("ws://invalid".to_string()),
        quic_addr: None,
        pubkey_hex: "ff".repeat(32),
        rpc_url: None,
        lan_addrs: vec!["".to_string()],
    }];
    for i in 0..(DISCOVERY_MAX_CONTACTS + 20) {
        contacts.push(ContactBundle {
            peer_id: format!("peer-{i}"),
            ws_url: Some(" ws://example.com/ws ".to_string()),
            quic_addr: Some(" 127.0.0.1:9444 ".to_string()),
            pubkey_hex: format!("{:064x}", i + 1),
            rpc_url: Some(" https://example.com/rpc ".to_string()),
            lan_addrs: vec![" ".to_string(), "10.0.0.1:9333".to_string()],
        });
    }
    let response = handle_discovery_gossip(&state, DiscoveryGossipRequest { contacts }, 256);
    assert_eq!(state.contacts().len(), DISCOVERY_MAX_CONTACTS);
    assert_eq!(response.contacts.len(), DISCOVERY_MAX_CONTACTS);
    assert!(state
        .contacts()
        .iter()
        .all(|contact| contact.pubkey_hex.len() == 64
            && contact.pubkey_hex.chars().all(|c| c.is_ascii_hexdigit())));
}

#[test]
fn discovery_lookup_caps_requested_limit() {
    let state = NodeState::new("test");
    for i in 0..(DISCOVERY_MAX_CONTACTS + 40) {
        state.add_contact(ContactBundle {
            peer_id: format!("peer-{i}"),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: format!("{:064x}", i + 1),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });
    }
    let response = handle_discovery_lookup(
        &state,
        DiscoveryLookupRequest {
            peer_id: Some("peer-z".to_string()),
            pubkey_hex: None,
            limit: Some(usize::MAX),
        },
    );
    assert_eq!(response.contacts.len(), DISCOVERY_MAX_CONTACTS);
}

#[test]
fn with_auth_header_sets_token_header() {
    let client = reqwest::Client::new();
    let request = with_auth_header(client.post("http://example.com"), Some("secret-token"))
        .build()
        .expect("request");
    let header = request.headers().get("x-veil-token").expect("header");
    assert_eq!(header, "secret-token");
}

#[test]
fn with_auth_header_skips_empty_token() {
    let client = reqwest::Client::new();
    let request = with_auth_header(client.post("http://example.com"), Some("  "))
        .build()
        .expect("request");
    assert!(request.headers().get("x-veil-token").is_none());
}

#[test]
fn bounded_gossip_contacts_is_capped_and_non_zero() {
    assert_eq!(bounded_gossip_contacts(0), 1);
    assert_eq!(bounded_gossip_contacts(1), 1);
    assert_eq!(bounded_gossip_contacts(24), 24);
    assert_eq!(
        bounded_gossip_contacts(DISCOVERY_MAX_CONTACTS + 999),
        DISCOVERY_MAX_CONTACTS
    );
}

#[test]
fn discovery_gossip_response_caps_requested_max_contacts() {
    let state = NodeState::new("test");
    for i in 0..(DISCOVERY_MAX_CONTACTS + 40) {
        state.add_contact(ContactBundle {
            peer_id: format!("peer-{i}"),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: format!("{:064x}", i + 1),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });
    }
    let response = handle_discovery_gossip(
        &state,
        DiscoveryGossipRequest {
            contacts: Vec::new(),
        },
        usize::MAX,
    );
    assert_eq!(response.contacts.len(), DISCOVERY_MAX_CONTACTS);
}

#[tokio::test]
async fn discovery_payload_batch_caps_message_count() {
    let state = NodeState::new("test");
    let identity = state.identity();
    let protocol = ProtocolEngine::new(crate::default_protocol_config(
        "ws://127.0.0.1:9/ws".to_string(),
        "node-a".to_string(),
        32,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    ))
    .expect("protocol init");

    let mut items = Vec::new();
    for i in 0..(DISCOVERY_MAX_BATCH_MESSAGES + 20) {
        let msg = DiscoveryMessage::gossip(vec![ContactBundle {
            peer_id: format!("peer-{i}"),
            ws_url: None,
            quic_addr: Some(format!("127.0.0.1:{}", 9100 + i)),
            pubkey_hex: format!("{:064x}", i + 1),
            rpc_url: None,
            lan_addrs: Vec::new(),
        }]);
        items.push(serde_json::to_vec(&msg).expect("encode message"));
    }
    let mut payload = Vec::new();
    ciborium::ser::into_writer(&items, &mut payload).expect("encode payload");

    let handled = handle_discovery_payload(&state, &protocol, &payload).await;
    assert_eq!(handled, Some(()));
    assert_eq!(state.contacts().len(), DISCOVERY_MAX_BATCH_MESSAGES);
}

#[tokio::test]
async fn discovery_lookup_requires_reply_target() {
    let state = NodeState::new("test");
    state.add_contact(ContactBundle {
        peer_id: "peer-target".to_string(),
        ws_url: None,
        quic_addr: Some("127.0.0.1:9200".to_string()),
        pubkey_hex: "aa".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    });
    let identity = state.identity();
    let protocol = ProtocolEngine::new(crate::default_protocol_config(
        "ws://127.0.0.1:9/ws".to_string(),
        "node-a".to_string(),
        32,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    ))
    .expect("protocol init");
    let lookup = DiscoveryMessage::lookup(Some("peer-target".to_string()), None, "   ".to_string());
    let payload = serde_json::to_vec(&lookup).expect("encode lookup");

    let handled = handle_discovery_payload(&state, &protocol, &payload).await;
    assert_eq!(handled, None);
}

#[tokio::test]
async fn discovery_payload_rejects_oversized_input() {
    let state = NodeState::new("test");
    let identity = state.identity();
    let protocol = ProtocolEngine::new(crate::default_protocol_config(
        "ws://127.0.0.1:9/ws".to_string(),
        "node-a".to_string(),
        32,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    ))
    .expect("protocol init");
    let oversized = vec![0x42; DISCOVERY_MAX_PAYLOAD_BYTES + 1];

    let handled = handle_discovery_payload(&state, &protocol, &oversized).await;
    assert_eq!(handled, None);
    assert!(state.contacts().is_empty());
}

#[tokio::test]
async fn discovery_payload_batch_skips_oversized_message_items() {
    let state = NodeState::new("test");
    let identity = state.identity();
    let protocol = ProtocolEngine::new(crate::default_protocol_config(
        "ws://127.0.0.1:9/ws".to_string(),
        "node-a".to_string(),
        32,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    ))
    .expect("protocol init");

    let valid = DiscoveryMessage::gossip(vec![ContactBundle {
        peer_id: "peer-ok".to_string(),
        ws_url: None,
        quic_addr: Some("127.0.0.1:9300".to_string()),
        pubkey_hex: "bb".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    }]);
    let mut items = Vec::new();
    items.push(vec![0x55; DISCOVERY_MAX_MESSAGE_BYTES + 1]);
    items.push(serde_json::to_vec(&valid).expect("encode valid"));

    let mut payload = Vec::new();
    ciborium::ser::into_writer(&items, &mut payload).expect("encode payload");

    let handled = handle_discovery_payload(&state, &protocol, &payload).await;
    assert_eq!(handled, Some(()));
    assert_eq!(state.contacts().len(), 1);
    assert!(state.contacts().iter().any(|c| c.peer_id == "peer-ok"));
}

#[tokio::test]
async fn discovery_payload_announce_ignores_local_pubkey_spoof() {
    let state = NodeState::new("test");
    let identity = state.identity();
    let protocol = ProtocolEngine::new(crate::default_protocol_config(
        "ws://127.0.0.1:9/ws".to_string(),
        "node-a".to_string(),
        32,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    ))
    .expect("protocol init");
    let announce = DiscoveryMessage::announce(ContactBundle {
        peer_id: "peer-spoof".to_string(),
        ws_url: Some("ws://spoof.example/ws".to_string()),
        quic_addr: Some("127.0.0.1:9409".to_string()),
        pubkey_hex: hex::encode(identity.public_key),
        rpc_url: None,
        lan_addrs: Vec::new(),
    });
    let payload = serde_json::to_vec(&announce).expect("encode announce");

    let handled = handle_discovery_payload(&state, &protocol, &payload).await;
    assert_eq!(handled, None);
    assert!(state.contacts().is_empty());
    let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
    assert!(fast.is_empty());
    assert!(fallback.is_empty());
}

#[tokio::test]
async fn discovery_payload_gossip_ignores_self_contact() {
    let state = NodeState::new("test");
    let identity = state.identity();
    let protocol = ProtocolEngine::new(crate::default_protocol_config(
        "ws://127.0.0.1:9/ws".to_string(),
        "node-a".to_string(),
        32,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    ))
    .expect("protocol init");
    let gossip = DiscoveryMessage::gossip(vec![
        ContactBundle {
            peer_id: "node-a".to_string(),
            ws_url: Some("ws://self.example/ws".to_string()),
            quic_addr: Some("127.0.0.1:9400".to_string()),
            pubkey_hex: "aa".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        },
        ContactBundle {
            peer_id: "peer-ok".to_string(),
            ws_url: Some("ws://peer-ok.example/ws".to_string()),
            quic_addr: Some("127.0.0.1:9401".to_string()),
            pubkey_hex: "bb".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        },
        ContactBundle {
            peer_id: "peer-spoof-local".to_string(),
            ws_url: Some("ws://spoof.example/ws".to_string()),
            quic_addr: Some("127.0.0.1:9402".to_string()),
            pubkey_hex: hex::encode(identity.public_key),
            rpc_url: None,
            lan_addrs: Vec::new(),
        },
    ]);
    let payload = serde_json::to_vec(&gossip).expect("encode gossip");

    let handled = handle_discovery_payload(&state, &protocol, &payload).await;
    assert_eq!(handled, Some(()));

    let contacts = state.contacts();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].peer_id, "peer-ok");

    let (fast, fallback) = protocol.dynamic_peer_snapshot().await;
    assert_eq!(fast, vec!["127.0.0.1:9401".to_string()]);
    assert_eq!(fallback, vec!["ws://peer-ok.example/ws".to_string()]);
}
