use std::path::PathBuf;

use thiserror::Error;

/// Result alias used by every zenX crate.
pub type Result<T> = std::result::Result<T, ZenxError>;

/// The single error type for the whole platform.
///
/// Callers match on variants to choose an exit code; user-facing text is
/// produced by the `Display` implementation.
#[derive(Debug, Error)]
pub enum ZenxError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("storage error: {0}")]
    Storage(String),

    #[error("security violation: {0}")]
    Security(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("external command failed: {command} (exit code {code:?}): {stderr}")]
    External {
        command: String,
        code: Option<i32>,
        stderr: String,
    },

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("operation aborted: {0}")]
    Aborted(String),
}

impl ZenxError {
    /// Attach a path to an I/O error.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ZenxError::Io {
            path: path.into(),
            source,
        }
    }

    /// Documented, stable process exit codes.
    ///
    /// 0 success, 1 general failure, 2 invalid input or configuration,
    /// 3 security violation, 4 not found, 5 not implemented,
    /// 6 aborted by the user, 7 external command failure.
    pub fn exit_code(&self) -> i32 {
        match self {
            ZenxError::Invalid(_) | ZenxError::Config(_) => 2,
            ZenxError::Security(_) => 3,
            ZenxError::NotFound(_) => 4,
            ZenxError::NotImplemented(_) => 5,
            ZenxError::Aborted(_) => 6,
            ZenxError::External { .. } => 7,
            ZenxError::Io { .. } | ZenxError::Storage(_) => 1,
        }
    }

    /// Short machine-readable kind, used in JSON error output.
    pub fn kind(&self) -> &'static str {
        match self {
            ZenxError::Config(_) => "config",
            ZenxError::Io { .. } => "io",
            ZenxError::Storage(_) => "storage",
            ZenxError::Security(_) => "security",
            ZenxError::NotFound(_) => "not_found",
            ZenxError::Invalid(_) => "invalid",
            ZenxError::External { .. } => "external",
            ZenxError::NotImplemented(_) => "not_implemented",
            ZenxError::Aborted(_) => "aborted",
        }
    }
}

impl From<std::io::Error> for ZenxError {
    fn from(source: std::io::Error) -> Self {
        ZenxError::Io {
            path: PathBuf::from("<unknown>"),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(ZenxError::Invalid("x".into()).exit_code(), 2);
        assert_eq!(ZenxError::Config("x".into()).exit_code(), 2);
        assert_eq!(ZenxError::Security("x".into()).exit_code(), 3);
        assert_eq!(ZenxError::NotFound("x".into()).exit_code(), 4);
        assert_eq!(ZenxError::NotImplemented("x".into()).exit_code(), 5);
        assert_eq!(ZenxError::Aborted("x".into()).exit_code(), 6);
        assert_eq!(
            ZenxError::External {
                command: "git".into(),
                code: Some(1),
                stderr: "boom".into()
            }
            .exit_code(),
            7
        );
    }

    #[test]
    fn io_errors_keep_the_path() {
        let error = ZenxError::io(
            "/tmp/x",
            std::io::Error::other("nope"),
        );
        let text = error.to_string();
        assert!(text.contains("/tmp/x"));
        assert!(text.contains("nope"));
    }
}
