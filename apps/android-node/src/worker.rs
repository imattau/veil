use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;

use crate::api::QueueWorkerConfig;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;

#[derive(Clone)]
pub struct QueueWorker {
    state: Arc<NodeState>,
    protocol: Arc<ProtocolEngine>,
    config: QueueWorkerConfig,
}

impl QueueWorker {
    pub fn new(state: Arc<NodeState>, protocol: Arc<ProtocolEngine>, config: QueueWorkerConfig) -> Self {
        Self {
            state,
            protocol,
            config,
        }
    }

    pub async fn run(self) {
        let tick = Duration::from_millis(self.config.tick_ms.max(50));
        loop {
            if let Some(item) = self.state.take_next_queued() {
                let attempts = self.state.attempts_for(&item);
                if attempts > self.config.max_attempts {
                    self.state.complete_item(&item, false);
                } else {
                    let result = self
                        .protocol
                        .publish(item.payload.clone().into_bytes())
                        .await;
                    self.state.complete_item(&item, result.is_ok());
                }
            }
            sleep(tick).await;
        }
    }
}
