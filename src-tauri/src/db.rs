//! SQLite persistence for prompt history and key/value settings.
//!
//! `Db` wraps a `parking_lot::Mutex<rusqlite::Connection>` behind an `Arc`
//! so it can be cheaply cloned and shared across threads / tasks.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};

use crate::protocol::{AskRequest, AskResponse};

/// Shared database handle — cheap to clone.
#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    /// Open (or create) the database at `path`, running all migrations.
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        migrate(&conn)?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    /// Record the outcome of a prompt after `registry.ask()` returns.
    ///
    /// `created_at` is the instant the ask started (before parking);
    /// `resolved_at` is now (after returning). Failures are NOT propagated —
    /// callers log them and carry on.
    pub fn record_prompt(
        &self,
        req: &AskRequest,
        prompt_id: &str,
        outcome: &AskResponse,
        created_at: DateTime<Utc>,
    ) -> rusqlite::Result<()> {
        let resolved_at = Utc::now().to_rfc3339();
        let created_at_str = created_at.to_rfc3339();

        // Serialize enum fields as their serde wire strings.
        let input_kind = serde_json::to_value(&req.input.kind)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let flow: Option<String> = req
            .flow
            .as_ref()
            .and_then(|f| serde_json::to_value(f).ok())
            .and_then(|v| v.as_str().map(|s| s.to_string()));
        let urgency = serde_json::to_value(&req.urgency)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let (status, answer, via, elapsed_s): (&str, Option<String>, Option<String>, Option<f64>) =
            match outcome {
                AskResponse::Answered { answer, via, elapsed_s } => {
                    let via_str = serde_json::to_value(via)
                        .ok()
                        .and_then(|v| v.as_str().map(|s| s.to_string()));
                    ("answered", Some(answer.clone()), via_str, Some(*elapsed_s))
                }
                AskResponse::TimedOut { .. } => ("timed_out", None, None, None),
            };

        self.0.lock().execute(
            "INSERT INTO prompts \
             (prompt_id, title, body_md, input_kind, flow, urgency, \
              status, answer, via, elapsed_s, created_at, resolved_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                prompt_id,
                req.title,
                req.body_md,
                input_kind,
                flow,
                urgency,
                status,
                answer,
                via,
                elapsed_s,
                created_at_str,
                resolved_at,
            ],
        )?;
        Ok(())
    }

    /// Expose the raw connection lock for integration tests that need to query
    /// the DB directly. Not intended for production code paths.
    #[doc(hidden)]
    pub fn raw_conn(&self) -> Arc<Mutex<Connection>> {
        self.0.clone()
    }

    /// Read a setting value by key.
    pub fn get_setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Insert or replace a setting.
    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.0
            .lock()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map(|_| ())
    }

    /// Export prompt rows as JSON objects, optionally filtered to rows whose
    /// `created_at` is >= `since` (inclusive boundary).  Returns rows ordered
    /// oldest-first.  Field names are stable and match the schema column names.
    pub fn export_rows(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> rusqlite::Result<Vec<serde_json::Value>> {
        let conn = self.0.lock();
        let sql = "SELECT id, prompt_id, title, body_md, input_kind, flow, urgency, \
                          status, answer, via, elapsed_s, created_at, resolved_at \
                   FROM prompts \
                   WHERE (?1 IS NULL OR created_at >= ?1) \
                   ORDER BY created_at ASC, id ASC";
        let since_str: Option<String> = since.map(|dt| dt.to_rfc3339());
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![since_str], |row| {
            Ok(serde_json::json!({
                "id":          row.get::<_, i64>(0)?,
                "prompt_id":   row.get::<_, String>(1)?,
                "title":       row.get::<_, String>(2)?,
                "body_md":     row.get::<_, String>(3)?,
                "input_kind":  row.get::<_, Option<String>>(4)?,
                "flow":        row.get::<_, Option<String>>(5)?,
                "urgency":     row.get::<_, Option<String>>(6)?,
                "status":      row.get::<_, String>(7)?,
                "answer":      row.get::<_, Option<String>>(8)?,
                "via":         row.get::<_, Option<String>>(9)?,
                "elapsed_s":   row.get::<_, Option<f64>>(10)?,
                "created_at":  row.get::<_, String>(11)?,
                "resolved_at": row.get::<_, String>(12)?,
            }))
        })?;
        rows.collect()
    }
}

