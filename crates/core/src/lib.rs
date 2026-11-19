//! The castanea launcher without UI: the game build and its updates, launching, mods and the mod index. See
//! docs/design.md.

pub mod archive;
pub mod backend;
pub mod download;
pub mod error;
pub mod index;
pub mod install;
pub mod json_file;
pub mod launch;
pub mod migrate;
pub mod mods;
pub mod news;
pub mod osname;
pub mod paths;
pub mod playername;
pub mod selfupdate;
pub mod settings;
pub mod signature;
pub mod storage;
pub mod trust;
pub mod version;

pub use error::{Error, NOT_SIGNED, Result};

#[cfg(test)]
mod privacy_tests;
#[cfg(test)]
mod release_server;
#[cfg(test)]
mod test_server;
