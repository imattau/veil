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
        let config = normalize_worker_config(config);
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

        // Initial subscription sync
        {
            let channels = worker.state.get_subscriptions();
            let contacts = worker.state.contacts();
            worker
                .protocol
                .sync_subscriptions(&channels, &contacts)
                .await;
        }

        loop {
            let mut busy = false;
            worker.step = worker.step.saturating_add(1);
            match worker.protocol.pump_inbound().await {
                Ok(Some(ReceiveEvent::Delivered {
                    object_root,
                    payload,
                    namespace,
                    epoch,
                    tag,
                    flags,
                })) => {
                    busy = true;
                    if namespace == worker.protocol.discovery_namespace() {
                        let _ = handle_discovery_payload(&worker.state, &worker.protocol, &payload)
                            .await;
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
                Ok(Some(_)) => {} // Ignored variants (malformed, duplicate, etc.)
                Ok(None) => {}    // Nothing delivered this tick
                Err(e) => {
                    tracing::error!("Inbound pump error: {}", e);
                }
            }
            if worker.step.is_multiple_of(50) {
                worker.protocol.persist_cache_state().await;
                worker.state.persist();

                // Sync subscriptions from UI state to ProtocolEngine
                let channels = worker.state.get_subscriptions();
                let contacts = worker.state.contacts();
                worker
                    .protocol
                    .sync_subscriptions(&channels, &contacts)
                    .await;
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

fn normalize_worker_config(config: QueueWorkerConfig) -> QueueWorkerConfig {
    let tick_ms = config.tick_ms.max(50);
    let max_attempts = config.max_attempts.max(1);
    let backoff_base_ms = config.backoff_base_ms.max(50);
    let backoff_max_ms = config.backoff_max_ms.max(backoff_base_ms);
    QueueWorkerConfig {
        tick_ms,
        max_attempts,
        backoff_base_ms,
        backoff_max_ms,
    }
}

fn retry_backoff_ms(attempts: u32, base_ms: u64, max_ms: u64) -> u64 {
    let exponent = attempts.saturating_sub(1).min(10);
    let factor = 2u32.saturating_pow(exponent);
    let base = Duration::from_millis(base_ms);
    let max = Duration::from_millis(max_ms);
    let scaled = base.saturating_mul(factor);
    let bounded = scaled.clamp(base, max);
    bounded.as_millis().try_into().unwrap_or(u64::MAX)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(serde::Deserialize)]
struct QueuedPayloadEnvelope {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    payload_b64: Option<String>,
}

fn queued_payload_to_bytes(payload: &str) -> Vec<u8> {
    let parsed = match serde_json::from_str::<QueuedPayloadEnvelope>(payload) {
        Ok(value) => value,
        Err(_) => return payload.as_bytes().to_vec(),
    };
    match parsed.kind.as_deref() {
        Some("raw_object_b64") | Some("raw_b64") => {
            let Some(b64) = parsed.payload_b64.as_deref() else {
                return payload.as_bytes().to_vec();
            };
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap_or_else(|_| payload.as_bytes().to_vec())
        }
        _ => payload.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_worker_config, queued_payload_to_bytes, retry_backoff_ms};
    use crate::api::QueueWorkerConfig;

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

    #[test]
    fn wrapped_payload_without_payload_b64_falls_back_to_utf8_bytes() {
        let payload = r#"{"kind":"raw_b64"}"#;
        let bytes = queued_payload_to_bytes(payload);
        assert_eq!(bytes, payload.as_bytes());
    }

    #[test]
    fn normalize_worker_config_enforces_safe_minimums() {
        let normalized = normalize_worker_config(QueueWorkerConfig {
            tick_ms: 0,
            max_attempts: 0,
            backoff_base_ms: 0,
            backoff_max_ms: 0,
        });
        assert_eq!(normalized.tick_ms, 50);
        assert_eq!(normalized.max_attempts, 1);
        assert_eq!(normalized.backoff_base_ms, 50);
        assert_eq!(normalized.backoff_max_ms, 50);
    }

    #[test]
    fn normalize_worker_config_preserves_valid_values() {
        let normalized = normalize_worker_config(QueueWorkerConfig {
            tick_ms: 500,
            max_attempts: 3,
            backoff_base_ms: 500,
            backoff_max_ms: 20_000,
        });
        assert_eq!(normalized.tick_ms, 500);
        assert_eq!(normalized.max_attempts, 3);
        assert_eq!(normalized.backoff_base_ms, 500);
        assert_eq!(normalized.backoff_max_ms, 20_000);
    }

    #[test]
    fn retry_backoff_ms_scales_and_caps() {
        assert_eq!(retry_backoff_ms(1, 500, 20_000), 500);
        assert_eq!(retry_backoff_ms(2, 500, 20_000), 1_000);
        assert_eq!(retry_backoff_ms(3, 500, 20_000), 2_000);
        assert_eq!(retry_backoff_ms(20, 500, 20_000), 20_000);
    }
}
