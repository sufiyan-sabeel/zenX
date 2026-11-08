//! Security primitives shared by every zenX module.
//!
//! Nothing here is decorative: each function is on a path that touches
//! untrusted input (skill archives, agent payloads, user paths) and each one
//! has tests that exercise the hostile case.

pub mod archive;
pub mod fs;
pub mod redact;
pub mod tempdir;

pub use archive::{plan_extraction, sanitize_entry_name};
pub use fs::{copy_dir, is_within, list_files, sha256_file, sha256_hex};
pub use redact::{redact, redact_json, MASK};
pub use tempdir::TempDir;
