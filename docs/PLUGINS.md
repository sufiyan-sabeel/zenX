# Extending zenX

Two extension points exist today, and both are Rust traits — there is no dynamic
plugin loading, because a `cdylib` interface would add ABI risk without adding
capability at this stage.

## A trace adapter

```rust
use zenx_core::Result;
use zenx_trace::{EventInput, TraceAdapter};

pub struct MyAgent;

impl TraceAdapter for MyAgent {
    fn name(&self) -> &'static str { "my-agent" }

    fn description(&self) -> &'static str {
        "one JSON object per line with type/name/duration fields"
    }

    fn parse_line(&self, line: &str) -> Result<Vec<EventInput>> {
        // Parse, map onto zenX event kinds, return zero or more events.
        // Returning an error counts the line as skipped; it is logged, not lost.
        todo!("parse the line")
    }
}
```

Register it in `adapters()` in `crates/zenx-trace/src/adapters.rs`, add a real
sample transcript under `examples/trace/`, and test both a valid and a malformed
line. An adapter that has not been implemented and tested must not appear in the
registry — `zenx trace adapters` is a promise to users.

## A doctor detector

```rust
use std::path::Path;
use zenx_doctor::{Check, Detector, DoctorContext, Status};

pub struct GoDetector;

impl Detector for GoDetector {
    fn name(&self) -> &'static str { "go" }
    fn detect(&self, root: &Path) -> bool { root.join("go.mod").is_file() }

    fn checks(&self, root: &Path, context: &DoctorContext) -> Vec<Check> {
        vec![Check::new(
            "go.manifest",
            "go.mod parses",
            Status::Pass,
            "go.mod present".to_string(),
        )]
    }
}
```

Register it in `detectors()`. Follow the same rules the built-in detectors
follow: read-only, no writes to the repository, builds only when the context
allows them and with output redirected outside the tree.

## A skills target

No code needed. Add a block to `config.toml`:

```toml
[[skills.targets]]
name = "my-agent"
path = "/absolute/path/to/agent/skills"
layout = "skill-dir"   # or "flat" for a single SKILL.md copy
```

`zenx skills install` then copies into it, refusing to overwrite a directory
without a zenX manifest unless you pass `--force`.

## Public library API

Each module crate is usable without the CLI:

```rust
use zenx_core::paths::ZenxPaths;
use zenx_storage::Db;
use zenx_skills::{InstallRequest, SkillSource, SkillsManager};

let paths = ZenxPaths::resolve()?;
let db = Db::open(&paths.database_file())?;
let manager = SkillsManager::new(&db, &paths);
let report = manager.install(&InstallRequest::new(SkillSource::Local {
    path: "examples/skills/demo-skill".into(),
}))?;
println!("installed {} ({})", report.name, report.checksum_sha256);
```

The rule that keeps this stable: modules never depend on the CLI, and every
public function returns `zenx_core::Result`.
