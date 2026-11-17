//! `GameSource`: something that offers the newest game release and installs one into a folder.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::build::Build;
use super::release::Release;
use crate::Result;
use crate::download::{Client, Progress};

pub trait GameSource: Send + Sync {
    /// The newest release for this platform, `None` when nothing is published yet.
    fn latest(&self, client: &Client) -> Result<Option<Release>>;
    /// Downloads `release` (resuming a partial file), checks its SHA-256 and unpacks it into `dest` (which must not
    /// exist yet).
    fn install(
        &self,
        client: &Client,
        release: &Release,
        dest: &Path,
        downloads_dir: &Path,
        progress: Progress<'_>,
        cancel: &AtomicBool,
    ) -> Result<Build>;
}
