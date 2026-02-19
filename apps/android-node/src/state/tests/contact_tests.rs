use super::*;

#[test]
fn add_contact_caps_total_and_evicts_from_discovery_index() {
    let state = NodeState::new("0.1-test");
    for index in 0..(MAX_CONTACTS_TOTAL + 5) {
        state.add_contact(ContactBundle {
            peer_id: format!("peer-{index}"),
            ws_url: Some(format!("ws://relay-{index}.example/ws")),
            quic_addr: Some(format!("127.0.0.1:{}", 10_000 + (index % 50_000))),
            pubkey_hex: format!("{:064x}", index + 1),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });
    }

    let contacts = state.contacts();
    assert_eq!(contacts.len(), MAX_CONTACTS_TOTAL);
    assert!(!contacts.iter().any(|contact| contact.peer_id == "peer-0"));
    assert!(contacts
        .iter()
        .any(|contact| contact.peer_id == format!("peer-{}", MAX_CONTACTS_TOTAL + 4)));

    let lookup = state.discovery_lookup_peer("peer-0", MAX_CONTACTS_TOTAL);
    assert!(!lookup.iter().any(|contact| contact.peer_id == "peer-0"));
}

#[test]
fn add_contact_merges_lan_addresses_without_duplicates() {
    let state = NodeState::new("0.1-test");
    state.add_contact(ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://relay-a.example/ws".to_string()),
        quic_addr: Some("127.0.0.1:9444".to_string()),
        pubkey_hex: "11".repeat(32),
        rpc_url: None,
        lan_addrs: vec!["10.0.0.1:9333".to_string(), "10.0.0.2:9333".to_string()],
    });
    state.add_contact(ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: None,
        quic_addr: None,
        pubkey_hex: "11".repeat(32),
        rpc_url: None,
        lan_addrs: vec!["10.0.0.2:9333".to_string(), "10.0.0.3:9333".to_string()],
    });

    let contacts = state.contacts();
    assert_eq!(contacts.len(), 1);
    let merged = contacts
        .iter()
        .find(|contact| contact.peer_id == "peer-a")
        .expect("merged contact");
    assert_eq!(
        merged.lan_addrs,
        vec![
            "10.0.0.1:9333".to_string(),
            "10.0.0.2:9333".to_string(),
            "10.0.0.3:9333".to_string(),
        ]
    );
}

#[test]
fn add_contact_refreshes_endpoints_when_pubkey_matches() {
    let state = NodeState::new("0.1-test");
    state.add_contact(ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://relay-a.example/ws-v1".to_string()),
        quic_addr: Some("127.0.0.1:9444".to_string()),
        pubkey_hex: "11".repeat(32),
        rpc_url: Some("https://relay-a.example/rpc-v1".to_string()),
        lan_addrs: vec!["10.0.0.1:9333".to_string()],
    });
    state.add_contact(ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://relay-a.example/ws-v2".to_string()),
        quic_addr: Some("127.0.0.1:9555".to_string()),
        pubkey_hex: "11".repeat(32),
        rpc_url: Some("https://relay-a.example/rpc-v2".to_string()),
        lan_addrs: vec!["10.0.0.2:9333".to_string()],
    });

    let merged = state
        .contacts()
        .into_iter()
        .find(|contact| contact.peer_id == "peer-a")
        .expect("merged contact");
    assert_eq!(merged.ws_url.as_deref(), Some("ws://relay-a.example/ws-v2"));
    assert_eq!(merged.quic_addr.as_deref(), Some("127.0.0.1:9555"));
    assert_eq!(
        merged.rpc_url.as_deref(),
        Some("https://relay-a.example/rpc-v2")
    );
    assert_eq!(
        merged.lan_addrs,
        vec!["10.0.0.1:9333".to_string(), "10.0.0.2:9333".to_string()]
    );
}

#[test]
fn add_contact_ignores_endpoint_updates_when_pubkey_mismatches() {
    let state = NodeState::new("0.1-test");
    state.add_contact(ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://relay-a.example/ws-v1".to_string()),
        quic_addr: Some("127.0.0.1:9444".to_string()),
        pubkey_hex: "11".repeat(32),
        rpc_url: Some("https://relay-a.example/rpc-v1".to_string()),
        lan_addrs: vec!["10.0.0.1:9333".to_string()],
    });
    state.add_contact(ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://attacker.example/ws".to_string()),
        quic_addr: Some("127.0.0.1:9999".to_string()),
        pubkey_hex: "22".repeat(32),
        rpc_url: Some("https://attacker.example/rpc".to_string()),
        lan_addrs: vec!["10.0.0.9:9333".to_string()],
    });

    let merged = state
        .contacts()
        .into_iter()
        .find(|contact| contact.peer_id == "peer-a")
        .expect("merged contact");
    assert_eq!(merged.ws_url.as_deref(), Some("ws://relay-a.example/ws-v1"));
    assert_eq!(merged.quic_addr.as_deref(), Some("127.0.0.1:9444"));
    assert_eq!(
        merged.rpc_url.as_deref(),
        Some("https://relay-a.example/rpc-v1")
    );
    assert_eq!(merged.pubkey_hex, "11".repeat(32));
    assert_eq!(merged.lan_addrs, vec!["10.0.0.1:9333".to_string()]);
}
