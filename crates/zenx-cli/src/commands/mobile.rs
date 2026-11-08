use std::path::PathBuf;

use clap::{Args, Subcommand};
use zenx_core::Result;
use zenx_mobile::{backup, default_backup_dir, diagnose, restore};

use crate::context::Context;
use crate::OutputArg;

#[derive(Debug, Args)]
pub struct MobileArgs {
    #[command(subcommand)]
    pub command: Option<MobileCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum MobileCommand {
    /// Report the Android/Termux environment and installed toolchains (default)
    Diagnose,
    /// Copy configuration, database and skills into a backup directory
    Backup {
        /// Destination directory; defaults to <home>/backups
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
    },
    /// Restore a backup created by `zenx mobile backup`
    Restore {
        /// Backup directory to restore
        from: PathBuf,
        /// Overwrite existing configuration, database and skills
        #[arg(long)]
        overwrite: bool,
    },
}

pub fn run(args: &MobileArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let context = Context::load(output, no_color)?;
    let reporter = context.reporter;

    match args.command.clone().unwrap_or(MobileCommand::Diagnose) {
        MobileCommand::Diagnose => {
            let report = diagnose();
            if reporter.is_json() {
                reporter.json(&report)?;
                return Ok(0);
            }
            let environment = &report.environment;
            reporter.line(format!("os           {} ({})", environment.os, environment.arch));
            reporter.line(format!(
                "termux       {}",
                if environment.termux {
                    format!(
                        "yes ({})",
                        environment.termux_version.as_deref().unwrap_or("version unknown")
                    )
                } else {
                    "no".to_string()
                }
            ));
            reporter.line(format!(
                "android      {}",
                match (&environment.android, &environment.android_release) {
                    (true, Some(release)) => format!("yes (Android {release})"),
                    (true, None) => "yes".to_string(),
                    (false, _) => "no".to_string(),
                }
            ));
            reporter.line(format!("home         {}", environment.home.as_deref().unwrap_or("-")));

            reporter.heading("toolchains");
            for tool in &report.tools {
                let status = if tool.found { "found" } else { "missing" };
                reporter.line(format!(
                    "  {:<8} {:<8} {}{}",
                    tool.name,
                    status,
                    tool.version.clone().unwrap_or_default(),
                    if tool.found || !tool.required {
                        String::new()
                    } else {
                        format!("  (try: {})", tool.hint)
                    }
                ));
            }

            if !report.issues.is_empty() {
                reporter.heading("findings");
                for issue in &report.issues {
                    reporter.line(format!("  [{}] {}", issue.severity, issue.message));
                    reporter.line(format!("         {}", issue.hint));
                }
            }
            if !report.notes.is_empty() {
                reporter.heading("notes");
                for note in &report.notes {
                    reporter.line(format!("  - {note}"));
                }
            }
            Ok(0)
        }

        MobileCommand::Backup { out } => {
            let destination = out.unwrap_or_else(|| default_backup_dir(&context.paths));
            let report = backup(&context.paths, &destination)?;
            if reporter.is_json() {
                reporter.json(&report)?;
            } else {
                reporter.line(format!("backup written to {}", report.path));
                reporter.line(format!("entries: {}", report.entries.join(", ")));
            }
            Ok(0)
        }

        MobileCommand::Restore { from, overwrite } => {
            let report = restore(&context.paths, &from, overwrite)?;
            if reporter.is_json() {
                reporter.json(&report)?;
            } else {
                if report.restored.is_empty() {
                    reporter.line("nothing was restored");
                } else {
                    reporter.line(format!("restored: {}", report.restored.join(", ")));
                }
                for skip in &report.skipped {
                    reporter.note(format!("skipped {skip}"));
                }
            }
            Ok(0)
        }
    }
}
