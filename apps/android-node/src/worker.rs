use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;

use crate::api::QueueWorkerConfig;
use crate::state::NodeState;

#[derive(Clone)]
pub struct QueueWorker {
    state: Arc<NodeState>,
    config: QueueWorkerConfig,
}

impl QueueWorker {
    pub fn new(state: Arc<NodeState>, config: QueueWorkerConfig) -> Self {
        Self { state, config }
    }

    pub async fn run(self) {
        let tick = Duration::from_millis(self.config.tick_ms.max(50));
        loop {
            if let Some(item) = self.state.take_next_queued() {
                let attempts = self.state.attempts_for(&item);
                if attempts > self.config.max_attempts {
                    self.state.complete_item(&item, false);
                } else {
                    // Placeholder for real send path.
                    self.state.complete_item(&item, true);
                }
            }
            sleep(tick).await;
        }
    }
}
