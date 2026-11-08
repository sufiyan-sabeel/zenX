//! Versioned schema. Append to `MIGRATIONS`; never edit an applied entry.

/// Tables that exist after all migrations have run.
pub const TABLES: &[&str] = &[
    "sessions",
    "events",
    "skills",
    "skill_backups",
    "bench_runs",
    "bench_results",
    "doc_collections",
    "docs",
    "contract_runs",
];

/// One SQL batch per schema version.
pub const MIGRATIONS: &[&str] = &[
    // v1 — initial schema
    r#"
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    agent       TEXT NOT NULL,
    label       TEXT,
    cwd         TEXT NOT NULL,
    status      TEXT NOT NULL,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    metadata    TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    kind        TEXT NOT NULL,
    name        TEXT NOT NULL,
    started_at  TEXT NOT NULL,
    duration_ms INTEGER,
    status      TEXT NOT NULL,
    payload     TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);

CREATE TABLE IF NOT EXISTS skills (
    name         TEXT PRIMARY KEY,
    source       TEXT NOT NULL,
    commit_sha   TEXT,
    version      TEXT,
    installed_at TEXT NOT NULL,
    checksum     TEXT NOT NULL,
    manifest     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_backups (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    path       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    reason     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_skill_backups_name ON skill_backups(name, created_at);

CREATE TABLE IF NOT EXISTS bench_runs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    label      TEXT NOT NULL,
    task_file  TEXT NOT NULL,
    repeat     INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    ended_at   TEXT,
    summary    TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS bench_results (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      INTEGER NOT NULL REFERENCES bench_runs(id) ON DELETE CASCADE,
    task_id     TEXT NOT NULL,
    iteration   INTEGER NOT NULL,
    passed      INTEGER NOT NULL,
    exit_code   INTEGER,
    duration_ms INTEGER NOT NULL,
    stdout      TEXT NOT NULL DEFAULT '',
    stderr      TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_bench_results_run ON bench_results(run_id, task_id);

CREATE TABLE IF NOT EXISTS doc_collections (
    collection  TEXT NOT NULL,
    version     TEXT NOT NULL,
    source      TEXT NOT NULL,
    imported_at TEXT NOT NULL,
    doc_count   INTEGER NOT NULL,
    PRIMARY KEY (collection, version)
);

CREATE TABLE IF NOT EXISTS docs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    collection  TEXT NOT NULL,
    version     TEXT NOT NULL,
    path        TEXT NOT NULL,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    UNIQUE (collection, version, path)
);
CREATE INDEX IF NOT EXISTS idx_docs_collection ON docs(collection, version);

CREATE TABLE IF NOT EXISTS contract_runs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    base_path  TEXT NOT NULL,
    head_path  TEXT NOT NULL,
    breaking   INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    report     TEXT NOT NULL
);
"#,
];
