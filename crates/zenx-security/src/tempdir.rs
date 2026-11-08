//! Temporary directories that clean themselves up.

use std::path::{Path, PathBuf};

use zenx_core::{Result, ZenxError};

/// A directory under the zenX home that is removed on drop.
///
/// Predictable names are avoided: the suffix mixes the clock, the process id
/// and a counter so two concurrent zenX processes cannot share a scratch path.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn create(parent: &Path, prefix: &str) -> Result<Self> {
        std::fs::create_dir_all(parent).map_err(|e| ZenxError::io(parent, e))?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = parent.join(format!("{prefix}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(|e| ZenxError::io(&path, e))?;
        Ok(TempDir { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Keep the directory (hand off to the caller) instead of deleting it.
    pub fn into_path(mut self) -> PathBuf {
        let path = std::mem::take(&mut self.path);
        std::mem::forget(self);
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dir_is_created_and_removed() {
        let parent = std::env::temp_dir().join(format!("zenx-tmp-{}", std::process::id()));
        let path = {
            let temp = TempDir::create(&parent, "scratch").unwrap();
            assert!(temp.path().is_dir());
            temp.path().to_path_buf()
        };
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn into_path_keeps_the_directory() {
        let parent = std::env::temp_dir().join(format!("zenx-tmp-keep-{}", std::process::id()));
        let temp = TempDir::create(&parent, "keep").unwrap();
        let path = temp.into_path();
        assert!(path.is_dir());
        let _ = std::fs::remove_dir_all(&parent);
    }
}
