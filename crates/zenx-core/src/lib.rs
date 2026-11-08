//! zenX core engine.
//!
//! Every zenX crate depends on this one and nothing in it depends on a module.
//! It owns the pieces that must behave identically everywhere: the unified error
//! type, configuration loading and validation, filesystem path resolution,
//! output formatting (human or JSON) and structured logging.

pub mod config;
pub mod error;
pub mod logging;
pub mod module;
pub mod output;
pub mod paths;
pub mod util;

pub use error::{Result, ZenxError};
