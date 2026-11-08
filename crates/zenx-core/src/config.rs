use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Result, ZenxError};

/// Schema version of `config.toml`. Bumped whenever a field changes meaning.
pub const CONFIG_VERSION: u32 = 1;

/// Versioned, forward-compatible configuration.
///
/// Every struct uses `#[serde(default)]`, so a config file written by an older
/// build keeps loading after new fields appear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub output: OutputSettings,
    pub trace: TraceSettings,
    pub skills: SkillsSettings,
    pub docs: DocsSettings,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: CONFIG_VERSION,
            output: OutputSettings::default(),
            trace: TraceSettings::default(),
            skills: SkillsSettings::default(),
            docs: DocsSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    /// `human` or `json`.
    pub format: String,
    pub color: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        OutputSettings {
            format: "human".to_string(),
            color: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraceSettings {
    /// Redact secrets before they are written to the database.
    pub redact: bool,
    pub capture_file_changes: bool,
    /// Optional retention window; `None` keeps sessions forever.
    pub retention_days: Option<u32>,
}

impl Default for TraceSettings {
    fn default() -> Self {
        TraceSettings {
            redact: true,
            capture_file_changes: true,
            retention_days: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsSettings {
    /// Override for the skill store. Defaults to `<home>/skills`.
    pub store: Option<PathBuf>,
    /// Network operations (git clone) require this to be true.
    pub allow_network: bool,
    /// Extra agent directories that receive copies of installed skills.
    pub targets: Vec<SkillTarget>,
}

impl Default for SkillsSettings {
    fn default() -> Self {
        SkillsSettings {
            store: None,
            allow_network: true,
            targets: Vec::new(),
        }
    }
}

/// An agent-specific skill directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillTarget {
    pub name: String,
    pub path: PathBuf,
    /// `skill-dir` copies the whole skill directory, `flat` copies only `SKILL.md`.
    pub layout: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocsSettings {
    /// Files larger than this are skipped when importing documentation.
    pub max_file_bytes: u64,
}

impl Default for DocsSettings {
    fn default() -> Self {
        DocsSettings {
            max_file_bytes: 2 * 1024 * 1024,
        }
    }
}

impl Config {
    /// Load from disk. A missing file yields defaults so first-run works.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let config: Config = toml::from_str(&text)
                    .map_err(|e| ZenxError::Config(format!("{}: {e}", path.display())))?;
                config.validate()?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(ZenxError::io(path, e)),
        }
    }

    /// Write atomically: temp file then rename, so an interrupted write cannot
    /// truncate a working config.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ZenxError::io(parent, e))?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| ZenxError::Config(format!("could not serialise config: {e}")))?;
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text).map_err(|e| ZenxError::io(&temp, e))?;
        std::fs::rename(&temp, path).map_err(|e| ZenxError::io(path, e))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.version > CONFIG_VERSION {
            return Err(ZenxError::Config(format!(
                "config version {} is newer than this build supports ({CONFIG_VERSION}); upgrade zenX",
                self.version
            )));
        }
        match self.output.format.as_str() {
            "human" | "json" => {}
            other => {
                return Err(ZenxError::Config(format!(
                    "output.format must be \"human\" or \"json\", got {other:?}"
                )))
            }
        }
        for target in &self.skills.targets {
            match target.layout.as_str() {
                "skill-dir" | "flat" => {}
                other => {
                    return Err(ZenxError::Config(format!(
                        "skills target {:?} has unsupported layout {other:?}",
                        target.name
                    )))
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("zenx-config-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path.join("config.toml")
    }

    #[test]
    fn missing_file_yields_defaults() {
        let path = temp_path("missing");
        let config = Config::load(&path).unwrap();
        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.output.format, "human");
        assert!(config.trace.redact);
    }

    #[test]
    fn roundtrip_preserves_values() {
        let path = temp_path("roundtrip");
        let mut config = Config::default();
        config.output.format = "json".to_string();
        config.trace.retention_days = Some(7);
        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(config, loaded);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn unknown_keys_and_missing_fields_are_tolerated() {
        let path = temp_path("forward");
        std::fs::write(&path, "version = 1\n[output]\nformat = \"json\"\n").unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(config.output.format, "json");
        assert!(config.trace.redact);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn newer_version_is_rejected() {
        let path = temp_path("newer");
        std::fs::write(&path, format!("version = {}\n", CONFIG_VERSION + 1)).unwrap();
        assert!(Config::load(&path).is_err());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn invalid_format_is_rejected() {
        let mut config = Config::default();
        config.output.format = "yaml".to_string();
        assert!(config.validate().is_err());
    }
}
