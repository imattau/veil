use std::collections::HashMap;

use std::collections::HashSet;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use veil_android_node::NodeState as AppNodeState;
use veil_codec::object::{
    decode_object_cbor, encode_object_cbor, object_signature_message_digest, ObjectV1, Signature,
    OBJECT_FLAG_SIGNED, OBJECT_V1_VERSION,
};
use veil_codec::shard::decode_shard_cbor;
use veil_core::hash::blake3_32;
use veil_core::tags::derive_feed_tag;
use veil_core::{Epoch, Namespace};
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier, Signer, Verifier};
use veil_fec::sharder::{derive_object_root, reconstruct_object_with_mode};
use veil_node::batch::FeedBatcher;
use veil_node::config::NodeRuntimeConfig;
use veil_node::publish::{
    publish_service_tick_multi_lane, PublishQueueTickParams, PublishServiceTickParams,
};
use veil_node::runtime::{
    pump_multi_lane_tick_with_config, ConfigMultiLanePumpParams, RuntimeStats,
};
use veil_node::state::NodeState as VeilNodeState;
use veil_transport::adapter::{route_in_memory_outbound, InMemoryAdapter, TransportAdapter};
use veil_transport_websocket::{
    WebSocketAdapter, WebSocketAdapterConfig, WebSocketServerAdapter, WebSocketServerAdapterConfig,
};

struct NodeSim {
    name: String,
    state: VeilNodeState,
    fast: InMemoryAdapter,
    fallback: InMemoryAdapter,
    batcher: FeedBatcher,
    cfg: NodeRuntimeConfig,
    encrypt_key: [u8; 32],
    signer: Ed25519Signer,
    stats: RuntimeStats,
}

impl NodeSim {
    fn new(name: &str, encrypt_key: [u8; 32]) -> Self {
        let cfg = NodeRuntimeConfig::builder()
            .base_fast_fanout(4)
            .base_fallback_fanout(2)
            .fallback_redundancy_fanout(2)
            .build();
        Self {
            name: name.to_string(),
            state: VeilNodeState::default(),
            fast: InMemoryAdapter::default(),
            fallback: InMemoryAdapter::default(),
            batcher: FeedBatcher::default(),
            cfg,
            encrypt_key,
            signer: Ed25519Signer::from_secret([0x11; 32]),
            stats: RuntimeStats::default(),
        }
    }

    fn publish(&mut self, payload: Vec<u8>, tag: [u8; 32], now_step: u64) {
        self.publish_with_flags(payload, Namespace(32), tag, now_step, 0);
    }

