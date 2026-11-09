//! Replacing the running binary with a verified release artifact.

use std::path::{Path, PathBuf};

use serde::Serialize;
use zenx_core::{Result, ZenxError};
use zenx_security::TempDir;

use crate::install::{classify, current_exe, ensure_zenx_binary, InstallMethod};
use crate::release::{
    fetch, fetch_text, is_newer, parse_checksums, resolve_release, verify_archive,
};

#[derive(Debug, Clone, Default)]
pub struct UpdateOptions {
    /// Report whether an update exists without downloading it.
    pub check_only: bool,
    /// Install a specific release tag instead of the latest.
    pub version: Option<String>,
    /// Download and verify, but do not replace the binary.
    pub dry_run: bool,
    /// Install path. Defaults to the running executable; used by tests so they
    /// never overwrite the test binary itself.
    pub target: Option<PathBuf>,
    /// Download root override, for tests and CI only.
    pub base_url: Option<String>,
    /// Version of the running binary; the CLI passes its own so the report is
    /// accurate even though this crate versions independently.
    pub current_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    /// The installed version is already the latest.
    UpToDate,
    /// A newer release exists and `--check` was requested.
    Available,
    /// Downloaded and verified, but not installed (`--dry-run`).
    Verified,
    /// Installed.
    Installed,
    /// Installed by cargo; cargo must perform the update.
    Manual,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateReport {
    pub current_version: String,
    pub target_version: String,
    pub method: InstallMethod,
    pub action: UpdateAction,
    /// True only when a checksum was published and matched.
    pub verified: bool,
    pub checksum: Option<String>,
    pub backup: Option<String>,
    pub message: String,
}

fn sibling(exe: &Path, suffix: &str) -> PathBuf {
    let name = exe
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "zenx".to_string());
    exe.with_file_name(format!("{name}.{suffix}"))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| ZenxError::io(path, error))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| ZenxError::io(path, error))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Extract an archive with `tar`, which handles both `.tar.gz` and `.zip`
