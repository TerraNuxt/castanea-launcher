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
pub mod settings;
pub mod version;

pub use error::{Error, Result};

#[cfg(test)]
mod test_server;
