//! The release API client (docs/release-api.md): the newest build for this platform, and its resumable download.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::build::Build;
use super::notes;
use super::release::{Channel, Platform, Release};
use super::source::GameSource;
use crate::archive::{self, Limits};
use crate::download::{Client, Progress, Request, UrlPolicy};
use crate::settings::{DEFAULT_RELEASE_URL, Settings};
use crate::{Error, Result, version};

/// Overrides the server address (tests, staging); wins over the setting.
pub const URL_ENV: &str = "CASTANEA_RELEASE_URL";
const MAX_ARCHIVE: u64 = 4 << 30;

pub struct ReleaseApi {
    /// No trailing slash.
    pub base: String,
    pub platform: Platform,
    /// Whether `beta` builds are offered next to `stable` ones.
    pub beta: bool,
}

impl ReleaseApi {
    /// The server from `CASTANEA_RELEASE_URL`, else the setting, for the platform this launcher runs on.
    pub fn from_settings(settings: &Settings) -> Result<Self> {
        let env = std::env::var(URL_ENV).ok();
        Self::new(&base_url(settings, env.as_deref()), Platform::detect()?, settings.beta)
    }

    /// `base` is `https://…`, or `http://` to the loopback (tests).
    pub fn new(base: &str, platform: Platform, beta: bool) -> Result<Self> {
        let base = if base.trim().is_empty() { DEFAULT_RELEASE_URL } else { base.trim() };
        let base = base.trim_end_matches('/').to_owned();
        UrlPolicy { allow_loopback_http: true }.check(&base)?;
        Ok(Self { base, platform, beta })
    }

    /// A client for this server: plain HTTP only when the server is on the loopback.
    pub fn client(&self) -> Client {
        Client::new(UrlPolicy { allow_loopback_http: loopback(&self.base) })
    }

    /// The newest release of one channel, or `None` when there is none yet.
    pub fn latest_in(&self, client: &Client, channel: Channel) -> Result<Option<Release>> {
        let url = format!(
            "{}/v1/releases/latest?platform={}&channel={}",
            self.base,
            self.platform.as_str(),
            channel.as_str()
        );
        let fetched = client.get_retry(&url, &[("Accept", "application/json".to_owned())], &AtomicBool::new(false))?;
        let status = fetched.status;
        let body = fetched.read_limited(&url, crate::download::MAX_DOCUMENT)?;
        match status {
            200 => Ok(Some(self.release(&body)?)),
            404 if no_release(&body) => Ok(None),
            404 => Err(Error::http(&url, "HTTP 404: the release server has no such address (is it the right server?)")),
            429 => Err(Error::http(&url, "HTTP 429: the release server is busy, try again in a minute")),
            code => Err(Error::http(&url, format!("HTTP {code}"))),
        }
    }

    /// All releases of a channel, newest first.
    pub fn list(&self, client: &Client, channel: Channel) -> Result<Vec<Release>> {
        let url = format!("{}/v1/releases?platform={}&channel={}", self.base, self.platform.as_str(), channel.as_str());
        let bytes = client.get_bytes(&url, &[("Accept", "application/json".to_owned())])?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| Error::json(&url, error))?;
        let items =
            value.get("releases").and_then(|r| r.as_array()).ok_or_else(|| Error::json(&url, "no `releases` list"))?;
        items.iter().map(|item| self.release_value(item.clone())).collect()
    }

    fn release(&self, body: &[u8]) -> Result<Release> {
        let value = serde_json::from_slice(body).map_err(|error| Error::json("release", error))?;
        self.release_value(value)
    }

    /// Parses one release and checks that it is for this platform and that its download is on this server.
    pub(super) fn release_value(&self, value: serde_json::Value) -> Result<Release> {
        let release = Release::parse(value)?;
        if !release.platform.is_empty() && release.platform != self.platform.as_str() {
            return Err(Error::invalid(format!(
                "release {}: it is for {}, not {}",
                release.version,
                release.platform,
                self.platform.as_str()
            )));
        }
        if !release.url.starts_with(&format!("{}/", self.base)) {
            return Err(Error::refused(format!(
                "release {}: the download address {} is not on {}",
                release.version, release.url, self.base
            )));
        }
        Ok(release)
    }
}

/// The server address: the environment value if it is not empty, else the setting.
pub fn base_url(settings: &Settings, env: Option<&str>) -> String {
    env.map(str::trim).filter(|v| !v.is_empty()).unwrap_or(&settings.release_url).to_owned()
}

/// The 404 body of the contract: `{"error":"no release"}`.
fn no_release(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|e| e == "no release"))
        .unwrap_or(false)
}

fn loopback(url: &str) -> bool {
    UrlPolicy { allow_loopback_http: true }.check(url).is_ok() && url.starts_with("http://")
}

impl GameSource for ReleaseApi {
    /// The newest stable release, or the newest of stable and beta when beta builds are on.
    fn latest(&self, client: &Client) -> Result<Option<Release>> {
        let stable = self.latest_in(client, Channel::Stable)?;
        if !self.beta {
            return Ok(stable);
        }
        let beta = self.latest_in(client, Channel::Beta)?;
        Ok(match (stable, beta) {
            (Some(s), Some(b)) => Some(if version::newer(&b.version, &s.version) { b } else { s }),
            (s, b) => s.or(b),
        })
    }

    fn install(
        &self,
        client: &Client,
        release: &Release,
        dest: &Path,
        downloads_dir: &Path,
        progress: Progress<'_>,
        cancel: &AtomicBool,
    ) -> Result<Build> {
        let zip = downloads_dir.join(format!("{}.zip", release.sha256));
        let request = Request {
            url: &release.url,
            sha256: &release.sha256,
            size: Some(release.size),
            max_bytes: MAX_ARCHIVE,
            headers: vec![("Accept", "application/zip".to_owned())],
        };
        client.download_resumable(&request, &zip, progress, cancel)?;
        let unpacked = archive::extract(&zip, dest, Limits::GAME);
        let _ = fs::remove_file(&zip);
        unpacked?;
        let binary = dest.join(self.platform.binary());
        if !binary.is_file() || !dest.join("assets").is_dir() {
            let _ = fs::remove_dir_all(dest);
            return Err(Error::invalid(format!(
                "release {}: the archive must hold {} and assets/ at its top",
                release.version,
                self.platform.binary()
            )));
        }
        let built = Build {
            version: release.version.clone(),
            channel: release.channel.clone(),
            released_at: release.released_at.clone(),
            notes: notes::plain(&release.notes),
            sha256: Some(release.sha256.clone()),
            root: dest.to_path_buf(),
            binary,
        };
        built.write()?;
        Ok(built)
    }
}
