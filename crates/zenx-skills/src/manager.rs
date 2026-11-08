use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rusqlite::params;
use walkdir::WalkDir;
use zenx_core::config::SkillTarget;
use zenx_core::paths::ZenxPaths;
use zenx_core::{Result, ZenxError};
use zenx_security::{copy_dir, list_files, sha256_file, TempDir};
use zenx_storage::{map_err, Db};

use crate::git;
use crate::manifest::{Manifest, MANIFEST_FILE};
use crate::meta::{parse_skill_markdown, SkillMeta};

/// Where a skill comes from.
#[derive(Debug, Clone)]
pub enum SkillSource {
    Git { url: String, rev: Option<String> },
    Local { path: PathBuf },
}

impl SkillSource {
    /// Accept a git URL, an `owner/repo` shorthand, or a local directory.
    pub fn parse(raw: &str) -> SkillSource {
        let looks_git = raw.starts_with("http://")
            || raw.starts_with("https://")
            || raw.starts_with("git@")
            || raw.starts_with("ssh://")
            || raw.ends_with(".git");
        if looks_git {
            SkillSource::Git {
                url: raw.to_string(),
                rev: None,
            }
        } else {
            SkillSource::Local {
                path: PathBuf::from(raw),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub source: SkillSource,
    /// Override the name declared in SKILL.md.
    pub name: Option<String>,
    pub dry_run: bool,
    /// Replace an existing installation.
    pub force: bool,
    /// Explicit consent for network operations.
    pub allow_network: bool,
    pub targets: Vec<SkillTarget>,
}

impl InstallRequest {
    pub fn new(source: SkillSource) -> Self {
        InstallRequest {
            source,
            name: None,
            dry_run: false,
            force: false,
            allow_network: false,
            targets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallReport {
    pub name: String,
    pub description: String,
    pub source: String,
    pub commit: Option<String>,
    pub version: Option<String>,
    pub checksum_sha256: String,
    pub files: usize,
    pub dry_run: bool,
    pub path: String,
    pub replaced_backup: Option<String>,
    pub linked_targets: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifyReport {
    pub name: String,
    pub ok: bool,
    pub expected_checksum: String,
    pub actual_checksum: String,
    pub problems: Vec<String>,
    pub missing_files: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateReport {
    pub name: String,
    pub changed: bool,
    pub old_commit: Option<String>,
    pub new_commit: Option<String>,
    pub old_checksum: String,
    pub new_checksum: String,
    pub backup: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RemoveReport {
    pub name: String,
    pub removed: bool,
    pub backup_path: String,
}

fn short(checksum: &str) -> &str {
    checksum.get(..12).unwrap_or(checksum)
}

/// Reject names that could escape the store or collide with the manifest.
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 100 {
        return Err(ZenxError::Invalid(
            "skill name must be 1-100 characters".into(),
        ));
    }
    if name.starts_with('.') {
        return Err(ZenxError::Invalid(format!(
            "skill name must not start with a dot: {name:?}"
        )));
    }
    let allowed = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !allowed {
        return Err(ZenxError::Security(format!(
            "skill name contains characters that are not allowed: {name:?}"
        )));
    }
    Ok(())
}

/// Copy a skill tree, skipping `.git` so a clone does not carry its history
/// into the store.
fn copy_skill_tree(source: &Path, destination: &Path) -> Result<u64> {
    std::fs::create_dir_all(destination).map_err(|e| ZenxError::io(destination, e))?;
    let mut copied = 0u64;
    let walker = WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != OsStr::new(".git"));
    for entry in walker {
        let entry = entry.map_err(|e| ZenxError::Invalid(format!("walk {}: {e}", source.display())))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|e| ZenxError::Invalid(e.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| ZenxError::io(&target, e))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ZenxError::io(parent, e))?;
            }
            std::fs::copy(entry.path(), &target).map_err(|e| ZenxError::io(&target, e))?;
            copied += 1;
        }
    }
    Ok(copied)
}

/// Locate the directory that holds `SKILL.md`.
fn find_skill_root(directory: &Path) -> Result<PathBuf> {
    if directory.join("SKILL.md").is_file() {
        return Ok(directory.to_path_buf());
    }
    let mut candidates = Vec::new();
    let entries = std::fs::read_dir(directory).map_err(|e| ZenxError::io(directory, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| ZenxError::io(directory, e))?;
        if entry.file_name() == OsStr::new(".git") {
            continue;
        }
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").is_file() {
            candidates.push(path);
        }
    }
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(ZenxError::Invalid(format!(
            "no SKILL.md in {} or its immediate subdirectories",
            directory.display()
        ))),
        _ => {
            let list = candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(ZenxError::Invalid(format!(
                "several skills found ({list}); point zenx at the specific skill directory"
            )))
        }
    }
}

fn read_meta(root: &Path) -> Result<SkillMeta> {
    let path = root.join("SKILL.md");
    let text = std::fs::read_to_string(&path).map_err(|e| ZenxError::io(&path, e))?;
    parse_skill_markdown(&text)
}

/// Installed skills and their manifests.
pub struct SkillsManager<'a> {
    db: &'a Db,
    paths: &'a ZenxPaths,
}

impl<'a> SkillsManager<'a> {
    pub fn new(db: &'a Db, paths: &'a ZenxPaths) -> Self {
        SkillsManager { db, paths }
    }

