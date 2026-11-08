//! Where the running binary lives, and how it was installed.

use std::path::{Path, PathBuf};

use serde::Serialize;
use zenx_core::{Result, ZenxError};

/// How this copy of zenX was installed, inferred from the executable path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum InstallMethod {
    /// A `cargo install` binary; cargo owns it, so cargo must replace it.
    Cargo { bin_dir: String },
    /// A release archive installed by the shell or PowerShell installer.
    Binary { path: String },
}

pub fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| ZenxError::io("<current executable>", e))
}

pub fn detect_method() -> Result<InstallMethod> {
    classify(&current_exe()?)
}

/// Classify an executable path. Cargo's bin directory is `~/.cargo/bin`.
pub fn classify(exe: &Path) -> Result<InstallMethod> {
    let text = exe.to_string_lossy().replace('\\', "/");
    if text.contains("/.cargo/bin/") {
        let bin_dir = exe
            .parent()
            .map(|parent| parent.display().to_string())
            .unwrap_or_default();
        return Ok(InstallMethod::Cargo { bin_dir });
    }
    Ok(InstallMethod::Binary {
        path: exe.display().to_string(),
    })
}

/// Refuse to touch anything that is not unmistakably the zenX executable.
///
/// Every destructive path in this crate funnels through here, so a wrong path
/// can never delete an unrelated program.
pub fn ensure_zenx_binary(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name != "zenx" && name != "zenx.exe" {
        return Err(ZenxError::Security(format!(
            "refusing to modify {}: the file is not named zenx",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_install_is_detected() {
        let method = classify(Path::new("/home/you/.cargo/bin/zenx")).unwrap();
        match method {
            InstallMethod::Cargo { bin_dir } => assert!(bin_dir.ends_with(".cargo/bin")),
            other => panic!("expected cargo, got {other:?}"),
        }
    }

    #[test]
    fn release_binary_is_detected() {
        let method = classify(Path::new("/usr/local/bin/zenx")).unwrap();
        assert!(matches!(method, InstallMethod::Binary { .. }));
    }

    #[test]
    fn windows_cargo_path_is_detected() {
        let method = classify(Path::new(r"C:\Users\you\.cargo\bin\zenx.exe")).unwrap();
        assert!(matches!(method, InstallMethod::Cargo { .. }));
    }

    #[test]
    fn only_zenx_executables_are_accepted() {
        assert!(ensure_zenx_binary(Path::new("/usr/local/bin/zenx")).is_ok());
        assert!(ensure_zenx_binary(Path::new("zenx")).is_ok());
        // Path::file_name only treats the platform separator as a separator, so
        // each platform is checked with a path it would actually produce.
        #[cfg(windows)]
        assert!(ensure_zenx_binary(Path::new(r"C:\tools\zenx.exe")).is_ok());
        #[cfg(not(windows))]
        assert!(ensure_zenx_binary(Path::new("/opt/zenx/bin/zenx.exe")).is_ok());
        let error = ensure_zenx_binary(Path::new("/usr/bin/rm")).unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));
        assert_eq!(error.exit_code(), 3);
        assert!(ensure_zenx_binary(Path::new("/tmp/zenx-old")).is_err());
    }
}
