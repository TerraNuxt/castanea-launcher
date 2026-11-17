//! Game builds: the registry of installed builds, finding them, and the sources they come from.

pub mod detect;
pub mod github;
pub mod local;
pub mod registry;
pub mod source;
pub mod token;

pub use registry::{Build, Origin, Registry};
pub use source::{GameSource, RemoteBuild};

#[cfg(test)]
mod tests;