    fn publish_with_flags(
        &mut self,
        payload: Vec<u8>,
        namespace: Namespace,
        tag: [u8; 32],
        now_step: u64,
        flags: u16,
    ) {
        self.batcher.enqueue(payload);
        let peers = vec![
            self.name.clone(),
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        let _ = publish_service_tick_multi_lane(
            &mut self.state,
            &mut self.fast,
            &mut self.fallback,
            PublishServiceTickParams {
                batcher: &mut self.batcher,
                publish: PublishQueueTickParams {
                    namespace,
                    epoch: Epoch(1),
                    tag,
                    encrypt_key: &self.encrypt_key,
                    now_step,
                    flags,
                    interactive_flush: true,
                    fast_peers: &peers,
                    fallback_peers: &peers,
                },
            },
            &self.cfg,
            &XChaCha20Poly1305Cipher,
            Some(&self.signer),
        );
    }

    fn pump(&mut self, tag: [u8; 32], now_step: u64) -> Option<Vec<u8>> {
        let peers = vec![
            self.name.clone(),
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        pump_multi_lane_tick_with_config(
            &mut self.state,
            &mut self.fast,
            &mut self.fallback,
            ConfigMultiLanePumpParams {
                fast_peers: &peers,
                fallback_peers: &peers,
                now_step,
                decrypt_key: &self.encrypt_key,
                config: &self.cfg,
                stats: &mut self.stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .ok()?
        .and_then(|event| match event {
            veil_node::receive::ReceiveEvent::Delivered {
                payload, tag: got, ..
            } if got == tag => Some(payload),
            _ => None,
        })
    }
}

#[test]
fn swarm_sim_multilane_in_memory() {
    let mut encrypt_key = [0u8; 32];
    encrypt_key[0] = 0xAA;
    let payload = b"hello swarm".to_vec();
    let pubkey = [0x44; 32];
    let tag = derive_feed_tag(&pubkey, Namespace(32));

    let mut nodes = [
        NodeSim::new("node-a", encrypt_key),
        NodeSim::new("node-b", encrypt_key),
        NodeSim::new("node-c", encrypt_key),
    ];

    for node in nodes.iter_mut() {
        node.state.subscriptions.insert(tag);
    }

    nodes[0].publish(payload.clone(), tag, 1);

    let mut delivered: HashMap<String, Vec<u8>> = HashMap::new();

    let mut publisher = NodeSim::new("publisher", encrypt_key);
    publisher.state.subscriptions.insert(tag);
    for step in 1..=20 {
        publisher.publish(payload.clone(), tag, step);
        for node in nodes.iter_mut() {
            route_in_memory_outbound(&mut publisher.fast, &mut node.fast, "publisher");
            route_in_memory_outbound(&mut publisher.fallback, &mut node.fallback, "publisher");
        }
        for i in 0..nodes.len() {
            for j in 0..nodes.len() {
                if i == j {
                    continue;
                }
                let (from, to) = if i < j {
                    let (left, right) = nodes.split_at_mut(j);
                    (&mut left[i], &mut right[0])
                } else {
                    let (left, right) = nodes.split_at_mut(i);
                    (&mut right[0], &mut left[j])
                };
                route_in_memory_outbound(&mut from.fast, &mut to.fast, from.name.clone());
            }
        }
        for i in 0..nodes.len() {
            for j in 0..nodes.len() {
                if i == j {
                    continue;
                }
                let (from, to) = if i < j {
                    let (left, right) = nodes.split_at_mut(j);
                    (&mut left[i], &mut right[0])
                } else {
                    let (left, right) = nodes.split_at_mut(i);
                    (&mut right[0], &mut left[j])
                };
                route_in_memory_outbound(&mut from.fallback, &mut to.fallback, from.name.clone());
            }
        }

        for _ in 0..3 {
            for node in nodes.iter_mut() {
                if let Some(received) = node.pump(tag, step) {
                    delivered.entry(node.name.clone()).or_insert(received);
                }
            }
        }
    }

    assert!(delivered.contains_key("node-b") || delivered.contains_key("node-c"));
}

#[test]
fn swarm_sim_multilane_ws_with_fallback() {
    let mut encrypt_key = [0u8; 32];
    encrypt_key[0] = 0xBB;
    let pubkey = [0x55; 32];
    let tag = derive_feed_tag(&pubkey, Namespace(32));
    let payload = b"hello ws swarm".to_vec();

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind temp");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);

    let ws_bind = format!("127.0.0.1:{port}");
    let ws_url = format!("ws://127.0.0.1:{port}");
    let mut server = WebSocketServerAdapter::listen(WebSocketServerAdapterConfig::new(ws_bind))
        .expect("ws server");

    let mut nodes = [
        (
            "node-a",
            WebSocketAdapter::connect(WebSocketAdapterConfig::new(ws_url.clone(), "node-a"))
                .expect("ws client"),
            WebSocketAdapter::connect(WebSocketAdapterConfig::new(ws_url.clone(), "node-a-fb"))
                .expect("ws fallback"),
            VeilNodeState::default(),
            FeedBatcher::default(),
            RuntimeStats::default(),
        ),
        (
            "node-b",
            WebSocketAdapter::connect(WebSocketAdapterConfig::new(ws_url.clone(), "node-b"))
                .expect("ws client"),
            WebSocketAdapter::connect(WebSocketAdapterConfig::new(ws_url.clone(), "node-b-fb"))
                .expect("ws fallback"),
            VeilNodeState::default(),
            FeedBatcher::default(),
            RuntimeStats::default(),
        ),
        (
            "node-c",
            WebSocketAdapter::connect(WebSocketAdapterConfig::new(ws_url.clone(), "node-c"))
                .expect("ws client"),
            WebSocketAdapter::connect(WebSocketAdapterConfig::new(ws_url.clone(), "node-c-fb"))
                .expect("ws fallback"),
            VeilNodeState::default(),
            FeedBatcher::default(),
            RuntimeStats::default(),
        ),
    ];

    for (_, _, _, state, _, _) in nodes.iter_mut() {
        state.subscriptions.insert(tag);
    }

    thread::sleep(Duration::from_millis(100));

    let cfg = NodeRuntimeConfig::builder()
        .base_fast_fanout(3)
        .base_fallback_fanout(1)
        .fallback_redundancy_fanout(1)
        .build();
    let signer = Ed25519Signer::from_secret([0x11; 32]);
    let peers = vec!["peer".to_string()];

    let mut connected: HashSet<String> = HashSet::new();
    let mut delivered: HashMap<String, Vec<u8>> = HashMap::new();

    // Prime the server connection list by having each client send a small hello.
    for (_, fast, fallback, _, _, _) in nodes.iter_mut() {
        let _ = fast.send(&peers[0], b"hello");
        let _ = fallback.send(&peers[0], b"hello");
    }
    thread::sleep(Duration::from_millis(50));
    while let Some((peer, _)) = server.recv() {
        connected.insert(peer);
    }

    {
        let (head, _) = nodes.split_at_mut(1);
        let (_, fast, fallback, state, batcher, _) = &mut head[0];
        batcher.enqueue(payload.clone());
        let _ = publish_service_tick_multi_lane(
            state,
            fast,
            fallback,
            PublishServiceTickParams {
                batcher,
                publish: PublishQueueTickParams {
                    namespace: Namespace(32),
                    epoch: Epoch(1),
                    tag,
                    encrypt_key: &encrypt_key,
                    now_step: 1,
                    flags: 0,
                    interactive_flush: true,
                    fast_peers: &peers,
                    fallback_peers: &peers,
                },
            },
            &cfg,
            &XChaCha20Poly1305Cipher,
            Some(&signer),
        );
    }

    for step in 1..=50 {
        {
            let (head, _) = nodes.split_at_mut(1);
            let (_, fast, fallback, state, batcher, _) = &mut head[0];
            batcher.enqueue(payload.clone());
            let _ = publish_service_tick_multi_lane(
                state,
                fast,
                fallback,
                PublishServiceTickParams {
                    batcher,
                    publish: PublishQueueTickParams {
                        namespace: Namespace(32),
                        epoch: Epoch(1),
                        tag,
                        encrypt_key: &encrypt_key,
                        now_step: step,
                        flags: 0,
                        interactive_flush: true,
                        fast_peers: &peers,
                        fallback_peers: &peers,
                    },
                },
                &cfg,
                &XChaCha20Poly1305Cipher,
                Some(&signer),
            );
        }
        while let Some((peer, bytes)) = server.recv() {
            connected.insert(peer.clone());
            for target in connected.iter() {
                if target != &peer {
                    let _ = server.send(target, &bytes);
                }
            }
        }

        for _ in 0..3 {
            for (_, fast, fallback, state, _, stats) in nodes.iter_mut() {
                if let Ok(Some(veil_node::receive::ReceiveEvent::Delivered { payload, .. })) =
                    pump_multi_lane_tick_with_config(
                        state,
                        fast,
                        fallback,
                        ConfigMultiLanePumpParams {
                            fast_peers: &peers,
                            fallback_peers: &peers,
                            now_step: step,
                            decrypt_key: &encrypt_key,
                            config: &cfg,
                            stats,
                        },
                        &XChaCha20Poly1305Cipher,
                        &Ed25519Verifier,
                    )
                {
                    delivered.insert("delivered".to_string(), payload);
                }
            }
        }
        thread::sleep(Duration::from_millis(25));
    }

    for step in 51..=60 {
        for (_, fast, fallback, state, _, stats) in nodes.iter_mut() {
            if let Ok(Some(veil_node::receive::ReceiveEvent::Delivered { payload, .. })) =
                pump_multi_lane_tick_with_config(
                    state,
                    fast,
                    fallback,
                    ConfigMultiLanePumpParams {
                        fast_peers: &peers,
                        fallback_peers: &peers,
                        now_step: step,
                        decrypt_key: &encrypt_key,
                        config: &cfg,
                        stats,
                    },
                    &XChaCha20Poly1305Cipher,
                    &Ed25519Verifier,
                )
            {
                delivered.insert("delivered".to_string(), payload);
            }
        }
    }

    assert!(!delivered.is_empty());
}

#[test]
fn swarm_sim_delivered_payload_is_signed_with_identity() {
    let mut encrypt_key = [0u8; 32];
    encrypt_key[0] = 0xCC;
    let identity_signer = Ed25519Signer::from_secret([0x22; 32]);
    let identity_pubkey = identity_signer.public_key();
    let tag = derive_feed_tag(&identity_pubkey, Namespace(32));
    let payload = b"identity-bound payload".to_vec();

    let mut publisher = NodeSim::new("publisher", encrypt_key);
    publisher.signer = identity_signer.clone();
    publisher.publish_with_flags(payload.clone(), Namespace(32), tag, 1, OBJECT_FLAG_SIGNED);

    let mut shard_bytes = Vec::new();
    shard_bytes.extend(publisher.fast.take_outbound().into_iter().map(|(_, b)| b));
    shard_bytes.extend(
        publisher
            .fallback
            .take_outbound()
            .into_iter()
            .map(|(_, b)| b),
    );

    assert!(!shard_bytes.is_empty(), "expected outbound shards");

    let mut shards = Vec::new();
    for bytes in shard_bytes {
        if let Ok(shard) = decode_shard_cbor(&bytes) {
            shards.push(shard);
        }
    }
    assert!(!shards.is_empty(), "expected decoded shards");

    let expected_encoded = build_signed_encoded_object(
        &payload,
        Namespace(32),
        Epoch(1),
        tag,
        &encrypt_key,
        1,
        &identity_signer,
    );
    let expected_root = derive_object_root(&expected_encoded);
    let reconstructed = reconstruct_object_with_mode(
        &shards,
        expected_encoded.len(),
        expected_root,
        publisher.cfg.erasure_coding_mode,
    )
    .expect("reconstruct encoded object");
    let object = decode_object_cbor(&reconstructed).expect("decode object");
    assert_eq!(object.sender_pubkey, Some(identity_pubkey));
    let digest = object_signature_message_digest(&object).expect("digest");
    let signature = object.signature.expect("signature");
    let ok = Ed25519Verifier
        .verify(identity_pubkey, &digest, signature.0)
        .expect("verify");
    assert!(ok);
}

#[test]
fn swarm_sim_required_signed_namespace_rejects_unsigned() {
    let mut encrypt_key = [0u8; 32];
    encrypt_key[0] = 0xDD;
    let namespace = Namespace(44);
    let tag = derive_feed_tag(&[0x66; 32], namespace);
    let payload = b"unsigned payload".to_vec();

    let mut publisher = NodeSim::new("publisher", encrypt_key);
    publisher.cfg.required_signed_namespaces.insert(namespace.0);

    publisher.publish_with_flags(payload.clone(), namespace, tag, 1, 0);

    let mut shard_bytes = Vec::new();
    shard_bytes.extend(publisher.fast.take_outbound().into_iter().map(|(_, b)| b));
    shard_bytes.extend(
        publisher
            .fallback
            .take_outbound()
            .into_iter()
            .map(|(_, b)| b),
    );

    assert!(!shard_bytes.is_empty(), "expected outbound shards");

    let mut shards = Vec::new();
    for bytes in shard_bytes {
        if let Ok(shard) = decode_shard_cbor(&bytes) {
            shards.push(shard);
        }
    }
    assert!(!shards.is_empty(), "expected decoded shards");

    let expected_encoded =
        build_unsigned_encoded_object(&payload, namespace, Epoch(1), tag, &encrypt_key, 1);
    let expected_root = derive_object_root(&expected_encoded);
    let reconstructed = reconstruct_object_with_mode(
        &shards,
        expected_encoded.len(),
        expected_root,
        publisher.cfg.erasure_coding_mode,
    )
    .expect("reconstruct encoded object");
    let object = decode_object_cbor(&reconstructed).expect("decode object");
    assert_eq!(object.flags & OBJECT_FLAG_SIGNED, 0);
}

#[test]
fn swarm_sim_multilane_emits_feed_bundle_event() {
    let mut encrypt_key = [0u8; 32];
    encrypt_key[0] = 0xEE;
    let pubkey = [0x77; 32];
    let tag = derive_feed_tag(&pubkey, Namespace(32));

    let bundle = veil_schema_feed::FeedBundle::Post(veil_schema_feed::PostBundle {
        meta: veil_schema_feed::BundleMeta {
            version: 1,
            created_at: 1_700_000_050,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: "aa".repeat(32),
        text: "hello feed".to_string(),
        media_roots: vec![],
        reply_to_root: None,
    });
    let payload = serde_json::to_vec(&bundle).expect("encode feed bundle");

    let mut nodes = [
        NodeSim::new("node-a", encrypt_key),
        NodeSim::new("node-b", encrypt_key),
        NodeSim::new("node-c", encrypt_key),
    ];

    for node in nodes.iter_mut() {
        node.state.subscriptions.insert(tag);
    }

    let mut publisher = NodeSim::new("publisher", encrypt_key);
    publisher.state.subscriptions.insert(tag);

    let mut delivered = None;

    for step in 1..=20 {
        publisher.publish(payload.clone(), tag, step);
        for node in nodes.iter_mut() {
            route_in_memory_outbound(&mut publisher.fast, &mut node.fast, "publisher");
            route_in_memory_outbound(&mut publisher.fallback, &mut node.fallback, "publisher");
        }
        for i in 0..nodes.len() {
            for j in 0..nodes.len() {
                if i == j {
                    continue;
                }
                let (from, to) = if i < j {
                    let (left, right) = nodes.split_at_mut(j);
                    (&mut left[i], &mut right[0])
                } else {
                    let (left, right) = nodes.split_at_mut(i);
                    (&mut right[0], &mut left[j])
                };
                route_in_memory_outbound(&mut from.fast, &mut to.fast, from.name.clone());
                route_in_memory_outbound(&mut from.fallback, &mut to.fallback, from.name.clone());
            }
        }

        for _ in 0..3 {
            for node in nodes.iter_mut() {
                if let Some(received) = node.pump(tag, step) {
                    delivered = Some(received);
                }
            }
        }
        if delivered.is_some() {
            break;
        }
    }

    let payload = delivered.expect("expected delivered payload");
    let app_state = AppNodeState::new("0.1-test");
    app_state.emit_payload(&[0x11; 32], &payload, 32, 1, &[0x22; 32], 0);
    let (backlog, _) = app_state.subscribe_events_since(Some(0));
    let kinds: Vec<_> = backlog.iter().map(|event| event.event.as_str()).collect();
    assert!(kinds.contains(&"payload"));
    assert!(kinds.contains(&"feed_bundle"));
}

fn build_signed_encoded_object(
    payload_item: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: [u8; 32],
    encrypt_key: &[u8; 32],
    now_step: u64,
    signer: &Ed25519Signer,
) -> Vec<u8> {
    let mut batch_payload = Vec::new();
    ciborium::ser::into_writer(&vec![payload_item.to_vec()], &mut batch_payload)
        .expect("encode batch");
    let nonce = derive_object_nonce(tag, namespace, epoch, now_step, &batch_payload);
    let aad = build_veil_aad(tag, namespace, epoch);
    let envelope = XChaCha20Poly1305Cipher
        .encrypt(encrypt_key, nonce, &aad, &batch_payload)
        .expect("encrypt payload");
    let payload_root = derive_object_root(&batch_payload);
    let mut object = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags: OBJECT_FLAG_SIGNED,
        tag,
        object_root: payload_root,
        sender_pubkey: Some(signer.public_key()),
        signature: Some(Signature([0_u8; 64])),
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
        padding: vec![0_u8; 8],
    };
    let digest = object_signature_message_digest(&object).expect("digest");
    let signature = signer.sign(&digest).expect("sign");
    object.signature = Some(Signature(signature));
    encode_object_cbor(&object).expect("encode object")
}

fn build_unsigned_encoded_object(
    payload_item: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: [u8; 32],
    encrypt_key: &[u8; 32],
    now_step: u64,
) -> Vec<u8> {
    let mut batch_payload = Vec::new();
    ciborium::ser::into_writer(&vec![payload_item.to_vec()], &mut batch_payload)
        .expect("encode batch");
    let nonce = derive_object_nonce(tag, namespace, epoch, now_step, &batch_payload);
    let aad = build_veil_aad(tag, namespace, epoch);
    let envelope = XChaCha20Poly1305Cipher
        .encrypt(encrypt_key, nonce, &aad, &batch_payload)
        .expect("encrypt payload");
    let payload_root = derive_object_root(&batch_payload);
    let object = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags: 0,
        tag,
        object_root: payload_root,
        sender_pubkey: None,
        signature: None,
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
        padding: vec![0_u8; 8],
    };
    encode_object_cbor(&object).expect("encode object")
}

fn derive_object_nonce(
    tag: [u8; 32],
    namespace: Namespace,
    epoch: Epoch,
    now_step: u64,
    payload: &[u8],
) -> [u8; 24] {
    let mut preimage = Vec::with_capacity(10 + 32 + 2 + 4 + 8 + 32);
    preimage.extend_from_slice(b"objnonce-v1");
    preimage.extend_from_slice(&tag);
    preimage.extend_from_slice(&namespace.0.to_be_bytes());
    preimage.extend_from_slice(&epoch.0.to_be_bytes());
    preimage.extend_from_slice(&now_step.to_be_bytes());
    preimage.extend_from_slice(&blake3_32(payload));
    let hash = blake3_32(&preimage);
    let mut nonce = [0_u8; 24];
    nonce.copy_from_slice(&hash[..24]);
    nonce
}