    pub fn store(&self) -> PathBuf {
        self.paths.skills_dir()
    }

    fn backup_root(&self) -> PathBuf {
        self.paths.backups_dir().join("skills")
    }

    pub fn list(&self) -> Result<Vec<Manifest>> {
        let mut statement = self
            .db
            .conn()
            .prepare("SELECT manifest FROM skills ORDER BY name ASC")
            .map_err(map_err)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(map_err)?;
        let mut manifests = Vec::new();
        for row in rows {
            let text = row.map_err(map_err)?;
            if let Ok(manifest) = serde_json::from_str::<Manifest>(&text) {
                manifests.push(manifest);
            }
        }
        Ok(manifests)
    }

    /// Search installed skills by name or description.
    ///
    /// zenX has no remote registry, so this searches what is on disk. Nothing
    /// is queried over the network.
    pub fn search(&self, query: &str) -> Result<Vec<Manifest>> {
        let needle = query.to_ascii_lowercase();
        let mut matches: Vec<Manifest> = self
            .list()?
            .into_iter()
            .filter(|manifest| {
                manifest.name.to_ascii_lowercase().contains(&needle)
                    || manifest.description.to_ascii_lowercase().contains(&needle)
            })
            .collect();
        matches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(matches)
    }

    pub fn install(&self, request: &InstallRequest) -> Result<InstallReport> {
        self.paths.ensure()?;
        std::fs::create_dir_all(self.store()).map_err(|e| ZenxError::io(self.store(), e))?;

        let stage = TempDir::create(&self.paths.tmp_dir(), "skill-install")?;
        let (root, source_label, commit) = match &request.source {
            SkillSource::Git { url, rev } => {
                if !request.allow_network {
                    return Err(ZenxError::Security(format!(
                        "network access is disabled for skill installs; set skills.allow_network = true or pass --allow-network to fetch {url}"
                    )));
                }
                if !git::is_available() {
                    return Err(ZenxError::External {
                        command: "git --version".into(),
                        code: None,
                        stderr: "git is not installed or not on PATH".into(),
                    });
                }
                let destination = stage.path().join("repo");
                git::clone_shallow(url, &destination, rev.as_deref())?;
                let commit = git::head_commit(&destination).ok();
                let root = find_skill_root(&destination)?;
                (root, format!("git:{url}"), commit)
            }
            SkillSource::Local { path } => {
                let canonical = std::fs::canonicalize(path).map_err(|e| ZenxError::io(path, e))?;
                if !canonical.is_dir() {
                    return Err(ZenxError::Invalid(format!(
                        "{} is not a directory",
                        canonical.display()
                    )));
                }
                let root = find_skill_root(&canonical)?;
                (root, format!("local:{}", canonical.display()), None)
            }
        };

        let meta = read_meta(&root)?;
        let name = request.name.clone().unwrap_or_else(|| meta.name.clone());
        validate_name(&name)?;
        let checksum = sha256_file(&root.join("SKILL.md"))?;
        let files: Vec<String> = list_files(&root)?
            .into_iter()
            .filter(|path| path.as_str() != MANIFEST_FILE)
            .collect();
        let target = self.store().join(&name);
        let existing = target.is_dir();

        if existing && !request.force && !request.dry_run {
            return Err(ZenxError::Invalid(format!(
                "skill {name} is already installed at {}; pass --force to replace it",
                target.display()
            )));
        }

        if request.dry_run {
            return Ok(InstallReport {
                name: name.clone(),
                description: meta.description.clone(),
                source: source_label,
                commit,
                version: meta.version.clone(),
                checksum_sha256: checksum,
                files: files.len(),
                dry_run: true,
                path: target.display().to_string(),
                replaced_backup: None,
                linked_targets: Vec::new(),
            });
        }

        let replaced_backup = if existing {
            let backup = self.backup(&target, &name, "install-replace")?;
            std::fs::remove_dir_all(&target).map_err(|e| ZenxError::io(&target, e))?;
            Some(backup.display().to_string())
        } else {
            None
        };

        copy_skill_tree(&root, &target)?;

        let mut manifest = Manifest::build(&meta, &source_label, commit, checksum, files);
        for target_config in &request.targets {
            self.link_into(target_config, &name, &target, request.force)?;
            manifest.linked_targets.push(target_config.name.clone());
        }
        manifest.write(&target)?;
        self.record(&manifest)?;

        Ok(InstallReport {
            name,
            description: manifest.description.clone(),
            source: manifest.source.clone(),
            commit: manifest.commit.clone(),
            version: manifest.version.clone(),
            checksum_sha256: manifest.checksum_sha256.clone(),
            files: manifest.files.len(),
            dry_run: false,
            path: target.display().to_string(),
            replaced_backup,
            linked_targets: manifest.linked_targets.clone(),
        })
    }

