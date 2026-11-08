use clap::Args;
use serde_json::json;
use zenx_core::Result;
use zenx_selfupdate::{rollback, update, UpdateAction, UpdateOptions};

use crate::OutputArg;

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Report whether a newer release exists, without installing it
    #[arg(long)]
    pub check: bool,

    /// Install a specific release tag, e.g. v0.2.0
    #[arg(long = "tag", value_name = "TAG")]
    pub tag: Option<String>,

    /// Download and verify the release, but do not replace the binary
    #[arg(long)]
    pub dry_run: bool,

    /// Restore the binary saved by the previous update
    #[arg(long)]
    pub rollback: bool,
}

pub fn run(args: &UpdateArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let reporter = crate::reporter_for(output, no_color)?;

    if args.rollback {
        let restored = rollback()?;
        if reporter.is_json() {
            reporter.json(&json!({ "ok": true, "restored": restored.display().to_string() }))?;
        } else {
            reporter.line(format!(
                "restored the previous binary at {}",
                restored.display()
            ));
        }
        return Ok(0);
    }

    let report = update(&UpdateOptions {
        check_only: args.check,
        version: args.tag.clone(),
        dry_run: args.dry_run,
        target: None,
        base_url: None,
        current_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    })?;

    if reporter.is_json() {
        reporter.json(&report)?;
        return Ok(0);
    }

    match report.action {
        UpdateAction::UpToDate => reporter.line(report.message),
        UpdateAction::Available => {
            reporter.line(report.message);
            reporter.note("run `zenx update` to install it");
        }
        UpdateAction::Verified => {
            reporter.line(report.message);
            reporter.line(format!(
                "checksum {}",
                report.checksum.as_deref().unwrap_or("-")
            ));
        }
        UpdateAction::Installed => {
            reporter.line(report.message);
            reporter.line(format!(
                "checksum {}",
                report.checksum.as_deref().unwrap_or("-")
            ));
            reporter.note("undo with `zenx update --rollback`");
        }
        UpdateAction::Manual => {
            reporter.line(report.message);
        }
    }
    Ok(0)
}
