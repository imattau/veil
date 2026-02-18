use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::admin_auth::AdminAuthState;
use crate::http_server;
use crate::logger::LogBuffer;
use crate::metrics_state::MetricsState;

pub(super) async fn init_admin_runtime(
    admin_session_db_path: &Path,
    settings_db_path: &Path,
    node_pubkey: [u8; 32],
    node_pubkey_hex: &str,
    node_secret_hex: &str,
    node_secret_nsec: &str,
    health_bind: &str,
    health_port: u16,
    metrics: &Arc<MetricsState>,
    peer_snapshot: &Arc<Mutex<Vec<String>>>,
    feed_history: &Arc<Mutex<VecDeque<serde_json::Value>>>,
    discovery_table: &Arc<Mutex<HashMap<String, veil_android_node::ContactBundle>>>,
    shutdown: &Arc<AtomicBool>,
    log_buffer: &Arc<LogBuffer>,
    runtime_config: &Arc<Mutex<veil_node::config::NodeRuntimeConfig>>,
) -> Result<Arc<AdminAuthState>, String> {
    if let Err(err) = AdminAuthState::bootstrap_session_db(admin_session_db_path) {
        return Err(format!("fatal: admin auth bootstrap failed: {err}"));
    }
    let restored_sessions = AdminAuthState::load_sessions_from_db(admin_session_db_path);
    if !restored_sessions.is_empty() {
        tracing::info!(
            "admin auth: restored {} active sessions from {}",
            restored_sessions.len(),
            admin_session_db_path.display()
        );
    }
    let admin_auth = Arc::new(AdminAuthState {
        server_pubkey: node_pubkey,
        server_pubkey_hex: node_pubkey_hex.to_string(),
        server_secret_hex: node_secret_hex.to_string(),
        server_secret_nsec: node_secret_nsec.to_string(),
        session_ttl_secs: 24 * 60 * 60,
        session_db_path: admin_session_db_path.to_path_buf(),
        settings_db_path: settings_db_path.to_path_buf(),
        sessions: Mutex::new(restored_sessions),
    });
    if health_port != 0 {
        let app_state = http_server::VpsAppState {
            metrics: Arc::clone(metrics),
            peer_snapshot: Arc::clone(peer_snapshot),
            feed_history: Arc::clone(feed_history),
            discovery_table: Arc::clone(discovery_table),
            admin_auth: Arc::clone(&admin_auth),
            shutdown: Arc::clone(shutdown),
            log_buffer: Arc::clone(log_buffer),
            runtime_config: Arc::clone(runtime_config),
        };
        if let Err(err) =
            http_server::spawn_health_server(app_state, health_bind, health_port).await
        {
            return Err(err.to_string());
        }
    }
    Ok(admin_auth)
}
