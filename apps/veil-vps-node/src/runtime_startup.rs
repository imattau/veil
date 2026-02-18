use std::path::Path;
use std::sync::Arc;

use tracing::{info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::logger::{AdminLoggerLayer, LogBuffer};
use crate::settings_runtime::apply_settings_db_overrides;

pub(super) fn init_tracing(log_buffer: &Arc<LogBuffer>) {
    let filter = std::env::var("VEIL_LOG").unwrap_or_else(|_| "info".to_string());

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .with(AdminLoggerLayer {
            buffer: Arc::clone(log_buffer),
        })
        .init();
}

pub(super) fn load_env_and_log_startup(config_path: Option<&Path>) {
    dotenvy::dotenv().ok();
    if let Ok(cwd) = std::env::current_dir() {
        info!("starting veil-vps-node in {}", cwd.display());
        if cwd.to_string_lossy().starts_with("/home/")
            && !cwd.to_string_lossy().contains("workspace")
        {
            warn!("Node is running from a home directory. Ensure this is intentional and that absolute paths are set for 'data/' directories if running as a service user.");
        }
    }

    if config_path.and_then(|p| p.extension().and_then(|ext| ext.to_str())) == Some("env") {
        let config_path = config_path.expect("config_path should exist for .env branch");
        match dotenvy::from_path(config_path) {
            Ok(_) => info!("pre-loaded environment from {}", config_path.display()),
            Err(err) => warn!(
                "failed to pre-load .env from {}: {}",
                config_path.display(),
                err
            ),
        }
    }
}

pub(super) fn apply_settings_mode(safe_mode: bool, settings_db_path: &Path) {
    if safe_mode {
        warn!(
            "SAFE MODE ENABLED: Ignoring all settings overrides from {}",
            settings_db_path.display()
        );
    } else {
        apply_settings_db_overrides(settings_db_path);
    }
}
