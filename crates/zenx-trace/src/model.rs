use serde::{Deserialize, Serialize};

use zenx_core::{Result, ZenxError};

/// The kinds of event a coding agent emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Message,
    ToolCall,
    ToolResult,
    FileChange,
    Decision,
    Error,
    Custom,
}

impl EventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventKind::Message => "message",
            EventKind::ToolCall => "tool_call",
            EventKind::ToolResult => "tool_result",
            EventKind::FileChange => "file_change",
            EventKind::Decision => "decision",
            EventKind::Error => "error",
            EventKind::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "message" => Ok(EventKind::Message),
            "tool_call" => Ok(EventKind::ToolCall),
            "tool_result" => Ok(EventKind::ToolResult),
            "file_change" => Ok(EventKind::FileChange),
            "decision" => Ok(EventKind::Decision),
            "error" => Ok(EventKind::Error),
            "custom" => Ok(EventKind::Custom),
            other => Err(ZenxError::Invalid(format!(
                "unknown event kind {other:?}; expected one of message, tool_call, tool_result, file_change, decision, error, custom"
            ))),
        }
    }

    pub fn all() -> &'static [&'static str] {
        &[
            "message",
            "tool_call",
            "tool_result",
            "file_change",
            "decision",
            "error",
            "custom",
        ]
    }
}

/// One event as supplied by an agent adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInput {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub payload: serde_json::Value,
    /// Files touched by this event, when the adapter can report them.
    #[serde(default)]
    pub files: Vec<String>,
}

impl EventInput {
    pub fn new(kind: EventKind, name: impl Into<String>) -> Self {
        EventInput {
            kind: kind.as_str().to_string(),
            name: name.into(),
            status: None,
            duration_ms: None,
            payload: serde_json::Value::Null,
            files: Vec::new(),
        }
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub fn with_duration_ms(mut self, duration_ms: i64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }
}

/// A recorded event, as read back from storage.
#[derive(Debug, Clone, Serialize)]
pub struct TraceEvent {
    pub seq: i64,
    pub kind: String,
    pub name: String,
    pub started_at: String,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub agent: String,
    pub label: Option<String>,
    pub status: String,
    pub cwd: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub event_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    #[serde(flatten)]
    pub summary: SessionSummary,
    pub events: Vec<TraceEvent>,
}

/// Filters applied in `zenx trace sessions`.
#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub agent: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
}

impl SessionFilter {
    pub fn matches(&self, session: &SessionSummary) -> bool {
        if let Some(agent) = &self.agent {
            if !session.agent.eq_ignore_ascii_case(agent) {
                return false;
            }
        }
        if let Some(status) = &self.status {
            if !session.status.eq_ignore_ascii_case(status) {
                return false;
            }
        }
        if let Some(needle) = &self.search {
            let needle = needle.to_ascii_lowercase();
            let haystack = format!(
                "{} {} {}",
                session.id,
                session.agent,
                session.label.clone().unwrap_or_default()
            )
            .to_ascii_lowercase();
            if !haystack.contains(&needle) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Markdown,
}

impl ExportFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "json" => Some(ExportFormat::Json),
            "markdown" | "md" => Some(ExportFormat::Markdown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    pub session_id: String,
    pub adapter: String,
    pub imported: usize,
    pub skipped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip() {
        for kind in EventKind::all() {
            assert_eq!(EventKind::parse(kind).unwrap().as_str(), *kind);
        }
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert!(EventKind::parse("teleport").is_err());
    }

    #[test]
    fn filters_match_on_agent_and_label() {
        let session = SessionSummary {
            id: "sess_1".into(),
            agent: "codex".into(),
            label: Some("Fix parser".into()),
            status: "completed".into(),
            cwd: "/repo".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            ended_at: None,
            event_count: 0,
        };
        assert!(SessionFilter {
            agent: Some("CODEX".into()),
            ..Default::default()
        }
        .matches(&session));
        assert!(SessionFilter {
            search: Some("parser".into()),
            ..Default::default()
        }
        .matches(&session));
        assert!(!SessionFilter {
            status: Some("failed".into()),
            ..Default::default()
        }
        .matches(&session));
    }

    #[test]
    fn export_formats_parse() {
        assert_eq!(ExportFormat::parse("md"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::parse("json"), Some(ExportFormat::Json));
        assert_eq!(ExportFormat::parse("xml"), None);
    }
}
