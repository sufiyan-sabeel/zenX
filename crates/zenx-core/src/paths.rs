use std::path::{Component, Path, PathBuf};

use crate::error::{Result, ZenxError};

/// Environment variable that overrides the zenX home directory.
pub const ROOT_ENV: &str = "ZENX_HOME";

/// Resolved locations of everything zenX stores on disk.
///
/// The layout is deliberately flat so a user can back up one directory:
/// `config.toml`, `zenx.db`, `skills/`, `docs/`, `backups/`, `tmp/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZenxPaths {
    root: PathBuf,
}

impl ZenxPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        ZenxPaths { root: root.into() }
    }

    /// Resolve the home directory from `ZENX_HOME`, else `<home>/.zenx`.
    pub fn resolve() -> Result<Self> {
        if let Some(value) = std::env::var_os(ROOT_ENV) {
            if value.is_empty() {
                return Err(ZenxError::Config(format!("{ROOT_ENV} is set but empty")));
            }
            return Ok(ZenxPaths::new(PathBuf::from(value)));
        }
        let home = home_dir().ok_or_else(|| {
            ZenxError::Config(format!(
                "could not determine the home directory; set {ROOT_ENV} explicitly"
            ))
        })?;
        Ok(ZenxPaths::new(home.join(".zenx")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn database_file(&self) -> PathBuf {
        self.root.join("zenx.db")
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
    }

    pub fn docs_dir(&self) -> PathBuf {
        self.root.join("docs")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// Create the directory layout. Idempotent.
    pub fn ensure(&self) -> Result<()> {
        let dirs = [
            self.root.clone(),
            self.skills_dir(),
            self.backups_dir(),
            self.tmp_dir(),
        ];
        for dir in dirs {
            std::fs::create_dir_all(&dir).map_err(|e| ZenxError::io(&dir, e))?;
        }
        Ok(())
    }

    /// True when the home directory has been initialised.
    pub fn is_initialised(&self) -> bool {
        self.config_file().is_file()
    }
}

#[cfg(windows)]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

#[cfg(not(windows))]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// Join a user-supplied relative path onto `base`, refusing anything that
/// could escape it.
///
/// Absolute paths, drive prefixes and `..` components are rejected outright
/// rather than normalised away, because silently rewriting a path is how
/// traversal bugs get shipped.
pub fn safe_join(base: &Path, relative: impl AsRef<Path>) -> Result<PathBuf> {
    let relative = relative.as_ref();
    if relative.as_os_str().is_empty() {
        return Err(ZenxError::Security("empty path component rejected".into()));
    }
    let mut out = base.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ZenxError::Security(format!(
                    "path traversal rejected: {}",
                    relative.display()
                )))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ZenxError::Security(format!(
                    "absolute path rejected: {}",
                    relative.display()
                )))
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_accepts_relative_paths() {
        let joined = safe_join(Path::new("/base"), "a/b/c.md").unwrap();
        assert_eq!(joined, Path::new("/base/a/b/c.md"));
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let error = safe_join(Path::new("/base"), "../etc/passwd").unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));
        assert_eq!(error.exit_code(), 3);
    }

    #[test]
    fn safe_join_rejects_absolute_paths() {
        assert!(safe_join(Path::new("/base"), "/etc/passwd").is_err());
    }

    #[test]
    fn safe_join_rejects_empty() {
        assert!(safe_join(Path::new("/base"), "").is_err());
    }

    #[test]
    fn layout_is_under_root() {
        let paths = ZenxPaths::new("/tmp/zenx-home");
        assert_eq!(paths.config_file(), Path::new("/tmp/zenx-home/config.toml"));
        assert_eq!(paths.database_file(), Path::new("/tmp/zenx-home/zenx.db"));
        assert!(paths.skills_dir().starts_with(paths.root()));
    }
}
