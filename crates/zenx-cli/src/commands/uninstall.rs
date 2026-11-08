use clap::Args;
use serde_json::json;
use zenx_core::Result;
use zenx_selfupdate::{uninstall, UninstallOptions};

use crate::OutputArg;

#[derive(Debug, Args)]
pub struct UninstallArgs {
    /// Actually remove files; without this the plan is only printed
    #[arg(long)]
    pub yes: bool,

    /// Also delete the zenX home directory: configuration, database and skills
    #[arg(long)]
    pub purge_data: bool,
}

pub fn run(args: &UninstallArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let reporter = crate::reporter_for(output, no_color)?;
    if args.purge_data && !args.yes {
        reporter.note("--purge-data needs --yes as well: it deletes configuration, sessions and skills");
    }

    let report = uninstall(&UninstallOptions {
        confirm: args.yes,
        purge_data: args.purge_data,
        data_root: None,
    })?;

    if reporter.is_json() {
        reporter.json(&report)?;
        return Ok(0);
    }

    reporter.line(format!("binary  {}", report.binary));
    if report.binary_removed {
        reporter.line("removed the zenX executable");
    }
    if let Some(root) = &report.data_root {
        reporter.line(format!("data    {root}"));
        if report.data_removed {
            reporter.line("deleted the zenX home directory");
        }
    }
    for note in &report.notes {
        reporter.line(format!("  {note}"));
    }
    Ok(0)
}
