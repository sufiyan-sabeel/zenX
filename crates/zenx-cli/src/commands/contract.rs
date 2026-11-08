use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::json;
use zenx_contract::{compare, load_spec};
use zenx_core::{Result, ZenxError};

use crate::OutputArg;

#[derive(Debug, Args)]
pub struct ContractArgs {
    #[command(subcommand)]
    pub command: ContractCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ContractCommand {
    /// Compare a base specification with a head specification
    Compare {
        /// Base (old) OpenAPI document
        base: PathBuf,
        /// Head (new) OpenAPI document
        head: PathBuf,
        /// Write a Markdown report to this file
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
    },
}

pub fn run(args: &ContractArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let reporter = crate::reporter_for(output, no_color)?;
    match &args.command {
        ContractCommand::Compare { base, head, out } => {
            let base_spec = load_spec(base)?;
            let head_spec = load_spec(head)?;
            let comparison = compare(&base_spec, &head_spec);

            if let Some(path) = out {
                let markdown = comparison.render_markdown(
                    &base.display().to_string(),
                    &head.display().to_string(),
                );
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| ZenxError::io(parent, e))?;
                    }
                }
                std::fs::write(path, markdown).map_err(|e| ZenxError::io(path, e))?;
                reporter.note(format!("wrote {}", path.display()));
            }

            if reporter.is_json() {
                reporter.json(&json!({
                    "compatible": comparison.is_compatible(),
                    "breaking": comparison.breaking,
                    "non_breaking": comparison.non_breaking,
                    "informational": comparison.informational,
                    "changes": comparison.changes,
                }))?;
            } else {
                reporter.line(format!(
                    "{} → {}",
                    comparison.base_title, comparison.head_title
                ));
                if comparison.changes.is_empty() {
                    reporter.line("no differences found");
                }
                for change in &comparison.changes {
                    reporter.line(format!(
                        "  [{:?}] {:<26} {:<24} {}",
                        change.severity, change.kind, change.endpoint, change.detail
                    ));
                }
                reporter.line("");
                reporter.line(format!(
                    "{} breaking, {} non-breaking, {} informational",
                    comparison.breaking, comparison.non_breaking, comparison.informational
                ));
            }

            // Exit 1 on breaking changes so CI can gate on it.
            Ok(if comparison.is_compatible() { 0 } else { 1 })
        }
    }
}
