use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use tracing::{error, info};

/// SQLite-backed persistence for active game sessions.
///
/// Uses `std::sync::Mutex` to make `Connection` `Send`, since
/// `rusqlite::Connection` is `!Send` (internal `RefCell`).
/// All operations acquire the lock briefly for a single SQL statement.
pub struct GameDb {
    conn: Mutex<Connection>,
}

impl GameDb {
    /// Open (or create) the game database at the given path.
    /// Enables WAL mode and creates the schema if needed.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS game_sessions (
                game_code TEXT PRIMARY KEY,
                session_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )?;
        info!("Game database opened at {}", path.display());
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Persist a game session (upsert).
    pub fn save_session(&self, game_code: &str, json: &str) -> rusqlite::Result<()> {
        let now = now_epoch();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO game_sessions (game_code, session_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(game_code) DO UPDATE SET session_json = ?2, updated_at = ?3",
            params![game_code, json, now],
        )?;
        Ok(())
    }

    /// Load all persisted sessions. Returns (game_code, json) pairs.
    pub fn load_all(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT game_code, session_json FROM game_sessions")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            match row {
                Ok(pair) => results.push(pair),
                Err(e) => error!("Failed to read persisted session row: {}", e),
            }
        }
        Ok(results)
    }

    /// Delete a session by game code.
    pub fn delete_session(&self, game_code: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM game_sessions WHERE game_code = ?1",
            params![game_code],
        )?;
        Ok(())
    }

    /// Delete sessions older than `max_age_secs` seconds.
    pub fn delete_stale(&self, max_age_secs: u64) -> rusqlite::Result<usize> {
        let cutoff = now_epoch().saturating_sub(max_age_secs);
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM game_sessions WHERE updated_at < ?1",
            params![cutoff],
        )?;
        Ok(deleted)
    }
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn test_db() -> GameDb {
        let file = NamedTempFile::new().unwrap();
        GameDb::open(file.path()).unwrap()
    }

    #[test]
    fn save_and_load_roundtrip() {
        let db = test_db();
        db.save_session("ABC123", r#"{"game_code":"ABC123"}"#)
            .unwrap();
        let all = db.load_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, "ABC123");
        assert!(all[0].1.contains("ABC123"));
    }

    #[test]
    fn upsert_overwrites() {
        let db = test_db();
        db.save_session("ABC123", "v1").unwrap();
        db.save_session("ABC123", "v2").unwrap();
        let all = db.load_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].1, "v2");
    }

    #[test]
    fn delete_session_removes_row() {
        let db = test_db();
        db.save_session("ABC123", "data").unwrap();
        db.delete_session("ABC123").unwrap();
        let all = db.load_all().unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn delete_stale_removes_old_entries() {
        let db = test_db();
        // Insert with a very old timestamp
        db.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO game_sessions (game_code, session_json, updated_at) VALUES (?1, ?2, ?3)",
                params!["OLD001", "old", 1000u64],
            )
            .unwrap();
        db.save_session("NEW001", "new").unwrap();

        let deleted = db.delete_stale(86400).unwrap();
        assert_eq!(deleted, 1);

        let all = db.load_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, "NEW001");
    }
}
