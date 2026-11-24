use std::path::Path;
use std::sync::{Arc, Mutex};
use layer0_contracts::{now_ms, HarnessEvent, HarnessEventKind, SnapshotRecord};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("store lock poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub created_at_ms: i64,
    pub forked_from_session_id: Option<String>,
    pub forked_at_seq: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value_json: String,
    pub pinned: bool,
    pub updated_at_ms: i64,
}

/// Thread-safe SQLite WAL persistence store for append-only trajectory events,
/// session fork/replay/resume, atomic Layer-1 graph snapshots, and glass-box memory.
#[derive(Clone)]
pub struct EventStore {
    conn: Arc<Mutex<Connection>>,
}

impl EventStore {
    /// Opens (or creates) a disk-backed SQLite database configured in WAL mode.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_pragmas_and_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_pragmas_and_schema()?;
        Ok(store)
    }

    fn init_pragmas_and_schema(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS sessions (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 created_at_ms INTEGER NOT NULL,
                 forked_from_session_id TEXT,
                 forked_at_seq INTEGER
             );
             CREATE TABLE IF NOT EXISTS events (
                 seq INTEGER PRIMARY KEY AUTOINCREMENT,
                 session_id TEXT NOT NULL,
                 event_type TEXT NOT NULL,
                 payload_json TEXT NOT NULL,
                 created_at_ms INTEGER NOT NULL,
                 FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_events_session_seq ON events(session_id, seq);
             CREATE TABLE IF NOT EXISTS atomic_snapshots (
                 id TEXT PRIMARY KEY,
                 label TEXT NOT NULL,
                 graph_json TEXT NOT NULL,
                 created_at_ms INTEGER NOT NULL,
                 is_good INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS memory_entries (
                 key TEXT PRIMARY KEY,
                 value_json TEXT NOT NULL,
                 pinned INTEGER NOT NULL,
                 updated_at_ms INTEGER NOT NULL
             );",
        )?;
        Ok(())
    }

    pub fn journal_mode(&self) -> Result<String, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let mode: String = conn.query_row("PRAGMA journal_mode;", [], |r| r.get(0))?;
        Ok(mode)
    }

    pub fn create_session(
        &self,
        id: impl Into<String>,
        title: impl Into<String>,
    ) -> Result<SessionInfo, StoreError> {
        let id = id.into();
        let title = title.into();
        let ts = now_ms();
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, title, created_at_ms, forked_from_session_id, forked_at_seq)
             VALUES (?1, ?2, ?3, NULL, NULL)",
            params![id, title, ts],
        )?;
        let info = Self::get_session_locked(&conn, &id)?
            .ok_or_else(|| StoreError::SessionNotFound(id))?;
        Ok(info)
    }

    pub fn ensure_session(&self, id: &str, default_title: &str) -> Result<SessionInfo, StoreError> {
        self.create_session(id, default_title)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at_ms, forked_from_session_id, forked_at_seq
             FROM sessions ORDER BY created_at_ms DESC, id ASC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SessionInfo {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    created_at_ms: r.get(2)?,
                    forked_from_session_id: r.get(3)?,
                    forked_at_seq: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn resume_session(&self, id: &str) -> Result<(SessionInfo, Vec<HarnessEvent>), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let info = Self::get_session_locked(&conn, id)?
            .ok_or_else(|| StoreError::SessionNotFound(id.to_string()))?;
        let events = Self::load_events_locked(&conn, id, None)?;
        Ok((info, events))
    }

    pub fn append_event(
        &self,
        session_id: &str,
        kind: HarnessEventKind,
        payload: Value,
    ) -> Result<HarnessEvent, StoreError> {
        let ts = now_ms();
        let event_type = serde_json::to_value(kind)?
            .as_str()
            .unwrap_or("error")
            .to_string();
        let payload_json = serde_json::to_string(&payload)?;

        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, title, created_at_ms) VALUES (?1, ?2, ?3)",
            params![session_id, "Active Session", ts],
        )?;
        conn.execute(
            "INSERT INTO events (session_id, event_type, payload_json, created_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![session_id, event_type, payload_json, ts],
        )?;
        let seq = conn.last_insert_rowid();
        Ok(HarnessEvent {
            seq,
            session_id: session_id.to_string(),
            timestamp_ms: ts,
            kind,
            payload,
        })
    }

    /// Pure deterministic projection over the append-only log.
    pub fn project_session_events(&self, session_id: &str) -> Result<Vec<HarnessEvent>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        Self::load_events_locked(&conn, session_id, None)
    }

    /// Atomically forks a session up to `up_to_seq` (inclusive) into a new independent session.
    pub fn fork_session(
        &self,
        source_session_id: &str,
        up_to_seq: i64,
        new_session_id: &str,
        new_title: Option<&str>,
    ) -> Result<SessionInfo, StoreError> {
        let mut conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let source = Self::get_session_locked(&conn, source_session_id)?
            .ok_or_else(|| StoreError::SessionNotFound(source_session_id.to_string()))?;
        let prefix = Self::load_events_locked(&conn, source_session_id, Some(up_to_seq))?;
        let ts = now_ms();
        let title = new_title
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("{} (fork@{})", source.title, up_to_seq));

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO sessions (id, title, created_at_ms, forked_from_session_id, forked_at_seq)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_session_id, title, ts, source_session_id, up_to_seq],
        )?;
        for ev in prefix {
            let ev_type = serde_json::to_value(ev.kind)?
                .as_str()
                .unwrap_or("error")
                .to_string();
            let payload_json = serde_json::to_string(&ev.payload)?;
            tx.execute(
                "INSERT INTO events (session_id, event_type, payload_json, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![new_session_id, ev_type, payload_json, ev.timestamp_ms],
            )?;
        }
        tx.commit()?;

        Ok(SessionInfo {
            id: new_session_id.to_string(),
            title,
            created_at_ms: ts,
            forked_from_session_id: Some(source_session_id.to_string()),
            forked_at_seq: Some(up_to_seq),
        })
    }

    pub fn commit_snapshot(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        graph_json: impl Into<String>,
        is_good: bool,
    ) -> Result<SnapshotRecord, StoreError> {
        let record = SnapshotRecord {
            id: id.into(),
            label: label.into(),
            graph_json: graph_json.into(),
            created_at_ms: now_ms(),
            is_good,
        };
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        conn.execute(
            "INSERT OR REPLACE INTO atomic_snapshots (id, label, graph_json, created_at_ms, is_good)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.id,
                record.label,
                record.graph_json,
                record.created_at_ms,
                if record.is_good { 1 } else { 0 }
            ],
        )?;
        Ok(record)
    }

    pub fn latest_good_snapshot(&self) -> Result<Option<SnapshotRecord>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let row = conn
            .query_row(
                "SELECT id, label, graph_json, created_at_ms, is_good
                 FROM atomic_snapshots
                 WHERE is_good = 1
                 ORDER BY rowid DESC
                 LIMIT 1",
                [],
                |r| {
                    Ok(SnapshotRecord {
                        id: r.get(0)?,
                        label: r.get(1)?,
                        graph_json: r.get(2)?,
                        created_at_ms: r.get(3)?,
                        is_good: r.get::<_, i64>(4)? != 0,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    pub fn upsert_memory(&self, key: &str, value: Value, pinned: bool) -> Result<(), StoreError> {
        let json = serde_json::to_string(&value)?;
        let ts = now_ms();
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        conn.execute(
            "INSERT OR REPLACE INTO memory_entries (key, value_json, pinned, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![key, json, if pinned { 1 } else { 0 }, ts],
        )?;
        Ok(())
    }

    pub fn list_memory(&self) -> Result<Vec<MemoryEntry>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let mut stmt = conn.prepare(
            "SELECT key, value_json, pinned, updated_at_ms FROM memory_entries ORDER BY pinned DESC, key ASC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(MemoryEntry {
                    key: r.get(0)?,
                    value_json: r.get(1)?,
                    pinned: r.get::<_, i64>(2)? != 0,
                    updated_at_ms: r.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_memory(&self, key: &str) -> Result<bool, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let changed = conn.execute("DELETE FROM memory_entries WHERE key = ?1", params![key])?;
        Ok(changed > 0)
    }

    pub fn stats(&self) -> Result<(usize, u64), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        let sessions: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
        let events: i64 = conn.query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
        Ok((sessions.max(0) as usize, events.max(0) as u64))
    }

    fn get_session_locked(conn: &Connection, id: &str) -> Result<Option<SessionInfo>, StoreError> {
        let row = conn
            .query_row(
                "SELECT id, title, created_at_ms, forked_from_session_id, forked_at_seq
                 FROM sessions WHERE id = ?1",
                params![id],
                |r| {
                    Ok(SessionInfo {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        created_at_ms: r.get(2)?,
                        forked_from_session_id: r.get(3)?,
                        forked_at_seq: r.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn load_events_locked(
        conn: &Connection,
        session_id: &str,
        up_to_seq: Option<i64>,
    ) -> Result<Vec<HarnessEvent>, StoreError> {
        let mut events = Vec::new();
        let sql = if up_to_seq.is_some() {
            "SELECT seq, session_id, event_type, payload_json, created_at_ms
             FROM events WHERE session_id = ?1 AND seq <= ?2 ORDER BY seq ASC"
        } else {
            "SELECT seq, session_id, event_type, payload_json, created_at_ms
             FROM events WHERE session_id = ?1 ORDER BY seq ASC"
        };
        let mut stmt = conn.prepare(sql)?;
        let map_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, String, String, String, i64)> {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        };
        let rows: Vec<_> = if let Some(limit) = up_to_seq {
            stmt.query_map(params![session_id, limit], map_row)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![session_id], map_row)?
                .collect::<Result<Vec<_>, _>>()?
        };

        for (seq, sid, ev_type, payload_json, ts) in rows {
            let kind: HarnessEventKind =
                serde_json::from_value(Value::String(ev_type)).unwrap_or(HarnessEventKind::Error);
            let payload: Value = serde_json::from_str(&payload_json)?;
            events.push(HarnessEvent {
                seq,
                session_id: sid,
                timestamp_ms: ts,
                kind,
                payload,
            });
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn sqlite_wal_append_replay_fork_and_persist_across_reopen() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("carnival.db");

        let seq2;
        {
            let store = EventStore::open(&db_path).unwrap();
            assert_eq!(store.journal_mode().unwrap().to_lowercase(), "wal");

            store.create_session("s1", "Main Session").unwrap();
            let e1 = store
                .append_event("s1", HarnessEventKind::TextDelta, json!({"text": "one"}))
                .unwrap();
            let e2 = store
                .append_event("s1", HarnessEventKind::TextDelta, json!({"text": "two"}))
                .unwrap();
            let _e3 = store
                .append_event("s1", HarnessEventKind::TurnComplete, json!({"ok": true}))
                .unwrap();
            seq2 = e2.seq;
            assert!(e1.seq < e2.seq);

            let forked = store
                .fork_session("s1", seq2, "s1-fork", Some("Forked Branch"))
                .unwrap();
            assert_eq!(forked.forked_from_session_id.as_deref(), Some("s1"));
            assert_eq!(forked.forked_at_seq, Some(seq2));

            let fork_events = store.project_session_events("s1-fork").unwrap();
            assert_eq!(fork_events.len(), 2);
            assert_eq!(fork_events[0].payload, json!({"text": "one"}));
            assert_eq!(fork_events[1].payload, json!({"text": "two"}));

            store
                .commit_snapshot("snap-1", "baseline", r#"{"seams":["router"]}"#, true)
                .unwrap();
        }

        // Reopen database from disk and verify persistence + resume.
        let reopened = EventStore::open(&db_path).unwrap();
        let (info, resumed_events) = reopened.resume_session("s1").unwrap();
        assert_eq!(info.title, "Main Session");
        assert_eq!(resumed_events.len(), 3);
        let snap = reopened.latest_good_snapshot().unwrap().unwrap();
        assert_eq!(snap.id, "snap-1");
    }
}
