//! Starting the game: the plan, the game folder, and the running process.

pub mod detach;
pub mod plan;
pub mod prepare;
pub mod run;
pub mod start;
pub mod tail;

pub use plan::{Plan, plan, plan_for};
pub use run::{Outcome, Running, library_package, missing_library};
pub use start::Start;

use crate::install::Build;
use crate::paths::Paths;
use crate::settings::GameOptions;
use crate::{Error, Result};

/// Prepares the game folder and starts `build` in it with `options`, at the game's start screen.
pub fn launch(paths: &Paths, build: &Build, options: &GameOptions, log_limit: usize) -> Result<(Plan, Running)> {
    launch_at(paths, build, options, &Start::Menu, log_limit)
}

/// `launch`, but into the world or onto the server `start` names.
pub fn launch_at(
    paths: &Paths,
    build: &Build,
    options: &GameOptions,
    start: &Start,
    log_limit: usize,
) -> Result<(Plan, Running)> {
    if !build.is_present() {
        return Err(Error::invalid(format!("{} is missing", build.binary.display())));
    }
    let game_dir = paths.game();
    let plan = plan_for(options, build, game_dir.clone(), start)?;
    prepare::prepare(&game_dir, build)?;
    let running = Running::start(&plan, log_limit)?;
    Ok((plan, running))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod independence_tests;
