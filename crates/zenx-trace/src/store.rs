use std::io::{BufRead, BufReader};
use std::path::Path;

use rusqlite::params;
use zenx_core::logging;
use zenx_core::{Result, ZenxError};
use zenx_security::redact_json;
use zenx_storage::{map_err, Db};

use crate::adapters::TraceAdapter;
use crate::model::{
    EventInput, EventKind, ImportSummary, SessionDetail, SessionFilter, SessionSummary, TraceEvent,
};
use zenx_core::util::now_rfc3339;

/// Open a new session. Returns its id.
pub fn create_session(db: &Db, agent: &str, cwd: &Path, label: Option<&str>) -> Result<String> {
    if agent.trim().is_empty() {
        return Err(ZenxError::Invalid("agent name must not be empty".into()));
    }
    let id = zenx_core::util::short_id("sess");
    db.conn()
        .execute(
            "INSERT INTO sessions (id, agent, label, cwd, status, started_at, metadata)
             VALUES (?1, ?2, ?3, ?4, 'running', ?5, '{}')",
            params![
                &id,
                agent,
                label,
                cwd.to_string_lossy().to_string(),
                now_rfc3339()
            ],
        )
        .map_err(map_err)?;
    Ok(id)
}

fn session_exists(db: &Db, session_id: &str) -> Result<bool> {
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .map_err(map_err)?;
    Ok(count > 0)
}

/// Close a session with a terminal status.
pub fn finish_session(db: &Db, session_id: &str, status: &str) -> Result<()> {
    let updated = db
        .conn()
        .execute(
            "UPDATE sessions SET status = ?1, ended_at = ?2 WHERE id = ?3",
            params![status, now_rfc3339(), session_id],
        )
        .map_err(map_err)?;
    if updated == 0 {
        return Err(ZenxError::NotFound(format!("session {session_id}")));
    }
    Ok(())
}

/// Append one event. Payloads are redacted when `redact` is set.
pub fn append_event(db: &Db, session_id: &str, input: &EventInput, redact: bool) -> Result<i64> {
    if !session_exists(db, session_id)? {
        return Err(ZenxError::NotFound(format!("session {session_id}")));
    }
    EventKind::parse(&input.kind)?;
    if input.name.trim().is_empty() {
        return Err(ZenxError::Invalid("event name must not be empty".into()));
    }

    let seq: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM events WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .map_err(map_err)?;

    let mut payload = if redact {
        redact_json(&input.payload)
    } else {
        input.payload.clone()
    };
    if !input.files.is_empty() {
        if let serde_json::Value::Object(map) = &mut payload {
            map.insert(
                "files".to_string(),
                serde_json::Value::Array(
                    input
                        .files
                        .iter()
                        .map(|f| serde_json::Value::String(f.clone()))
                        .collect(),
                ),
            );
        } else {
            payload = serde_json::json!({ "files": input.files, "payload": payload });
        }
    }

    let payload_text = serde_json::to_string(&payload)
        .map_err(|e| ZenxError::Invalid(format!("could not serialise payload: {e}")))?;
    let status = input.status.clone().unwrap_or_else(|| "ok".to_string());

    db.conn()
        .execute(
            "INSERT INTO events (session_id, seq, kind, name, started_at, duration_ms, status, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session_id,
                seq,
                input.kind,
                input.name,
                now_rfc3339(),
                input.duration_ms,
                status,
                payload_text
            ],
        )
        .map_err(map_err)?;
    Ok(seq)
}

/// Sessions newest first, filtered in Rust so the SQL stays trivial.
pub fn list_sessions(db: &Db, filter: &SessionFilter) -> Result<Vec<SessionSummary>> {
    let mut statement = db
        .conn()
        .prepare(
            "SELECT s.id, s.agent, s.label, s.status, s.cwd, s.started_at, s.ended_at,
                    (SELECT COUNT(*) FROM events e WHERE e.session_id = s.id)
             FROM sessions s
             ORDER BY s.started_at DESC, s.id DESC",
        )
        .map_err(map_err)?;

    let rows = statement
        .query_map([], |row| {
            Ok(SessionSummary {
                id: row.get(0)?,
                agent: row.get(1)?,
                label: row.get(2)?,
                status: row.get(3)?,
                cwd: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                event_count: row.get(7)?,
            })
        })
        .map_err(map_err)?;

    let mut sessions = Vec::new();
    for row in rows {
        let session = row.map_err(map_err)?;
        if filter.matches(&session) {
            sessions.push(session);
        }
    }
    if let Some(limit) = filter.limit {
        sessions.truncate(limit);
    }
    Ok(sessions)
}

