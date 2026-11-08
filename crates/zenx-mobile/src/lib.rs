//! zenX Mobile — Android and Termux as first-class targets.
//!
//! Everything here is read-only unless the user asks for a backup or restore.
//! No command needs root, and no package manager is ever invoked: missing tools
//! are reported with the exact command the user could run themselves.

use std::path::{Path, PathBuf};

use serde::Serialize;
use zenx_core::paths::ZenxPaths;
use zenx_core::{Result, ZenxError};
use zenx_security::copy_dir;

/// A tool zenX cares about, and whether it is installed.
#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub name: &'static str,
    pub required: bool,
    pub found: bool,
    pub version: Option<String>,
    pub hint: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    pub os: String,
    pub arch: String,
    pub termux: bool,
    pub android: bool,
    pub termux_version: Option<String>,
    pub android_release: Option<String>,
    pub home: Option<String>,
    pub is_64bit: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub severity: &'static str,
    pub id: &'static str,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReport {
    pub environment: Environment,
    pub tools: Vec<ToolStatus>,
    pub issues: Vec<Issue>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupReport {
    pub path: String,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    pub source: String,
    pub restored: Vec<String>,
    pub skipped: Vec<String>,
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Detect the runtime environment without touching the network.
pub fn detect_environment() -> Environment {
    let termux_version = env_non_empty("TERMUX_VERSION");
    let prefix_is_termux = env_non_empty("PREFIX")
        .map(|prefix| prefix.contains("com.termux"))
        .unwrap_or(false);
    let termux = termux_version.is_some() || prefix_is_termux;
    let android = termux || Path::new("/system/build.prop").is_file();

    let android_release = if android {
        std::fs::read_to_string("/system/build.prop")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find(|line| line.starts_with("ro.build.version.release="))
                    .map(|line| {
                        line.trim_start_matches("ro.build.version.release=")
                            .to_string()
                    })
            })
    } else {
        None
    };

    let arch = std::env::consts::ARCH.to_string();
    Environment {
        os: std::env::consts::OS.to_string(),
        is_64bit: arch.contains("64"),
        arch,
        termux,
        android,
        termux_version,
        android_release,
        home: env_non_empty("HOME").or_else(|| env_non_empty("USERPROFILE")),
    }
}

/// Tools zenX checks for, with the Termux package that provides each.
const TOOLS: &[(&str, bool, &str)] = &[
    ("git", true, "pkg install git"),
    ("rustc", false, "pkg install rust"),
    ("cargo", false, "pkg install rust"),
    ("python3", false, "pkg install python"),
    ("node", false, "pkg install nodejs"),
    ("npm", false, "pkg install nodejs"),
    ("clang", false, "pkg install clang"),
    ("make", false, "pkg install make"),
    ("curl", false, "pkg install curl"),
];

fn probe(name: &str) -> Option<String> {
    let output = std::process::Command::new(name)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty());
    match first {
        Some(line) => Some(line),
        None => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            stderr
                .lines()
                .next()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
        }
    }
}

pub fn detect_tools() -> Vec<ToolStatus> {
    TOOLS
        .iter()
        .map(|&(name, required, hint)| {
            let version = probe(name);
            ToolStatus {
                name,
                required,
                found: version.is_some(),
                version,
                hint,
            }
        })
        .collect()
}

