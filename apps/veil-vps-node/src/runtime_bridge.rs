use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use veil_crypto::aead::AeadCipher;
use veil_crypto::signing::Signer;
use veil_node::batch::FeedBatcher;
use veil_node::config::NodeRuntimeConfig;
use veil_node::publish::{publish_queue_tick_multi_lane, PublishQueueTickParams};
use veil_node::state::NodeState;
use veil_transport::adapter::TransportAdapter;

use crate::metrics_state::MetricsState;
use crate::nostr_bridge::BridgedItem;
use crate::runtime_payloads::process_bridged_item;

pub(super) fn drain_bridged_items(
    rx: &mut tokio::sync::mpsc::Receiver<BridgedItem>,
    metrics: &MetricsState,
    feed_history: &Arc<Mutex<VecDeque<serde_json::Value>>>,
    bridge_batcher: &mut FeedBatcher,
    max_items: usize,
) {
    for _ in 0..max_items {
        match rx.try_recv() {
            Ok(item) => process_bridged_item(item, metrics, feed_history, bridge_batcher),
            Err(_) => break,
        }
    }
}

pub(super) fn publish_bridge_batch<AFast, AFallback, S>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    bridge_batcher: &mut FeedBatcher,
    params: PublishQueueTickParams<'_, AFast::Peer, AFallback::Peer>,
    config: &NodeRuntimeConfig,
    cipher: &impl AeadCipher,
    signer: Option<&S>,
) where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    S: Signer,
{
    let _ = publish_queue_tick_multi_lane(
        node,
        fast_adapter,
        fallback_adapter,
        bridge_batcher,
        params,
        config,
        cipher,
        signer,
    );
}
