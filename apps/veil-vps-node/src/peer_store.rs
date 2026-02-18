use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};
use tracing::error;

pub(super) fn open_peer_db(path: &Path) -> Option<Connection> {
    if let Err(err) = ensure_parent(path) {
        error!("failed to create peer db dir: {err}");
        return None;
    }
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(err) => {
            error!("failed to open peer db: {err}");
            return None;
        }
    };
    let _ = conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
    );
    if let Err(err) = conn.execute(
        "CREATE TABLE IF NOT EXISTS peers (peer TEXT PRIMARY KEY, last_seen_ms INTEGER NOT NULL)",
        [],
    ) {
        eprintln!("failed to init peer db: {err}");
        return None;
    }
    Some(conn)
}

pub(super) fn load_peer_list(conn: &Connection, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare("SELECT peer FROM peers ORDER BY last_seen_ms DESC LIMIT ?1")
    {
        Ok(stmt) => stmt,
        Err(_) => return out,
    };
    let rows = stmt.query_map([limit as i64], |row| row.get::<_, String>(0));
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            out.push(row);
        }
    }
    out
}

pub(super) fn save_peer_list(conn: &Connection, peers: &[String], max_rows: usize) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    for peer in peers {
        let _ = conn.execute(
            "INSERT INTO peers (peer, last_seen_ms) VALUES (?1, ?2)\n             ON CONFLICT(peer) DO UPDATE SET last_seen_ms=excluded.last_seen_ms",
            params![peer, now_ms],
        );
    }
    let keep = max_rows.max(1) as i64;
    let _ = conn.execute(
        "DELETE FROM peers WHERE peer IN (
            SELECT peer FROM peers
            ORDER BY last_seen_ms DESC, peer ASC
            LIMIT -1 OFFSET ?1
        )",
        params![keep],
    );
}

fn ensure_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{load_peer_list, save_peer_list};

    fn in_memory_peer_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        conn.execute(
            "CREATE TABLE peers (peer TEXT PRIMARY KEY, last_seen_ms INTEGER NOT NULL)",
            [],
        )
        .expect("create peers table");
        conn
    }

    #[test]
    fn save_peer_list_prunes_to_max_rows() {
        let conn = in_memory_peer_db();
        save_peer_list(
            &conn,
            &[
                "peer-a".to_string(),
                "peer-b".to_string(),
                "peer-c".to_string(),
            ],
            2,
        );
        let loaded = load_peer_list(&conn, 10);
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn save_peer_list_keeps_recent_rows_with_pruning() {
        let conn = in_memory_peer_db();
        save_peer_list(&conn, &["peer-a".to_string(), "peer-b".to_string()], 2);
        std::thread::sleep(std::time::Duration::from_millis(2));
        save_peer_list(&conn, &["peer-z".to_string()], 2);

        let loaded = load_peer_list(&conn, 10);
        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains(&"peer-z".to_string()));
    }
}
