use std::fmt::Display;

use serde::Serialize;

use crate::error::{Result, ZenxError};

/// How a command renders its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "human" | "text" => Some(OutputFormat::Human),
            "json" => Some(OutputFormat::Json),
            _ => None,
        }
    }
}

/// Prints results in the selected format.
///
/// In JSON mode nothing except the JSON document may reach stdout, otherwise
/// automation breaks. Human chatter therefore goes to stderr.
#[derive(Debug, Clone, Copy)]
pub struct Reporter {
    format: OutputFormat,
    color: bool,
}

impl Reporter {
    pub fn new(format: OutputFormat, color: bool) -> Self {
        Reporter { format, color }
    }

    pub fn format(&self) -> OutputFormat {
        self.format
    }

    pub fn is_json(&self) -> bool {
        self.format == OutputFormat::Json
    }

    pub fn color(&self) -> bool {
        self.color
    }

    /// Emit the structured result of a command.
    pub fn json<T: Serialize>(&self, value: &T) -> Result<()> {
        let text = serde_json::to_string_pretty(value)
            .map_err(|e| ZenxError::Invalid(format!("could not serialise output: {e}")))?;
        println!("{text}");
        Ok(())
    }

    /// Human-readable line on stdout. Suppressed in JSON mode.
    pub fn line(&self, text: impl Display) {
        if !self.is_json() {
            println!("{text}");
        }
    }

    /// Human-readable line on stdout regardless of format.
    pub fn raw(&self, text: impl Display) {
        println!("{text}");
    }

    /// Progress and diagnostics on stderr; safe in every mode.
    pub fn note(&self, text: impl Display) {
        eprintln!("{text}");
    }

    pub fn heading(&self, text: impl Display) {
        if !self.is_json() {
            println!("\n{text}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formats() {
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::parse("human"), Some(OutputFormat::Human));
        assert_eq!(OutputFormat::parse("text"), Some(OutputFormat::Human));
        assert_eq!(OutputFormat::parse("xml"), None);
    }

    #[test]
    fn json_mode_is_detected() {
        let reporter = Reporter::new(OutputFormat::Json, false);
        assert!(reporter.is_json());
    }
}
