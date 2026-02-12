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
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|e| format!("open settings db: {e}"))?;
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

    pub fn set_if_absent(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![key, value, now_ms()],
            )
            .map(|_| ())
            .map_err(|e| format!("set_if_absent({key}): {e}"))
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

    pub fn is_empty(&self) -> bool {
        self.conn
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get::<_, i64>(0))
            .map(|count| count == 0)
            .unwrap_or(false)
    }

    pub fn import_env_file(&self, path: &Path) -> Result<usize, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("read env file: {e}"))?;
        let mut inserted = 0usize;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() {
                continue;
            }
            if self.set_if_absent(key, value).is_ok() {
                inserted += 1;
            }
        }
        Ok(inserted)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
