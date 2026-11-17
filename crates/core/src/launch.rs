//! Starting the game: the plan, the game folder, and the running process.

pub mod plan;
pub mod prepare;
pub mod run;

pub use plan::{Plan, plan};
pub use run::{Outcome, Running};

use crate::install::Build;
use crate::paths::Paths;
use crate::settings::GameOptions;
use crate::{Error, Result};

/// Prepares the game folder and starts `build` in it with `options`.
pub fn launch(paths: &Paths, build: &Build, options: &GameOptions, log_limit: usize) -> Result<(Plan, Running)> {
    if !build.is_present() {
        return Err(Error::invalid(format!("{} is missing", build.binary.display())));
    }
    let game_dir = paths.game();
    let plan = plan(options, build, game_dir.clone())?;
    prepare::prepare(&game_dir, build, &options.backend_url)?;
    let running = Running::start(&plan, log_limit)?;
    Ok((plan, running))
}

#[cfg(test)]
mod tests;