pub fn get_session(db: &Db, session_id: &str) -> Result<SessionDetail> {
    let summary = db
        .conn()
        .query_row(
            "SELECT s.id, s.agent, s.label, s.status, s.cwd, s.started_at, s.ended_at,
                    (SELECT COUNT(*) FROM events e WHERE e.session_id = s.id)
             FROM sessions s WHERE s.id = ?1",
            params![session_id],
            |row| {
                Ok(SessionSummary {
                    id: row.get(0)?,
                    agent: row.get(1)?,
                    label: row.get(2)?,
                    status: row.get(3)?,
                    cwd: row.get(4)?,
                    started_at: row.get(5)?,
                    ended_at: row.get(6)?,
                    event_count: row.get(7)?,
                })
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                ZenxError::NotFound(format!("session {session_id}"))
            }
            other => map_err(other),
        })?;

    let mut statement = db
        .conn()
        .prepare(
            "SELECT seq, kind, name, started_at, duration_ms, status, payload
             FROM events WHERE session_id = ?1 ORDER BY seq ASC",
        )
        .map_err(map_err)?;
    let rows = statement
        .query_map(params![session_id], |row| {
            let payload: String = row.get(6)?;
            Ok(TraceEvent {
                seq: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                started_at: row.get(3)?,
                duration_ms: row.get(4)?,
                status: row.get(5)?,
                payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
            })
        })
        .map_err(map_err)?;

    let mut events = Vec::new();
    for row in rows {
        events.push(row.map_err(map_err)?);
    }
    Ok(SessionDetail { summary, events })
}

pub fn delete_session(db: &Db, session_id: &str) -> Result<()> {
    let deleted = db
        .conn()
        .execute("DELETE FROM sessions WHERE id = ?1", params![session_id])
        .map_err(map_err)?;
    if deleted == 0 {
        return Err(ZenxError::NotFound(format!("session {session_id}")));
    }
    Ok(())
}

/// Drop finished sessions older than `days`. Returns how many were removed.
pub fn prune_sessions(db: &Db, days: u32) -> Result<usize> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let cutoff_text = cutoff.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let removed = db
        .conn()
        .execute(
            "DELETE FROM sessions WHERE started_at < ?1 AND status != 'running'",
            params![cutoff_text],
        )
        .map_err(map_err)?;
    Ok(removed)
}

