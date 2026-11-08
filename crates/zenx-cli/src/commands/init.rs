use std::path::PathBuf;

use clap::Args;
use serde_json::json;
use zenx_core::config::{Config, CONFIG_VERSION};
use zenx_core::paths::ZenxPaths;
use zenx_core::Result;
use zenx_storage::Db;

use crate::OutputArg;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Use this directory as the zenX home instead of $ZENX_HOME or ~/.zenx
    #[arg(long, value_name = "DIR")]
    pub home: Option<PathBuf>,

    /// Overwrite an existing config.toml with defaults
    #[arg(long)]
    pub force: bool,
}

pub fn run(args: &InitArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let reporter = crate::reporter_for(output, no_color)?;
    let paths = match &args.home {
        Some(home) => ZenxPaths::new(home),
        None => ZenxPaths::resolve()?,
    };
    paths.ensure()?;

    let config_path = paths.config_file();
    let mut created: Vec<String> = Vec::new();
    if config_path.is_file() && !args.force {
        // Keep what the user already has; re-validate it instead of replacing it.
        let config = Config::load(&config_path)?;
        config.save(&config_path)?;
    } else {
        Config::default().save(&config_path)?;
        created.push("config.toml".to_string());
    }

    let database = paths.database_file();
    let existed = database.is_file();
    let db = Db::open(&database)?;
    if !existed {
        created.push("zenx.db".to_string());
    }
    let schema_version = db.schema_version()?;

    if reporter.is_json() {
        reporter.json(&json!({
            "ok": true,
            "home": paths.root().display().to_string(),
            "config": config_path.display().to_string(),
            "database": database.display().to_string(),
            "schema_version": schema_version,
            "config_version": CONFIG_VERSION,
            "created": created,
        }))?;
        return Ok(0);
    }

    reporter.line(format!("zenX home: {}", paths.root().display()));
    reporter.line(format!("configuration: {}", config_path.display()));
    reporter.line(format!(
        "database: {} (schema v{schema_version})",
        database.display()
    ));
    if created.is_empty() {
        reporter.line("nothing needed creating; existing state was validated");
    } else {
        reporter.line(format!("created: {}", created.join(", ")));
    }
    if args.home.is_some() && std::env::var_os(zenx_core::paths::ROOT_ENV).is_none() {
        reporter.note(format!(
            "note: export {}={} so other commands use this home",
            zenx_core::paths::ROOT_ENV,
            paths.root().display()
        ));
    }
    reporter.line("");
    reporter.line("next steps:");
    reporter.line("  zenx doctor            # check a repository");
    reporter.line("  zenx trace --help      # record agent sessions");
    reporter.line("  zenx skills --help     # manage agent skills");
    Ok(0)
}
