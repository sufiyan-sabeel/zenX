use std::collections::BTreeMap;

use clap::Args;
use serde::Serialize;
use zenx_core::module::{ModuleInfo, MODULES};
use zenx_core::Result;

use crate::context::Context;
use crate::OutputArg;

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Skip opening the database
    #[arg(long)]
    pub no_database: bool,
}

#[derive(Serialize)]
struct StatusReport {
    home: String,
    initialised: bool,
    config_version: u32,
    output_format: String,
    database: Option<DatabaseReport>,
    modules: &'static [ModuleInfo],
}

#[derive(Serialize)]
struct DatabaseReport {
    path: String,
    schema_version: i64,
    rows: BTreeMap<String, i64>,
}

pub fn run(args: &StatusArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let context = Context::load(output, no_color)?;
    let reporter = context.reporter;

    let database = if args.no_database || !context.paths.database_file().is_file() {
        None
    } else {
        let db = context.db()?;
        let mut rows = BTreeMap::new();
        for table in zenx_storage::schema::TABLES {
            rows.insert(table.to_string(), db.count(table)?);
        }
        Some(DatabaseReport {
            path: context.paths.database_file().display().to_string(),
            schema_version: db.schema_version()?,
            rows,
        })
    };

    let report = StatusReport {
        home: context.paths.root().display().to_string(),
        initialised: context.paths.is_initialised(),
        config_version: context.config.version,
        output_format: context.config.output.format.clone(),
        database,
        modules: MODULES,
    };

    if reporter.is_json() {
        reporter.json(&report)?;
        return Ok(0);
    }

    reporter.line(format!("zenX home      {}", report.home));
    reporter.line(format!(
        "initialised    {}",
        if report.initialised {
            "yes"
        } else {
            "no — run `zenx init`"
        }
    ));
    reporter.line(format!("config version {}", report.config_version));
    reporter.line(format!("output format  {}", report.output_format));
    match &report.database {
        Some(database) => {
            reporter.line(format!(
                "database       {} (schema v{})",
                database.path, database.schema_version
            ));
            let summary: Vec<String> = database
                .rows
                .iter()
                .filter(|(_, count)| **count > 0)
                .map(|(table, count)| format!("{table}={count}"))
                .collect();
            if summary.is_empty() {
                reporter.line("rows           none recorded yet");
            } else {
                reporter.line(format!("rows           {}", summary.join(" ")));
            }
        }
        None => reporter.line("database       not created yet — run `zenx init`"),
    }

    reporter.heading("modules");
    for module in MODULES {
        let status = match module.status {
            zenx_core::module::ModuleStatus::Implemented => "implemented",
            zenx_core::module::ModuleStatus::Partial => "partial    ",
            zenx_core::module::ModuleStatus::Planned => "planned    ",
        };
        reporter.line(format!(
            "  {status}  {:<14} {}",
            module.name, module.summary
        ));
    }
    Ok(0)
}
