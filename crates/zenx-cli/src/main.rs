//! zenX command line interface.
//!
//! Every command here is wired to real functionality in a module crate. There
//! are no placeholder subcommands: a capability that is not implemented simply
//! has no command, and `zenx status` reports it as planned.

use clap::{Parser, Subcommand, ValueEnum};
use zenx_core::output::{OutputFormat, Reporter};
use zenx_core::Result;

mod commands;
mod context;

use commands::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputArg {
    /// Human-readable text
    Human,
    /// Machine-readable JSON on stdout
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = "zenx",
    version,
    about = "zenX — build smarter, debug deeper, develop anywhere",
    long_about = "A local-first developer infrastructure platform: agent tracing, skill management, \
repository diagnostics, Termux tooling, benchmarking, API contract checks and offline documentation.",
    propagate_version = true,
    disable_help_subcommand = false
)]
struct Cli {
    /// Output format; overrides output.format from config.toml
    #[arg(long, global = true, value_enum, value_name = "FORMAT")]
    output: Option<OutputArg>,

    /// Disable colour in human-readable output
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Command,
}

fn main() -> std::process::ExitCode {
    zenx_core::logging::init_from_env();
    let cli = Cli::parse();

    // Resolve the format before dispatch so errors are rendered the same way as
    // results: JSON errors go to stdout as a single document.
    let format = resolve_format(cli.output);
    match commands::dispatch(&cli.command, cli.output, cli.no_color) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            match format {
                OutputFormat::Json => {
                    let payload = serde_json::json!({
                        "ok": false,
                        "error": { "kind": error.kind(), "message": error.to_string() }
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&payload).unwrap_or_default()
                    );
                }
                OutputFormat::Human => eprintln!("zenx: {error}"),
            }
            std::process::ExitCode::from(error.exit_code() as u8)
        }
    }
}

fn resolve_format(output: Option<OutputArg>) -> OutputFormat {
    match output {
        Some(OutputArg::Json) => OutputFormat::Json,
        Some(OutputArg::Human) => OutputFormat::Human,
        None => {
            let path = match zenx_core::paths::ZenxPaths::resolve() {
                Ok(paths) => paths.config_file(),
                Err(_) => return OutputFormat::Human,
            };
            match zenx_core::config::Config::load(&path) {
                Ok(config) => {
                    OutputFormat::parse(&config.output.format).unwrap_or(OutputFormat::Human)
                }
                Err(_) => OutputFormat::Human,
            }
        }
    }
}

/// Helper used by command modules that need a reporter without a full context.
pub(crate) fn reporter_for(output: Option<OutputArg>, no_color: bool) -> Result<Reporter> {
    let format = match output {
        Some(OutputArg::Json) => OutputFormat::Json,
        Some(OutputArg::Human) => OutputFormat::Human,
        None => resolve_format(None),
    };
    Ok(Reporter::new(format, !no_color))
}