/// Full environment diagnosis with actionable issues.
pub fn diagnose() -> DiagnosticReport {
    let environment = detect_environment();
    let tools = detect_tools();
    let mut issues = Vec::new();

    for tool in &tools {
        if tool.found {
            continue;
        }
        if tool.required {
            issues.push(Issue {
                severity: "error",
                id: "tool.required-missing",
                message: format!("{} is required but was not found on PATH", tool.name),
                hint: if environment.termux {
                    tool.hint.to_string()
                } else {
                    format!("install {} using your platform package manager", tool.name)
                },
            });
        } else {
            issues.push(Issue {
                severity: "info",
                id: "tool.optional-missing",
                message: format!("{} is not installed", tool.name),
                hint: if environment.termux {
                    tool.hint.to_string()
                } else {
                    format!("install {} if you need that toolchain", tool.name)
                },
            });
        }
    }

    if environment.termux && !environment.is_64bit {
        issues.push(Issue {
            severity: "warn",
            id: "arch.32bit",
            message: format!("Termux is running on a {} build", environment.arch),
            hint: "many Rust crates need a 64-bit build; the Play Store Termux is 32-bit, use the F-Droid or GitHub release".into(),
        });
    }

    let mut notes = Vec::new();
    if environment.termux {
        notes.push(
            "Termux needs clang to build zenX because SQLite is compiled from source by the bundled rusqlite feature.".to_string(),
        );
        notes.push(
            "No command requires root; zenX never calls pkg or apt on your behalf.".to_string(),
        );
    }
    if environment.android && !environment.termux {
        notes.push("Android detected without Termux: most zenX commands expect a POSIX shell and will not work here.".to_string());
    }
    if environment.os == "windows" {
        notes.push("On Windows, skill targets and repository paths use backslashes; zenX normalises them for display only.".to_string());
    }

    DiagnosticReport {
        environment,
        tools,
        issues,
        notes,
    }
}

fn stamp() -> String {
    zenx_core::util::now_rfc3339().replace([':', '-'], "")
}

#[derive(Serialize, serde::Deserialize)]
struct BackupManifest {
    created_at: String,
    entries: Vec<String>,
    zenx_version: String,
}

/// Copy configuration, database and installed skills into `destination`.
pub fn backup(paths: &ZenxPaths, destination: &Path) -> Result<BackupReport> {
    paths.ensure()?;
    std::fs::create_dir_all(destination).map_err(|e| ZenxError::io(destination, e))?;
    let backup_dir = destination.join(format!("zenx-backup-{}", stamp()));
    std::fs::create_dir_all(&backup_dir).map_err(|e| ZenxError::io(&backup_dir, e))?;

    let mut entries = Vec::new();
    for (label, source) in [
        ("config.toml", paths.config_file()),
        ("zenx.db", paths.database_file()),
    ] {
        if source.is_file() {
            let target = backup_dir.join(label);
            std::fs::copy(&source, &target).map_err(|e| ZenxError::io(&target, e))?;
            entries.push(label.to_string());
        }
    }

    if paths.skills_dir().is_dir() {
        let target = backup_dir.join("skills");
        // Create the destination explicitly: copy_dir writes nothing for an
        // empty tree, and a manifest entry must never describe a path the
        // backup does not actually contain.
        std::fs::create_dir_all(&target).map_err(|e| ZenxError::io(&target, e))?;
        copy_dir(&paths.skills_dir(), &target)?;
        entries.push("skills/".to_string());
    }

    let manifest = BackupManifest {
        created_at: zenx_core::util::now_rfc3339(),
        entries: entries.clone(),
        zenx_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let manifest_path = backup_dir.join("manifest.json");
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|e| ZenxError::Invalid(format!("could not serialise backup manifest: {e}")))?;
    std::fs::write(&manifest_path, text).map_err(|e| ZenxError::io(&manifest_path, e))?;

    Ok(BackupReport {
        path: backup_dir.display().to_string(),
        entries,
    })
}

