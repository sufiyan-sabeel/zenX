use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::json;
use zenx_bench::{compare, load_tasks, RunOptions};
use zenx_core::Result;
use zenx_storage::map_err;

use crate::context::Context;
use crate::OutputArg;

#[derive(Debug, Args)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub command: Option<BenchCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum BenchCommand {
    /// Run every task in a TOML task file
    Run {
        /// Task file
        #[arg(default_value = "zenx-bench.toml")]
        tasks: PathBuf,
        /// Iterations per task
        #[arg(long, default_value_t = 1)]
        repeat: u32,
        /// Label recorded with the run
        #[arg(long)]
        label: Option<String>,
    },
    /// Compare two recorded runs by id
    Compare { base: i64, head: i64 },
    /// List recorded runs
    History {
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

pub fn run(args: &BenchArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let context = Context::load(output, no_color)?;
    let reporter = context.reporter;

    match args
        .command
        .clone()
        .unwrap_or(BenchCommand::History { limit: 10 })
    {
        BenchCommand::Run {
            tasks,
            repeat,
            label,
        } => {
            let db = context.db()?;
            let definitions = load_tasks(&tasks)?;
            let summary =
                zenx_bench::run(&db, &definitions, &RunOptions { repeat, label }, &tasks)?;
            if reporter.is_json() {
                reporter.json(&summary)?;
                return Ok(0);
            }
            reporter.line(format!(
                "run #{} \"{}\" ({} iteration(s) per task)",
                summary.run_id, summary.label, summary.repeat
            ));
            reporter.line(format!(
                "{:<24} {:>10} {:>10} {:>12}",
                "TASK", "PASSED", "RATE", "MEAN"
            ));
            for task in &summary.tasks {
                reporter.line(format!(
                    "{:<24} {:>10} {:>9.0}% {:>10.1}ms",
                    task.task_id,
                    format!("{}/{}", task.passed, task.iterations),
                    task.completion_rate * 100.0,
                    task.mean_duration_ms
                ));
                for sample in &task.failure_samples {
                    reporter.line(format!("        {sample}"));
                }
            }
            reporter.line(format!(
                "overall completion {:.0}% (measured)",
                summary.overall_completion_rate() * 100.0
            ));
            Ok(0)
        }

        BenchCommand::Compare { base, head } => {
            let db = context.db()?;
            let comparison = compare(&db, base, head)?;
            if reporter.is_json() {
                reporter.json(&comparison)?;
                return Ok(0);
            }
            reporter.line(format!("comparing run #{base} → #{head}"));
            reporter.line(format!(
                "{:<24} {:>10} {:>10} {:>12}  {}",
                "TASK", "BASE", "HEAD", "DURATION", "NOTE"
            ));
            for task in &comparison.tasks {
                reporter.line(format!(
                    "{:<24} {:>9.0}% {:>9.0}% {:>+11.0}%  {}",
                    task.task_id,
                    task.base_completion * 100.0,
                    task.head_completion * 100.0,
                    task.duration_delta_pct,
                    if task.regression { "REGRESSION" } else { "" }
                ));
                if let Some(reason) = &task.reason {
                    reporter.line(format!("        {reason}"));
                }
            }
            reporter.line(format!("{} regression(s)", comparison.regressions));
            Ok(if comparison.regressions > 0 { 1 } else { 0 })
        }

        BenchCommand::History { limit } => {
            let db = context.db()?;
            let mut statement = db
                .conn()
                .prepare(
                    "SELECT id, label, repeat, started_at, ended_at FROM bench_runs
                     ORDER BY id DESC LIMIT ?1",
                )
                .map_err(map_err)?;
            let rows = statement
                .query_map([limit as i64], |row| {
                    Ok(json!({
                        "run_id": row.get::<_, i64>(0)?,
                        "label": row.get::<_, String>(1)?,
                        "repeat": row.get::<_, i64>(2)?,
                        "started_at": row.get::<_, String>(3)?,
                        "ended_at": row.get::<_, Option<String>>(4)?,
                    }))
                })
                .map_err(map_err)?;
            let mut runs = Vec::new();
            for row in rows {
                runs.push(row.map_err(map_err)?);
            }
            if reporter.is_json() {
                reporter.json(&json!({ "runs": runs }))?;
                return Ok(0);
            }
            if runs.is_empty() {
                reporter.line("no benchmark runs recorded");
                return Ok(0);
            }
            reporter.line(format!(
                "{:<6} {:<32} {:<22} {}",
                "RUN", "LABEL", "STARTED", "ENDED"
            ));
            for run in &runs {
                reporter.line(format!(
                    "#{:<5} {:<32} {:<22} {}",
                    run["run_id"],
                    run["label"].as_str().unwrap_or_default(),
                    run["started_at"].as_str().unwrap_or_default(),
                    run["ended_at"].as_str().unwrap_or("(running)")
                ));
            }
            Ok(0)
        }
    }
}
