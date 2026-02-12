use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::time::sleep;

use crate::api::QueueWorkerConfig;
use crate::discovery::handle_discovery_payload;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;
use veil_node::receive::ReceiveEvent;

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
        let tick = Duration::from_millis(worker.config.tick_ms.max(50));
        loop {
            worker.step = worker.step.saturating_add(1);
            if let Ok(event) = worker.protocol.pump_inbound().await {
                if let Some(ReceiveEvent::Delivered {
                    object_root,
                    payload,
                    namespace,
                    epoch,
                    tag,
                    flags,
                }) = event
                {
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
            }
            if worker.step % 50 == 0 {
                worker.protocol.persist_cache_state().await;
                worker.state.persist();
            }
            let details = worker.protocol.lane_details().await;
            let any_connected = details.iter().any(|detail| detail.connected);
            worker.state.mark_lane_details(details);
            if any_connected {
                let now_ms = now_millis();
                if let Some(item) = worker.state.take_next_queued(now_ms) {
                    let attempts = worker.state.attempts_for(&item);
                    if attempts > worker.config.max_attempts {
                        worker.state.drop_item(&item);
                    } else {
                        let result = worker
                            .protocol
                            .publish(item.payload.clone().into_bytes(), Some(item.namespace))
                            .await;
                        if result.is_ok() {
                            worker.state.complete_success(&item);
                        } else {
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
            sleep(tick).await;
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
