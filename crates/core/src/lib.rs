//! The castanea launcher without UI: the game build and its updates, launching, mods and the mod index. See
//! docs/design.md.

pub mod archive;
pub mod download;
pub mod error;
pub mod index;
pub mod install;
pub mod json_file;
pub mod launch;
pub mod migrate;
pub mod mods;
pub mod paths;
pub mod selfupdate;
pub mod settings;
pub mod signature;
pub mod trust;
pub mod version;

pub use error::{Error, NOT_SIGNED, Result};

#[cfg(test)]
mod release_server;
#[cfg(test)]
mod test_server;