    /// Copy an installed skill into an agent-specific directory.
    fn link_into(
        &self,
        target_config: &SkillTarget,
        name: &str,
        skill_dir: &Path,
        force: bool,
    ) -> Result<()> {
        if !target_config.path.is_absolute() {
            return Err(ZenxError::Config(format!(
                "skills target {:?} must use an absolute path",
                target_config.name
            )));
        }
        std::fs::create_dir_all(&target_config.path)
            .map_err(|e| ZenxError::io(&target_config.path, e))?;
        match target_config.layout.as_str() {
            "skill-dir" => {
                let destination = target_config.path.join(name);
                if destination.exists() {
                    let managed = destination.join(MANIFEST_FILE).is_file();
                    if !managed && !force {
                        return Err(ZenxError::Security(format!(
                            "{} exists and was not installed by zenX; refusing to overwrite it",
                            destination.display()
                        )));
                    }
                    std::fs::remove_dir_all(&destination)
                        .map_err(|e| ZenxError::io(&destination, e))?;
                }
                copy_skill_tree(skill_dir, &destination)?;
            }
            "flat" => {
                let destination = target_config.path.join(format!("{name}.md"));
                if destination.exists() && !force {
                    return Err(ZenxError::Security(format!(
                        "{} already exists; pass --force to overwrite it",
                        destination.display()
                    )));
                }
                std::fs::copy(skill_dir.join("SKILL.md"), &destination)
                    .map_err(|e| ZenxError::io(&destination, e))?;
            }
            other => {
                return Err(ZenxError::Config(format!(
                    "unsupported skills target layout {other:?}"
                )))
            }
        }
        Ok(())
    }

    pub fn verify(&self, name: &str) -> Result<VerifyReport> {
        let directory = self.store().join(name);
        if !directory.is_dir() {
            return Err(ZenxError::NotFound(format!(
                "skill {name} is not installed at {}",
                directory.display()
            )));
        }
        let manifest = Manifest::read(&directory)?;
        let actual = Manifest::current_checksum(&directory)?;
        let missing_files: Vec<String> = manifest
            .files
            .iter()
            .filter(|file| !directory.join(file).is_file())
            .cloned()
            .collect();

        let mut problems = Vec::new();
        if actual != manifest.checksum_sha256 {
            problems.push(format!(
                "SKILL.md changed since install (manifest {}, on disk {})",
                short(&manifest.checksum_sha256),
                short(&actual)
            ));
        }
        if !missing_files.is_empty() {
            problems.push(format!(
                "{} file(s) listed in the manifest are missing",
                missing_files.len()
            ));
        }

        Ok(VerifyReport {
            name: manifest.name.clone(),
            ok: problems.is_empty(),
            expected_checksum: manifest.checksum_sha256.clone(),
            actual_checksum: actual,
            problems,
            missing_files,
        })
    }

