use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::Deserialize;
use tracing::warn;

use crate::time_utils::now_unix_secs;

#[derive(Debug)]
pub struct AdminAuthState {
    pub server_pubkey: [u8; 32],
    pub server_pubkey_hex: String,
    pub server_secret_hex: String,
    pub server_secret_nsec: String,
    pub session_ttl_secs: u64,
    pub session_db_path: PathBuf,
    pub settings_db_path: PathBuf,
    pub sessions: Mutex<HashMap<String, u64>>,
}

#[derive(Debug, Deserialize)]
pub struct AdminLoginRequest {
    pub secret: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminSettingUpsertRequest {
    pub key: String,
    pub value: String,
}

impl AdminAuthState {
    pub(super) fn bootstrap_session_db(path: &Path) -> Result<(), String> {
        if let Err(err) = ensure_parent(path) {
            return Err(format!(
                "failed to create session db parent {}: {err}",
                path.display()
            ));
        }
        match Connection::open(path) {
            Ok(conn) => {
                if let Err(err) = conn.execute(
                    "CREATE TABLE IF NOT EXISTS admin_sessions (
                        token TEXT PRIMARY KEY,
                        expires_at INTEGER NOT NULL
                    )",
                    params![],
                ) {
                    return Err(format!(
                        "failed to initialize session table {}: {err}",
                        path.display()
                    ));
                }
                let now = now_unix_secs() as i64;
                let _ = conn.execute(
                    "DELETE FROM admin_sessions WHERE expires_at <= ?1",
                    params![now],
                );
                Ok(())
            }
            Err(err) => Err(format!(
                "failed to open session db {}: {err}",
                path.display()
            )),
        }
    }

    pub(super) fn load_sessions_from_db(path: &Path) -> HashMap<String, u64> {
        let mut out = HashMap::new();
        let conn = match Connection::open(path) {
            Ok(conn) => conn,
            Err(err) => {
                warn!("admin auth: could not open session db for loading: {err}");
                return out;
            }
        };
        let now = now_unix_secs() as i64;
        let _ = conn.execute(
            "DELETE FROM admin_sessions WHERE expires_at <= ?1",
            params![now],
        );
        let Ok(mut stmt) = conn.prepare("SELECT token, expires_at FROM admin_sessions") else {
            return out;
        };
        let rows = stmt.query_map(params![], |row| {
            let token: String = row.get(0)?;
            let expires_at: i64 = row.get(1)?;
            Ok((token, expires_at))
        });
        if let Ok(rows) = rows {
            for (token, expires_at) in rows.flatten() {
                if expires_at > 0 {
                    out.insert(token, expires_at as u64);
                }
            }
        }
        out
    }

    fn persist_session_insert(&self, token: &str, expires: u64) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO admin_sessions (token, expires_at) VALUES (?1, ?2)",
                params![token, expires as i64],
            );
        }
    }

    pub fn persist_session_remove(&self, token: &str) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "DELETE FROM admin_sessions WHERE token = ?1",
                params![token],
            );
        }
    }

    pub fn persist_expired_prune(&self, now: u64) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "DELETE FROM admin_sessions WHERE expires_at <= ?1",
                params![now as i64],
            );
        }
    }

    pub fn add_session(&self, token: String, expires: u64) {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token.clone(), expires);
        self.persist_session_insert(&token, expires);
    }

    pub fn revoke_session(&self, token: &str) -> bool {
        let removed = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(token)
            .is_some();
        self.persist_session_remove(token);
        removed
    }
}

fn ensure_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
