use zenx_core::Result;

use crate::model::EventInput;

/// A coding agent's transcript format.
///
/// An adapter exists only when its format has been implemented here and covered
/// by a test with a real sample line. Nothing is advertised as "supported"
/// merely because it is planned.
pub trait TraceAdapter: Send + Sync {
    /// Stable identifier used on the command line.
    fn name(&self) -> &'static str;
    /// One line explaining the accepted format.
    fn description(&self) -> &'static str;
    /// Parse one transcript line into zero or more events.
    fn parse_line(&self, line: &str) -> Result<Vec<EventInput>>;
}

/// zenX's own JSONL format: one JSON object per line.
///
/// ```json
/// {"kind":"tool_call","name":"read_file","status":"ok","duration_ms":12,"payload":{},"files":["src/main.rs"]}
/// ```
pub struct GenericJsonl;

impl TraceAdapter for GenericJsonl {
    fn name(&self) -> &'static str {
        "generic-jsonl"
    }

    fn description(&self) -> &'static str {
        "zenX JSONL: one event object per line with kind, name and optional status, duration_ms, payload, files"
    }

    fn parse_line(&self, line: &str) -> Result<Vec<EventInput>> {
        let event: EventInput = serde_json::from_str(line)
            .map_err(|e| zenx_core::ZenxError::Invalid(format!("not a zenX JSONL event: {e}")))?;
        Ok(vec![event])
    }
}

/// Every adapter this build actually implements.
pub fn adapters() -> Vec<Box<dyn TraceAdapter>> {
    vec![Box::new(GenericJsonl)]
}

pub fn adapter_by_name(name: &str) -> Option<Box<dyn TraceAdapter>> {
    adapters()
        .into_iter()
        .find(|adapter| adapter.name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_adapter_parses_a_full_event() {
        let line = r#"{"kind":"tool_call","name":"read_file","status":"ok","duration_ms":12,"payload":{"path":"a.rs"},"files":["a.rs"]}"#;
        let events = GenericJsonl.parse_line(line).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "read_file");
        assert_eq!(events[0].duration_ms, Some(12));
        assert_eq!(events[0].files, vec!["a.rs".to_string()]);
    }

    #[test]
    fn generic_adapter_rejects_garbage() {
        assert!(GenericJsonl.parse_line("not json").is_err());
        assert!(GenericJsonl.parse_line("{\"kind\":\"tool_call\"}").is_err());
    }

    #[test]
    fn registry_lists_only_implemented_adapters() {
        let names: Vec<&str> = adapters().iter().map(|a| a.name()).collect();
        assert_eq!(names, vec!["generic-jsonl"]);
        assert!(adapter_by_name("generic-jsonl").is_some());
        assert!(adapter_by_name("claude-code").is_none());
    }
}
