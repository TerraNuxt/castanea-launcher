//! The castanea launcher without UI: game builds, profiles, launching, mods and the mod index. See docs/design.md.

pub mod archive;
pub mod download;
pub mod error;
pub mod install;
pub mod json_file;
pub mod launch;
pub mod mods;
pub mod paths;
pub mod profile;
pub mod settings;
pub mod version;

pub use error::{Error, Result};

#[cfg(test)]
mod test_server;