    pub fn update(&self, name: &str) -> Result<UpdateReport> {
        let directory = self.store().join(name);
        let manifest = Manifest::read(&directory)?;
        let url = manifest.source.strip_prefix("git:").ok_or_else(|| {
            ZenxError::Invalid(format!(
                "skill {name} was installed from {} and has no remote to update from",
                manifest.source
            ))
        })?;

        if !git::is_available() {
            return Err(ZenxError::External {
                command: "git --version".into(),
                code: None,
                stderr: "git is not installed or not on PATH".into(),
            });
        }

        let stage = TempDir::create(&self.paths.tmp_dir(), "skill-update")?;
        let destination = stage.path().join("repo");
        git::clone_shallow(url, &destination, None)?;
        let root = find_skill_root(&destination)?;
        let new_checksum = sha256_file(&root.join("SKILL.md"))?;
        let new_commit = git::head_commit(&destination).ok();

        if new_checksum == manifest.checksum_sha256 {
            return Ok(UpdateReport {
                name: manifest.name.clone(),
                changed: false,
                old_commit: manifest.commit.clone(),
                new_commit,
                old_checksum: manifest.checksum_sha256.clone(),
                new_checksum,
                backup: None,
            });
        }

        let meta = read_meta(&root)?;
        let backup = self.backup(&directory, name, "update")?;
        std::fs::remove_dir_all(&directory).map_err(|e| ZenxError::io(&directory, e))?;
        copy_skill_tree(&root, &directory)?;

        let files: Vec<String> = list_files(&directory)?
            .into_iter()
            .filter(|path| path.as_str() != MANIFEST_FILE)
            .collect();
        let mut updated = Manifest::build(&meta, &manifest.source, new_commit.clone(), new_checksum.clone(), files);
        updated.linked_targets = manifest.linked_targets.clone();
        updated.write(&directory)?;
        self.record(&updated)?;

        Ok(UpdateReport {
            name: manifest.name.clone(),
            changed: true,
            old_commit: manifest.commit.clone(),
            new_commit,
            old_checksum: manifest.checksum_sha256.clone(),
            new_checksum,
            backup: Some(backup.display().to_string()),
        })
    }

    /// Restore the most recent backup of a skill.
    pub fn rollback(&self, name: &str) -> Result<Manifest> {
        let recorded: Option<String> = self
            .db
            .conn()
            .query_row(
                "SELECT path FROM skill_backups WHERE name = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
                params![name],
                |row| row.get(0),
            )
            .ok();

        let backup_path = recorded.ok_or_else(|| {
            ZenxError::NotFound(format!("no backup has been recorded for skill {name}"))
        })?;
        let backup_directory = PathBuf::from(backup_path);
        if !backup_directory.is_dir() {
            return Err(ZenxError::NotFound(format!(
                "recorded backup {} no longer exists",
                backup_directory.display()
            )));
        }
        let restored = Manifest::read(&backup_directory)?;

        let target = self.store().join(name);
        if target.exists() {
            self.backup(&target, name, "rollback-current")?;
            std::fs::remove_dir_all(&target).map_err(|e| ZenxError::io(&target, e))?;
        }
        copy_dir(&backup_directory, &target)?;
        self.record(&restored)?;
        Ok(restored)
    }

