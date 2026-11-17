//! The game build: the one installed release, where releases come from, and updating to the newest one.

pub mod build;
pub mod github;
pub mod local;
pub mod source;
pub mod token;
pub mod update;

pub use build::Build;
pub use source::{GameSource, RemoteBuild};

#[cfg(test)]
mod tests;