/// Import a JSONL transcript through an adapter.
///
/// Malformed lines are counted and logged, never silently dropped: a partially
/// readable transcript still yields a session, and the caller sees the skip count.
pub fn import_jsonl(
    db: &Db,
    path: &Path,
    adapter: &dyn TraceAdapter,
    agent: &str,
    label: Option<&str>,
) -> Result<ImportSummary> {
    let file = std::fs::File::open(path).map_err(|e| ZenxError::io(path, e))?;
    let reader = BufReader::new(file);
    let cwd = path.parent().unwrap_or_else(|| Path::new("."));
    let session_id = create_session(db, agent, cwd, label)?;

    let mut imported = 0usize;
    let mut skipped = 0usize;
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| ZenxError::io(path, e))?;
        if line.trim().is_empty() {
            continue;
        }
        match adapter.parse_line(&line) {
            Ok(events) => {
                for event in events {
                    append_event(db, &session_id, &event, true)?;
                    imported += 1;
                }
            }
            Err(error) => {
                skipped += 1;
                logging::warn(
                    "trace.import",
                    &format!("{}:{}: {error}", path.display(), index + 1),
                );
            }
        }
    }
    finish_session(db, &session_id, "imported")?;
    Ok(ImportSummary {
        session_id,
        adapter: adapter.name().to_string(),
        imported,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::GenericJsonl;

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn session_lifecycle() {
        let db = db();
        let id = create_session(&db, "codex", Path::new("/repo"), Some("demo")).unwrap();
        finish_session(&db, &id, "completed").unwrap();
        let sessions = list_sessions(&db, &SessionFilter::default()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, "completed");
        assert!(sessions[0].ended_at.is_some());
    }

    #[test]
    fn events_are_sequenced() {
        let db = db();
        let id = create_session(&db, "codex", Path::new("/repo"), None).unwrap();
        let first = append_event(
            &db,
            &id,
            &EventInput::new(EventKind::ToolCall, "read_file"),
            true,
        )
        .unwrap();
        let second = append_event(
            &db,
            &id,
            &EventInput::new(EventKind::ToolResult, "read_file"),
            true,
        )
        .unwrap();
        assert_eq!((first, second), (1, 2));
        let detail = get_session(&db, &id).unwrap();
        assert_eq!(detail.events.len(), 2);
        assert_eq!(detail.summary.event_count, 2);
    }

    #[test]
    fn payloads_are_redacted_on_write() {
        let db = db();
        let id = create_session(&db, "codex", Path::new("/repo"), None).unwrap();
        append_event(
            &db,
            &id,
            &EventInput::new(EventKind::ToolCall, "http")
                .with_payload(serde_json::json!({ "authorization": "Bearer abcdefghijklmnop" })),
            true,
        )
        .unwrap();
        let detail = get_session(&db, &id).unwrap();
        let stored = serde_json::to_string(&detail.events[0].payload).unwrap();
        assert!(!stored.contains("abcdefghijklmnop"));
    }

    #[test]
    fn unknown_session_is_not_found() {
        let db = db();
        let error = append_event(
            &db,
            "sess_missing",
            &EventInput::new(EventKind::Message, "hi"),
            true,
        )
        .unwrap_err();
        assert!(matches!(error, ZenxError::NotFound(_)));
        assert_eq!(error.exit_code(), 4);
    }

    #[test]
    fn invalid_kind_is_rejected() {
        let db = db();
        let id = create_session(&db, "codex", Path::new("/repo"), None).unwrap();
        let error = append_event(
            &db,
            &id,
            &EventInput::new(EventKind::Custom, "x").with_status("ok"),
            true,
        );
        assert!(error.is_ok());
        let mut bad = EventInput::new(EventKind::Message, "x");
        bad.kind = "teleport".to_string();
        assert!(append_event(&db, &id, &bad, true).is_err());
    }

    #[test]
    fn filters_and_limits_apply() {
        let db = db();
        create_session(&db, "codex", Path::new("/a"), Some("one")).unwrap();
        create_session(&db, "claude-code", Path::new("/b"), Some("two")).unwrap();
        let only_codex = list_sessions(
            &db,
            &SessionFilter {
                agent: Some("codex".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(only_codex.len(), 1);
        let limited = list_sessions(
            &db,
            &SessionFilter {
                limit: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn jsonl_import_creates_session() {
        let db = db();
        let dir = std::env::temp_dir().join(format!("zenx-trace-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.jsonl");
        std::fs::write(
            &path,
            "{\"kind\":\"tool_call\",\"name\":\"read_file\"}\n\n{\"kind\":\"tool_result\",\"name\":\"read_file\",\"duration_ms\":12}\nnot json\n",
        )
        .unwrap();
        let summary = import_jsonl(&db, &path, &GenericJsonl, "codex", Some("import")).unwrap();
        assert_eq!(summary.imported, 2);
        assert_eq!(summary.skipped, 1);
        let detail = get_session(&db, &summary.session_id).unwrap();
        assert_eq!(detail.summary.status, "imported");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pruning_removes_old_finished_sessions_only() {
        let db = db();
        let old = create_session(&db, "codex", Path::new("/a"), None).unwrap();
        finish_session(&db, &old, "completed").unwrap();
        db.conn()
            .execute(
                "UPDATE sessions SET started_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                params![&old],
            )
            .unwrap();
        let running = create_session(&db, "codex", Path::new("/b"), None).unwrap();
        db.conn()
            .execute(
                "UPDATE sessions SET started_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                params![&running],
            )
            .unwrap();
        assert_eq!(prune_sessions(&db, 30).unwrap(), 1);
        assert_eq!(
            list_sessions(&db, &SessionFilter::default()).unwrap().len(),
            1
        );
    }
}
