//! zenX Skills — one skill store, many agents.
//!
//! Skills live in a store owned by zenX; agent-specific directories receive
//! copies. Every install writes a manifest with a checksum so `verify` can
//! detect drift, and every replacement is backed up first so `rollback` always
//! has something to restore.

pub mod git;
pub mod manager;
pub mod manifest;
pub mod meta;

pub use manager::{
    InstallReport, InstallRequest, RemoveReport, SkillSource, SkillsManager, UpdateReport,
    VerifyReport,
};
pub use manifest::Manifest;
pub use meta::{parse_skill_markdown, SkillMeta};
