use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tracing::{error, info};
use veil_node::persistence::save_state_to_path;
use veil_node::state::NodeState;
use veil_transport::adapter::TransportHealthSnapshot;

use crate::fallback_transport::FallbackPeer;
use crate::metrics_state::MetricsState;
use crate::peer_runtime::persist_peer_snapshot;
use crate::runtime_metrics::apply_transport_health;

pub(super) fn handle_shutdown_if_requested(
    shutdown: &AtomicBool,
    state_path: &Path,
    state: &mut NodeState,
    peer_db: Option<&Connection>,
    peer_snapshot: &Arc<Mutex<Vec<String>>>,
    fast_seen: Vec<String>,
    fallback_seen: Vec<FallbackPeer>,
    max_peer_db_rows: usize,
) -> bool {
    if !shutdown.load(Ordering::Relaxed) {
        return false;
    }
    if let Err(err) = save_state_to_path(state_path, state) {
        error!("snapshot failed on shutdown: {err}");
    }
    persist_peer_snapshot(
        peer_db,
        peer_snapshot,
        fast_seen,
        fallback_seen,
        max_peer_db_rows,
    );
    true
}

pub(super) fn maybe_snapshot_state(
    last_snapshot: &mut Instant,
    snapshot_interval: Duration,
    state_path: &Path,
    state: &mut NodeState,
    peer_db: Option<&Connection>,
    peer_snapshot: &Arc<Mutex<Vec<String>>>,
    fast_seen: Vec<String>,
    fallback_seen: Vec<FallbackPeer>,
    max_peer_db_rows: usize,
) {
    if last_snapshot.elapsed() < snapshot_interval {
        return;
    }
    if let Err(err) = save_state_to_path(state_path, state) {
        error!("snapshot failed: {err}");
    }
    persist_peer_snapshot(
        peer_db,
        peer_snapshot,
        fast_seen,
        fallback_seen,
        max_peer_db_rows,
    );
    *last_snapshot = Instant::now();
}

pub(super) fn maybe_log_transport_health(
    last_health_log: &mut Instant,
    health_log_interval: Duration,
    metrics: &MetricsState,
    fast_lane: &TransportHealthSnapshot,
    fallback_lane: &TransportHealthSnapshot,
) {
    if last_health_log.elapsed() < health_log_interval {
        return;
    }
    apply_transport_health(metrics, fast_lane, fallback_lane);
    info!(
        "fast_lane: {:?}, fallback_lane: {:?}",
        fast_lane, fallback_lane
    );
    *last_health_log = Instant::now();
}

#[cfg(test)]
mod tests {
    use super::handle_shutdown_if_requested;
    use crate::fallback_transport::FallbackPeer;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use veil_node::state::NodeState;

    #[test]
    fn shutdown_handler_persists_peers_and_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.cbor");
        let mut state = NodeState::default();
        let shutdown = AtomicBool::new(true);
        let peer_snapshot = Arc::new(Mutex::new(Vec::new()));

        let did_shutdown = handle_shutdown_if_requested(
            &shutdown,
            &state_path,
            &mut state,
            None,
            &peer_snapshot,
            vec!["peer-fast-a".to_string()],
            vec![FallbackPeer::WebSocket("relay-a".to_string())],
            128,
        );

        assert!(did_shutdown);
        assert!(state_path.exists(), "state snapshot should be written");
        let peers = peer_snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        assert_eq!(
            peers,
            vec!["peer-fast-a".to_string(), "ws:relay-a".to_string()]
        );
    }

    #[test]
    fn shutdown_handler_noops_when_not_requested() {
        let state_path = PathBuf::from("unused.cbor");
        let mut state = NodeState::default();
        let shutdown = AtomicBool::new(false);
        let peer_snapshot = Arc::new(Mutex::new(vec!["existing".to_string()]));

        let did_shutdown = handle_shutdown_if_requested(
            &shutdown,
            &state_path,
            &mut state,
            None,
            &peer_snapshot,
            vec!["peer-fast-a".to_string()],
            vec![FallbackPeer::WebSocket("relay-a".to_string())],
            128,
        );

        assert!(!did_shutdown);
        let peers = peer_snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        assert_eq!(peers, vec!["existing".to_string()]);
    }
}
