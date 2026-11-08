//! End-to-end CLI tests.
//!
//! These run the real binary in a throwaway ZENX_HOME, so they exercise the same
//! paths a user does: argument parsing, storage, JSON output and exit codes.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn temp_home(tag: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!(
        "zenx-cli-{}-{}-{}",
        std::process::id(),
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    home
}

fn zenx(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zenx"));
    command
        .env("ZENX_HOME", home)
        .env("ZENX_LOG", "off")
        .env("NO_COLOR", "1");
    command
}

fn run(home: &Path, args: &[&str]) -> Output {
    zenx(home).args(args).output().expect("failed to run zenx")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(output)).unwrap_or_else(|error| {
        panic!(
            "stdout was not JSON ({error}).\nstdout:\n{}\nstderr:\n{}",
            stdout(output),
            stderr(output)
        )
    })
}

#[test]
fn help_lists_every_advertised_command() {
    let home = temp_home("help");
    let output = run(&home, &["--help"]);
    assert!(output.status.success());
    let text = stdout(&output);
    for command in [
        "init", "status", "config", "trace", "skills", "doctor", "mobile", "bench", "contract",
        "docs",
    ] {
        assert!(text.contains(command), "help is missing {command}");
    }
    // The dashboard is not implemented, so it must not be advertised.
    assert!(!text.contains("dashboard"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn init_creates_state_and_status_reads_it_back() {
    let home = temp_home("init");
    let output = run(&home, &["init", "--output", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let payload = json(&output);
    assert_eq!(payload["ok"], true);
    assert!(home.join("config.toml").is_file());
    assert!(home.join("zenx.db").is_file());

    let output = run(&home, &["status", "--output", "json"]);
    assert!(output.status.success());
    let payload = json(&output);
    assert_eq!(payload["initialised"], true);
    assert!(payload["database"]["schema_version"].as_i64().unwrap() >= 1);
    let modules = payload["modules"].as_array().unwrap();
    let dashboard = modules
        .iter()
        .find(|module| module["id"] == "dashboard")
        .unwrap();
    assert_eq!(dashboard["status"], "planned");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn trace_records_from_stdin_and_inspects_the_session() {
    let home = temp_home("trace");
    assert!(run(&home, &["init"]).status.success());

    let mut child = zenx(&home)
        .args([
            "trace", "record", "--agent", "codex", "--label", "cli-test", "--output", "json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(
            b"{\"kind\":\"tool_call\",\"name\":\"read_file\",\"duration_ms\":12}\n{\"kind\":\"tool_result\",\"name\":\"read_file\"}\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let payload = json(&output);
    assert_eq!(payload["imported"], 2);
    let session = payload["session"].as_str().unwrap().to_string();

    let output = run(&home, &["trace", "inspect", &session, "--output", "json"]);
    assert!(output.status.success());
    let payload = json(&output);
    assert_eq!(payload["agent"], "codex");
    assert_eq!(payload["events"].as_array().unwrap().len(), 2);
    assert_eq!(payload["events"][0]["seq"], 1);

    let output = run(&home, &["trace", "sessions"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains(&session));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn missing_session_exits_with_not_found_code_and_json_error() {
    let home = temp_home("notfound");
    assert!(run(&home, &["init"]).status.success());
    let output = run(
        &home,
        &["trace", "inspect", "sess_missing", "--output", "json"],
    );
    assert_eq!(output.status.code(), Some(4));
    let payload = json(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["kind"], "not_found");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn invalid_config_value_is_rejected() {
    let home = temp_home("config");
    assert!(run(&home, &["init"]).status.success());
    let output = run(&home, &["config", "set", "output.format", "yaml"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("human or json"));

    let output = run(&home, &["config", "set", "trace.retention_days", "30"]);
    assert!(output.status.success());
    let output = run(&home, &["config", "get", "trace.retention_days"]);
    assert_eq!(stdout(&output).trim(), "30");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn skills_install_verify_and_remove_a_local_skill() {
    let home = temp_home("skills");
    assert!(run(&home, &["init"]).status.success());

    let source = home.join("source-skill");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(
        source.join("SKILL.md"),
        "---\nname: demo\ndescription: A demo skill\nversion: 0.1.0\n---\n\nBody\n",
    )
    .unwrap();

    let output = run(
        &home,
        &[
            "skills",
            "install",
            source.to_str().unwrap(),
            "--output",
            "json",
        ],
    );
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(json(&output)["name"], "demo");

    let output = run(&home, &["skills", "verify", "demo", "--output", "json"]);
    assert!(output.status.success());
    assert_eq!(json(&output)["ok"], true);

    let output = run(&home, &["skills", "remove", "demo", "--output", "json"]);
    assert!(output.status.success());
    assert_eq!(json(&output)["removed"], true);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn doctor_fails_a_repository_without_a_readme() {
    let home = temp_home("doctor");
    assert!(run(&home, &["init"]).status.success());
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    let output = run(
        &home,
        &["doctor", repo.to_str().unwrap(), "--output", "json"],
    );
    assert_eq!(output.status.code(), Some(1));
    let payload = json(&output);
    assert!(payload["report"]["summary"]["fail"].as_u64().unwrap() >= 1);

    // --fix alone must not write anything.
    let output = run(
        &home,
        &[
            "doctor",
            repo.to_str().unwrap(),
            "--fix",
            "--output",
            "json",
        ],
    );
    assert!(output.status.success() || output.status.code() == Some(1));
    assert!(!repo.join("README.md").exists());

    let output = run(
        &home,
        &["doctor", repo.to_str().unwrap(), "--fix", "--apply"],
    );
    assert!(output.status.success() || output.status.code() == Some(1));
    assert!(repo.join("README.md").is_file());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn docs_import_and_search() {
    let home = temp_home("docs");
    assert!(run(&home, &["init"]).status.success());
    let corpus = home.join("corpus");
    std::fs::create_dir_all(&corpus).unwrap();
    std::fs::write(
        corpus.join("install.md"),
        "# Installing\n\nRun cargo build --release.\n",
    )
    .unwrap();

    let output = run(
        &home,
        &[
            "docs",
            "add",
            corpus.to_str().unwrap(),
            "--collection",
            "test",
            "--output",
            "json",
        ],
    );
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(json(&output)["imported"], 1);

    let output = run(&home, &["docs", "search", "release", "--output", "json"]);
    assert!(output.status.success());
    let payload = json(&output);
    assert_eq!(payload["hits"].as_array().unwrap().len(), 1);
    assert_eq!(payload["hits"][0]["title"], "Installing");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn contract_compare_exits_one_on_breaking_changes() {
    let home = temp_home("contract");
    let base = home.join("base.yaml");
    let head = home.join("head.yaml");
    std::fs::write(
        &base,
        "openapi: 3.0.0\ninfo:\n  title: T\n  version: '1'\npaths:\n  /a:\n    get:\n      responses:\n        \"200\":\n          description: ok\n",
    )
    .unwrap();
    std::fs::write(
        &head,
        "openapi: 3.0.0\ninfo:\n  title: T\n  version: '2'\npaths: {}\n",
    )
    .unwrap();

    let output = run(
        &home,
        &[
            "contract",
            "compare",
            base.to_str().unwrap(),
            head.to_str().unwrap(),
            "--output",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let payload = json(&output);
    assert_eq!(payload["compatible"], false);
    assert_eq!(payload["breaking"], 1);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn mobile_diagnose_always_answers() {
    let home = temp_home("mobile");
    let output = run(&home, &["mobile", "diagnose", "--output", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let payload = json(&output);
    assert!(payload["environment"]["os"].is_string());
    assert!(payload["tools"].is_array());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn every_command_and_subcommand_help_is_valid() {
    let home = temp_home("help-all");
    for command in [
        "init",
        "status",
        "config",
        "trace",
        "skills",
        "doctor",
        "mobile",
        "bench",
        "contract",
        "docs",
        "update",
        "uninstall",
    ] {
        let output = run(&home, &[command, "--help"]);
        assert!(
            output.status.success(),
            "`zenx {command} --help` failed: {}",
            stderr(&output)
        );
    }

    // Argument conflicts inside a subcommand only surface when that subcommand
    // is built, so every nested command is constructed here too.
    let nested: &[(&str, &str)] = &[
        ("config", "show"),
        ("config", "get"),
        ("config", "set"),
        ("config", "path"),
        ("trace", "sessions"),
        ("trace", "inspect"),
        ("trace", "export"),
        ("trace", "record"),
        ("trace", "start"),
        ("trace", "end"),
        ("trace", "import"),
        ("trace", "prune"),
        ("trace", "adapters"),
        ("trace", "delete"),
        ("skills", "list"),
        ("skills", "search"),
        ("skills", "install"),
        ("skills", "verify"),
        ("skills", "update"),
        ("skills", "rollback"),
        ("skills", "remove"),
        ("mobile", "diagnose"),
        ("mobile", "backup"),
        ("mobile", "restore"),
        ("bench", "run"),
        ("bench", "compare"),
        ("bench", "history"),
        ("contract", "compare"),
        ("docs", "add"),
        ("docs", "search"),
        ("docs", "list"),
        ("docs", "remove"),
    ];
    for (parent, child) in nested {
        let output = run(&home, &[parent, child, "--help"]);
        assert!(
            output.status.success(),
            "`zenx {parent} {child} --help` failed: {}",
            stderr(&output)
        );
    }
    let _ = std::fs::remove_dir_all(&home);
}
