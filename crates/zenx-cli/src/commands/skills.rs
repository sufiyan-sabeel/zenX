use clap::{Args, Subcommand};
use serde_json::json;
use zenx_core::Result;
use zenx_skills::{InstallRequest, SkillSource, SkillsManager};

use crate::context::Context;
use crate::OutputArg;

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: Option<SkillsCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SkillsCommand {
    /// List installed skills (default when no subcommand is given)
    List,
    /// Search installed skills by name or description
    Search { query: String },
    /// Install a skill from a git URL or a local directory
    Install {
        /// Git URL or local path containing SKILL.md
        source: String,
        /// Override the name declared in SKILL.md
        #[arg(long)]
        name: Option<String>,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
        /// Replace an existing installation
        #[arg(long)]
        force: bool,
        /// Consent to network access for this install
        #[arg(long)]
        allow_network: bool,
        /// Do not copy into the agent targets configured in config.toml
        #[arg(long)]
        no_targets: bool,
    },
    /// Verify an installed skill against its manifest checksum
    Verify { name: String },
    /// Re-fetch a skill from its original git source
    Update { name: String },
    /// Restore the most recent backup of a skill
    Rollback { name: String },
    /// Remove a skill, keeping a backup
    Remove { name: String },
}

pub fn run(args: &SkillsArgs, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    let context = Context::load(output, no_color)?;
    let reporter = context.reporter;
    let db = context.db()?;
    let manager = SkillsManager::new(&db, &context.paths);

    match args.command.clone().unwrap_or(SkillsCommand::List) {
        SkillsCommand::List => {
            let skills = manager.list()?;
            if reporter.is_json() {
                reporter.json(&json!({ "skills": skills, "count": skills.len() }))?;
                return Ok(0);
            }
            if skills.is_empty() {
                reporter.line("no skills installed");
                reporter.note("install one with: zenx skills install <git-url-or-path>");
                return Ok(0);
            }
            reporter.line(format!("{:<24} {:<10} {}", "NAME", "VERSION", "SOURCE"));
            for skill in &skills {
                reporter.line(format!(
                    "{:<24} {:<10} {}",
                    skill.name,
                    skill.version.as_deref().unwrap_or("-"),
                    skill.source
                ));
            }
            Ok(0)
        }

        SkillsCommand::Search { query } => {
            let matches = manager.search(&query)?;
            if reporter.is_json() {
                reporter.json(&json!({ "query": query, "matches": matches }))?;
            } else if matches.is_empty() {
                reporter.line(format!("no installed skill matches {query:?}"));
            } else {
                for skill in &matches {
                    reporter.line(format!("{:<24} {}", skill.name, skill.description));
                }
            }
            Ok(0)
        }

        SkillsCommand::Install {
            source,
            name,
            dry_run,
            force,
            allow_network,
            no_targets,
        } => {
            let mut request = InstallRequest::new(SkillSource::parse(&source));
            request.name = name;
            request.dry_run = dry_run;
            request.force = force;
            request.allow_network = allow_network || context.config.skills.allow_network;
            if !no_targets {
                request.targets = context.skill_targets();
            }
            let report = manager.install(&request)?;

            if reporter.is_json() {
                reporter.json(&report)?;
                return Ok(0);
            }
            if report.dry_run {
                reporter.line(format!("dry run: would install {}", report.name));
                reporter.line(format!("  source    {}", report.source));
                reporter.line(format!(
                    "  version   {}",
                    report.version.as_deref().unwrap_or("-")
                ));
                reporter.line(format!("  files     {}", report.files));
                reporter.line(format!("  checksum  {}", report.checksum_sha256));
                reporter.line(format!("  target    {}", report.path));
                return Ok(0);
            }
            reporter.line(format!(
                "installed {} {} ({} file(s))",
                report.name,
                report.version.as_deref().unwrap_or(""),
                report.files
            ));
            reporter.line(format!("  target    {}", report.path));
            reporter.line(format!("  checksum  {}", report.checksum_sha256));
            if let Some(backup) = &report.replaced_backup {
                reporter.line(format!("  previous  backed up to {backup}"));
            }
            if !report.linked_targets.is_empty() {
                reporter.line(format!("  linked to {}", report.linked_targets.join(", ")));
            }
            Ok(0)
        }

        SkillsCommand::Verify { name } => {
            let report = manager.verify(&name)?;
            if reporter.is_json() {
                reporter.json(&report)?;
                return Ok(0);
            }
            if report.ok {
                reporter.line(format!(
                    "{} is intact ({})",
                    report.name, report.expected_checksum
                ));
                return Ok(0);
            }
            reporter.line(format!("{} has changed since install:", report.name));
            for problem in &report.problems {
                reporter.line(format!("  - {problem}"));
            }
            for file in &report.missing_files {
                reporter.line(format!("  - missing {file}"));
            }
            Ok(1)
        }

        SkillsCommand::Update { name } => {
            let report = manager.update(&name)?;
            if reporter.is_json() {
                reporter.json(&report)?;
                return Ok(0);
            }
            if report.changed {
                reporter.line(format!(
                    "updated {} ({:?} → {:?})",
                    report.name, report.old_commit, report.new_commit
                ));
                if let Some(backup) = &report.backup {
                    reporter.line(format!("  previous  backed up to {backup}"));
                }
            } else {
                reporter.line(format!("{} is already up to date", report.name));
            }
            Ok(0)
        }

        SkillsCommand::Rollback { name } => {
            let manifest = manager.rollback(&name)?;
            if reporter.is_json() {
                reporter.json(&json!({ "ok": true, "skill": manifest }))?;
            } else {
                reporter.line(format!(
                    "rolled {} back to {}",
                    manifest.name,
                    manifest.version.as_deref().unwrap_or("the previous copy")
                ));
            }
            Ok(0)
        }

        SkillsCommand::Remove { name } => {
            let report = manager.remove(&name)?;
            if reporter.is_json() {
                reporter.json(&report)?;
            } else {
                reporter.line(format!(
                    "removed {} (backup kept at {})",
                    report.name, report.backup_path
                ));
            }
            Ok(0)
        }
    }
}
