//! zenX Bench — measured benchmarking for agent tasks.
//!
//! Every number this module reports is measured: it comes from a real process
//! execution that zenX started and waited for. There is no estimated score and
//! no synthetic baseline anywhere in the code, and a task that cannot run is a
//! failure, not a skipped line that flatters the result.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rusqlite::params;
use serde::{Deserialize, Serialize};
use zenx_core::{Result, ZenxError};
use zenx_storage::{map_err, Db};

/// Applied when a task does not set its own timeout.
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// One benchmark task, as declared in a TOML task file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDef {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Program and arguments. Never a shell string: no shell is involved.
    pub command: Vec<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub expect_exit: Option<i32>,
    #[serde(default)]
    pub expect_stdout_contains: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TaskFile {
    #[serde(default)]
    task: Vec<TaskDef>,
}

/// Load and validate a task file.
pub fn load_tasks(path: &Path) -> Result<Vec<TaskDef>> {
    let text = std::fs::read_to_string(path).map_err(|e| ZenxError::io(path, e))?;
    let file: TaskFile = toml::from_str(&text)
        .map_err(|e| ZenxError::Invalid(format!("{}: {e}", path.display())))?;
    if file.task.is_empty() {
        return Err(ZenxError::Invalid(format!(
            "{} defines no [[task]] entries",
            path.display()
        )));
    }
    let mut seen = std::collections::BTreeSet::new();
    for task in &file.task {
        if task.id.trim().is_empty() {
            return Err(ZenxError::Invalid("every task needs a non-empty id".into()));
        }
        if task.command.is_empty() {
            return Err(ZenxError::Invalid(format!(
                "task {:?} has an empty command",
                task.id
            )));
        }
        if !seen.insert(task.id.clone()) {
            return Err(ZenxError::Invalid(format!(
                "duplicate task id {:?}",
                task.id
            )));
        }
    }
    Ok(file.task)
}

#[derive(Debug, Clone)]
struct Execution {
    passed: bool,
    exit_code: Option<i32>,
    duration_ms: i64,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

fn execute(task: &TaskDef) -> Result<Execution> {
    let program = &task.command[0];
    let mut command = Command::new(program);
    command
        .args(&task.command[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let start = Instant::now();
    let mut child = command.spawn().map_err(|error| ZenxError::External {
        command: task.command.join(" "),
        code: None,
        stderr: format!("could not start {program}: {error}"),
    })?;

    // Drain the pipes on their own threads, otherwise a chatty task fills the
    // pipe buffer and blocks before it can exit.
    let out_handle = child.stdout.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut buffer = String::new();
            let _ = pipe.read_to_string(&mut buffer);
            buffer
        })
    });
    let err_handle = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut buffer = String::new();
            let _ = pipe.read_to_string(&mut buffer);
            buffer
        })
    });

    let timeout = Duration::from_secs(task.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                return Err(ZenxError::External {
                    command: task.command.join(" "),
                    code: None,
                    stderr: error.to_string(),
                })
            }
        }
    }

    let status = child.wait().map_err(|error| ZenxError::External {
        command: task.command.join(" "),
        code: None,
        stderr: error.to_string(),
    })?;
    let duration_ms = start.elapsed().as_millis() as i64;
    let stdout = out_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = err_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let exit_code = status.code();

    let mut passed = !timed_out && status.success();
    if let Some(expected) = task.expect_exit {
        passed = passed && exit_code == Some(expected);
    }
    if let Some(needle) = &task.expect_stdout_contains {
        passed = passed && stdout.contains(needle);
    }

    Ok(Execution {
        passed,
        exit_code,
        duration_ms,
        stdout,
        stderr,
        timed_out,
    })
}

