use veil_core::{Epoch, Namespace};
use veil_node::service::PublisherTickInput;

use super::ProtocolRuntime;

pub(super) fn enqueue_payload(runtime: &mut ProtocolRuntime, payload: Vec<u8>) {
    runtime.enqueue(payload);
}

pub(super) fn enqueue_payload_batch(runtime: &mut ProtocolRuntime, payloads: Vec<Vec<u8>>) {
    for payload in payloads {
        runtime.enqueue(payload);
    }
}

pub(super) fn tick_publish(
    runtime: &mut ProtocolRuntime,
    namespace: Namespace,
    epoch: Epoch,
    tag: [u8; 32],
    now_step: u64,
    fast_peers: &[String],
    fallback_peers: &[String],
    interactive_flush: bool,
) -> Result<(), String> {
    runtime
        .tick(PublisherTickInput {
            namespace,
            epoch,
            tag,
            now_step,
            flags: 0,
            interactive_flush,
            fast_peers,
            fallback_peers,
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}