/// Run schema migrations.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS prompts (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            prompt_id  TEXT NOT NULL,
            title      TEXT NOT NULL,
            body_md    TEXT NOT NULL DEFAULT '',
            input_kind TEXT,
            flow       TEXT,
            urgency    TEXT,
            status     TEXT NOT NULL CHECK(status IN ('answered', 'timed_out')),
            answer     TEXT,
            via        TEXT,
            elapsed_s  REAL,
            created_at TEXT NOT NULL,
            resolved_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_prompts_created_at ON prompts(created_at);
        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AskRequest, AskResponse, Flow, InputKind, InputSpec, Urgency, Via};
    use chrono::Utc;
    use tempfile::tempdir;

    /// Returns (Db, TempDir) — caller must hold TempDir alive for the test duration.
    fn open_temp_db() -> (Db, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();
        (db, dir)
    }

    fn basic_req() -> AskRequest {
        serde_json::from_str(r#"{"title":"Deploy?","body_md":"Ship it?","timeout_s":30}"#).unwrap()
    }

    #[test]
    fn answered_row_roundtrip() {
        let (db, _dir) = open_temp_db();
        let req = basic_req();
        let created_at = Utc::now();
        let outcome = AskResponse::Answered {
            answer: "yes".to_string(),
            via: Via::Text,
            elapsed_s: 3.5,
        };
        db.record_prompt(&req, "p_0", &outcome, created_at).unwrap();

        let conn = db.0.lock();
        let (status, answer, via, elapsed_s): (String, Option<String>, Option<String>, Option<f64>) =
            conn.query_row(
                "SELECT status, answer, via, elapsed_s FROM prompts WHERE prompt_id = 'p_0'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(status, "answered");
        assert_eq!(answer.as_deref(), Some("yes"));
        assert_eq!(via.as_deref(), Some("text"));
        assert!((elapsed_s.unwrap() - 3.5).abs() < 0.001);
    }

    #[test]
    fn timed_out_row_has_null_answer() {
        let (db, _dir) = open_temp_db();
        let req = basic_req();
        let created_at = Utc::now();
        let outcome = AskResponse::TimedOut { answered: false, prompt_id: "p_1".to_string() };
        db.record_prompt(&req, "p_1", &outcome, created_at).unwrap();

        let conn = db.0.lock();
        let (status, answer, via): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, answer, via FROM prompts WHERE prompt_id = 'p_1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(status, "timed_out");
        assert!(answer.is_none(), "answer must be NULL for timed_out, got {answer:?}");
        assert!(via.is_none(), "via must be NULL for timed_out");
    }

    #[test]
    fn settings_get_set_and_overwrite() {
        let (db, _dir) = open_temp_db();

        // get on missing key returns None
        assert_eq!(db.get_setting("foo").unwrap(), None);

        // set then get
        db.set_setting("foo", "bar").unwrap();
        assert_eq!(db.get_setting("foo").unwrap(), Some("bar".to_string()));

        // overwrite
        db.set_setting("foo", "baz").unwrap();
        assert_eq!(db.get_setting("foo").unwrap(), Some("baz".to_string()));
    }

    #[test]
    fn input_kind_and_flow_stored_as_serde_strings() {
        let (db, _dir) = open_temp_db();
        let mut req = basic_req();
        req.input = InputSpec { kind: InputKind::VoiceText };
        req.flow = Some(Flow::Mood);
        req.urgency = Urgency::High;
        let created_at = Utc::now();
        let outcome = AskResponse::Answered {
            answer: "ok".to_string(),
            via: Via::Choice,
            elapsed_s: 1.0,
        };
        db.record_prompt(&req, "p_2", &outcome, created_at).unwrap();

        let conn = db.0.lock();
        let (input_kind, flow, urgency, via): (String, Option<String>, String, Option<String>) =
            conn.query_row(
                "SELECT input_kind, flow, urgency, via FROM prompts WHERE prompt_id = 'p_2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(input_kind, "voice_text");
        assert_eq!(flow.as_deref(), Some("mood"));
        assert_eq!(urgency, "high");
        assert_eq!(via.as_deref(), Some("choice"));
    }

    #[test]
    fn export_rows_empty_db_returns_empty_vec() {
        let (db, _dir) = open_temp_db();
        let rows = db.export_rows(None).unwrap();
        assert!(rows.is_empty(), "expected empty vec, got {rows:?}");
    }

    #[test]
    fn export_rows_roundtrip() {
        let (db, _dir) = open_temp_db();
        let req = basic_req();
        let created_at = Utc::now();
        let outcome = AskResponse::Answered {
            answer: "exported".to_string(),
            via: Via::Text,
            elapsed_s: 2.0,
        };
        db.record_prompt(&req, "p_exp", &outcome, created_at).unwrap();

        let rows = db.export_rows(None).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["prompt_id"], "p_exp");
        assert_eq!(row["title"], "Deploy?");
        assert_eq!(row["status"], "answered");
        assert_eq!(row["answer"], "exported");
        assert_eq!(row["via"], "text");
    }

    #[test]
    fn export_rows_since_filter_inclusive() {
        let (db, _dir) = open_temp_db();
        let req = basic_req();
        let t0 = Utc::now();

        // Insert row BEFORE the since boundary.
        let before = t0 - chrono::Duration::seconds(10);
        db.record_prompt(&req, "before", &AskResponse::TimedOut { answered: false, prompt_id: "before".into() }, before).unwrap();

        // Insert row AT the boundary (should be included — inclusive).
        db.record_prompt(&req, "at", &AskResponse::Answered { answer: "y".into(), via: Via::Text, elapsed_s: 1.0 }, t0).unwrap();

        // Insert row AFTER the boundary.
        let after = t0 + chrono::Duration::seconds(1);
        db.record_prompt(&req, "after", &AskResponse::Answered { answer: "z".into(), via: Via::Text, elapsed_s: 1.0 }, after).unwrap();

        // since = t0: "before" excluded, "at" and "after" included.
        let rows = db.export_rows(Some(t0)).unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r["prompt_id"].as_str().unwrap()).collect();
        assert!(!ids.contains(&"before"), "before should be excluded: {ids:?}");
        assert!(ids.contains(&"at"), "at-boundary row must be included: {ids:?}");
        assert!(ids.contains(&"after"), "after row must be included: {ids:?}");
    }
}