fn describe_failure(execution: &Execution, task: &TaskDef) -> String {
    if execution.timed_out {
        return format!(
            "timed out after {}s",
            task.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS)
        );
    }
    if let Some(expected) = task.expect_exit {
        if execution.exit_code != Some(expected) {
            return format!("exit code {:?}, expected {expected}", execution.exit_code);
        }
    }
    if let Some(needle) = &task.expect_stdout_contains {
        if !execution.stdout.contains(needle) {
            return format!("stdout did not contain {needle:?}");
        }
    }
    match execution.exit_code {
        Some(code) => format!("exit code {code}"),
        None => "terminated by a signal".to_string(),
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        let mut end = max;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…[truncated]", &text[..end])
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub repeat: u32,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskOutcome {
    pub task_id: String,
    pub iterations: u32,
    pub passed: u32,
    pub failed: u32,
    pub timed_out: bool,
    /// Measured, never estimated.
    pub completion_rate: f64,
    pub mean_duration_ms: f64,
    pub min_duration_ms: i64,
    pub max_duration_ms: i64,
    pub failure_samples: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub run_id: i64,
    pub label: String,
    pub repeat: u32,
    pub started_at: String,
    pub ended_at: String,
    pub tasks: Vec<TaskOutcome>,
}

impl RunSummary {
    pub fn overall_completion_rate(&self) -> f64 {
        let iterations: u32 = self.tasks.iter().map(|task| task.iterations).sum();
        if iterations == 0 {
            return 0.0;
        }
        let passed: u32 = self.tasks.iter().map(|task| task.passed).sum();
        passed as f64 / iterations as f64
    }
}

/// Execute every task `repeat` times and record the results.
pub fn run(
    db: &Db,
    tasks: &[TaskDef],
    options: &RunOptions,
    task_file: &Path,
) -> Result<RunSummary> {
    let repeat = options.repeat.max(1);
    let started_at = zenx_core::util::now_rfc3339();
    let label = options.label.clone().unwrap_or_else(|| {
        let stem = task_file
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "bench".to_string());
        format!("{stem}-{}", zenx_core::util::now_rfc3339())
    });

    db.conn()
        .execute(
            "INSERT INTO bench_runs (label, task_file, repeat, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                label,
                task_file.to_string_lossy().to_string(),
                repeat as i64,
                started_at
            ],
        )
        .map_err(map_err)?;
    let run_id = db.conn().last_insert_rowid();

    let mut outcomes = Vec::new();
    for task in tasks {
        let mut durations = Vec::new();
        let mut passed = 0u32;
        let mut timed_out = false;
        let mut failure_samples = Vec::new();

        for iteration in 0..repeat {
            let execution = execute(task)?;
            if execution.passed {
                passed += 1;
            } else if failure_samples.len() < 3 {
                failure_samples.push(format!(
                    "iteration {}: {}",
                    iteration + 1,
                    describe_failure(&execution, task)
                ));
            }
            if execution.timed_out {
                timed_out = true;
            }
            durations.push(execution.duration_ms);

            db.conn()
                .execute(
                    "INSERT INTO bench_results
                        (run_id, task_id, iteration, passed, exit_code, duration_ms, stdout, stderr)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        run_id,
                        task.id,
                        iteration as i64,
                        if execution.passed { 1i64 } else { 0i64 },
                        execution.exit_code,
                        execution.duration_ms,
                        truncate(&execution.stdout, 4000),
                        truncate(&execution.stderr, 4000)
                    ],
                )
                .map_err(map_err)?;
        }

        let total: i64 = durations.iter().sum();
        let count = durations.len().max(1) as i64;
        outcomes.push(TaskOutcome {
            task_id: task.id.clone(),
            iterations: repeat,
            passed,
            failed: repeat - passed,
            timed_out,
            completion_rate: passed as f64 / repeat as f64,
            mean_duration_ms: total as f64 / count as f64,
            min_duration_ms: durations.iter().copied().min().unwrap_or(0),
            max_duration_ms: durations.iter().copied().max().unwrap_or(0),
            failure_samples,
        });
    }

    let ended_at = zenx_core::util::now_rfc3339();
    let summary = RunSummary {
        run_id,
        label: label.clone(),
        repeat,
        started_at,
        ended_at: ended_at.clone(),
        tasks: outcomes,
    };
    let summary_text = serde_json::to_string(&summary)
        .map_err(|e| ZenxError::Invalid(format!("could not serialise run summary: {e}")))?;
    db.conn()
        .execute(
            "UPDATE bench_runs SET ended_at = ?1, summary = ?2 WHERE id = ?3",
            params![ended_at, summary_text, run_id],
        )
        .map_err(map_err)?;
    Ok(summary)
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskDelta {
    pub task_id: String,
    pub base_completion: f64,
    pub head_completion: f64,
    pub completion_delta: f64,
    pub base_mean_ms: f64,
    pub head_mean_ms: f64,
    pub duration_delta_pct: f64,
    pub regression: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    pub base_run: i64,
    pub head_run: i64,
    pub tasks: Vec<TaskDelta>,
    pub regressions: usize,
}

