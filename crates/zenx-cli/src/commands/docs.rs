use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::json;
use zenx_core::Result;
use zenx_docs::{add_collection, list_collections, remove_collection, search};

use crate::context::Context;
use crate::OutputArg;

#[derive(Debug, Args)]
pub struct DocsArgs {
    #[command(subcommand)]
    pub command: Option<DocsCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DocsCommand {
    /// Import a directory of Markdown or text files
    Add {
        /// Directory containing the documentation
        path: PathBuf,
        #[arg(long, default_value = "local")]
        collection: String,
        #[arg(long = "doc-version", default_value = "latest")]
        doc_version: String,
    },
    /// Search imported documentation (default when no subcommand is given)
    Search {
        query: String,
        #[arg(long)]
        collection: Option<String>,
        #[arg(long = "doc-version")]
        doc_version: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List imported collections
    List,
    /// Remove an imported collection
    Remove {
        collection: String,
        #[arg(long = "doc-version", default_value = "latest")]
        doc_version: String,
    },
}

pub fn run(args: &DocsArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let context = Context::load(output, no_color)?;
    let reporter = context.reporter;
    let db = context.db()?;

    match args.command.clone() {
        Some(DocsCommand::Add {
            path,
            collection,
            doc_version,
        }) => {
            let report = add_collection(
                &db,
                &path,
                &collection,
                &doc_version,
                context.config.docs.max_file_bytes,
            )?;
            if reporter.is_json() {
                reporter.json(&report)?;
                return Ok(0);
            }
            reporter.line(format!(
                "imported {} document(s) into {collection}@{doc_version}",
                report.imported
            ));
            for skip in &report.skipped {
                reporter.note(format!("skipped {skip}"));
            }
            Ok(0)
        }

        Some(DocsCommand::List) => {
            let collections = list_collections(&db)?;
            if reporter.is_json() {
                reporter.json(&json!({ "collections": collections }))?;
                return Ok(0);
            }
            if collections.is_empty() {
                reporter.line("no documentation imported");
                reporter.note("import some with: zenx docs add <directory>");
                return Ok(0);
            }
            reporter.line(format!(
                "{:<24} {:<12} {:>6}  {}",
                "COLLECTION", "VERSION", "DOCS", "SOURCE"
            ));
            for collection in &collections {
                reporter.line(format!(
                    "{:<24} {:<12} {:>6}  {}",
                    collection.collection,
                    collection.version,
                    collection.doc_count,
                    collection.source
                ));
            }
            Ok(0)
        }

        Some(DocsCommand::Remove {
            collection,
            doc_version,
        }) => {
            let removed = remove_collection(&db, &collection, &doc_version)?;
            if reporter.is_json() {
                reporter.json(&json!({ "ok": true, "removed": removed }))?;
            } else {
                reporter.line(format!(
                    "removed {removed} document(s) from {collection}@{doc_version}"
                ));
            }
            Ok(0)
        }

        Some(DocsCommand::Search {
            query,
            collection,
            doc_version,
            limit,
        }) => search_and_print(&db, &reporter, &query, collection, doc_version, limit),

        None => {
            // No subcommand and no query: list what is available.
            let collections = list_collections(&db)?;
            if reporter.is_json() {
                reporter.json(&json!({ "collections": collections }))?;
            } else if collections.is_empty() {
                reporter.line("no documentation imported");
                reporter.note("usage: zenx docs search <query>");
            } else {
                reporter.line("usage: zenx docs search <query>");
                for collection in &collections {
                    reporter.line(format!(
                        "  {:<24} {:<12} {} doc(s)",
                        collection.collection, collection.version, collection.doc_count
                    ));
                }
            }
            Ok(0)
        }
    }
}

fn search_and_print(
    db: &zenx_storage::Db,
    reporter: &zenx_core::output::Reporter,
    query: &str,
    collection: Option<String>,
    doc_version: Option<String>,
    limit: usize,
) -> Result<u8> {
    let hits = search(db, query, collection.as_deref(), doc_version.as_deref(), limit)?;
    if reporter.is_json() {
        reporter.json(&json!({ "query": query, "hits": hits }))?;
        return Ok(0);
    }
    if hits.is_empty() {
        reporter.line(format!("no results for {query:?}"));
        return Ok(0);
    }
    for hit in &hits {
        reporter.line(format!(
            "{}  [{}@{}]  score {}",
            hit.title, hit.collection, hit.version, hit.score
        ));
        reporter.line(format!("  {}", hit.path));
        reporter.line(format!("  {}", hit.snippet));
        reporter.line("");
    }
    Ok(0)
}