/// on the platforms zenX ships for (bsdtar on Windows 10+).
fn extract(archive: &Path, destination: &Path) -> Result<()> {
    let output = std::process::Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(destination)
        .output()
        .map_err(|error| ZenxError::External {
            command: "tar".to_string(),
            code: None,
            stderr: format!("tar is required to unpack the release: {error}"),
        })?;
    if !output.status.success() {
        return Err(ZenxError::External {
            command: format!("tar -xf {}", archive.display()),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

fn find_binary(directory: &Path, depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(directory).ok()?;
    let mut subdirectories = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirectories.push(path);
            continue;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if name == "zenx" || name == "zenx.exe" {
            return Some(path);
        }
    }
    for subdirectory in subdirectories {
        if let Some(found) = find_binary(&subdirectory, depth - 1) {
            return Some(found);
        }
    }
    None
}

/// Replace `exe` with `new_binary`, keeping the previous copy as `<name>.old`.
fn install_binary(exe: &Path, new_binary: &Path) -> Result<PathBuf> {
    ensure_zenx_binary(exe)?;
    if exe.parent().is_none() {
        return Err(ZenxError::Invalid(format!(
            "{} has no parent directory",
            exe.display()
        )));
    }
    let staged = sibling(exe, "new");
    std::fs::copy(new_binary, &staged).map_err(|error| ZenxError::io(&staged, error))?;
    make_executable(&staged)?;

    let backup = sibling(exe, "old");
    if exe.exists() {
        #[cfg(windows)]
        {
            // Windows refuses to overwrite or delete a running executable, but
            // it does allow renaming it, which frees the name for the new file.
            let _ = std::fs::remove_file(&backup);
            std::fs::rename(exe, &backup).map_err(|error| ZenxError::io(&backup, error))?;
        }
        #[cfg(not(windows))]
        {
            std::fs::copy(exe, &backup).map_err(|error| ZenxError::io(&backup, error))?;
        }
    }
    std::fs::rename(&staged, exe).map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        ZenxError::io(exe, error)
    })?;
    Ok(backup)
}

/// Download, verify and install a release.
pub fn update(options: &UpdateOptions) -> Result<UpdateReport> {
    let exe = match &options.target {
        Some(path) => path.clone(),
        None => current_exe()?,
    };
    ensure_zenx_binary(&exe)?;
    let method = classify(&exe)?;
    let current_version = options
        .current_version
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    if let InstallMethod::Cargo { .. } = method {
        return Ok(UpdateReport {
            current_version,
            target_version: options
                .version
                .clone()
                .unwrap_or_else(|| "latest".to_string()),
            method,
            action: UpdateAction::Manual,
            verified: false,
            checksum: None,
            backup: None,
            message: "zenX was installed with cargo; update it with `cargo install zenx --force` \
                      (or `cargo install --git https://github.com/sufiyan-sabeel/zenX zenx`)"
                .to_string(),
        });
    }

    let release = resolve_release(options.version.as_deref(), options.base_url.as_deref())?;
    let target_version = release.tag.trim_start_matches('v').to_string();

    if options.version.is_none() && !is_newer(&target_version, &current_version) {
        return Ok(UpdateReport {
            current_version: current_version.clone(),
            target_version: target_version.clone(),
            method,
            action: UpdateAction::UpToDate,
            verified: false,
            checksum: None,
            backup: None,
            message: format!("zenX {current_version} is the latest release"),
        });
    }

    if options.check_only {
        return Ok(UpdateReport {
            current_version: current_version.clone(),
            target_version: target_version.clone(),
            method,
            action: UpdateAction::Available,
            verified: false,
            checksum: None,
            backup: None,
            message: format!("zenX {target_version} is available (installed: {current_version})"),
        });
    }

    let asset = release
        .archive_url
        .rsplit('/')
        .next()
        .unwrap_or("archive")
        .to_string();
    let checksums_text = fetch_text(&release.checksums_url)?;
    let checksums = parse_checksums(&checksums_text);
    let archive = fetch(&release.archive_url)?;
    let checksum = verify_archive(&archive, &asset, &checksums)?;

    if options.dry_run {
        return Ok(UpdateReport {
            current_version,
            target_version,
            method,
            action: UpdateAction::Verified,
            verified: true,
            checksum: Some(checksum),
            backup: None,
            message: format!("{asset} downloaded and verified; nothing was installed"),
        });
    }

    let stage = TempDir::create(&std::env::temp_dir(), "zenx-update")?;
    let archive_path = stage.path().join(&asset);
    std::fs::write(&archive_path, &archive).map_err(|e| ZenxError::io(&archive_path, e))?;
    extract(&archive_path, stage.path())?;
    let new_binary = find_binary(stage.path(), 3).ok_or_else(|| {
        ZenxError::NotFound(format!(
            "the release archive {asset} does not contain a zenx binary"
        ))
    })?;

    let backup = install_binary(&exe, &new_binary)?;

    Ok(UpdateReport {
        current_version,
        target_version: target_version.clone(),
        method,
        action: UpdateAction::Installed,
        verified: true,
        checksum: Some(checksum),
        backup: Some(backup.display().to_string()),
        message: format!(
            "updated to {target_version}; the previous binary is kept at {}",
            backup.display()
        ),
    })
}

/// Restore the binary saved by the last update.
pub fn rollback() -> Result<PathBuf> {
    let exe = current_exe()?;
    ensure_zenx_binary(&exe)?;
    let backup = sibling(&exe, "old");
    if !backup.is_file() {
        return Err(ZenxError::NotFound(format!(
            "no previous binary at {}; nothing to roll back",
            backup.display()
        )));
    }
    #[cfg(windows)]
    {
        // The running executable cannot be replaced, so it is moved aside first
        // and left as `<name>.new` for the next update to reuse.
        let parked = sibling(&exe, "new");
        let _ = std::fs::remove_file(&parked);
        std::fs::rename(&exe, &parked).map_err(|error| ZenxError::io(&parked, error))?;
    }
    std::fs::rename(&backup, &exe).map_err(|error| ZenxError::io(&exe, error))?;
    Ok(exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("zenx-update-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn install_refuses_to_replace_a_non_zenx_file() {
        let root = fixture("guard");
        let target = root.join("not-zenx");
        std::fs::write(&target, b"keep me").unwrap();
        let new_binary = root.join("new");
        std::fs::write(&new_binary, b"new").unwrap();
        let error = install_binary(&target, &new_binary).unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rollback_without_a_backup_is_not_found() {
        let error = rollback();
        // Either there is no backup (NotFound) or the running binary is not
        // named zenx (Security); both are honest refusals.
        match error {
            Ok(_) => {}
            Err(error) => assert!(matches!(
                error,
                ZenxError::NotFound(_) | ZenxError::Security(_)
            )),
        }
    }

    #[cfg(unix)]
    #[test]
    fn update_installs_a_verified_release() {
        let root = fixture("install");
        let release_dir = root.join("release");
        let stage = root.join("stage");
        let package = stage.join("zenx-test");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("zenx"), b"#!/bin/sh\necho fake zenx\n").unwrap();

        std::fs::create_dir_all(&release_dir).unwrap();
        let triple = crate::release::target_triple().expect("prebuilt target");
        let asset = crate::release::archive_name(triple);
        let tar = std::process::Command::new("tar")
            .arg("-czf")
            .arg(release_dir.join(&asset))
            .arg("-C")
            .arg(&stage)
            .arg("zenx-test")
            .output()
            .expect("tar available");
        assert!(tar.status.success(), "tar failed: {:?}", tar);

        let bytes = std::fs::read(release_dir.join(&asset)).unwrap();
        let hash = zenx_security::sha256_hex(&bytes);
        std::fs::write(
            release_dir.join("SHA256SUMS.txt"),
            format!("{hash}  {asset}\n"),
        )
        .unwrap();

        let bin_dir = root.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let target = bin_dir.join("zenx");
        std::fs::write(&target, b"#!/bin/sh\necho old zenx\n").unwrap();

        let report = update(&UpdateOptions {
            version: Some("9.9.9".to_string()),
            target: Some(target.clone()),
            base_url: Some(format!("file://{}", release_dir.display())),
            ..Default::default()
        })
        .unwrap();

        assert_eq!(report.action, UpdateAction::Installed);
        assert!(report.verified);
        assert_eq!(report.target_version, "9.9.9");
        let installed = std::fs::read_to_string(&target).unwrap();
        assert!(installed.contains("fake zenx"), "installed: {installed}");
        assert!(bin_dir.join("zenx.old").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn update_refuses_a_mismatched_checksum_and_leaves_the_binary_alone() {
        let root = fixture("badsum");
        let release_dir = root.join("release");
        let stage = root.join("stage");
        let package = stage.join("zenx-test");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("zenx"), b"#!/bin/sh\necho fake\n").unwrap();
        std::fs::create_dir_all(&release_dir).unwrap();
        let triple = crate::release::target_triple().expect("prebuilt target");
        let asset = crate::release::archive_name(triple);
        let tar = std::process::Command::new("tar")
            .arg("-czf")
            .arg(release_dir.join(&asset))
            .arg("-C")
            .arg(&stage)
            .arg("zenx-test")
            .output()
            .expect("tar available");
        assert!(tar.status.success());

        std::fs::write(
            release_dir.join("SHA256SUMS.txt"),
            format!("{}  {asset}\n", "0".repeat(64)),
        )
        .unwrap();

        let bin_dir = root.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let target = bin_dir.join("zenx");
        std::fs::write(&target, b"original").unwrap();

        let error = update(&UpdateOptions {
            version: Some("9.9.9".to_string()),
            target: Some(target.clone()),
            base_url: Some(format!("file://{}", release_dir.display())),
            ..Default::default()
        })
        .unwrap_err();

        assert!(matches!(error, ZenxError::Security(_)));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "original");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn dry_run_verifies_without_installing() {
        let root = fixture("dryrun");
        let release_dir = root.join("release");
        let stage = root.join("stage");
        let package = stage.join("zenx-test");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("zenx"), b"#!/bin/sh\necho fake\n").unwrap();
        std::fs::create_dir_all(&release_dir).unwrap();
        let triple = crate::release::target_triple().expect("prebuilt target");
        let asset = crate::release::archive_name(triple);
        assert!(std::process::Command::new("tar")
            .arg("-czf")
            .arg(release_dir.join(&asset))
            .arg("-C")
            .arg(&stage)
            .arg("zenx-test")
            .output()
            .expect("tar available")
            .status
            .success());
        let bytes = std::fs::read(release_dir.join(&asset)).unwrap();
        std::fs::write(
            release_dir.join("SHA256SUMS.txt"),
            format!("{}  {asset}\n", zenx_security::sha256_hex(&bytes)),
        )
        .unwrap();

        let bin_dir = root.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let target = bin_dir.join("zenx");
        std::fs::write(&target, b"original").unwrap();

        let report = update(&UpdateOptions {
            version: Some("9.9.9".to_string()),
            dry_run: true,
            target: Some(target.clone()),
            base_url: Some(format!("file://{}", release_dir.display())),
            ..Default::default()
        })
        .unwrap();

        assert_eq!(report.action, UpdateAction::Verified);
        assert!(report.verified);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "original");
        let _ = std::fs::remove_dir_all(&root);
    }
}