    /// Remove a skill, keeping a backup first.
    pub fn remove(&self, name: &str) -> Result<RemoveReport> {
        let target = self.store().join(name);
        if !target.is_dir() {
            return Err(ZenxError::NotFound(format!("skill {name} is not installed")));
        }
        if !target.join(MANIFEST_FILE).is_file() {
            return Err(ZenxError::Security(format!(
                "{} was not installed by zenX; refusing to delete it",
                target.display()
            )));
        }
        let backup = self.backup(&target, name, "remove")?;
        std::fs::remove_dir_all(&target).map_err(|e| ZenxError::io(&target, e))?;
        self.db
            .conn()
            .execute("DELETE FROM skills WHERE name = ?1", params![name])
            .map_err(map_err)?;
        Ok(RemoveReport {
            name: name.to_string(),
            removed: true,
            backup_path: backup.display().to_string(),
        })
    }

    fn backup(&self, source: &Path, name: &str, reason: &str) -> Result<PathBuf> {
        let root = self.backup_root();
        std::fs::create_dir_all(&root).map_err(|e| ZenxError::io(&root, e))?;
        let stamp = zenx_core::util::now_rfc3339()
            .replace(':', "")
            .replace('-', "");
        let mut destination = root.join(format!("{name}-{stamp}"));
        let mut counter = 1;
        while destination.exists() {
            destination = root.join(format!("{name}-{stamp}-{counter}"));
            counter += 1;
        }
        copy_dir(source, &destination)?;
        self.db
            .conn()
            .execute(
                "INSERT INTO skill_backups (name, path, created_at, reason) VALUES (?1, ?2, ?3, ?4)",
                params![
                    name,
                    destination.to_string_lossy().to_string(),
                    zenx_core::util::now_rfc3339(),
                    reason
                ],
            )
            .map_err(map_err)?;
        Ok(destination)
    }

