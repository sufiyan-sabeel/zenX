use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::json;
use zenx_core::config::Config;
use zenx_core::{Result, ZenxError};

use crate::context::Context;
use crate::OutputArg;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the whole configuration
    Show,
    /// Print the path of config.toml
    Path,
    /// Read one value, e.g. output.format
    Get {
        /// Dotted key, e.g. trace.redact
        key: String,
    },
    /// Write one value
    Set {
        /// Dotted key, e.g. output.format
        key: String,
        value: String,
    },
}

const KEYS: &[&str] = &[
    "output.format",
    "output.color",
    "trace.redact",
    "trace.capture_file_changes",
    "trace.retention_days",
    "skills.allow_network",
    "skills.store",
    "docs.max_file_bytes",
];

fn read_value(config: &Config, key: &str) -> Result<serde_json::Value> {
    let value = match key {
        "output.format" => json!(config.output.format),
        "output.color" => json!(config.output.color),
        "trace.redact" => json!(config.trace.redact),
        "trace.capture_file_changes" => json!(config.trace.capture_file_changes),
        "trace.retention_days" => json!(config.trace.retention_days),
        "skills.allow_network" => json!(config.skills.allow_network),
        "skills.store" => match &config.skills.store {
            Some(path) => json!(path.display().to_string()),
            None => json!(null),
        },
        "docs.max_file_bytes" => json!(config.docs.max_file_bytes),
        other => {
            return Err(ZenxError::Invalid(format!(
                "unknown configuration key {other:?}; known keys: {}",
                KEYS.join(", ")
            )))
        }
    };
    Ok(value)
}

fn write_value(config: &mut Config, key: &str, raw: &str) -> Result<()> {
    match key {
        "output.format" => {
            if zenx_core::output::OutputFormat::parse(raw).is_none() {
                return Err(ZenxError::Invalid(
                    "output.format must be human or json".into(),
                ));
            }
            config.output.format = raw.to_string();
        }
        "output.color" => config.output.color = parse_bool(raw)?,
        "trace.redact" => config.trace.redact = parse_bool(raw)?,
        "trace.capture_file_changes" => config.trace.capture_file_changes = parse_bool(raw)?,
        "trace.retention_days" => {
            config.trace.retention_days = if raw.eq_ignore_ascii_case("none") || raw.is_empty() {
                None
            } else {
                Some(raw.parse::<u32>().map_err(|_| {
                    ZenxError::Invalid(format!("{raw:?} is not a number of days"))
                })?)
            }
        }
        "skills.allow_network" => config.skills.allow_network = parse_bool(raw)?,
        "skills.store" => {
            config.skills.store = if raw.eq_ignore_ascii_case("none") || raw.is_empty() {
                None
            } else {
                Some(PathBuf::from(raw))
            }
        }
        "docs.max_file_bytes" => {
            config.docs.max_file_bytes = raw
                .parse::<u64>()
                .map_err(|_| ZenxError::Invalid(format!("{raw:?} is not a byte count")))?
        }
        other => {
            return Err(ZenxError::Invalid(format!(
                "unknown configuration key {other:?}; known keys: {}",
                KEYS.join(", ")
            )))
        }
    }
    config.validate()
}

fn parse_bool(raw: &str) -> Result<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(ZenxError::Invalid(format!(
            "{raw:?} is not a boolean; use true or false"
        ))),
    }
}

pub fn run(args: &ConfigArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let context = Context::load(output, no_color)?;
    let reporter = context.reporter;
    let config_path = context.paths.config_file();

    match args.command.as_ref().unwrap_or(&ConfigCommand::Show) {
        ConfigCommand::Show => {
            let text = toml::to_string_pretty(&context.config)
                .map_err(|e| ZenxError::Config(e.to_string()))?;
            if reporter.is_json() {
                reporter.json(&context.config)?;
            } else {
                reporter.raw(format!("# {}", config_path.display()));
                reporter.raw(text);
            }
        }
        ConfigCommand::Path => {
            if reporter.is_json() {
                reporter.json(&json!({ "path": config_path.display().to_string() }))?;
            } else {
                reporter.line(config_path.display().to_string());
            }
        }
        ConfigCommand::Get { key } => {
            let value = read_value(&context.config, key)?;
            if reporter.is_json() {
                reporter.json(&json!({ "key": key, "value": value }))?;
            } else {
                match value {
                    serde_json::Value::String(text) => reporter.line(text),
                    serde_json::Value::Null => reporter.line("none"),
                    other => reporter.line(other),
                }
            }
        }
        ConfigCommand::Set { key, value } => {
            let mut config = context.config.clone();
            write_value(&mut config, key, value)?;
            config.save(&config_path)?;
            if reporter.is_json() {
                reporter.json(&json!({ "ok": true, "key": key, "value": read_value(&config, key)? }))?;
            } else {
                reporter.line(format!("{key} = {value}"));
            }
        }
    }
    Ok(0)
}
