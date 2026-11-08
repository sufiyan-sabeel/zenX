use std::io::{self, BufRead};
use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::json;
use zenx_core::util::format_duration_ms;
use zenx_core::{Result, ZenxError};
use zenx_trace::{
    adapter_by_name, adapters, append_event, create_session, delete_session, export_session,
    finish_session, get_session, import_jsonl, list_sessions, prune_sessions, ExportFormat,
    SessionFilter,
};

use crate::context::Context;
use crate::OutputArg;

#[derive(Debug, Args)]
pub struct TraceArgs {
    #[command(subcommand)]
    pub command: Option<TraceCommand>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportArg {
    Json,
    Markdown,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TraceCommand {
    /// List recorded sessions (default when no subcommand is given)
    Sessions {
        /// Only sessions from this agent
        #[arg(long)]
        agent: Option<String>,
        /// Only sessions with this status
        #[arg(long)]
        status: Option<String>,
        /// Match the session id or label
        #[arg(long)]
        search: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Show one session with its event timeline
    Inspect {
        /// Session id
        session: String,
    },
    /// Export a session to a file
    Export {
        session: String,
        #[arg(long, value_enum, default_value = "markdown")]
        format: ExportArg,
        /// Output file; defaults to <session>.<ext> in the current directory
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
    },
    /// Append events from JSONL (file or stdin) to a session
    Record {
        /// Existing session id; a new session is created when omitted
        #[arg(long)]
        session: Option<String>,
        /// Agent name used when creating a session
        #[arg(long, default_value = "manual")]
        agent: String,
        #[arg(long)]
        label: Option<String>,
        /// Read JSONL from this file instead of stdin
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// Adapter name (see `zenx trace adapters`)
        #[arg(long, default_value = "generic-jsonl")]
        adapter: String,
    },
    /// Start an empty session and print its id
    Start {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        label: Option<String>,
        /// Working directory recorded with the session
        #[arg(long, value_name = "DIR")]
        cwd: Option<PathBuf>,
    },
    /// Close a session
    End {
        session: String,
        #[arg(long, default_value = "completed")]
        status: String,
    },
    /// Import an agent transcript file
    Import {
        file: PathBuf,
        #[arg(long, default_value = "generic-jsonl")]
        adapter: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        label: Option<String>,
    },
    /// Delete finished sessions older than N days
    Prune {
        #[arg(long)]
        days: u32,
    },
    /// List the transcript adapters this build implements
    Adapters,
    /// Delete one session
    Delete { session: String },
}

pub fn run(args: &TraceArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let context = Context::load(output, no_color)?;
    let reporter = context.reporter;
    let command = args.command.clone().unwrap_or(TraceCommand::Sessions {
        agent: None,
        status: None,
        search: None,
        limit: 20,
    });

    match command {
        TraceCommand::Sessions {
            agent,
            status,
            search,
            limit,
        } => {
            let db = context.db()?;
            let filter = SessionFilter {
                agent,
                status,
                search,
                limit: Some(limit),
            };
            let sessions = list_sessions(&db, &filter)?;
            if reporter.is_json() {
                reporter.json(&json!({ "sessions": sessions, "count": sessions.len() }))?;
                return Ok(0);
            }
            if sessions.is_empty() {
                reporter.line("no sessions recorded yet");
                reporter.note("record one with: zenx trace record --agent <name> < events.jsonl");
                return Ok(0);
            }
            reporter.line(format!(
                "{:<18} {:<14} {:<12} {:>6}  {}",
                "SESSION", "AGENT", "STATUS", "EVENTS", "STARTED"
            ));
            for session in &sessions {
                reporter.line(format!(
                    "{:<18} {:<14} {:<12} {:>6}  {}{}",
                    session.id,
                    session.agent,
                    session.status,
                    session.event_count,
                    session.started_at,
                    session
                        .label
                        .as_ref()
                        .map(|label| format!("  ({label})"))
                        .unwrap_or_default()
                ));
            }
            Ok(0)
        }

        TraceCommand::Inspect { session } => {
            let db = context.db()?;
            let detail = get_session(&db, &session)?;
            if reporter.is_json() {
                reporter.json(&detail)?;
                return Ok(0);
            }
            reporter.line(format!("session  {}", detail.summary.id));
            reporter.line(format!("agent    {}", detail.summary.agent));
            if let Some(label) = &detail.summary.label {
                reporter.line(format!("label    {label}"));
            }
            reporter.line(format!("status   {}", detail.summary.status));
            reporter.line(format!("cwd      {}", detail.summary.cwd));
            reporter.line(format!("started  {}", detail.summary.started_at));
            reporter.line(format!(
                "ended    {}",
                detail.summary.ended_at.as_deref().unwrap_or("(running)")
            ));
            reporter.heading(format!("timeline ({} events)", detail.events.len()));
            for event in &detail.events {
                let duration = event
                    .duration_ms
                    .map(|ms| format_duration_ms(ms.max(0) as u64))
                    .unwrap_or_else(|| "-".to_string());
                let detail_text = if event.payload.is_null() {
                    String::new()
                } else {
                    let text = serde_json::to_string(&event.payload).unwrap_or_default();
                    format!("  {}", zenx_core::util::truncate(&text, 70))
                };
                reporter.line(format!(
                    "  {:>3}  {:<12} {:<24} {:<8} {:>8}{}",
                    event.seq, event.kind, event.name, event.status, duration, detail_text
                ));
            }
            Ok(0)
        }

        TraceCommand::Export {
            session,
            format,
            out,
        } => {
            let db = context.db()?;
            let export_format = match format {
                ExportArg::Json => ExportFormat::Json,
                ExportArg::Markdown => ExportFormat::Markdown,
            };
            let extension = match export_format {
                ExportFormat::Json => "json",
                ExportFormat::Markdown => "md",
            };
            let path = out.unwrap_or_else(|| PathBuf::from(format!("{session}.{extension}")));
            let bytes = export_session(&db, &session, export_format, &path)?;
            if reporter.is_json() {
                reporter.json(&json!({
                    "ok": true,
                    "path": path.display().to_string(),
                    "bytes": bytes,
                }))?;
            } else {
                reporter.line(format!("wrote {} ({bytes} bytes)", path.display()));
            }
            Ok(0)
        }

        TraceCommand::Record {
            session,
            agent,
            label,
            file,
            adapter,
        } => {
            let db = context.db()?;
            let adapter = adapter_by_name(&adapter).ok_or_else(|| {
                ZenxError::Invalid(format!(
                    "unknown adapter {adapter:?}; available: {}",
                    adapters()
                        .iter()
                        .map(|a| a.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
            let (session_id, created) = match session {
                Some(id) => {
                    get_session(&db, &id)?;
                    (id, false)
                }
                None => {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    let id = create_session(&db, &agent, &cwd, label.as_deref())?;
                    (id, true)
                }
            };

            let mut imported = 0usize;
            let mut skipped = 0usize;
            for (index, line) in read_lines(&file)?.into_iter().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                match adapter.parse_line(&line) {
                    Ok(events) => {
                        for event in events {
                            append_event(&db, &session_id, &event, context.config.trace.redact)?;
                            imported += 1;
                        }
                    }
                    Err(error) => {
                        skipped += 1;
                        zenx_core::logging::warn(
                            "trace.record",
                            &format!("line {}: {error}", index + 1),
                        );
                    }
                }
            }

            if reporter.is_json() {
                reporter.json(&json!({
                    "ok": true,
                    "session": session_id,
                    "created": created,
                    "imported": imported,
                    "skipped": skipped,
                }))?;
            } else {
                reporter.line(format!(
                    "session {session_id}: recorded {imported} event(s)"
                ));
                if skipped > 0 {
                    reporter.note(format!(
                        "{skipped} line(s) could not be parsed (see ZENX_LOG=warn)"
                    ));
                }
                if created {
                    reporter.note(format!("close it with: zenx trace end {session_id}"));
                }
            }
            Ok(0)
        }

        TraceCommand::Start { agent, label, cwd } => {
            let db = context.db()?;
            let cwd = match cwd {
                Some(path) => path,
                None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            };
            let id = create_session(&db, &agent, &cwd, label.as_deref())?;
            if reporter.is_json() {
                reporter.json(&json!({ "ok": true, "session": id }))?;
            } else {
                reporter.line(id);
            }
            Ok(0)
        }

        TraceCommand::End { session, status } => {
            let db = context.db()?;
            finish_session(&db, &session, &status)?;
            if reporter.is_json() {
                reporter.json(&json!({ "ok": true, "session": session, "status": status }))?;
            } else {
                reporter.line(format!("session {session} closed as {status}"));
            }
            Ok(0)
        }

        TraceCommand::Import {
            file,
            adapter,
            agent,
            label,
        } => {
            let db = context.db()?;
            let adapter = adapter_by_name(&adapter)
                .ok_or_else(|| ZenxError::Invalid(format!("unknown adapter {adapter:?}")))?;
            let summary = import_jsonl(&db, &file, adapter.as_ref(), &agent, label.as_deref())?;
            if reporter.is_json() {
                reporter.json(&summary)?;
            } else {
                reporter.line(format!(
                    "imported {} event(s) into {} ({} skipped)",
                    summary.imported, summary.session_id, summary.skipped
                ));
            }
            Ok(0)
        }

        TraceCommand::Prune { days } => {
            let db = context.db()?;
            let removed = prune_sessions(&db, days)?;
            if reporter.is_json() {
                reporter.json(&json!({ "ok": true, "removed": removed }))?;
            } else {
                reporter.line(format!(
                    "removed {removed} session(s) older than {days} day(s)"
                ));
            }
            Ok(0)
        }

        TraceCommand::Adapters => {
            let list: Vec<serde_json::Value> = adapters()
                .iter()
                .map(|adapter| {
                    json!({ "name": adapter.name(), "description": adapter.description() })
                })
                .collect();
            if reporter.is_json() {
                reporter.json(&json!({ "adapters": list }))?;
            } else {
                for entry in list {
                    reporter.line(format!(
                        "{}  {}",
                        entry["name"].as_str().unwrap_or_default(),
                        entry["description"].as_str().unwrap_or_default()
                    ));
                }
            }
            Ok(0)
        }

        TraceCommand::Delete { session } => {
            let db = context.db()?;
            delete_session(&db, &session)?;
            if reporter.is_json() {
                reporter.json(&json!({ "ok": true, "deleted": session }))?;
            } else {
                reporter.line(format!("deleted session {session}"));
            }
            Ok(0)
        }
    }
}

fn read_lines(file: &Option<PathBuf>) -> Result<Vec<String>> {
    match file {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| ZenxError::io(path, e))?;
            Ok(text.lines().map(|line| line.to_string()).collect())
        }
        None => {
            let stdin = io::stdin();
            let mut lines = Vec::new();
            for line in stdin.lock().lines() {
                lines.push(line.map_err(|e| ZenxError::io("<stdin>", e))?);
            }
            Ok(lines)
        }
    }
}
