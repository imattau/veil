use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;

use crate::metrics_state::MetricsState;

pub(super) struct RuntimeStateSetup {
    pub metrics: Arc<MetricsState>,
    pub shutdown: Arc<AtomicBool>,
    pub peer_snapshot: Arc<Mutex<Vec<String>>>,
    pub feed_history: Arc<Mutex<VecDeque<serde_json::Value>>>,
}

pub(super) fn init_runtime_state_setup(
    nostr_bridge_enabled: bool,
    nostr_bridge_relays_count: usize,
) -> RuntimeStateSetup {
    let setup = build_runtime_state_setup(nostr_bridge_enabled, nostr_bridge_relays_count);
    let _ = flag::register(SIGTERM, Arc::clone(&setup.shutdown));
    let _ = flag::register(SIGINT, Arc::clone(&setup.shutdown));
    setup
}

fn build_runtime_state_setup(
    nostr_bridge_enabled: bool,
    nostr_bridge_relays_count: usize,
) -> RuntimeStateSetup {
    let metrics = Arc::new(MetricsState::default());
    metrics
        .nostr_bridge_enabled
        .store(u64::from(nostr_bridge_enabled), Ordering::Relaxed);
    metrics
        .nostr_bridge_relays_configured
        .store(nostr_bridge_relays_count as u64, Ordering::Relaxed);

    RuntimeStateSetup {
        metrics,
        shutdown: Arc::new(AtomicBool::new(false)),
        peer_snapshot: Arc::new(Mutex::new(Vec::new())),
        feed_history: Arc::new(Mutex::new(VecDeque::with_capacity(50))),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::build_runtime_state_setup;

    #[test]
    fn build_runtime_state_setup_initializes_bridge_metrics_and_buffers() {
        let setup = build_runtime_state_setup(true, 3);

        assert_eq!(
            setup.metrics.nostr_bridge_enabled.load(Ordering::Relaxed),
            1
        );
        assert_eq!(
            setup
                .metrics
                .nostr_bridge_relays_configured
                .load(Ordering::Relaxed),
            3
        );
        assert!(setup
            .peer_snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty());
        assert!(setup
            .feed_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty());
    }
}
