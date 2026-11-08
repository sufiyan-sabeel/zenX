use std::path::Path;

use zenx_core::{Result, ZenxError};
use zenx_storage::Db;

use crate::model::{ExportFormat, SessionDetail};
use crate::store::get_session;

/// Render a session as Markdown: header, summary, then the event timeline.
pub fn render_markdown(detail: &SessionDetail) -> String {
    let summary = &detail.summary;
    let mut out = String::new();
    out.push_str(&format!("# Agent session {}\n\n", summary.id));
    out.push_str(&format!("- Agent: {}\n", summary.agent));
    if let Some(label) = &summary.label {
        out.push_str(&format!("- Label: {label}\n"));
    }
    out.push_str(&format!("- Status: {}\n", summary.status));
    out.push_str(&format!("- Working directory: {}\n", summary.cwd));
    out.push_str(&format!("- Started: {}\n", summary.started_at));
    out.push_str(&format!(
        "- Ended: {}\n",
        summary.ended_at.as_deref().unwrap_or("(running)")
    ));
    out.push_str(&format!("- Events: {}\n\n", detail.events.len()));

    if detail.events.is_empty() {
        out.push_str("_No events were recorded for this session._\n");
        return out;
    }

    out.push_str("## Timeline\n\n");
    out.push_str("| # | Time | Kind | Name | Status | Duration | Detail |\n");
    out.push_str("|---|------|------|------|--------|----------|--------|\n");
    for event in &detail.events {
        let duration = event
            .duration_ms
            .map(|ms| zenx_core::util::format_duration_ms(ms.max(0) as u64))
            .unwrap_or_else(|| "-".to_string());
        let detail_text = if event.payload.is_null() {
            String::new()
        } else {
            let text = serde_json::to_string(&event.payload).unwrap_or_default();
            zenx_core::util::truncate(&text.replace('|', "\\|"), 80)
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            event.seq,
            event.started_at,
            event.kind,
            event.name.replace('|', "\\|"),
            event.status,
            duration,
            detail_text
        ));
    }
    out
}

/// Write a session to `out` in the requested format.
pub fn export_session(db: &Db, session_id: &str, format: ExportFormat, out: &Path) -> Result<usize> {
    let detail = get_session(db, session_id)?;
    let text = match format {
        ExportFormat::Json => serde_json::to_string_pretty(&detail)
            .map_err(|e| ZenxError::Invalid(format!("could not serialise session: {e}")))?,
        ExportFormat::Markdown => render_markdown(&detail),
    };
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| ZenxError::io(parent, e))?;
        }
    }
    std::fs::write(out, &text).map_err(|e| ZenxError::io(out, e))?;
    Ok(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventInput, EventKind, SessionSummary, TraceEvent};
    use crate::store::{append_event, create_session};

    fn sample_detail() -> SessionDetail {
        SessionDetail {
            summary: SessionSummary {
                id: "sess_1".into(),
                agent: "codex".into(),
                label: Some("demo".into()),
                status: "completed".into(),
                cwd: "/repo".into(),
                started_at: "2026-01-01T00:00:00Z".into(),
                ended_at: Some("2026-01-01T00:01:00Z".into()),
                event_count: 1,
            },
            events: vec![TraceEvent {
                seq: 1,
                kind: "tool_call".into(),
                name: "read_file".into(),
                started_at: "2026-01-01T00:00:10Z".into(),
                duration_ms: Some(12),
                status: "ok".into(),
                payload: serde_json::json!({ "path": "a.rs" }),
            }],
        }
    }

    #[test]
    fn markdown_contains_the_timeline() {
        let text = render_markdown(&sample_detail());
        assert!(text.contains("# Agent session sess_1"));
        assert!(text.contains("| 1 |"));
        assert!(text.contains("read_file"));
        assert!(text.contains("12ms"));
    }

    #[test]
    fn markdown_escapes_pipes() {
        let mut detail = sample_detail();
        detail.events[0].name = "grep|pipe".into();
        let text = render_markdown(&detail);
        assert!(text.contains("grep\\|pipe"));
    }

    #[test]
    fn exports_write_both_formats() {
        let db = Db::open_in_memory().unwrap();
        let id = create_session(&db, "codex", Path::new("/repo"), None).unwrap();
        append_event(
            &db,
            &id,
            &EventInput::new(EventKind::ToolCall, "read_file"),
            true,
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("zenx-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let md = dir.join("session.md");
        let json = dir.join("session.json");
        export_session(&db, &id, ExportFormat::Markdown, &md).unwrap();
        export_session(&db, &id, ExportFormat::Json, &json).unwrap();
        let md_text = std::fs::read_to_string(&md).unwrap();
        assert!(md_text.contains("read_file"));
        let json_text = std::fs::read_to_string(&json).unwrap();
        assert!(json_text.contains("\"agent\": \"codex\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exporting_a_missing_session_fails() {
        let db = Db::open_in_memory().unwrap();
        let out = std::env::temp_dir().join("zenx-missing.md");
        let error = export_session(&db, "sess_missing", ExportFormat::Markdown, &out).unwrap_err();
        assert!(matches!(error, ZenxError::NotFound(_)));
    }
}
