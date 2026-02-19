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
