//! Removing the installed binary, and only that.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::Serialize;
use zenx_core::{Result, ZenxError};

use crate::install::{classify, current_exe, ensure_zenx_binary, InstallMethod};

#[derive(Debug, Clone, Default)]
pub struct UninstallOptions {
    /// Perform the removal. Without it, uninstall only reports the plan.
    pub confirm: bool,
    /// Also delete the zenX home directory (configuration, database, skills).
    pub purge_data: bool,
    /// Data directory override, for tests.
    pub data_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallReport {
    pub method: InstallMethod,
    pub binary: String,
    pub binary_removed: bool,
    pub data_root: Option<String>,
    pub data_removed: bool,
    pub notes: Vec<String>,
}

/// Refuse to delete anything that is not clearly a zenX home directory.
///
/// A data purge is the one irreversible thing this crate can do, so the check
/// is deliberately strict: the directory must be named `.zenx` or contain a
/// zenX database or config file, and it must not be a filesystem root.
fn guard_data_root(root: &Path) -> Result<()> {
    if root.parent().is_none() || root == Path::new("/") {
        return Err(ZenxError::Security(format!(
            "refusing to delete {}: it is a filesystem root",
            root.display()
        )));
    }
    let named_like_home = root
        .file_name()
        .map(|name| name == OsStr::new(".zenx"))
        .unwrap_or(false);
    let contains_state = root.join("zenx.db").is_file() || root.join("config.toml").is_file();
    if !named_like_home && !contains_state {
        return Err(ZenxError::Security(format!(
            "refusing to delete {}: it does not look like a zenX home directory",
            root.display()
        )));
    }
    Ok(())
}

pub fn uninstall(options: &UninstallOptions) -> Result<UninstallReport> {
    let exe = current_exe()?;
    ensure_zenx_binary(&exe)?;
    let method = classify(&exe)?;
    let mut notes = Vec::new();
    let mut binary_removed = false;

    match &method {
        InstallMethod::Cargo { bin_dir } => notes.push(format!(
            "zenX was installed with cargo, so cargo owns the binary in {bin_dir}; remove it with `cargo uninstall zenx`"
        )),
        InstallMethod::Binary { .. } => {
            if options.confirm {
                std::fs::remove_file(&exe).map_err(|error| ZenxError::io(&exe, error))?;
                binary_removed = true;
                notes.push(format!("removed {}", exe.display()));
            } else {
                notes.push(format!(
                    "would remove {} (pass --yes to do it)",
                    exe.display()
                ));
            }
        }
    }

    let data_root = options.data_root.clone().or_else(|| {
        zenx_core::paths::ZenxPaths::resolve()
            .ok()
            .map(|paths| paths.root().to_path_buf())
    });
    let mut data_removed = false;
    if let Some(root) = &data_root {
        if options.purge_data {
            guard_data_root(root)?;
            if options.confirm {
                std::fs::remove_dir_all(root).map_err(|error| ZenxError::io(root, error))?;
                data_removed = true;
                notes.push(format!("deleted {}", root.display()));
            } else {
                notes.push(format!(
                    "would delete {} (pass --yes to do it)",
                    root.display()
                ));
            }
        } else if root.exists() {
            notes.push(format!(
                "configuration and data stay in {}; delete them with --purge-data --yes",
                root.display()
            ));
        }
    }

    Ok(UninstallReport {
        method,
        binary: exe.display().to_string(),
        binary_removed,
        data_root: data_root.map(|root| root.display().to_string()),
        data_removed,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("zenx-uninstall-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn data_guard_accepts_a_real_home_and_rejects_the_rest() {
        let root = fixture("guard");
        let home = root.join(".zenx");
        std::fs::create_dir_all(&home).unwrap();
        assert!(guard_data_root(&home).is_ok());

        let with_db = root.join("custom");
        std::fs::create_dir_all(&with_db).unwrap();
        std::fs::write(with_db.join("zenx.db"), b"").unwrap();
        assert!(guard_data_root(&with_db).is_ok());

        let unrelated = root.join("documents");
        std::fs::create_dir_all(&unrelated).unwrap();
        let error = guard_data_root(&unrelated).unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));

        assert!(guard_data_root(Path::new("/")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn uninstall_reports_before_it_deletes() {
        // Runs against the test binary, which is not named zenx, so the guard
        // must refuse; that refusal is the behaviour worth pinning down.
        let error = uninstall(&UninstallOptions {
            confirm: false,
            purge_data: false,
            data_root: None,
        })
        .unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));
    }
}
