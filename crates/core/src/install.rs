//! The game build: the one installed release, the release API it comes from, and updating to the newest one.

pub mod build;
pub mod local;
pub mod notes;
pub mod release;
pub mod release_api;
pub mod source;
pub mod update;

pub use build::Build;
pub use release::{Channel, Platform, Product, Release};
pub use release_api::ReleaseApi;
pub use source::GameSource;

#[cfg(test)]
mod tests;
