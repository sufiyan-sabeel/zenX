use zenx_core::config::Config;
use zenx_core::output::{OutputFormat, Reporter};
use zenx_core::paths::ZenxPaths;
use zenx_core::Result;
use zenx_storage::Db;

use crate::OutputArg;

/// Everything a command needs: where state lives, how to render output, and a
/// handle to the database on demand.
pub struct Context {
    pub paths: ZenxPaths,
    pub config: Config,
    pub reporter: Reporter,
}

impl Context {
    pub fn load(output: Option<OutputArg>, no_color: bool) -> Result<Context> {
        let paths = ZenxPaths::resolve()?;
        let config = Config::load(&paths.config_file())?;
        let format = match output {
            Some(OutputArg::Json) => OutputFormat::Json,
            Some(OutputArg::Human) => OutputFormat::Human,
            None => OutputFormat::parse(&config.output.format).unwrap_or(OutputFormat::Human),
        };
        let color = config.output.color && !no_color;
        Ok(Context {
            paths,
            config,
            reporter: Reporter::new(format, color),
        })
    }

    /// Open (and migrate) the database, creating the home directory if needed.
    pub fn db(&self) -> Result<Db> {
        self.paths.ensure()?;
        Db::open(&self.paths.database_file())
    }

    /// Skills targets from configuration.
    pub fn skill_targets(&self) -> Vec<zenx_core::config::SkillTarget> {
        self.config.skills.targets.clone()
    }
}
