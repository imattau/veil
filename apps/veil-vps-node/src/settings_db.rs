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

            // Check if the directory is writable by attempting to create a dummy file
            let abs_parent = std::env::current_dir()
                .map(|c| c.join(parent))
                .unwrap_or_else(|_| parent.to_path_buf());
            let test_path = parent.join(".veil_write_test");
            let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

            match fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&test_path)
            {
                Ok(_) => {
                    let _ = fs::remove_file(test_path);
                }
                Err(e) => {
                    return Err(format!(
                        "directory {} (absolute: {}) is not writable for user '{}': {}",
                        parent.display(),
                        abs_parent.display(),
                        user,
                        e
                    ));
                }
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
