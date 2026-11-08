use std::path::Path;

use zenx_core::{Result, ZenxError};
use zenx_security::sha256_file;

use crate::meta::SkillMeta;

/// Name of the manifest file written inside each installed skill.
pub const MANIFEST_FILE: &str = ".zenx-manifest.json";

/// Everything zenX knows about an installed skill.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub name: String,
    pub description: String,
    pub source: String,
    pub commit: Option<String>,
    pub version: Option<String>,
    pub checksum_sha256: String,
    pub installed_at: String,
    pub files: Vec<String>,
    /// Agent adapters this skill was linked into at install time.
    #[serde(default)]
    pub linked_targets: Vec<String>,
}

impl Manifest {
    pub fn build(
        meta: &SkillMeta,
        source: &str,
        commit: Option<String>,
        checksum_sha256: String,
        files: Vec<String>,
    ) -> Self {
        Manifest {
            name: meta.name.clone(),
            description: meta.description.clone(),
            source: source.to_string(),
            commit,
            version: meta.version.clone(),
            checksum_sha256,
            installed_at: zenx_core::util::now_rfc3339(),
            files,
            linked_targets: Vec::new(),
        }
    }

    pub fn read(skill_dir: &Path) -> Result<Manifest> {
        let path = skill_dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| ZenxError::io(&path, e))?;
        serde_json::from_str(&text)
            .map_err(|e| ZenxError::Invalid(format!("{}: {e}", path.display())))
    }

    pub fn write(&self, skill_dir: &Path) -> Result<()> {
        let path = skill_dir.join(MANIFEST_FILE);
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| ZenxError::Invalid(format!("could not serialise manifest: {e}")))?;
        std::fs::write(&path, text).map_err(|e| ZenxError::io(&path, e))
    }

    /// Recompute the SKILL.md checksum of an installed directory.
    pub fn current_checksum(skill_dir: &Path) -> Result<String> {
        let path = skill_dir.join("SKILL.md");
        if !path.is_file() {
            return Err(ZenxError::NotFound(format!("{}", path.display())));
        }
        sha256_file(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> SkillMeta {
        SkillMeta {
            name: "demo".into(),
            description: "demo skill".into(),
            version: Some("0.1.0".into()),
            license: None,
        }
    }

    #[test]
    fn manifest_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("zenx-manifest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "---\nname: demo\ndescription: d\n---\n").unwrap();
        let checksum = Manifest::current_checksum(&dir).unwrap();
        let manifest = Manifest::build(&meta(), "local:/tmp/demo", None, checksum, vec!["SKILL.md".into()]);
        manifest.write(&dir).unwrap();
        let read = Manifest::read(&dir).unwrap();
        assert_eq!(read, manifest);
        assert_eq!(read.version.as_deref(), Some("0.1.0"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_manifest_is_an_io_error() {
        let dir = std::env::temp_dir().join(format!("zenx-manifest-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(Manifest::read(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