    fn record(&self, manifest: &Manifest) -> Result<()> {
        let text = serde_json::to_string(manifest)
            .map_err(|e| ZenxError::Invalid(format!("could not serialise manifest: {e}")))?;
        self.db
            .conn()
            .execute(
                "INSERT INTO skills (name, source, commit_sha, version, installed_at, checksum, manifest)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(name) DO UPDATE SET
                    source = excluded.source,
                    commit_sha = excluded.commit_sha,
                    version = excluded.version,
                    installed_at = excluded.installed_at,
                    checksum = excluded.checksum,
                    manifest = excluded.manifest",
                params![
                    manifest.name,
                    manifest.source,
                    manifest.commit,
                    manifest.version,
                    manifest.installed_at,
                    manifest.checksum_sha256,
                    text
                ],
            )
            .map_err(map_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
        paths: ZenxPaths,
        db: Db,
    }

    impl Fixture {
        fn new(tag: &str) -> Fixture {
            let root = std::env::temp_dir().join(format!(
                "zenx-skills-{}-{}",
                std::process::id(),
                tag
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let paths = ZenxPaths::new(root.join("home"));
            paths.ensure().unwrap();
            let db = Db::open_in_memory().unwrap();
            Fixture { root, paths, db }
        }

        fn write_skill(&self, name: &str, body: &str) -> PathBuf {
            let directory = self.root.join(format!("src-{name}"));
            std::fs::create_dir_all(directory.join("scripts")).unwrap();
            std::fs::write(
                directory.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: test skill\nversion: 1.0.0\n---\n\n{body}\n"),
            )
            .unwrap();
            std::fs::write(directory.join("scripts/run.sh"), "echo hi\n").unwrap();
            directory
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn installs_from_a_local_directory() {
        let fixture = Fixture::new("install");
        let source = fixture.write_skill("demo", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        let report = manager
            .install(&InstallRequest::new(SkillSource::Local { path: source }))
            .unwrap();
        assert_eq!(report.name, "demo");
        assert!(!report.dry_run);
        assert_eq!(report.files, 2);
        assert!(Path::new(&report.path).join("SKILL.md").is_file());
        assert_eq!(manager.list().unwrap().len(), 1);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let fixture = Fixture::new("dry");
        let source = fixture.write_skill("demo", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        let mut request = InstallRequest::new(SkillSource::Local { path: source });
        request.dry_run = true;
        let report = manager.install(&request).unwrap();
        assert!(report.dry_run);
        assert!(!fixture.paths.skills_dir().join("demo").exists());
        assert!(manager.list().unwrap().is_empty());
    }

    #[test]
    fn existing_install_requires_force() {
        let fixture = Fixture::new("conflict");
        let source = fixture.write_skill("demo", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        manager
            .install(&InstallRequest::new(SkillSource::Local { path: source.clone() }))
            .unwrap();
        let error = manager
            .install(&InstallRequest::new(SkillSource::Local { path: source.clone() }))
            .unwrap_err();
        assert!(matches!(error, ZenxError::Invalid(_)));

        let mut request = InstallRequest::new(SkillSource::Local { path: source });
        request.force = true;
        let report = manager.install(&request).unwrap();
        assert!(report.replaced_backup.is_some());
    }

    #[test]
    fn verify_detects_tampering() {
        let fixture = Fixture::new("verify");
        let source = fixture.write_skill("demo", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        manager
            .install(&InstallRequest::new(SkillSource::Local { path: source }))
            .unwrap();
        assert!(manager.verify("demo").unwrap().ok);

        let skill_md = fixture.paths.skills_dir().join("demo/SKILL.md");
        std::fs::write(&skill_md, "---\nname: demo\ndescription: edited\n---\n").unwrap();
        let report = manager.verify("demo").unwrap();
        assert!(!report.ok);
        assert!(!report.problems.is_empty());
    }

    #[test]
    fn remove_backs_up_and_refuses_unmanaged_directories() {
        let fixture = Fixture::new("remove");
        let source = fixture.write_skill("demo", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        manager
            .install(&InstallRequest::new(SkillSource::Local { path: source }))
            .unwrap();
        let report = manager.remove("demo").unwrap();
        assert!(report.removed);
        assert!(Path::new(&report.backup_path).is_dir());
        assert!(!fixture.paths.skills_dir().join("demo").exists());

        let unmanaged = fixture.paths.skills_dir().join("handmade");
        std::fs::create_dir_all(&unmanaged).unwrap();
        std::fs::write(unmanaged.join("SKILL.md"), "x").unwrap();
        let error = manager.remove("handmade").unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));
        assert!(unmanaged.exists());
    }

    #[test]
    fn rollback_restores_the_previous_copy() {
        let fixture = Fixture::new("rollback");
        let source = fixture.write_skill("demo", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        manager
            .install(&InstallRequest::new(SkillSource::Local { path: source.clone() }))
            .unwrap();

        // Replace with an edited copy so a backup exists.
        std::fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: test skill\nversion: 2.0.0\n---\n\nchanged\n",
        )
        .unwrap();
        let mut request = InstallRequest::new(SkillSource::Local { path: source });
        request.force = true;
        manager.install(&request).unwrap();
        assert_eq!(
            manager.list().unwrap()[0].version.as_deref(),
            Some("2.0.0")
        );

        let restored = manager.rollback("demo").unwrap();
        assert_eq!(restored.version.as_deref(), Some("1.0.0"));
        assert!(manager.verify("demo").unwrap().ok);
    }

    #[test]
    fn update_requires_a_git_source() {
        let fixture = Fixture::new("update");
        let source = fixture.write_skill("demo", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        manager
            .install(&InstallRequest::new(SkillSource::Local { path: source }))
            .unwrap();
        assert!(manager.update("demo").is_err());
    }

    #[test]
    fn git_install_requires_network_consent() {
        let fixture = Fixture::new("network");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        let request = InstallRequest::new(SkillSource::Git {
            url: "https://example.invalid/skill.git".into(),
            rev: None,
        });
        let error = manager.install(&request).unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));
    }

    #[test]
    fn names_cannot_escape_the_store() {
        assert!(validate_name("../../etc").is_err());
        assert!(validate_name("..").is_err());
        assert!(validate_name("").is_err());
        assert!(validate_name("good-name_1.2").is_ok());
    }

    #[test]
    fn search_matches_name_and_description() {
        let fixture = Fixture::new("search");
        let source = fixture.write_skill("pdf-tools", "hello");
        let manager = SkillsManager::new(&fixture.db, &fixture.paths);
        manager
            .install(&InstallRequest::new(SkillSource::Local { path: source }))
            .unwrap();
        assert_eq!(manager.search("pdf").unwrap().len(), 1);
        assert_eq!(manager.search("test skill").unwrap().len(), 1);
        assert_eq!(manager.search("nope").unwrap().len(), 0);
    }
}
