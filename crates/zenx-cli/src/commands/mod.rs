use clap::Subcommand;
use zenx_core::Result;

use crate::OutputArg;

pub mod bench;
pub mod config_cmd;
pub mod contract;
pub mod docs;
pub mod doctor;
pub mod init;
pub mod mobile;
pub mod skills;
pub mod status;
pub mod trace;

/// The complete zenX command surface.
///
/// A module appears here only when its functionality exists in this build.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialise the zenX home directory, configuration and database
    Init(init::InitArgs),
    /// Show the resolved configuration, storage state and module status
    Status(status::StatusArgs),
    /// Read and change configuration values
    Config(config_cmd::ConfigArgs),
    /// Record and inspect AI agent sessions
    Trace(trace::TraceArgs),
    /// Install, verify, update and roll back agent skills
    Skills(skills::SkillsArgs),
    /// Diagnose repository health and plan repairs
    Doctor(doctor::DoctorArgs),
    /// Android and Termux environment tooling
    Mobile(mobile::MobileArgs),
    /// Run and compare reproducible benchmarks
    Bench(bench::BenchArgs),
    /// Compare OpenAPI specifications for breaking changes
    Contract(contract::ContractArgs),
    /// Import and search offline documentation
    Docs(docs::DocsArgs),
}

/// Command names, used to validate documented commands in `zenx doctor`.
pub fn command_names() -> Vec<String> {
    [
        "init", "status", "config", "trace", "skills", "doctor", "mobile", "bench", "contract",
        "docs",
    ]
    .iter()
    .map(|name| name.to_string())
    .collect()
}

pub fn dispatch(command: &Command, output: Option<OutputArg>, no_color: bool) -> Result<u8> {
    match command {
        Command::Init(args) => init::run(args, output, no_color),
        Command::Status(args) => status::run(args, output, no_color),
        Command::Config(args) => config_cmd::run(args, output, no_color),
        Command::Trace(args) => trace::run(args, output, no_color),
        Command::Skills(args) => skills::run(args, output, no_color),
        Command::Doctor(args) => doctor::run(args, output, no_color),
        Command::Mobile(args) => mobile::run(args, output, no_color),
        Command::Bench(args) => bench::run(args, output, no_color),
        Command::Contract(args) => contract::run(args, output, no_color),
        Command::Docs(args) => docs::run(args, output, no_color),
    }
}
