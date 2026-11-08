use std::path::{Path, PathBuf};

use zenx_security::TempDir;

use crate::report::{Check, Status};
use crate::{binary_available, find_first, relative};

/// Everything a detector may do during a run.
#[derive(Debug, Clone, Default)]
pub struct DoctorContext {
    pub run_build: bool,
    pub run_tests: bool,
    pub known_commands: Vec<String>,
}

/// One ecosystem's checks.
pub trait Detector: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self, root: &Path) -> bool;
    fn checks(&self, root: &Path, context: &DoctorContext) -> Vec<Check>;
}

pub fn detectors() -> Vec<Box<dyn Detector>> {
    vec![
        Box::new(RustDetector),
        Box::new(NodeDetector),
        Box::new(PythonDetector),
    ]
}

/// Run a command in `cwd`, capturing output. Never inherits the terminal.
fn run(program: &str, args: &[&str], cwd: &Path, envs: &[(&str, String)]) -> Result<(bool, String), String> {
    let mut command = std::process::Command::new(program);
    command.args(args).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().map_err(|e| format!("{program}: {e}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok((output.status.success(), text))
}

fn tail(text: &str, lines: usize) -> String {
    let collected: Vec<&str> = text.lines().collect();
    let start = collected.len().saturating_sub(lines);
    collected[start..].join("\n")
}

// ---------------------------------------------------------------- Rust

pub struct RustDetector;

impl Detector for RustDetector {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("Cargo.toml").is_file()
    }

    fn checks(&self, root: &Path, context: &DoctorContext) -> Vec<Check> {
        let mut checks = Vec::new();
        let manifest = root.join("Cargo.toml");

        match std::fs::read_to_string(&manifest)
            .map_err(|e| e.to_string())
            .and_then(|text| toml::from_str::<toml::Value>(&text).map_err(|e| e.to_string()))
        {
            Ok(value) => {
                let mut check = Check::new(
                    "rust.manifest",
                    "Cargo manifest parses",
                    Status::Pass,
                    "Cargo.toml is valid TOML".to_string(),
                );
                if let Some(members) = value
                    .get("workspace")
                    .and_then(|workspace| workspace.get("members"))
                    .and_then(|members| members.as_array())
                {
                    let mut missing = Vec::new();
                    for member in members.iter().filter_map(|m| m.as_str()) {
                        if !root.join(member).is_dir() {
                            missing.push(member.to_string());
                        }
                    }
                    if missing.is_empty() {
                        check.detail = format!("Cargo.toml valid, {} workspace member(s) present", members.len());
                    } else {
                        check.status = Status::Fail;
                        check.detail = format!("{} workspace member path(s) do not exist", missing.len());
                        check.evidence = missing;
                    }
                }
                checks.push(check);
            }
            Err(error) => checks.push(Check::new(
                "rust.manifest",
                "Cargo manifest parses",
                Status::Fail,
                format!("Cargo.toml is invalid: {error}"),
            )),
        }

        checks.push(Check::new(
            "rust.lockfile",
            "Cargo.lock committed",
            if root.join("Cargo.lock").is_file() {
                Status::Pass
            } else {
                Status::Warn
            },
            if root.join("Cargo.lock").is_file() {
                "Cargo.lock present".to_string()
            } else {
                "no Cargo.lock; builds are not reproducible".to_string()
            },
        ));

        if !context.run_build && !context.run_tests {
            checks.push(Check::new(
                "rust.build",
                "cargo check",
                Status::Skip,
                "pass --build to run the compiler".to_string(),
            ));
            return checks;
        }

        if !binary_available("cargo") {
            checks.push(Check::new(
                "rust.build",
                "cargo check",
                Status::Skip,
                "cargo is not installed".to_string(),
            ));
            return checks;
        }

        // Build outside the repository so a diagnostic run leaves no target/.
        let scratch = match TempDir::create(&std::env::temp_dir(), "zenx-doctor-target") {
            Ok(dir) => dir,
            Err(error) => {
                checks.push(Check::new(
                    "rust.build",
                    "cargo check",
                    Status::Skip,
                    format!("could not create a scratch target directory: {error}"),
                ));
                return checks;
            }
        };
        let target_dir = scratch.path().to_string_lossy().to_string();

        let (ok, output) = match run(
            "cargo",
            &["check", "--all-targets", "--quiet"],
            root,
            &[("CARGO_TARGET_DIR", target_dir.clone())],
        ) {
            Ok(result) => result,
            Err(error) => {
                checks.push(Check::new("rust.build", "cargo check", Status::Skip, error));
                return checks;
            }
        };
        let mut check = Check::new(
            "rust.build",
            "cargo check",
            if ok { Status::Pass } else { Status::Fail },
            if ok {
                "the crate graph compiles".to_string()
            } else {
                "the compiler reported errors".to_string()
            },
        );
        if !ok {
            check.evidence.push(tail(&output, 40));
        }
        checks.push(check);

        if context.run_tests {
            let (ok, output) = match run(
                "cargo",
                &["test", "--no-run", "--quiet"],
                root,
                &[("CARGO_TARGET_DIR", target_dir)],
            ) {
                Ok(result) => result,
                Err(error) => {
                    checks.push(Check::new("rust.tests", "cargo test --no-run", Status::Skip, error));
                    return checks;
                }
            };
            let mut check = Check::new(
                "rust.tests",
                "cargo test --no-run",
                if ok { Status::Pass } else { Status::Fail },
                if ok {
                    "test targets compile".to_string()
                } else {
                    "test targets do not compile".to_string()
                },
            );
            if !ok {
                check.evidence.push(tail(&output, 40));
            }
            checks.push(check);
        }

        checks
    }
}

// ---------------------------------------------------------------- Node

pub struct NodeDetector;

impl Detector for NodeDetector {
    fn name(&self) -> &'static str {
        "node"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("package.json").is_file()
    }

    fn checks(&self, root: &Path, context: &DoctorContext) -> Vec<Check> {
        let mut checks = Vec::new();
        let manifest = root.join("package.json");

        match std::fs::read_to_string(&manifest)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).map_err(|e| e.to_string()))
        {
            Ok(value) => {
                checks.push(Check::new(
                    "node.manifest",
                    "package.json parses",
                    Status::Pass,
                    "package.json is valid JSON".to_string(),
                ));
                let test_script = value
                    .get("scripts")
                    .and_then(|scripts| scripts.get("test"))
                    .and_then(|script| script.as_str());
                checks.push(Check::new(
                    "node.scripts.test",
                    "Test script defined",
                    if test_script.is_some() {
                        Status::Pass
                    } else {
                        Status::Warn
                    },
                    match test_script {
                        Some(script) => format!("npm test → {script}"),
                        None => "package.json has no test script".to_string(),
                    },
                ));
            }
            Err(error) => checks.push(Check::new(
                "node.manifest",
                "package.json parses",
                Status::Fail,
                format!("package.json is invalid: {error}"),
            )),
        }

        let lockfiles = ["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lockb"];
        let found = find_first(root, &lockfiles);
        checks.push(Check::new(
            "node.lockfile",
            "Lockfile committed",
            if found.is_some() { Status::Pass } else { Status::Warn },
            match found {
                Some(path) => relative(root, &path),
                None => "no lockfile; dependency versions float".to_string(),
            },
        ));

        if context.run_tests {
            if !binary_available("npm") {
                checks.push(Check::new(
                    "node.tests",
                    "npm test",
                    Status::Skip,
                    "npm is not installed".to_string(),
                ));
            } else {
                let (ok, output) = match run("npm", &["test", "--silent"], root, &[]) {
                    Ok(result) => result,
                    Err(error) => {
                        checks.push(Check::new("node.tests", "npm test", Status::Skip, error));
                        return checks;
                    }
                };
                let mut check = Check::new(
                    "node.tests",
                    "npm test",
                    if ok { Status::Pass } else { Status::Fail },
                    if ok {
                        "npm test succeeded".to_string()
                    } else {
                        "npm test failed".to_string()
                    },
                );
                if !ok {
                    check.evidence.push(tail(&output, 30));
                }
                checks.push(check);
            }
        }

        checks
    }
}

