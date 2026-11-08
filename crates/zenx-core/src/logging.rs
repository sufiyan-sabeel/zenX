use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::json;

use crate::util::now_rfc3339;

/// Log severity. Ordered so `enabled` is a simple comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Off = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Off => "off",
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Warn as u8);

/// Read `ZENX_LOG` (off|error|warn|info|debug). Defaults to `warn`.
pub fn init_from_env() {
    let raw = std::env::var("ZENX_LOG").unwrap_or_default();
    let level = match raw.to_ascii_lowercase().as_str() {
        "off" => Level::Off,
        "error" => Level::Error,
        "warn" | "" => Level::Warn,
        "info" => Level::Info,
        "debug" | "trace" => Level::Debug,
        _ => Level::Warn,
    };
    set_level(level);
}

pub fn set_level(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

pub fn enabled(level: Level) -> bool {
    LEVEL.load(Ordering::Relaxed) >= level as u8
}

/// One JSON object per line on stderr: greppable, never mixes with stdout.
pub fn log(level: Level, event: &str, message: &str) {
    if !enabled(level) {
        return;
    }
    let record = json!({
        "ts": now_rfc3339(),
        "level": level.as_str(),
        "event": event,
        "message": message,
    });
    eprintln!("{record}");
}

pub fn error(event: &str, message: &str) {
    log(Level::Error, event, message);
}

pub fn warn(event: &str, message: &str) {
    log(Level::Warn, event, message);
}

pub fn info(event: &str, message: &str) {
    log(Level::Info, event, message);
}

pub fn debug(event: &str, message: &str) {
    log(Level::Debug, event, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_ordered() {
        assert!(Level::Debug > Level::Info);
        assert!(Level::Info > Level::Warn);
        assert!(Level::Warn > Level::Error);
        assert!(Level::Error > Level::Off);
    }

    #[test]
    fn debug_is_hidden_at_warn_level() {
        set_level(Level::Warn);
        assert!(!enabled(Level::Debug));
        assert!(!enabled(Level::Info));
        assert!(enabled(Level::Warn));
        assert!(enabled(Level::Error));
    }
}
