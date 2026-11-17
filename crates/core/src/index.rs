//! The mod index: its format, the static-file client with a cache, installs and updates. See docs/mod-index.md.

pub mod client;
pub mod format;
pub mod install;

pub use client::{Freshness, Loaded, ModIndex, StaticIndex};
pub use format::{Index, IndexMod, IndexVersion, News};

#[cfg(test)]
mod tests;
