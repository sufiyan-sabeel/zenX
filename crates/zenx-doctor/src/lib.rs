//! zenX Doctor — repository health diagnostics.
//!
//! A diagnostic run is read-only with one deliberate exception: language builds
//! run with their output redirected outside the repository, so `cargo` cannot
//! leave a `target/` directory behind. Repairs are never applied implicitly;
//! `zenx doctor --fix` prints the plan and `--apply` performs it.

pub mod detector;
pub mod fix;
pub mod report;

use std::path::{Path, PathBuf};

use zenx_core::{Result, ZenxError};

pub use detector::{Detector, DoctorContext};
pub use fix::{apply_fixes, Fix, FixAction, FixOutcome};
pub use report::{Check, DoctorReport, Status, Summary};

/// What the caller allows a diagnostic run to do.
#[derive(Debug, Clone, Default)]
pub struct DoctorOptions {
    /// Run the ecosystem build (e.g. `cargo check`) in an isolated target dir.
    pub run_build: bool,
    /// Run the ecosystem test command.
    pub run_tests: bool,
    /// Commands `zenx` itself advertises, used to validate documentation.
    pub known_commands: Vec<String>,
}

/// Run every applicable detector plus the ecosystem-independent checks.
pub fn diagnose(root: &Path, options: &DoctorOptions) -> Result<DoctorReport> {
    let root = std::fs::canonicalize(root).map_err(|e| ZenxError::io(root, e))?;
    if !root.is_dir() {
        return Err(ZenxError::Invalid(format!(
            "{} is not a directory",
            root.display()
        )));
    }

    let context = DoctorContext {
        run_build: options.run_build,
        run_tests: options.run_tests,
        known_commands: options.known_commands.clone(),
    };

    let mut checks = common_checks(&root, &context);
    let mut ecosystems = Vec::new();
    for detector in detector::detectors() {
        if detector.detect(&root) {
            ecosystems.push(detector.name().to_string());
            checks.extend(detector.checks(&root, &context));
        }
    }

    if ecosystems.is_empty() {
        checks.push(Check::new(
            "ecosystem.none",
            "Project ecosystem detected",
            Status::Warn,
            "no Rust, Node or Python project files found at this path".to_string(),
        ));
    }

    let mut report = DoctorReport {
        root: root.display().to_string(),
        generated_at: zenx_core::util::now_rfc3339(),
        ecosystems,
        checks,
        summary: Summary::default(),
        fixes: Vec::new(),
    };
    report.summary = Summary::from_checks(&report.checks);
    report.fixes = fix::plan_fixes(&root, &report);
    Ok(report)
}

fn common_checks(root: &Path, context: &DoctorContext) -> Vec<Check> {
    let mut checks = Vec::new();

    // Documentation
    match find_first(root, &["README.md", "README.rst", "README.txt", "readme.md"]) {
        Some(path) => checks.push(Check::new(
            "docs.readme",
            "README present",
            Status::Pass,
            format!("found {}", relative(root, &path)),
        )),
        None => checks.push(Check::new(
            "docs.readme",
            "README present",
            Status::Fail,
            "no README file at the repository root".to_string(),
        )),
    }

    // Documented commands
    if let Some(readme) = find_first(root, &["README.md", "readme.md"]) {
        checks.push(documented_commands_check(&readme, context));
    }

    // License
    match find_first(root, &["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"]) {
        Some(path) => {
            let detected = detect_license(&path);
            checks.push(Check::new(
                "license.present",
                "License present",
                Status::Pass,
                match detected {
                    Some(name) => format!("{} ({name})", relative(root, &path)),
                    None => format!("{} (unrecognised text)", relative(root, &path)),
                },
            ));
        }
        None => checks.push(Check::new(
            "license.present",
            "License present",
            Status::Warn,
            "no LICENSE file; redistribution terms are unclear".to_string(),
        )),
    }

    // Ignore file
    match find_first(root, &[".gitignore"]) {
        Some(path) => checks.push(Check::new(
            "repo.gitignore",
            ".gitignore present",
            Status::Pass,
            relative(root, &path),
        )),
        None => checks.push(Check::new(
            "repo.gitignore",
            ".gitignore present",
            Status::Warn,
            "no .gitignore; build output may be committed by accident".to_string(),
        )),
    }

    // CI configuration
    let mut ci_files = Vec::new();
    let workflows = root.join(".github").join("workflows");
    if let Ok(entries) = std::fs::read_dir(&workflows) {
        for entry in entries.flatten() {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("yml") | Some("yaml")
            ) {
                ci_files.push(relative(root, &path));
            }
        }
    }
    for candidate in [".gitlab-ci.yml", ".circleci/config.yml", "azure-pipelines.yml"] {
        let path = root.join(candidate);
        if path.is_file() {
            ci_files.push(candidate.to_string());
        }
    }
    ci_files.sort();
    if ci_files.is_empty() {
        checks.push(Check::new(
            "ci.present",
            "CI configuration present",
            Status::Warn,
            "no GitHub Actions, GitLab CI or CircleCI configuration found".to_string(),
        ));
    } else {
        let mut check = Check::new(
            "ci.present",
            "CI configuration present",
            Status::Pass,
            format!("{} configuration file(s)", ci_files.len()),
        );
        check.evidence = ci_files;
        checks.push(check);
    }

    // Git state
    match run_git(root, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(output) if output.trim() == "true" => {
            let branch = run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
                .unwrap_or_else(|_| "unknown".to_string());
            let dirty = run_git(root, &["status", "--porcelain"])
                .map(|text| text.lines().count())
                .unwrap_or(0);
            let status = if dirty == 0 { Status::Pass } else { Status::Warn };
            let detail = if dirty == 0 {
                format!("branch {branch}, working tree clean")
            } else {
                format!("branch {branch}, {dirty} uncommitted path(s)")
            };
            checks.push(Check::new("git.repo", "Git repository", status, detail));
        }
        _ => checks.push(Check::new(
            "git.repo",
            "Git repository",
            Status::Skip,
            "not a git repository, or git is not installed".to_string(),
        )),
    }

    checks
}