/// Restore a backup created by [`backup`].
///
/// Refuses to overwrite existing state unless `overwrite` is set: restoring over
/// a live database is destructive and must be an explicit decision.
pub fn restore(paths: &ZenxPaths, source: &Path, overwrite: bool) -> Result<RestoreReport> {
    if !source.is_dir() {
        return Err(ZenxError::NotFound(format!(
            "backup directory {}",
            source.display()
        )));
    }
    let manifest_path = source.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(ZenxError::Invalid(format!(
            "{} has no manifest.json; this is not a zenX backup",
            source.display()
        )));
    }
    let text =
        std::fs::read_to_string(&manifest_path).map_err(|e| ZenxError::io(&manifest_path, e))?;
    let manifest: BackupManifest = serde_json::from_str(&text)
        .map_err(|e| ZenxError::Invalid(format!("{}: {e}", manifest_path.display())))?;

    // Deliberately no ensure(): creating the skills directory here would make
    // a restore report it as "already exists" and skip it.
    std::fs::create_dir_all(paths.root()).map_err(|e| ZenxError::io(paths.root(), e))?;
    let mut restored = Vec::new();
    let mut skipped = Vec::new();

    for entry in &manifest.entries {
        match entry.as_str() {
            "config.toml" | "zenx.db" => {
                let from = source.join(entry);
                let to = if entry == "config.toml" {
                    paths.config_file()
                } else {
                    paths.database_file()
                };
                if !from.is_file() {
                    skipped.push(format!("{entry} (missing in backup)"));
                    continue;
                }
                if to.exists() && !overwrite {
                    skipped.push(format!("{entry} (already exists, pass --overwrite)"));
                    continue;
                }
                std::fs::copy(&from, &to).map_err(|e| ZenxError::io(&to, e))?;
                restored.push(entry.clone());
            }
            "skills/" => {
                let from = source.join("skills");
                let to = paths.skills_dir();
                if !from.is_dir() {
                    skipped.push("skills/ (missing in backup)".to_string());
                    continue;
                }
                if to.exists() && !overwrite {
                    skipped.push("skills/ (already exists, pass --overwrite)".to_string());
                    continue;
                }
                copy_dir(&from, &to)?;
                restored.push("skills/".to_string());
            }
            other => skipped.push(format!("{other} (unknown entry)")),
        }
    }

    Ok(RestoreReport {
        source: source.display().to_string(),
        restored,
        skipped,
    })
}

/// Where a backup would be written by default.
pub fn default_backup_dir(paths: &ZenxPaths) -> PathBuf {
    paths.backups_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> (PathBuf, ZenxPaths) {
        let root = std::env::temp_dir().join(format!("zenx-mobile-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let paths = ZenxPaths::new(root.join("home"));
        paths.ensure().unwrap();
        (root, paths)
    }

    #[test]
    fn environment_detection_never_panics() {
        let environment = detect_environment();
        assert!(!environment.os.is_empty());
        assert!(!environment.arch.is_empty());
    }

    #[test]
    fn tool_probe_reports_known_and_unknown_tools() {
        let tools = detect_tools();
        assert_eq!(tools.len(), TOOLS.len());
        // A tool that cannot exist is reported as missing, not as an error.
        let status = probe("zenx-definitely-not-installed");
        assert!(status.is_none());
    }

    #[test]
    fn backup_then_restore_round_trips() {
        let (root, paths) = fixture("roundtrip");
        std::fs::write(paths.config_file(), "version = 1\n").unwrap();
        std::fs::create_dir_all(paths.skills_dir().join("demo")).unwrap();
        std::fs::write(paths.skills_dir().join("demo/SKILL.md"), "x").unwrap();

        let report = backup(&paths, &root.join("backups")).unwrap();
        assert_eq!(
            report.entries,
            vec!["config.toml".to_string(), "skills/".to_string()]
        );

        // Wipe and restore.
        std::fs::remove_file(paths.config_file()).unwrap();
        std::fs::remove_dir_all(paths.skills_dir()).unwrap();
        let restored = restore(&paths, Path::new(&report.path), false).unwrap();
        assert_eq!(restored.restored.len(), 2);
        assert!(paths.config_file().is_file());
        assert!(paths.skills_dir().join("demo/SKILL.md").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_refuses_to_overwrite_without_consent() {
        let (root, paths) = fixture("overwrite");
        std::fs::write(paths.config_file(), "version = 1\n").unwrap();
        let report = backup(&paths, &root.join("backups")).unwrap();
        let restored = restore(&paths, Path::new(&report.path), false).unwrap();
        assert!(restored.restored.is_empty());
        assert!(!restored.skipped.is_empty());
        let restored = restore(&paths, Path::new(&report.path), true).unwrap();
        assert!(
            restored.restored.contains(&"config.toml".to_string()),
            "config.toml was not restored: {restored:?}"
        );
        assert!(
            restored.skipped.is_empty(),
            "nothing should be skipped with overwrite enabled, but: {:?}",
            restored.skipped
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_rejects_non_backup_directories() {
        let (root, paths) = fixture("invalid");
        let empty = root.join("not-a-backup");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(restore(&paths, &empty, true).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
