//! `GameSource`: something that lists game builds and installs one.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::registry::Build;
use crate::Result;
use crate::download::{Client, Progress};

/// One build a source offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteBuild {
    /// The id it gets when installed, such as the release tag.
    pub id: String,
    pub version: String,
    pub title: String,
    pub published: String,
    pub notes: String,
    pub prerelease: bool,
    /// The archive for this platform, or `None` when the release has none.
    pub asset: Option<Asset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    /// The URL to download without a token.
    pub download_url: String,
    /// The API URL that serves the bytes with a token (`Accept: application/octet-stream`).
    pub api_url: String,
    pub size: u64,
    /// From GitHub's `digest` field; a `SHA256SUMS` asset is the fallback.
    pub sha256: Option<String>,
    pub sums_url: Option<(String, String)>,
}

pub trait GameSource: Send + Sync {
    /// A name for the UI, such as `GitHub TerraNuxt/castanea`.
    fn name(&self) -> String;
    fn list(&self, client: &Client) -> Result<Vec<RemoteBuild>>;
    /// Installs `build` below `versions_dir` (or registers it in place) and returns it, not yet in the registry.
    fn install(
        &self,
        client: &Client,
        build: &RemoteBuild,
        versions_dir: &Path,
        downloads_dir: &Path,
        progress: Progress<'_>,
        cancel: &AtomicBool,
    ) -> Result<Build>;
}

/// The name part a release asset must carry for this platform.
pub fn platform_target() -> &'static str {
    if cfg!(windows) { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }
}

/// Whether an asset name is a game archive for `target`: `castanea-<version>-<target>.zip`, or the short
/// forms `...-windows.zip` and `...-linux.zip`.
pub fn asset_fits(name: &str, target: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(stem) = lower.strip_suffix(".zip") else { return false };
    if !stem.starts_with("castanea") {
        return false;
    }
    let os = if target.contains("windows") { "windows" } else { "linux" };
    stem.ends_with(target) || stem.ends_with(&format!("-{os}")) || stem.ends_with(&format!("-{os}-x86_64"))
}
