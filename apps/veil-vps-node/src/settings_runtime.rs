use std::path::{Path, PathBuf};

use tracing::{error, info, warn};

use crate::settings_db::SettingsStore;
use crate::{Commands, SettingsCommands};

pub(super) fn maybe_handle_settings_command(command: Option<&Commands>) -> bool {
    let Some(Commands::Settings { db, action }) = command else {
        return false;
    };

    let store = match SettingsStore::open(db) {
        Ok(store) => store,
        Err(err) => {
            error!("settings db open failed: {err}");
            std::process::exit(1);
        }
    };

    match action {
        SettingsCommands::List => match store.list() {
            Ok(items) => {
                for (k, v) in items {
                    println!("{k}={v}");
                }
            }
            Err(err) => {
                error!("{err}");
                std::process::exit(1);
            }
        },
        SettingsCommands::Get { key } => {
            if let Some(v) = store.get(key) {
                println!("{v}");
            } else {
                std::process::exit(3);
            }
        }
        SettingsCommands::Set { key, value } => {
            if let Err(err) = store.set(key, value.trim()) {
                error!("{err}");
                std::process::exit(1);
            }
            println!("ok");
        }
        SettingsCommands::Delete { key } => match store.delete(key) {
            Ok(true) => println!("deleted"),
            Ok(false) => std::process::exit(3),
            Err(err) => {
                error!("{err}");
                std::process::exit(1);
            }
        },
    }
    true
}

pub(super) fn settings_db_path_from_env() -> PathBuf {
    std::env::var("VEIL_VPS_SETTINGS_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/settings.db"))
}

pub(super) fn normalize_settings_key(key: &str) -> Option<&'static str> {
    match key {
        // Backward compatibility for older admin UI typo.
        "VEIL_VPS_NOSTR_BRIDGE_ENABLE" => Some("VEIL_VPS_NOSTR_BRIDGE_ENABLED"),
        "VEIL_VPS_OPEN_RELAY" => Some("VEIL_VPS_OPEN_RELAY"),
        "VEIL_VPS_NOSTR_BRIDGE_ENABLED" => Some("VEIL_VPS_NOSTR_BRIDGE_ENABLED"),
        "VEIL_VPS_NOSTR_RELAYS" => Some("VEIL_VPS_NOSTR_BRIDGE_RELAYS"),
        "VEIL_VPS_NOSTR_BRIDGE_RELAYS" => Some("VEIL_VPS_NOSTR_BRIDGE_RELAYS"),
        "VEIL_VPS_NOSTR_BRIDGE_CHANNEL_ID" => Some("VEIL_VPS_NOSTR_BRIDGE_CHANNEL_ID"),
        "VEIL_VPS_NOSTR_BRIDGE_NAMESPACE" => Some("VEIL_VPS_NOSTR_BRIDGE_NAMESPACE"),
        "VEIL_VPS_NOSTR_BRIDGE_SINCE" => Some("VEIL_VPS_NOSTR_BRIDGE_SINCE"),
        "VEIL_VPS_NOSTR_BRIDGE_STATE_PATH" => Some("VEIL_VPS_NOSTR_BRIDGE_STATE_PATH"),
        "VEIL_VPS_NOSTR_BRIDGE_MAX_SEEN_IDS" => Some("VEIL_VPS_NOSTR_BRIDGE_MAX_SEEN_IDS"),
        "VEIL_VPS_NOSTR_BRIDGE_PERSIST_EVERY_UPDATES" => {
            Some("VEIL_VPS_NOSTR_BRIDGE_PERSIST_EVERY_UPDATES")
        }
        "VEIL_VPS_ADAPTIVE_LANE_SCORING" => Some("VEIL_VPS_ADAPTIVE_LANE_SCORING"),
        "VEIL_VPS_PROBABILISTIC_FORWARDING" => Some("VEIL_VPS_PROBABILISTIC_FORWARDING"),
        "VEIL_VPS_BLOOM_EXCHANGE" => Some("VEIL_VPS_BLOOM_EXCHANGE"),
        // QUIC cert/key paths are always internal; do not allow DB overrides
        // that could point at stale external paths from older installs.
        "VEIL_VPS_QUIC_BIND" => Some("VEIL_VPS_QUIC_BIND"),
        "VEIL_VPS_WS_URL" => Some("VEIL_VPS_WS_URL"),
        "VEIL_VPS_WS_LISTEN" => Some("VEIL_VPS_WS_LISTEN"),
        "VEIL_VPS_WS_PEER" => Some("VEIL_VPS_WS_PEER"),
        "VEIL_VPS_TOR_SOCKS_ADDR" => Some("VEIL_VPS_TOR_SOCKS_ADDR"),
        _ => None,
    }
}

pub(super) fn apply_settings_db_overrides(path: &Path) {
    let store = match SettingsStore::open(path) {
        Ok(store) => store,
        Err(err) => {
            warn!("settings db unavailable at {}: {}", path.display(), err);
            return;
        }
    };

    let entries = match store.list() {
        Ok(entries) => entries,
        Err(err) => {
            warn!("settings db list failed at {}: {}", path.display(), err);
            return;
        }
    };

    let mut applied = 0usize;
    for (raw_key, raw_value) in entries {
        let Some(key) = normalize_settings_key(&raw_key) else {
            continue;
        };
        let value = raw_value.trim();
        std::env::set_var(key, value);
        applied = applied.saturating_add(1);
    }

    if applied > 0 {
        info!(
            "applied {} runtime setting overrides from {}",
            applied,
            path.display()
        );
    }
}
