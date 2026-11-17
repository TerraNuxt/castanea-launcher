//! Game builds from the releases of a GitHub repository (`TerraNuxt/castanea` by default).

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use serde::Deserialize;

use super::local;
use super::registry::{Build, Origin};
use super::source::{Asset, GameSource, RemoteBuild, asset_fits, platform_target};
use crate::archive::{self, Limits};
use crate::download::{self, Client, Progress, Request};
use crate::paths::safe_name;
use crate::{Error, Result};

pub const API: &str = "https://api.github.com";
const MAX_ARCHIVE: u64 = 4 << 30;

pub struct GithubReleases {
    pub repo: String,
    /// Sent only to the API and GitHub's download hosts; redirects drop it.
    pub token: Option<String>,
    /// `https://api.github.com`; tests point it at a local server.
    pub api: String,
    pub target: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    url: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

impl GithubReleases {
    pub fn new(repo: &str, token: Option<String>) -> Self {
        Self { repo: repo.to_owned(), token, api: API.to_owned(), target: platform_target().to_owned() }
    }

    fn headers(&self, accept: &str) -> Vec<(&'static str, String)> {
        let mut headers = vec![("Accept", accept.to_owned()), ("X-GitHub-Api-Version", "2022-11-28".to_owned())];
        if let Some(token) = &self.token {
            headers.push(("Authorization", format!("Bearer {token}")));
        }
        headers
    }

    /// Parses the API's release list; drafts and releases without a fitting asset are kept, the asset left empty.
    pub fn parse_releases(&self, json: &[u8]) -> Result<Vec<RemoteBuild>> {
        let releases: Vec<ApiRelease> =
            serde_json::from_slice(json).map_err(|error| Error::json("GitHub releases", error))?;
        Ok(releases
            .into_iter()
            .filter(|r| !r.draft)
            .map(|release| {
                let sums = release
                    .assets
                    .iter()
                    .find(|a| a.name == "SHA256SUMS")
                    .map(|a| (a.browser_download_url.clone(), a.url.clone()));
                let asset = release.assets.iter().find(|a| asset_fits(&a.name, &self.target)).map(|a| Asset {
                    name: a.name.clone(),
                    download_url: a.browser_download_url.clone(),
                    api_url: a.url.clone(),
                    size: a.size,
                    sha256: a.digest.as_deref().and_then(|d| download::normalise_sha256(d).ok()),
                    sums_url: sums.clone(),
                });
                RemoteBuild {
                    id: safe_name(&release.tag_name),
                    version: release.tag_name.trim_start_matches(['v', 'V']).to_owned(),
                    title: release.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| release.tag_name.clone()),
                    published: release.published_at.unwrap_or_default(),
                    notes: release.body.unwrap_or_default(),
                    prerelease: release.prerelease,
                    asset,
                }
            })
            .collect())
    }

    /// The asset's checksum: its digest, or its line in the release's `SHA256SUMS`.
    fn checksum(&self, client: &Client, asset: &Asset) -> Result<String> {
        if let Some(sha) = &asset.sha256 {
            return Ok(sha.clone());
        }
        let Some((plain, api)) = &asset.sums_url else {
            return Err(Error::refused(format!("{}: the release has no checksum (digest or SHA256SUMS)", asset.name)));
        };
        let (url, accept) = if self.token.is_some() { (api, "application/octet-stream") } else { (plain, "*/*") };
        let text = client.get_bytes(url, &self.headers(accept))?;
        parse_sums(&String::from_utf8_lossy(&text), &asset.name)
            .ok_or_else(|| Error::refused(format!("{}: not listed in SHA256SUMS", asset.name)))
    }
}

/// The hash for `name` in a `sha256sum` style file (`<hex>  <name>` or `<hex> *<name>`).
pub fn parse_sums(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        let file = file.trim().trim_start_matches('*');
        (file == name).then(|| download::normalise_sha256(hash).ok()).flatten()
    })
}

impl GameSource for GithubReleases {
    fn name(&self) -> String {
        format!("GitHub {}", self.repo)
    }

    fn list(&self, client: &Client) -> Result<Vec<RemoteBuild>> {
        let url = format!("{}/repos/{}/releases?per_page=30", self.api, self.repo);
        let bytes =
            client.get_bytes(&url, &self.headers("application/vnd.github+json")).map_err(|error| match error {
                Error::Http { url, message } if message.contains("404") => Error::Http {
                    url,
                    message: "HTTP 404: no such repository, or it is private and needs a token".to_owned(),
                },
                other => other,
            })?;
        self.parse_releases(&bytes)
    }

    fn install(
        &self,
        client: &Client,
        build: &RemoteBuild,
        versions_dir: &Path,
        downloads_dir: &Path,
        progress: Progress<'_>,
        cancel: &AtomicBool,
    ) -> Result<Build> {
        let asset = build
            .asset
            .as_ref()
            .ok_or_else(|| Error::invalid(format!("{}: no archive for {}", build.id, self.target)))?;
        let sha256 = self.checksum(client, asset)?;
        let (url, accept) = if self.token.is_some() {
            (&asset.api_url, "application/octet-stream")
        } else {
            (&asset.download_url, "*/*")
        };
        let zip = downloads_dir.join(format!("{sha256}.zip"));
        let request = Request {
            url,
            sha256: &sha256,
            size: Some(asset.size),
            max_bytes: MAX_ARCHIVE,
            headers: self.headers(accept),
        };
        client.download(&request, &zip, progress, cancel)?;
        let root = versions_dir.join(&build.id);
        let unpacked = archive::extract(&zip, &root, Limits::GAME);
        let _ = fs::remove_file(&zip);
        unpacked?;
        let Some(binary) = local::find_binary(&root) else {
            let _ = fs::remove_dir_all(&root);
            return Err(Error::invalid(format!("{}: the archive has no castanea binary next to assets/", asset.name)));
        };
        let built = Build {
            id: build.id.clone(),
            version: build.version.clone(),
            root,
            binary,
            origin: Origin::Github { repo: self.repo.clone(), tag: build.id.clone(), asset: asset.name.clone() },
            sha256: Some(sha256),
            managed: true,
        };
        built.write_install_file()?;
        Ok(built)
    }
}
