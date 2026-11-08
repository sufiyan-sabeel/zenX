//! Filesystem helpers that must be correct: hashing, containment checks and
//! directory copying.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use zenx_core::{Result, ZenxError};

/// Lowercase hex SHA-256 of a byte slice.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{:02x}", *byte)).collect()
}

/// Streaming SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| ZenxError::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| ZenxError::io(path, e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{:02x}", *byte)).collect())
}

/// Remove `.` components and resolve `..` lexically, without touching the disk.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// True when `candidate` resolves inside `base`.
pub fn is_within(base: &Path, candidate: &Path) -> bool {
    let base = normalize(base);
    let candidate = normalize(candidate);
    candidate == base || candidate.starts_with(&base)
}

/// Copy a directory tree. Symlinks are skipped rather than followed, so a
/// hostile archive cannot pull in files from outside the tree.
pub fn copy_dir(source: &Path, destination: &Path) -> Result<u64> {
    let mut copied = 0u64;
    for entry in WalkDir::new(source).follow_links(false) {
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

/// Sorted, forward-slash relative file paths under `root`.
pub fn list_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| ZenxError::Invalid(format!("walk {}: {e}", root.display())))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| ZenxError::Invalid(e.to_string()))?;
        files.push(relative.to_string_lossy().replace('\\', "/"));
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_stable() {
        // Known SHA-256 of "abc"
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn file_hash_matches_byte_hash() {
        let dir = std::env::temp_dir().join(format!("zenx-fs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, b"abc").unwrap();
        assert_eq!(sha256_file(&file).unwrap(), sha256_hex(b"abc"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn containment_rejects_escapes() {
        assert!(is_within(Path::new("/a/b"), Path::new("/a/b/c")));
        assert!(is_within(Path::new("/a/b"), Path::new("/a/b/c/../d")));
        assert!(!is_within(Path::new("/a/b"), Path::new("/a/bc")));
        assert!(!is_within(Path::new("/a/b"), Path::new("/a/b/../c")));
        assert!(!is_within(Path::new("/a/b"), Path::new("/etc/passwd")));
    }

    #[test]
    fn copy_dir_copies_files_and_skips_symlinks() {
        let base = std::env::temp_dir().join(format!("zenx-copy-{}", std::process::id()));
        let source = base.join("src");
        let destination = base.join("dst");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::write(source.join("nested/a.md"), b"hello").unwrap();
        let copied = copy_dir(&source, &destination).unwrap();
        assert_eq!(copied, 1);
        assert!(destination.join("nested/a.md").is_file());
        let files = list_files(&destination).unwrap();
        assert_eq!(files, vec!["nested/a.md".to_string()]);
        let _ = std::fs::remove_dir_all(&base);
    }
}
