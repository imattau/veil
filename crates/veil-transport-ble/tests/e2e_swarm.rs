use std::collections::HashMap;

use veil_codec::shard::encode_shard_cbor;
use veil_core::types::{Epoch, Namespace};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_fec::sharder::{derive_object_root, object_to_shards, shard_id};
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::NodeRuntime;
use veil_transport::adapter::InMemoryAdapter;
use veil_transport_ble::chunking::split_into_frames;
use veil_transport_ble::{BleAdapter, BleAdapterConfig, BlePeer, MockBleLink};

struct NodeHarness {
    name: String,
    runtime: NodeRuntime<BleAdapter<MockBleLink>, InMemoryAdapter, XChaCha20Poly1305Cipher, Ed25519Verifier>,
}

impl NodeHarness {
    fn new(name: &str, tag: [u8; 32]) -> Self {
        let mut state = veil_node::state::NodeState::default();
        state.subscriptions.insert(tag);

        let adapter = BleAdapter::new(MockBleLink::with_mtu(64), BleAdapterConfig::default());
        let fallback = InMemoryAdapter::default();
        let config = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();

        let runtime = NodeRuntime::new(
            state,
            adapter,
            fallback,
            config,
            [0xA5_u8; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );

        Self {
            name: name.to_string(),
            runtime,
        }
    }

    fn peer(&self) -> BlePeer {
        BlePeer::new(self.name.clone())
    }
}

fn route_ble_frames(nodes: &mut [NodeHarness]) {
    let mut index = HashMap::new();
    for (idx, node) in nodes.iter().enumerate() {
        index.insert(node.name.clone(), idx);
    }

    for i in 0..nodes.len() {
        let outbound = nodes[i].runtime.fast_adapter.link_mut().take_outbound();
        for (peer, frame) in outbound {
            if let Some(&dest) = index.get(&peer.addr) {
                let from_peer = BlePeer::new(nodes[i].name.clone());
                nodes[dest]
                    .runtime
                    .fast_adapter
                    .link_mut()
                    .enqueue_inbound(from_peer, frame);
            }
        }
    }
}

#[test]
fn ble_swarm_propagates_shard() {
    let tag = [0x11_u8; 32];
    let mut nodes = vec![
        NodeHarness::new("node-a", tag),
        NodeHarness::new("node-b", tag),
        NodeHarness::new("node-c", tag),
    ];

    let object_bytes = vec![0xAB; 2048];
    let object_root = derive_object_root(&object_bytes);
    let shards = object_to_shards(
        &object_bytes,
        Namespace(1),
        Epoch(1),
        tag,
        object_root,
    )
    .expect("shards");
    let shard = shards.first().expect("shard");
    let shard_bytes = encode_shard_cbor(shard).expect("encode shard");
    let sid = shard_id(shard).expect("shard id");

    let frames = split_into_frames(blake3::hash(&shard_bytes).as_bytes().to_owned(), &shard_bytes, 64);
    let origin = BlePeer::new("origin");
    for frame in frames {
        nodes[0]
            .runtime
            .fast_adapter
            .link_mut()
            .enqueue_inbound(origin.clone(), frame);
    }

    let peers = vec![nodes[0].peer(), nodes[1].peer(), nodes[2].peer()];

    for step in 0..6_u64 {
        for i in 0..nodes.len() {
            let mut peer_list = peers.clone();
            peer_list.retain(|p| p.addr != nodes[i].name);
            let _ = nodes[i].runtime.tick(step, &peer_list, &[]);
        }
        route_ble_frames(&mut nodes);
    }

    for node in nodes.iter().skip(1) {
        assert!(
            node.runtime.state.cache.contains_key(&sid),
            "{} should cache forwarded shard",
            node.name
        );
    }
}
