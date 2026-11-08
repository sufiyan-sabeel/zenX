use std::path::PathBuf;

use clap::Args;
use serde_json::json;
use zenx_core::{Result, ZenxError};
use zenx_doctor::{apply_fixes, diagnose, DoctorOptions};

use crate::OutputArg;

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Repository to inspect
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Print the repair plan without changing anything
    #[arg(long)]
    pub fix: bool,

    /// Apply the repair plan (requires --fix)
    #[arg(long)]
    pub apply: bool,

    /// Run the ecosystem build in an isolated target directory
    #[arg(long)]
    pub build: bool,

    /// Run the ecosystem test command
    #[arg(long)]
    pub tests: bool,

    /// Write a Markdown report to this file
    #[arg(long, value_name = "FILE")]
    pub report: Option<PathBuf>,
}

pub fn run(args: &DoctorArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let reporter = crate::reporter_for(output, no_color)?;
    if args.apply && !args.fix {
        return Err(ZenxError::Invalid(
            "--apply needs --fix: run `zenx doctor --fix` to review the plan first".into(),
        ));
    }

    let options = DoctorOptions {
        run_build: args.build,
        run_tests: args.tests,
        known_commands: crate::commands::command_names(),
    };
    let report = diagnose(&args.path, &options)?;

    // Collect everything first so JSON mode emits exactly one document.
    let outcomes = if args.fix {
        Some(apply_fixes(&args.path, &report.fixes, args.apply)?)
    } else {
        None
    };

    if let Some(path) = &args.report {
        let markdown = report.render_markdown();
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
            "report": report,
            "repairs": outcomes.as_ref().map(|outcomes| json!({
                "applied": args.apply,
                "outcomes": outcomes,
            })),
        }))?;
        return Ok(if report.is_healthy() { 0 } else { 1 });
    }

    reporter.line(format!("repository  {}", report.root));
    reporter.line(format!(
        "ecosystems  {}",
        if report.ecosystems.is_empty() {
            "none detected".to_string()
        } else {
            report.ecosystems.join(", ")
        }
    ));
    reporter.line("");
    for check in &report.checks {
        reporter.line(format!(
            "{:<5} {:<32} {}",
            check.status.symbol(),
            check.title,
            check.detail
        ));
        for item in check.evidence.iter().take(5) {
            for line in item.lines().take(10) {
                reporter.line(format!("        {line}"));
            }
        }
    }
    reporter.line("");
    reporter.line(format!(
        "{} pass, {} warn, {} fail, {} skip",
        report.summary.pass, report.summary.warn, report.summary.fail, report.summary.skip
    ));

    match outcomes {
        Some(outcomes) => {
            reporter.heading(if args.apply {
                "repairs applied"
            } else {
                "repair plan (nothing changed; add --apply to perform it)"
            });
            if outcomes.is_empty() {
                reporter.line("  no repairs available");
            }
            for outcome in &outcomes {
                reporter.line(format!("  {}", outcome.message));
            }
        }
        None if !report.fixes.is_empty() => {
            reporter.line("");
            reporter.line(format!(
                "{} repair(s) available; review with `zenx doctor --fix`",
                report.fixes.len()
            ));
        }
        None => {}
    }

    if !report.is_healthy() {
        reporter.note(format!("{} check(s) failed", report.summary.fail));
    }
    Ok(if report.is_healthy() { 0 } else { 1 })
}
