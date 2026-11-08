//! zenX Trace — AI agent observability.
//!
//! A session is a run of an agent; an event is one thing it did. Events are
//! appended with a monotonic sequence number so a timeline can be rebuilt
//! exactly, and payloads are redacted before they reach the database.

pub mod adapters;
pub mod export;
pub mod model;
pub mod store;

pub use adapters::{adapter_by_name, adapters, GenericJsonl, TraceAdapter};
pub use export::{export_session, render_markdown};
pub use model::{
    EventInput, EventKind, ExportFormat, ImportSummary, SessionDetail, SessionFilter,
    SessionSummary, TraceEvent,
};
pub use store::{
    append_event, create_session, delete_session, finish_session, get_session, import_jsonl,
    list_sessions, prune_sessions,
};
