//! `ModIndex` and the static-file index: a local JSON file or one `https://` URL, cached for offline use.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::format::Index;
use crate::download::{Client, MAX_DOCUMENT, sha256_hex};
use crate::json_file;
use crate::{Error, Result};

/// Where an index came from this time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// Fetched now (or a local file).
    Fresh,
    /// The server said it has not changed since the cached copy.
    NotModified,
    /// The cached copy, because the launcher is offline or the fetch failed.
    Cached,
}

#[derive(Clone, Debug)]
pub struct Loaded {
    pub index: Index,
    pub freshness: Freshness,
    /// Seconds since 1970 of the last good fetch.
    pub fetched_at: u64,
    /// Why the cached copy was used, when a fetch failed.
    pub warning: Option<String>,
}

/// Something that gives a mod index.
pub trait ModIndex: Send + Sync {
    fn location(&self) -> &str;
    /// `offline` uses only the cache (or a local file).
    fn load(&self, client: &Client, offline: bool) -> Result<Loaded>;
}

/// One JSON file: a path on disk, a `file://` URL, or an `https://` URL.
pub struct StaticIndex {
    pub location: String,
    pub cache_dir: PathBuf,
}

#[derive(Default, Serialize, Deserialize)]
struct Meta {
    etag: Option<String>,
    fetched_at: u64,
}

impl StaticIndex {
    pub fn new(location: &str, cache_dir: &Path) -> Self {
        Self { location: location.trim().to_owned(), cache_dir: cache_dir.to_path_buf() }
    }

    fn is_remote(&self) -> bool {
        self.location.starts_with("https://") || self.location.starts_with("http://")
    }

    fn cache_files(&self) -> (PathBuf, PathBuf) {
        let key = &sha256_hex(self.location.as_bytes())[..32];
        (self.cache_dir.join(format!("{key}.json")), self.cache_dir.join(format!("{key}.meta.json")))
    }

    fn cached(&self, client: &Client, warning: Option<String>) -> Result<Loaded> {
        let (body, meta) = self.cache_files();
        let bytes = fs::read(&body).map_err(|_| match &warning {
            Some(warning) => Error::invalid(warning.clone()),
            None => Error::invalid(format!("{}: not cached yet; go online once", self.location)),
        })?;
        let meta: Meta = json_file::load_or_default(&meta).unwrap_or_default();
        let index = Index::parse(&bytes, &self.location, client.policy)?;
        Ok(Loaded { index, freshness: Freshness::Cached, fetched_at: meta.fetched_at, warning })
    }

    fn fetch(&self, client: &Client) -> Result<Loaded> {
        let (body, meta_file) = self.cache_files();
        let meta: Meta = json_file::load_or_default(&meta_file).unwrap_or_default();
        let mut headers = vec![("Accept", "application/json".to_owned())];
        if let Some(etag) = meta.etag.as_ref().filter(|_| body.is_file()) {
            headers.push(("If-None-Match", etag.clone()));
        }
        let fetched = client.get(&self.location, &headers)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        match fetched.status {
            304 => {
                let mut loaded = self.cached(client, None)?;
                loaded.freshness = Freshness::NotModified;
                loaded.fetched_at = now;
                json_file::save(&meta_file, &Meta { etag: meta.etag, fetched_at: now })?;
                Ok(loaded)
            }
            200 => {
                let etag = fetched.etag.clone();
                let bytes = fetched.read_limited(&self.location, MAX_DOCUMENT)?;
                let index = Index::parse(&bytes, &self.location, client.policy)?;
                json_file::write_atomic(&body, &bytes)?;
                json_file::save(&meta_file, &Meta { etag, fetched_at: now })?;
                Ok(Loaded { index, freshness: Freshness::Fresh, fetched_at: now, warning: None })
            }
            status => Err(Error::http(&self.location, format!("HTTP {status}"))),
        }
    }
}

impl ModIndex for StaticIndex {
    fn location(&self) -> &str {
        &self.location
    }

    fn load(&self, client: &Client, offline: bool) -> Result<Loaded> {
        if !self.is_remote() {
            let path = PathBuf::from(self.location.strip_prefix("file://").unwrap_or(&self.location));
            let bytes = fs::read(&path).map_err(|error| Error::io(&path, error))?;
            let index = Index::parse(&bytes, &self.location, client.policy)?;
            return Ok(Loaded { index, freshness: Freshness::Fresh, fetched_at: 0, warning: None });
        }
        if offline {
            return self.cached(client, None);
        }
        match self.fetch(client) {
            Ok(loaded) => Ok(loaded),
            Err(Error::Refused(message)) => Err(Error::Refused(message)),
            Err(error) => self.cached(client, Some(error.to_string())),
        }
    }
}

/// Loads every index and merges them in order; failures are returned as warnings next to what loaded.
pub fn load_all(indexes: &[Box<dyn ModIndex>], client: &Client, offline: bool) -> (Index, Vec<String>) {
    let mut merged = Index { format: super::format::FORMAT, ..Index::default() };
    let mut warnings = Vec::new();
    for source in indexes {
        match source.load(client, offline) {
            Ok(loaded) => {
                if let Some(warning) = loaded.warning {
                    warnings.push(format!("{}: {warning} (showing the cached copy)", source.location()));
                }
                merged.merge(loaded.index);
            }
            Err(error) => warnings.push(error.to_string()),
        }
    }
    (merged, warnings)
}

/// A picture from the index (icon or screenshot), cached by URL; at most 4 MiB.
pub fn image(client: &Client, url: &str, cache_dir: &Path, offline: bool) -> Result<Vec<u8>> {
    let file = cache_dir.join("images").join(&sha256_hex(url.as_bytes())[..32]);
    if let Ok(bytes) = fs::read(&file) {
        return Ok(bytes);
    }
    if offline {
        return Err(Error::invalid(format!("{url}: not cached")));
    }
    let fetched = client.get(url, &[])?;
    if fetched.status != 200 {
        return Err(Error::http(url, format!("HTTP {}", fetched.status)));
    }
    let bytes = fetched.read_limited(url, 4 << 20)?;
    json_file::write_atomic(&file, &bytes)?;
    Ok(bytes)
}
