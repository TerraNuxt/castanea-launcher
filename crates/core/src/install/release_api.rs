//! The release API client (docs/release-api.md): the newest build for this platform, and its resumable download.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::build::Build;
use super::notes;
use super::release::{Channel, Platform, Product, Release};
use super::source::GameSource;
use crate::archive::{self, Limits};
use crate::download::{Client, Progress, Request, UrlPolicy};
use crate::settings::{DEFAULT_RELEASE_URL, Settings};
use crate::signature::{self, Fields};
use crate::trust::TrustStore;
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
    pub product: Product,
    /// The keys a release signature must verify against.
    pub trust: TrustStore,
    /// Installs a game build without a valid signature; only ever honoured by debug builds (`Settings::allow_unsigned`).
    pub allow_unsigned: bool,
}

impl ReleaseApi {
    /// The server from `CASTANEA_RELEASE_URL`, else the setting, for the platform this launcher runs on.
    pub fn from_settings(settings: &Settings) -> Result<Self> {
        let env = std::env::var(URL_ENV).ok();
        let mut api = Self::new(&base_url(settings, env.as_deref()), Platform::detect()?, settings.beta)?;
        api.allow_unsigned = settings.unsigned_allowed();
        Ok(api)
    }

    /// `base` is `https://…`, or `http://` to the loopback (tests).
    pub fn new(base: &str, platform: Platform, beta: bool) -> Result<Self> {
        let base = if base.trim().is_empty() { DEFAULT_RELEASE_URL } else { base.trim() };
        let base = base.trim_end_matches('/').to_owned();
        UrlPolicy { allow_loopback_http: true }.check(&base)?;
        Ok(Self { base, platform, beta, product: Product::Game, trust: TrustStore::current()?, allow_unsigned: false })
    }

    /// The same server for the launcher's own releases; an unsigned launcher is never accepted.
    pub fn for_launcher(mut self) -> Self {
        self.product = Product::Launcher;
        self.allow_unsigned = false;
        self
    }

    pub fn with_trust(mut self, trust: TrustStore) -> Self {
        self.trust = trust;
        self
    }

    /// Downloads the release's zip into `downloads_dir`, resuming a partial file, checks its SHA-256 and then its
    /// signature against the downloaded file's own hash and size. Nothing is unpacked before both hold.
    pub fn fetch_verified(
        &self,
        client: &Client,
        release: &Release,
        downloads_dir: &Path,
        max_bytes: u64,
        progress: Progress<'_>,
        cancel: &AtomicBool,
    ) -> Result<PathBuf> {
        let zip = downloads_dir.join(format!("{}.zip", release.sha256));
        let request = Request {
            url: &release.url,
            sha256: &release.sha256,
            size: Some(release.size),
            max_bytes,
            headers: vec![("Accept", "application/zip".to_owned())],
        };
        client.download_resumable(&request, &zip, progress, cancel)?;
        // `download_resumable` returned only after the file hashed to `release.sha256`; the size is the file's own.
        let size = fs::metadata(&zip).map_err(|error| Error::io(&zip, error))?.len();
        if let Err(error) = self.verify(release, &release.sha256, size) {
            let _ = fs::remove_file(&zip);
            return Err(error);
        }
        Ok(zip)
    }

    /// The signature check, with the API's fields and the file's real hash and size.
    pub fn verify(&self, release: &Release, sha256: &str, size: u64) -> Result<()> {
        let fields = Fields {
            product: self.product.as_str(),
            version: &release.version,
            platform: self.platform.as_str(),
            channel: &release.channel,
            sha256,
            size,
        };
        match signature::verify(&self.trust, &fields, release.signature.as_deref(), release.key_id.as_deref()) {
            Err(Error::Signature(_)) if self.allow_unsigned && self.product == Product::Game => Ok(()),
            result => result,
        }
    }

    /// A client for this server: plain HTTP only when the server is on the loopback.
    pub fn client(&self) -> Client {
        Client::new(UrlPolicy { allow_loopback_http: loopback(&self.base) })
    }

    /// The newest release of one channel, or `None` when there is none yet.
    pub fn latest_in(&self, client: &Client, channel: Channel) -> Result<Option<Release>> {
        let url = format!(
            "{}/v1/releases/latest?platform={}&channel={}{}",
            self.base,
            self.platform.as_str(),
            channel.as_str(),
            self.product_query()
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
        let url = format!(
            "{}/v1/releases?platform={}&channel={}{}",
            self.base,
            self.platform.as_str(),
            channel.as_str(),
            self.product_query()
        );
        let bytes = client.get_bytes(&url, &[("Accept", "application/json".to_owned())])?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| Error::json(&url, error))?;
        let items =
            value.get("releases").and_then(|r| r.as_array()).ok_or_else(|| Error::json(&url, "no `releases` list"))?;
        items.iter().map(|item| self.release_value(item.clone())).collect()
    }

    /// The game is the server's default product and is left out, so the game's requests stay as they were.
    fn product_query(&self) -> String {
        match self.product {
            Product::Game => String::new(),
            product => format!("&product={}", product.as_str()),
        }
    }

    fn release(&self, body: &[u8]) -> Result<Release> {
        let value = serde_json::from_slice(body).map_err(|error| Error::json("release", error))?;
        self.release_value(value)
    }

    /// Parses one release and checks that it is for this platform and that its download is on this server.
    pub(super) fn release_value(&self, value: serde_json::Value) -> Result<Release> {
        let release = Release::parse(value)?;
        if !release.product.is_empty() && release.product != self.product.as_str() {
            return Err(Error::invalid(format!(
                "release {}: it is a {} build, not a {} build",
                release.version,
                release.product,
                self.product.as_str()
            )));
        }
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

pub(crate) fn loopback(url: &str) -> bool {
    UrlPolicy { allow_loopback_http: true }.check(url).is_ok() && url.starts_with("http://")
}

impl ReleaseApi {
    /// The unpacked folder must hold this platform's binary and `assets/`; it is then described by its `install.json`.
    fn finish(&self, release: &Release, dest: &Path) -> Result<Build> {
        let binary = dest.join(self.platform.binary());
        if !binary.is_file() || !dest.join("assets").is_dir() {
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
        let zip = self.fetch_verified(client, release, downloads_dir, MAX_ARCHIVE, progress, cancel)?;
        let unpacked = archive::extract(&zip, dest, Limits::GAME);
        let _ = fs::remove_file(&zip);
        let built = unpacked.and_then(|()| self.finish(release, dest));
        // A failed unpack or check leaves no staging folder behind; the installed build is not touched.
        if built.is_err() {
            let _ = fs::remove_dir_all(dest);
        }
        built
    }
}
