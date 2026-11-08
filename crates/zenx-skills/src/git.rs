//! Thin wrapper over the `git` binary.
//!
//! zenX never shells out with a composed string: arguments are passed as a
//! vector, so a repository URL cannot smuggle extra flags or commands.

use std::path::Path;

use zenx_core::{Result, ZenxError};

/// Run git and return trimmed stdout.
pub fn run(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut command = std::process::Command::new("git");
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    // Never hang waiting for credentials on a non-interactive run.
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GIT_ASKPASS", "true");

    let output = command.output().map_err(|error| ZenxError::External {
        command: format!("git {}", args.join(" ")),
        code: None,
        stderr: error.to_string(),
    })?;

    if !output.status.success() {
        return Err(ZenxError::External {
            command: format!("git {}", args.join(" ")),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn is_available() -> bool {
    run(&["--version"], None).is_ok()
}

/// Shallow clone into `destination`.
pub fn clone_shallow(url: &str, destination: &Path, rev: Option<&str>) -> Result<()> {
    let destination_text = destination.to_string_lossy().to_string();
    match rev {
        Some(rev) => {
            run(
                &[
                    "clone",
                    "--depth",
                    "1",
                    "--branch",
                    rev,
                    "--",
                    url,
                    &destination_text,
                ],
                None,
            )?;
        }
        None => {
            run(
                &["clone", "--depth", "1", "--", url, &destination_text],
                None,
            )?;
        }
    }
    Ok(())
}

pub fn head_commit(repo: &Path) -> Result<String> {
    run(&["rev-parse", "HEAD"], Some(repo))
}
