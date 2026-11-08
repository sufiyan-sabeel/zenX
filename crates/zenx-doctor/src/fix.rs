//! Reviewable repairs.
//!
//! A fix is planned from the report, printed for review, and applied only when
//! the caller explicitly asks. Nothing here overwrites an existing file.

use std::path::Path;

use serde::Serialize;
use zenx_core::paths::safe_join;
use zenx_core::{Result, ZenxError};

use crate::report::{DoctorReport, Status};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixAction {
    CreateFile { content: String },
    CreateDir,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fix {
    pub id: String,
    pub title: String,
    /// Repository-relative path. Never absolute, never containing `..`.
    pub path: String,
    pub reason: String,
    pub action: FixAction,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixOutcome {
    pub id: String,
    pub path: String,
    pub applied: bool,
    pub message: String,
}

/// Decide which repairs the failed checks justify.
pub fn plan_fixes(root: &Path, report: &DoctorReport) -> Vec<Fix> {
    let mut fixes = Vec::new();
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    for check in &report.checks {
        if check.status != Status::Fail && check.status != Status::Warn {
            continue;
        }
        match check.id.as_str() {
            "docs.readme" => fixes.push(Fix {
                id: "fix.readme".into(),
                title: "Create a README".into(),
                path: "README.md".into(),
                reason: check.detail.clone(),
                action: FixAction::CreateFile {
                    content: format!(
                        "# {name}\n\n<!-- Describe what this project does and how to run it. -->\n\n## Install\n\n```bash\n# installation command\n```\n\n## Usage\n\n```bash\n# usage command\n```\n"
                    ),
                },
            }),
            "repo.gitignore" => fixes.push(Fix {
                id: "fix.gitignore".into(),
                title: "Create a .gitignore".into(),
                path: ".gitignore".into(),
                reason: check.detail.clone(),
                action: FixAction::CreateFile {
                    content: "/target\n/node_modules\n__pycache__/\n*.pyc\n.env\n".into(),
                },
            }),
            "tests.present" => fixes.push(Fix {
                id: "fix.tests-dir".into(),
                title: "Create a tests directory".into(),
                path: "tests".into(),
                reason: check.detail.clone(),
                action: FixAction::CreateDir,
            }),
            _ => {}
        }
    }
    fixes
}

/// Apply (or, when `apply` is false, describe) the planned repairs.
pub fn apply_fixes(root: &Path, fixes: &[Fix], apply: bool) -> Result<Vec<FixOutcome>> {
    let mut outcomes = Vec::new();
    for fix in fixes {
        let target = safe_join(root, &fix.path)?;
        let exists = target.exists();
        let (applied, message) = if exists {
            (
                false,
                format!("skipped: {} already exists", fix.path),
            )
        } else if !apply {
            (false, format!("would create {}", fix.path))
        } else {
            match &fix.action {
                FixAction::CreateFile { content } => {
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| ZenxError::io(parent, e))?;
                    }
                    std::fs::write(&target, content).map_err(|e| ZenxError::io(&target, e))?;
                    (true, format!("created {}", fix.path))
                }
                FixAction::CreateDir => {
                    std::fs::create_dir_all(&target).map_err(|e| ZenxError::io(&target, e))?;
                    (true, format!("created {}/", fix.path))
                }
            }
        };
        outcomes.push(FixOutcome {
            id: fix.id.clone(),
            path: fix.path.clone(),
            applied,
            message,
        });
    }
    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("zenx-fix-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fix(path: &str, content: &str) -> Fix {
        Fix {
            id: "fix.test".into(),
            title: "test".into(),
            path: path.into(),
            reason: "test".into(),
            action: FixAction::CreateFile {
                content: content.into(),
            },
        }
    }

    #[test]
    fn dry_run_changes_nothing() {
        let dir = fixture("dry");
        let outcomes = apply_fixes(&dir, &[fix("README.md", "hi")], false).unwrap();
        assert!(!outcomes[0].applied);
        assert!(!dir.join("README.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_creates_files_but_never_overwrites() {
        let dir = fixture("apply");
        let outcomes = apply_fixes(&dir, &[fix("README.md", "new")], true).unwrap();
        assert!(outcomes[0].applied);
        assert_eq!(std::fs::read_to_string(dir.join("README.md")).unwrap(), "new");

        let outcomes = apply_fixes(&dir, &[fix("README.md", "replacement")], true).unwrap();
        assert!(!outcomes[0].applied);
        assert_eq!(std::fs::read_to_string(dir.join("README.md")).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn traversal_in_a_fix_path_is_rejected() {
        let dir = fixture("traversal");
        let error = apply_fixes(&dir, &[fix("../escape.md", "x")], true).unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
