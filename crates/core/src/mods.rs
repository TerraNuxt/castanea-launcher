//! A profile's mods: reading `mod.json`, listing, enabling, removing, importing and checking.

pub mod checks;
pub mod import;
pub mod installed;
pub mod manifest;

pub use installed::{InstalledMod, Packaging};
pub use manifest::Manifest;

#[cfg(test)]
mod tests;