// ---------------------------------------------------------------- Python

pub struct PythonDetector;

impl Detector for PythonDetector {
    fn name(&self) -> &'static str {
        "python"
    }

    fn detect(&self, root: &Path) -> bool {
        find_first(
            root,
            &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"],
        )
        .is_some()
    }

    fn checks(&self, root: &Path, context: &DoctorContext) -> Vec<Check> {
        let mut checks = Vec::new();
        let manifest = find_first(
            root,
            &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"],
        );
        checks.push(Check::new(
            "python.manifest",
            "Python packaging metadata",
            if manifest.is_some() {
                Status::Pass
            } else {
                Status::Warn
            },
            match &manifest {
                Some(path) => relative(root, path),
                None => "no packaging metadata".to_string(),
            },
        ));

        let has_tests = root.join("tests").is_dir()
            || root.join("test").is_dir()
            || find_python_tests(root).is_some();
        checks.push(Check::new(
            "python.tests",
            "Tests present",
            if has_tests { Status::Pass } else { Status::Warn },
            if has_tests {
                "test files found".to_string()
            } else {
                "no tests directory or test_*.py files".to_string()
            },
        ));

        if context.run_tests {
            let python = if binary_available("python3") {
                "python3"
            } else {
                "python"
            };
            if !binary_available(python) {
                checks.push(Check::new(
                    "python.tests.collect",
                    "pytest collection",
                    Status::Skip,
                    "no python interpreter on PATH".to_string(),
                ));
            } else {
                let (ok, output) = match run(python, &["-m", "pytest", "--collect-only", "-q"], root, &[]) {
                    Ok(result) => result,
                    Err(error) => {
                        checks.push(Check::new("python.tests.collect", "pytest collection", Status::Skip, error));
                        return checks;
                    }
                };
                let missing_pytest = output.contains("No module named pytest");
                let mut check = Check::new(
                    "python.tests.collect",
                    "pytest collection",
                    if missing_pytest {
                        Status::Skip
                    } else if ok {
                        Status::Pass
                    } else {
                        Status::Fail
                    },
                    if missing_pytest {
                        "pytest is not installed".to_string()
                    } else if ok {
                        "pytest collected the suite".to_string()
                    } else {
                        "pytest could not collect the suite".to_string()
                    },
                );
                if !ok && !missing_pytest {
                    check.evidence.push(tail(&output, 30));
                }
                checks.push(check);
            }
        }

        checks
    }
}