struct RunStats {
    completion: f64,
    mean_ms: f64,
}

fn run_stats(db: &Db, run_id: i64) -> Result<std::collections::BTreeMap<String, RunStats>> {
    let mut statement = db
        .conn()
        .prepare(
            "SELECT task_id, COUNT(*), SUM(passed), AVG(duration_ms)
             FROM bench_results WHERE run_id = ?1 GROUP BY task_id",
        )
        .map_err(map_err)?;
    let rows = statement
        .query_map(params![run_id], |row| {
            let iterations: i64 = row.get(1)?;
            let passed: i64 = row.get(2)?;
            let mean: f64 = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                RunStats {
                    completion: if iterations == 0 {
                        0.0
                    } else {
                        passed as f64 / iterations as f64
                    },
                    mean_ms: mean,
                },
            ))
        })
        .map_err(map_err)?;

    let mut stats = std::collections::BTreeMap::new();
    for row in rows {
        let (task_id, values) = row.map_err(map_err)?;
        stats.insert(task_id, values);
    }
    Ok(stats)
}

fn run_exists(db: &Db, run_id: i64) -> Result<bool> {
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM bench_runs WHERE id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .map_err(map_err)?;
    Ok(count > 0)
}

/// Compare two recorded runs. A task regresses when its measured completion
/// rate drops, or when it is as correct as before but at least 20% slower.
pub fn compare(db: &Db, base_run: i64, head_run: i64) -> Result<Comparison> {
    for run in [base_run, head_run] {
        if !run_exists(db, run)? {
            return Err(ZenxError::NotFound(format!("bench run {run}")));
        }
    }
    let base = run_stats(db, base_run)?;
    let head = run_stats(db, head_run)?;

    let mut task_ids: Vec<String> = base.keys().cloned().collect();
    for id in head.keys() {
        if !task_ids.contains(id) {
            task_ids.push(id.clone());
        }
    }
    task_ids.sort();

    let mut tasks = Vec::new();
    let mut regressions = 0usize;
    for task_id in task_ids {
        let base_stats = base.get(&task_id);
        let head_stats = head.get(&task_id);
        let base_completion = base_stats.map(|s| s.completion).unwrap_or(0.0);
        let head_completion = head_stats.map(|s| s.completion).unwrap_or(0.0);
        let base_mean = base_stats.map(|s| s.mean_ms).unwrap_or(0.0);
        let head_mean = head_stats.map(|s| s.mean_ms).unwrap_or(0.0);
        let duration_delta_pct = if base_mean > 0.0 {
            (head_mean - base_mean) / base_mean * 100.0
        } else {
            0.0
        };

        let mut reason = None;
        let mut regression = false;
        if base_stats.is_none() {
            reason = Some("task is new in the head run".to_string());
        } else if head_stats.is_none() {
            regression = true;
            reason = Some("task is missing from the head run".to_string());
        } else if head_completion + 1e-9 < base_completion {
            regression = true;
            reason = Some(format!(
                "completion fell from {:.0}% to {:.0}%",
                base_completion * 100.0,
                head_completion * 100.0
            ));
        } else if (head_completion - base_completion).abs() < 1e-9 && duration_delta_pct >= 20.0 {
            regression = true;
            reason = Some(format!("same correctness, {duration_delta_pct:.0}% slower"));
        }
        if regression {
            regressions += 1;
        }

        tasks.push(TaskDelta {
            task_id,
            base_completion,
            head_completion,
            completion_delta: head_completion - base_completion,
            base_mean_ms: base_mean,
            head_mean_ms: head_mean,
            duration_delta_pct,
            regression,
            reason,
        });
    }

    Ok(Comparison {
        base_run,
        head_run,
        tasks,
        regressions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> (std::path::PathBuf, Db) {
        let root = std::env::temp_dir().join(format!("zenx-bench-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let db = Db::open_in_memory().unwrap();
        (root, db)
    }

    fn self_binary() -> String {
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .to_string()
    }

    fn passing_task(id: &str) -> TaskDef {
        TaskDef {
            id: id.into(),
            title: None,
            command: vec![self_binary(), "--list".into()],
            timeout_secs: Some(60),
            expect_exit: Some(0),
            expect_stdout_contains: None,
        }
    }

    #[test]
    fn task_file_requires_unique_ids_and_commands() {
        let (root, _db) = fixture("load");
        let path = root.join("tasks.toml");
        std::fs::write(
            &path,
            "[[task]]\nid = \"a\"\ncommand = [\"true\"]\n[[task]]\nid = \"a\"\ncommand = [\"true\"]\n",
        )
        .unwrap();
        assert!(load_tasks(&path).is_err());

        std::fs::write(&path, "[[task]]\nid = \"a\"\ncommand = []\n").unwrap();
        assert!(load_tasks(&path).is_err());

        std::fs::write(&path, "[[task]]\nid = \"a\"\ncommand = [\"true\"]\n").unwrap();
        assert_eq!(load_tasks(&path).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn run_records_measured_results() {
        let (root, db) = fixture("run");
        let task_file = root.join("tasks.toml");
        std::fs::write(&task_file, "[[task]]\nid = \"a\"\ncommand = [\"true\"]\n").unwrap();
        let tasks = vec![passing_task("a")];
        let summary = run(
            &db,
            &tasks,
            &RunOptions {
                repeat: 2,
                label: Some("test".into()),
            },
            &task_file,
        )
        .unwrap();
        assert_eq!(summary.run_id, 1);
        assert_eq!(summary.tasks[0].passed, 2);
        assert_eq!(summary.tasks[0].completion_rate, 1.0);
        assert_eq!(summary.overall_completion_rate(), 1.0);
        assert!(summary.tasks[0].mean_duration_ms >= 0.0);
        let recorded: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM bench_results", [], |row| row.get(0))
            .unwrap();
        assert_eq!(recorded, 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn expectations_are_enforced() {
        let (root, db) = fixture("expect");
        let task_file = root.join("tasks.toml");
        std::fs::write(&task_file, "[[task]]\nid = \"a\"\ncommand = [\"true\"]\n").unwrap();
        let mut task = passing_task("a");
        task.expect_exit = Some(3);
        let summary = run(&db, &[task], &RunOptions::default(), &task_file).unwrap();
        assert_eq!(summary.tasks[0].passed, 0);
        assert_eq!(summary.tasks[0].failed, 1);
        assert!(!summary.tasks[0].failure_samples.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_program_fails_the_task() {
        let (root, db) = fixture("missing");
        let task_file = root.join("tasks.toml");
        std::fs::write(&task_file, "[[task]]\nid = \"a\"\ncommand = [\"true\"]\n").unwrap();
        let task = TaskDef {
            id: "missing".into(),
            title: None,
            command: vec!["zenx-command-that-does-not-exist".into()],
            timeout_secs: None,
            expect_exit: None,
            expect_stdout_contains: None,
        };
        let error = run(&db, &[task], &RunOptions::default(), &task_file).unwrap_err();
        assert!(matches!(error, ZenxError::External { .. }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn compare_flags_completion_regressions() {
        let (root, db) = fixture("compare");
        let task_file = root.join("tasks.toml");
        std::fs::write(&task_file, "[[task]]\nid = \"a\"\ncommand = [\"true\"]\n").unwrap();

        let base = run(
            &db,
            &[passing_task("a")],
            &RunOptions::default(),
            &task_file,
        )
        .unwrap();
        let mut failing = passing_task("a");
        failing.expect_exit = Some(9);
        let head = run(&db, &[failing], &RunOptions::default(), &task_file).unwrap();

        let comparison = compare(&db, base.run_id, head.run_id).unwrap();
        assert_eq!(comparison.regressions, 1);
        assert!(comparison.tasks[0].regression);
        assert_eq!(comparison.tasks[0].base_completion, 1.0);
        assert_eq!(comparison.tasks[0].head_completion, 0.0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn compare_rejects_unknown_runs() {
        let (_root, db) = fixture("unknown");
        let error = compare(&db, 1, 2).unwrap_err();
        assert!(matches!(error, ZenxError::NotFound(_)));
    }

    #[test]
    fn truncation_is_char_safe() {
        assert_eq!(truncate("abc", 10), "abc");
        assert!(truncate("héllo wörld", 4).starts_with("hé"));
    }
}
