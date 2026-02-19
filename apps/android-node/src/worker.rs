use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;

use crate::api::QueueWorkerConfig;
use crate::discovery::handle_discovery_payload;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;
use veil_node::receive::ReceiveEvent;

mod helpers;

use self::helpers::{
    normalize_worker_config, now_millis, queued_payload_to_bytes, retry_backoff_with_jitter_ms,
};

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
                            let backoff = retry_backoff_with_jitter_ms(
                                attempts,
                                worker.config.backoff_base_ms,
                                worker.config.backoff_max_ms,
                                item.id.as_u128(),
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
