//! Starting the game: the plan, the game folder, and the running process.

pub mod plan;
pub mod prepare;
pub mod run;

pub use plan::{Plan, plan};
pub use run::{Outcome, Running};

use crate::install::{Build, Registry};
use crate::paths::Paths;
use crate::profile::Profile;
use crate::{Error, Result, version};

/// The profile's build: the one it names, or the newest present build.
pub fn pick_build<'a>(registry: &'a Registry, profile: &Profile) -> Result<&'a Build> {
    if let Some(id) = &profile.build {
        let build = registry.get(id).ok_or_else(|| Error::invalid(format!("build `{id}` is not installed")))?;
        if !build.is_present() {
            return Err(Error::invalid(format!("build `{id}`: {} is missing", build.binary.display())));
        }
        return Ok(build);
    }
    newest(registry).ok_or_else(|| Error::invalid("no game build is installed"))
}

/// The present build with the highest version; builds with unknown versions come last.
pub fn newest(registry: &Registry) -> Option<&Build> {
    registry.builds.iter().filter(|b| b.is_present()).max_by(|a, b| {
        match (version::parse(&a.version), version::parse(&b.version)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (None, None) => b.id.cmp(&a.id),
        }
    })
}

/// Prepares the game folder and starts the game for `profile`.
pub fn launch(paths: &Paths, registry: &Registry, profile: &Profile, log_limit: usize) -> Result<(Plan, Running)> {
    let build = pick_build(registry, profile)?;
    let game_dir = profile.game_dir(paths);
    let plan = plan(profile, build, game_dir.clone())?;
    prepare::prepare(&game_dir, build, &profile.backend_url)?;
    let running = Running::start(&plan, log_limit)?;
    Ok((plan, running))
}

#[cfg(test)]
mod tests;
