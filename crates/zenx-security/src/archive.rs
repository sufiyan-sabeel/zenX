//! Archive entry validation.
//!
//! zenX does not extract archives today (skills arrive over git), but any future
//! importer must route entry names through here first: zip-slip is the classic
//! way a "convenience" extraction feature writes outside its destination.

use std::path::{Path, PathBuf};

use zenx_core::{Result, ZenxError};

/// Validate a single archive entry name and return its safe relative path.
///
/// Rejects absolute paths, drive letters, any `..` segment and empty names.
/// Backslashes are treated as separators so a Windows-built archive cannot
/// smuggle a traversal past a Unix check.
pub fn sanitize_entry_name(name: &str) -> Result<PathBuf> {
    if name.trim().is_empty() {
        return Err(ZenxError::Security("empty archive entry name".into()));
    }
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(ZenxError::Security(format!(
            "absolute archive entry rejected: {name}"
        )));
    }
    if normalized.contains(':') {
        return Err(ZenxError::Security(format!(
            "archive entry with a drive or stream prefix rejected: {name}"
        )));
    }
    let mut out = PathBuf::new();
    for segment in normalized.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                return Err(ZenxError::Security(format!(
                    "path traversal in archive entry rejected: {name}"
                )))
            }
            other => out.push(other),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(ZenxError::Security(format!(
            "archive entry resolves to nothing: {name}"
        )));
    }
    Ok(out)
}

/// Validate a whole entry list and map it onto destination paths.
///
/// Fails on the first hostile entry and on duplicate targets, which would
/// otherwise let a later entry silently overwrite an earlier one.
pub fn plan_extraction(entries: &[String], destination: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut planned: Vec<(String, PathBuf)> = Vec::with_capacity(entries.len());
    for entry in entries {
        let relative = sanitize_entry_name(entry)?;
        let target = destination.join(&relative);
        if planned.iter().any(|(_, existing)| existing == &target) {
            return Err(ZenxError::Security(format!(
                "duplicate archive entry rejected: {entry}"
            )));
        }
        planned.push((entry.clone(), target));
    }
    Ok(planned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_entries_are_accepted() {
        assert_eq!(
            sanitize_entry_name("docs/guide.md").unwrap(),
            PathBuf::from("docs/guide.md")
        );
        assert_eq!(
            sanitize_entry_name("./docs/guide.md").unwrap(),
            PathBuf::from("docs/guide.md")
        );
    }

    #[test]
    fn traversal_is_rejected() {
        assert!(sanitize_entry_name("../etc/passwd").is_err());
        assert!(sanitize_entry_name("docs/../../etc/passwd").is_err());
        assert!(sanitize_entry_name("..\\windows\\system32").is_err());
    }

    #[test]
    fn absolute_entries_are_rejected() {
        assert!(sanitize_entry_name("/etc/passwd").is_err());
        assert!(sanitize_entry_name("C:/Windows/System32").is_err());
        assert!(sanitize_entry_name("C:stream").is_err());
    }

    #[test]
    fn empty_entries_are_rejected() {
        assert!(sanitize_entry_name("").is_err());
        assert!(sanitize_entry_name("   ").is_err());
        assert!(sanitize_entry_name("./").is_err());
    }

    #[test]
    fn plan_maps_into_destination() {
        let entries = vec!["a/b.md".to_string(), "c.md".to_string()];
        let plan = plan_extraction(&entries, Path::new("/dest")).unwrap();
        assert_eq!(plan[0].1, PathBuf::from("/dest/a/b.md"));
        assert_eq!(plan[1].1, PathBuf::from("/dest/c.md"));
    }

    #[test]
    fn plan_rejects_duplicates_and_hostile_entries() {
        let entries = vec!["a.md".to_string(), "./a.md".to_string()];
        assert!(plan_extraction(&entries, Path::new("/dest")).is_err());
        let entries = vec!["ok.md".to_string(), "../../evil".to_string()];
        assert!(plan_extraction(&entries, Path::new("/dest")).is_err());
    }
}