/// Every command in a README fenced block must resolve to something real.
fn documented_commands_check(readme: &Path, context: &DoctorContext) -> Check {
    let text = std::fs::read_to_string(readme).unwrap_or_default();
    let commands = extract_commands(&text);
    if commands.is_empty() {
        return Check::new(
            "docs.commands",
            "Documented commands resolve",
            Status::Skip,
            "no commands found in README code blocks".to_string(),
        );
    }

    let mut problems = Vec::new();
    let mut checked = Vec::new();
    for command in &commands {
        let program = command.split_whitespace().next().unwrap_or_default();
        if program.is_empty() || program.starts_with('#') {
            continue;
        }
        checked.push(command.clone());
        if program == "zenx" {
            if let Some(subcommand) = command.split_whitespace().nth(1) {
                if !subcommand.starts_with('-')
                    && !context.known_commands.is_empty()
                    && !context.known_commands.iter().any(|c| c == subcommand)
                {
                    problems.push(format!("`{command}`: zenx has no `{subcommand}` command"));
                }
            }
            continue;
        }
        if !binary_available(program) {
            problems.push(format!("`{command}`: {program} is not on PATH"));
        }
    }

    if problems.is_empty() {
        let mut check = Check::new(
            "docs.commands",
            "Documented commands resolve",
            Status::Pass,
            format!("{} documented command(s) verified", checked.len()),
        );
        check.evidence = checked;
        check
    } else {
        let mut check = Check::new(
            "docs.commands",
            "Documented commands resolve",
            Status::Fail,
            format!("{} documented command(s) cannot run", problems.len()),
        );
        check.evidence = problems;
        check
    }
}

/// Pull command lines out of fenced code blocks, ignoring comments and prompts.
pub fn extract_commands(markdown: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_fence = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cleaned = trimmed.trim_start_matches("$ ").trim();
        if cleaned.is_empty() {
            continue;
        }
        commands.push(cleaned.to_string());
    }
    commands
}

fn detect_license(path: &Path) -> Option<&'static str> {
    let text = std::fs::read_to_string(path).ok()?;
    let lower = text.to_ascii_lowercase();
    if lower.contains("apache license") && lower.contains("version 2.0") {
        Some("Apache-2.0")
    } else if lower.contains("mit license") {
        Some("MIT")
    } else if lower.contains("gnu general public license") {
        Some("GPL")
    } else if lower.contains("bsd 3-clause") || lower.contains("redistribution and use in source") {
        Some("BSD-style")
    } else {
        None
    }
}

pub fn binary_available(program: &str) -> bool {
    if program.contains('/') || program.contains('\\') {
        return Path::new(program).is_file();
    }
    let result = std::process::Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    matches!(result, Ok(output) if output.status.success() || !output.stderr.is_empty())
}

fn run_git(root: &Path, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| ZenxError::External {
            command: format!("git {}", args.join(" ")),
            code: None,
            stderr: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(ZenxError::External {
            command: format!("git {}", args.join(" ")),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn find_first(root: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_commands_from_fences_only() {
        let markdown = "# Title\n\nRun this:\n\n```bash\n$ cargo build\nnpm test\n# a comment\n```\n\nNot a command.\n";
        let commands = extract_commands(markdown);
        assert_eq!(commands, vec!["cargo build".to_string(), "npm test".to_string()]);
    }

    #[test]
    fn license_detection_reads_the_text() {
        let dir = std::env::temp_dir().join(format!("zenx-license-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("LICENSE");
        std::fs::write(&path, "Apache License\nVersion 2.0, January 2004").unwrap();
        assert_eq!(detect_license(&path), Some("Apache-2.0"));
        std::fs::write(&path, "MIT License\n\nPermission is hereby granted").unwrap();
        assert_eq!(detect_license(&path), Some("MIT"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn diagnose_reports_missing_files() {
        let dir = std::env::temp_dir().join(format!("zenx-doctor-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let report = diagnose(&dir, &DoctorOptions::default()).unwrap();
        let readme = report.checks.iter().find(|c| c.id == "docs.readme").unwrap();
        assert_eq!(readme.status, Status::Fail);
        assert!(!report.fixes.is_empty(), "a README fix should be planned");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
