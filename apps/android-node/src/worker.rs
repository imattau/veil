use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use tokio::time::sleep;

use crate::api::QueueWorkerConfig;
use crate::discovery::handle_discovery_payload;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;
use veil_node::receive::ReceiveEvent;

const APP_TARGET_BATCH_SIZE_BYTES: usize = 96 * 1024;
const APP_MAX_BATCH_ITEMS: usize = 64;
const APP_MAX_BATCHABLE_ITEM_BYTES: usize = 4 * 1024;

#[derive(Clone)]
pub struct QueueWorker {
    state: Arc<NodeState>,
    protocol: Arc<ProtocolEngine>,
    config: QueueWorkerConfig,
    step: u64,
}

impl QueueWorker {
    pub fn new(
        state: Arc<NodeState>,
        protocol: Arc<ProtocolEngine>,
        config: QueueWorkerConfig,
    ) -> Self {
        Self {
            state,
            protocol,
            config,
            step: 0,
        }
    }

    pub async fn run(self) {
        let mut worker = self;
        let base_tick = Duration::from_millis(worker.config.tick_ms.max(50));
        loop {
            let mut busy = false;
            worker.step = worker.step.saturating_add(1);
            if let Ok(Some(ReceiveEvent::Delivered {
                object_root,
                payload,
                namespace,
                epoch,
                tag,
                flags,
            })) = worker.protocol.pump_inbound().await
            {
                busy = true;
                if namespace == worker.protocol.discovery_namespace() {
                    let _ =
                        handle_discovery_payload(&worker.state, &worker.protocol, &payload).await;
                }
                worker.state.emit_payload(
                    &object_root,
                    &payload,
                    namespace.0,
                    epoch.0,
                    &tag,
                    flags,
                );
                if worker
                    .state
                    .ingest_endorsement_payload(&payload, worker.step)
                {
                    worker
                        .protocol
                        .update_wot_policy(worker.state.wot_policy())
                        .await;
                }
            }
            if worker.step.is_multiple_of(50) {
                worker.protocol.persist_cache_state().await;
                worker.state.persist();
            }
            let details = worker.protocol.lane_details().await;
            worker.state.mark_lane_details(details);

            // Do not gate outbound draining on "connected" snapshots. Some transports
            // can make progress while reconnecting and stale snapshots can otherwise
            // leave the queue permanently stalled.
            let now_ms = now_millis();
            let batch = worker.state.take_next_queued_batch(
                now_ms,
                APP_MAX_BATCH_ITEMS,
                APP_TARGET_BATCH_SIZE_BYTES,
                APP_MAX_BATCHABLE_ITEM_BYTES,
            );
            if !batch.is_empty() {
                busy = true;
                let mut executable = Vec::with_capacity(batch.len());
                let mut namespace = None;
                let mut payloads = Vec::with_capacity(batch.len());
                for item in batch {
                    let attempts = worker.state.attempts_for(&item);
                    if attempts > worker.config.max_attempts {
                        worker.state.drop_item(&item);
                        continue;
                    }
                    namespace = Some(item.namespace);
                    payloads.push(queued_payload_to_bytes(&item.payload));
                    executable.push(item);
                }
                if !executable.is_empty() {
                    let result = worker.protocol.publish_batch(payloads, namespace).await;
                    if result.is_ok() {
                        for item in &executable {
                            worker.state.complete_success(item);
                        }
                    } else {
                        for item in executable {
                            let attempts = worker.state.attempts_for(&item);
                            let backoff = retry_backoff_ms(
                                attempts,
                                worker.config.backoff_base_ms,
                                worker.config.backoff_max_ms,
                            );
                            worker.state.complete_failure(item, backoff);
                        }
                    }
                }
            }
            
            if busy {
                // Short sleep when busy to process backlogs quickly
                sleep(Duration::from_millis(10)).await;
            } else {
                sleep(base_tick).await;
            }
        }
    }
}

fn retry_backoff_ms(attempts: u32, base_ms: u64, max_ms: u64) -> u64 {
    let exponent = attempts.saturating_sub(1).min(10);
    let factor = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
    let raw = base_ms.saturating_mul(factor);
    raw.min(max_ms).max(base_ms)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn queued_payload_to_bytes(payload: &str) -> Vec<u8> {
    let parsed = match serde_json::from_str::<serde_json::Value>(payload) {
        Ok(value) => value,
        Err(_) => return payload.as_bytes().to_vec(),
    };
    let kind = parsed.get("kind").and_then(|v| v.as_str());
    if kind != Some("raw_b64") {
        return payload.as_bytes().to_vec();
    }
    let b64 = match parsed.get("payload_b64").and_then(|v| v.as_str()) {
        Some(value) => value,
        None => return payload.as_bytes().to_vec(),
    };
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .unwrap_or_else(|_| payload.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::queued_payload_to_bytes;

    #[test]
    fn decode_wrapped_raw_b64_payload() {
        let wrapped = serde_json::json!({
            "kind": "raw_b64",
            "payload_b64": "aGVsbG8=",
        })
        .to_string();
        let bytes = queued_payload_to_bytes(&wrapped);
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn non_wrapped_payload_falls_back_to_utf8_bytes() {
        let payload = r#"{"kind":"post","text":"hello"}"#;
        let bytes = queued_payload_to_bytes(payload);
        assert_eq!(bytes, payload.as_bytes());
    }
}
