//! zenX self-update and uninstall.
//!
//! The updater replaces the running binary, so it is deliberately paranoid:
//! it only ever touches a file named `zenx`, it refuses any URL that is not
//! `https://` (or `file://` for tests), and it will not extract or install an
//! archive whose SHA-256 does not match the published `SHA256SUMS.txt`. A
//! failed verification is a hard stop, never a warning.

pub mod install;
pub mod release;
pub mod uninstall;
pub mod update;

pub use install::{classify, current_exe, detect_method, ensure_zenx_binary, InstallMethod};
pub use release::{
    archive_name, is_newer, parse_checksums, resolve_release, target_triple, verify_archive,
    ReleaseAsset, BASE_URL_ENV, DEFAULT_REPO,
};
pub use uninstall::{uninstall, UninstallOptions, UninstallReport};
pub use update::{rollback, update, UpdateAction, UpdateOptions, UpdateReport};
