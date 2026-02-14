use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

pub struct SettingsStore {
    conn: Connection,
}

impl SettingsStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!("failed to create directory {}: {}", parent.display(), e)
                })?;
            }
        }
        let conn = Connection::open(path).map_err(|e| {
            format!(
                "open settings db at {}: {}",
                path.display(),
                e
            )
        })?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS settings (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL,
               updated_at INTEGER NOT NULL
             );",
        )
        .map_err(|e| format!("init settings db: {e}"))?;
        Ok(Self { conn })
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .ok()
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE
                 SET value=excluded.value, updated_at=excluded.updated_at",
                params![key, value, now_ms()],
            )
            .map(|_| ())
            .map_err(|e| format!("set({key}): {e}"))
    }

    pub fn delete(&self, key: &str) -> Result<bool, String> {
        self.conn
            .execute("DELETE FROM settings WHERE key=?1", [key])
            .map(|changed| changed > 0)
            .map_err(|e| format!("delete({key}): {e}"))
    }

    pub fn list(&self) -> Result<Vec<(String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key ASC")
            .map_err(|e| format!("list prepare: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("list query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            let (k, v) = row.map_err(|e| format!("list row: {e}"))?;
            out.push((k, v));
        }
        Ok(out)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