fn find_python_tests(root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("test_") && name.ends_with(".py") {
            return Some(entry.path());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zenx-detector-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rust_detector_flags_missing_workspace_members() {
        let dir = fixture("rust");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/present\", \"crates/absent\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("crates/present")).unwrap();
        let checks = RustDetector.checks(&dir, &DoctorContext::default());
        let manifest = checks.iter().find(|c| c.id == "rust.manifest").unwrap();
        assert_eq!(manifest.status, Status::Fail);
        assert_eq!(manifest.evidence, vec!["crates/absent".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rust_detector_skips_builds_without_consent() {
        let dir = fixture("rust-skip");
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\nversion = \"0.1.0\"\n").unwrap();
        let checks = RustDetector.checks(&dir, &DoctorContext::default());
        let build = checks.iter().find(|c| c.id == "rust.build").unwrap();
        assert_eq!(build.status, Status::Skip);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn node_detector_reports_test_script() {
        let dir = fixture("node");
        std::fs::write(dir.join("package.json"), "{\"scripts\":{\"test\":\"vitest\"}}").unwrap();
        std::fs::write(dir.join("package-lock.json"), "{}").unwrap();
        let checks = NodeDetector.checks(&dir, &DoctorContext::default());
        assert_eq!(
            checks.iter().find(|c| c.id == "node.scripts.test").unwrap().status,
            Status::Pass
        );
        assert_eq!(
            checks.iter().find(|c| c.id == "node.lockfile").unwrap().status,
            Status::Pass
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn python_detector_detects_project() {
        let dir = fixture("python");
        std::fs::write(dir.join("pyproject.toml"), "[project]\nname = \"x\"\n").unwrap();
        std::fs::create_dir_all(dir.join("tests")).unwrap();
        assert!(PythonDetector.detect(&dir));
        let checks = PythonDetector.checks(&dir, &DoctorContext::default());
        assert_eq!(
            checks.iter().find(|c| c.id == "python.tests").unwrap().status,
            Status::Pass
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
