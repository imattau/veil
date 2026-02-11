use std::net::UdpSocket;
use std::time::{Duration, Instant};

use veil_android_node::{
    build_self_contact, default_protocol_config, handle_discovery_payload, DiscoveryMessage,
    NodeState, ProtocolEngine,
};
use veil_android_node::discovery_tag;
use veil_codec::object::decode_object_cbor_prefix;
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};

fn reserve_addr() -> String {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    sock.local_addr().expect("local addr").to_string()
}

async fn drain_discovery(
    node: &NodeState,
    protocol: &ProtocolEngine,
    decrypt_key: &[u8; 32],
    deadline: Instant,
) -> usize {
    let mut handled = 0usize;
    let mut buffered_roots: Vec<[u8; 32]> = Vec::new();
    while Instant::now() < deadline {
        match protocol.pump_inbound().await {
            Ok(Some(event)) => {
                match &event {
                    veil_node::receive::ReceiveEvent::Delivered { payload, namespace, .. } => {
                        if *namespace == protocol.discovery_namespace() {
                            let _ = handle_discovery_payload(node, protocol, payload).await;
                            handled += 1;
                        }
                    }
                    veil_node::receive::ReceiveEvent::Buffered { object_root, .. } => {
                        buffered_roots.push(*object_root);
                    }
                    _ => {}
                }
            }
            Ok(None) => {}
            Err(err) => {
                let _ = err;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let cipher = XChaCha20Poly1305Cipher;
    let mut deduped = buffered_roots;
    deduped.sort();
    deduped.dedup();
    for root in deduped {
        if let Some(object_bytes) = protocol.reconstruct_object(root).await {
            match decode_object_cbor_prefix(&object_bytes) {
                Ok((object, _)) => {
                    if object.namespace == protocol.discovery_namespace() {
                        let aad = build_veil_aad(object.tag, object.namespace, object.epoch);
                        match cipher.decrypt(decrypt_key, object.nonce, &aad, &object.ciphertext) {
                            Ok(payload) => {
                                let _ = handle_discovery_payload(node, protocol, &payload).await;
                                handled += 1;
                            }
                            Err(err) => {
                                eprintln!("decrypt error: {err}");
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("decode object error: {err}");
                }
            }
        } else {
            eprintln!("reconstruct failed for root");
        }
    }
    handled
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_transport_announce_and_lookup() {
    std::env::set_var("VEIL_QUIC_INSECURE", "1");

    let addr_a = reserve_addr();
    let addr_b = reserve_addr();

    let node_a = NodeState::new("0.1-test");
    let node_b = NodeState::new("0.1-test");

    let identity_a = node_a.identity();
    let identity_b = node_b.identity();

    let mut cfg_a = default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "peer-a".to_string(),
        32,
        identity_a.public_key,
        identity_a.signer(),
    );
    cfg_a.ws_url = None;
    let shared_key = [0x42; 32];
    cfg_a.encrypt_key = shared_key;
    cfg_a.quic_bind_addr = addr_a.clone();
    cfg_a.quic_server_name = Some("veil-android-node".to_string());
    cfg_a.fast_peers = vec![addr_b.clone()];
    cfg_a.fallback_peers = vec![addr_b.clone()];

    let mut cfg_b = default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "peer-b".to_string(),
        32,
        identity_b.public_key,
        identity_b.signer(),
    );
    cfg_b.ws_url = None;
    cfg_b.encrypt_key = shared_key;
    cfg_b.quic_bind_addr = addr_b.clone();
    cfg_b.quic_server_name = Some("veil-android-node".to_string());
    cfg_b.fast_peers = vec![addr_a.clone()];
    cfg_b.fallback_peers = vec![addr_a.clone()];

    let protocol_a = ProtocolEngine::new(cfg_a).expect("protocol a");
    let protocol_b = ProtocolEngine::new(cfg_b).expect("protocol b");

    tokio::time::sleep(Duration::from_millis(300)).await;

    let contact_a = build_self_contact(&node_a, &protocol_a);
    let contact_b = build_self_contact(&node_b, &protocol_b);
    let discovery_ns = protocol_a.discovery_namespace();
    protocol_a
        .subscribe_pubkey(identity_b.public_key, discovery_ns)
        .await;
    protocol_b
        .subscribe_pubkey(identity_a.public_key, discovery_ns)
        .await;
    let expected_tag = discovery_tag(discovery_ns);
    protocol_b.subscribe_tag(expected_tag).await;
    protocol_a.subscribe_tag(expected_tag).await;
    assert!(protocol_b.has_subscription(expected_tag).await);

    let announce = DiscoveryMessage::announce(contact_a);
    protocol_a
        .publish_discovery(announce)
        .await
        .expect("announce publish");

    let deadline = Instant::now() + Duration::from_secs(6);
    let handled = drain_discovery(&node_b, &protocol_b, &shared_key, deadline).await;
    assert!(handled > 0, "expected discovery announce to be handled");
    assert!(node_b.contacts().iter().any(|c| c.peer_id == "peer-a"));

    let announce_b = DiscoveryMessage::announce(contact_b);
    protocol_b
        .publish_discovery(announce_b)
        .await
        .expect("announce b publish");

    let deadline = Instant::now() + Duration::from_secs(6);
    let _ = drain_discovery(&node_a, &protocol_a, &shared_key, deadline).await;

    assert!(node_a.contacts().iter().any(|c| c.peer_id == "peer-b"));
    assert!(node_b.contacts().iter().any(|c| c.peer_id == "peer-a"));
}
