use std::collections::HashMap;

use veil_core::{Epoch, Namespace};
use veil_core::tags::derive_feed_tag;
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier};
use veil_node::batch::FeedBatcher;
use veil_node::config::NodeRuntimeConfig;
use veil_node::publish::{publish_service_tick_multi_lane, PublishQueueTickParams, PublishServiceTickParams};
use veil_node::runtime::{pump_multi_lane_tick_with_config, ConfigMultiLanePumpParams, RuntimeStats};
use veil_node::state::NodeState;
use veil_transport::adapter::{route_in_memory_outbound, InMemoryAdapter};

struct NodeSim {
    name: String,
    state: NodeState,
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
            state: NodeState::default(),
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
                    namespace: Namespace(32),
                    epoch: Epoch(1),
                    tag,
                    encrypt_key: &self.encrypt_key,
                    now_step,
                    flags: 0,
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
            &Ed25519Verifier::default(),
        )
        .ok()?
        .and_then(|event| match event {
            veil_node::receive::ReceiveEvent::Delivered { payload, tag: got, .. } if got == tag => {
                Some(payload)
            }
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

    let mut nodes = vec![
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
        for i in 0..nodes.len() {
            route_in_memory_outbound(
                &mut publisher.fast,
                &mut nodes[i].fast,
                "publisher",
            );
            route_in_memory_outbound(
                &mut publisher.fallback,
                &mut nodes[i].fallback,
                "publisher",
            );
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
                route_in_memory_outbound(
                    &mut from.fallback,
                    &mut to.fallback,
                    from.name.clone(),
                );
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
