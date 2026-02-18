use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use veil_core::{Namespace, Tag};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::{NostrSigner, NostrVerifier};
use veil_node::batch::FeedBatcher;
use veil_node::config::NodeRuntimeConfig;
use veil_node::publish::PublishQueueTickParams;
use veil_node::service::{NodeRuntime, NodeRuntimeCallbacks};
use veil_transport_quic::QuicAdapter;

use crate::fallback_transport::{CombinedFallbackAdapter, FallbackPeer};
use crate::metrics_state::MetricsState;
use crate::nostr_bridge::BridgedItem;
use crate::peer_runtime::compute_peer_lists;
use crate::recording_adapter::RecordingAdapter;
use crate::runtime_bridge::{drain_bridged_items, publish_bridge_batch};
use crate::runtime_metrics::{note_ack_clears, note_send_failures, note_tick};
use crate::runtime_payloads::handle_runtime_delivered_payload;
use crate::time_utils::current_epoch;

pub(super) struct RuntimeTickInputs<'a> {
    pub runtime: &'a mut NodeRuntime<
        RecordingAdapter<QuicAdapter>,
        RecordingAdapter<CombinedFallbackAdapter>,
        XChaCha20Poly1305Cipher,
        NostrVerifier,
    >,
    pub runtime_config: &'a Arc<Mutex<NodeRuntimeConfig>>,
    pub fast_peers: &'a [String],
    pub fallback_peers: &'a [FallbackPeer],
    pub max_dynamic_peers: usize,
    pub metrics: &'a Arc<MetricsState>,
    pub discovery_table: &'a Arc<Mutex<HashMap<String, veil_android_node::ContactBundle>>>,
    pub feed_history: &'a Arc<Mutex<VecDeque<serde_json::Value>>>,
    pub nostr_bridge_rx: Option<&'a mut tokio::sync::mpsc::Receiver<BridgedItem>>,
    pub bridge_batcher: &'a mut FeedBatcher,
    pub bridge_namespace: Namespace,
    pub bridge_tag: Tag,
    pub now_step: u64,
    pub node_signer: &'a NostrSigner,
}

pub(super) fn run_runtime_tick(inputs: RuntimeTickInputs<'_>) -> u64 {
    let RuntimeTickInputs {
        runtime,
        runtime_config,
        fast_peers,
        fallback_peers,
        max_dynamic_peers,
        metrics,
        discovery_table,
        feed_history,
        nostr_bridge_rx,
        bridge_batcher,
        bridge_namespace,
        bridge_tag,
        now_step,
        node_signer,
    } = inputs;

    {
        let cfg = runtime_config.lock().unwrap_or_else(|e| e.into_inner());
        runtime.config = cfg.clone();
    }

    let discovered_fast_snapshot = runtime.fast_adapter.snapshot_seen();
    let discovered_fallback_snapshot = runtime.fallback_adapter.snapshot_seen();
    let (fast_peer_list, fallback_peer_list) = compute_peer_lists(
        fast_peers,
        fallback_peers,
        &discovered_fast_snapshot,
        &discovered_fallback_snapshot,
        max_dynamic_peers,
    );

    if let Some(rx) = nostr_bridge_rx {
        drain_bridged_items(rx, metrics.as_ref(), feed_history, bridge_batcher, 64);
        publish_bridge_batch(
            &mut runtime.state,
            &mut runtime.fast_adapter,
            &mut runtime.fallback_adapter,
            bridge_batcher,
            PublishQueueTickParams {
                namespace: bridge_namespace,
                epoch: current_epoch(),
                tag: bridge_tag,
                encrypt_key: &[0u8; 32],
                now_step,
                flags: veil_codec::object::OBJECT_FLAG_SIGNED
                    | veil_codec::object::OBJECT_FLAG_PUBLIC,
                interactive_flush: false,
                fast_peers: &fast_peer_list,
                fallback_peers: &fallback_peer_list,
            },
            &runtime.config,
            &XChaCha20Poly1305Cipher,
            Some(node_signer),
        );
    }

    let metrics_ref = Arc::clone(metrics);
    let feed_history_ref = Arc::clone(feed_history);
    let discovery_table_ref = Arc::clone(discovery_table);
    let _ = runtime.tick_with_callbacks(
        now_step,
        &fast_peer_list,
        &fallback_peer_list,
        NodeRuntimeCallbacks {
            on_delivered: Some(&mut |_root, payload| {
                handle_runtime_delivered_payload(
                    payload,
                    metrics_ref.as_ref(),
                    &discovery_table_ref,
                    &feed_history_ref,
                )
            }),
            on_send_failure: Some(&mut |count| {
                note_send_failures(metrics_ref.as_ref(), count);
            }),
            on_ack_cleared: Some(&mut |count| {
                note_ack_clears(metrics_ref.as_ref(), count);
            }),
            ..NodeRuntimeCallbacks::default()
        },
    );
    note_tick(metrics.as_ref());
    now_step.saturating_add(1)
}
