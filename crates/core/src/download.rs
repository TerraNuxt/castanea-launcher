//! HTTPS downloads with a size limit, a SHA-256 check, progress, cancel and an atomic rename.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::{Error, Result};

const USER_AGENT: &str = concat!("castanea-launcher/", env!("CARGO_PKG_VERSION"));
/// Small documents (an index, a release list) larger than this are refused.
pub const MAX_DOCUMENT: u64 = 16 << 20;

/// What URLs the client may fetch. Plain HTTP only to the loopback, and only when a test asks.
#[derive(Clone, Copy, Debug, Default)]
pub struct UrlPolicy {
    pub allow_loopback_http: bool,
}

impl UrlPolicy {
    pub fn check(self, url: &str) -> Result<()> {
        if url.starts_with("https://") {
            return Ok(());
        }
        if let Some(rest) = url.strip_prefix("http://") {
            let host = rest.split(['/', '?', '#']).next().unwrap_or("");
            let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
            if self.allow_loopback_http && (name == "127.0.0.1" || name == "localhost" || name == "[::1]") {
                return Ok(());
            }
        }
        Err(Error::refused(format!("{url}: only https:// downloads are allowed")))
    }
}

/// Progress of one download: bytes so far and the total when known.
pub type Progress<'a> = &'a mut dyn FnMut(u64, Option<u64>);

pub struct Client {
    agent: ureq::Agent,
    pub policy: UrlPolicy,
}

/// One response: status, a few headers and the body as a reader.
pub struct Fetched {
    pub status: u16,
    pub etag: Option<String>,
    pub length: Option<u64>,
    body: ureq::Body,
}

impl Client {
    pub fn new(policy: UrlPolicy) -> Self {
        let config = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .https_only(!policy.allow_loopback_http)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_recv_body(Some(Duration::from_secs(60)))
            .build();
        Self { agent: config.into(), policy }
    }

    /// A GET with extra headers; any status comes back, the caller decides.
    pub fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Fetched> {
        self.policy.check(url)?;
        let mut request = self.agent.get(url);
        for (name, value) in headers {
            request = request.header(*name, value.as_str());
        }
        let response = request.call().map_err(|error| Error::http(url, error))?;
        let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
        Ok(Fetched {
            status: response.status().as_u16(),
            etag: header("etag"),
            length: header("content-length").and_then(|v| v.parse().ok()),
            body: response.into_body(),
        })
    }

    /// A small document, such as an index or a release list, as bytes. Status other than 200 is an error.
    pub fn get_bytes(&self, url: &str, headers: &[(&str, String)]) -> Result<Vec<u8>> {
        let fetched = self.get(url, headers)?;
        if fetched.status != 200 {
            return Err(Error::http(url, format!("HTTP {}", fetched.status)));
        }
        fetched.read_limited(url, MAX_DOCUMENT)
    }

    /// Downloads `url` to `dest`, checking size and SHA-256 on the way. The file appears only when it is right.
    pub fn download(&self, request: &Request<'_>, dest: &Path, progress: Progress<'_>, cancel: &AtomicBool) -> Result<()> {
        let expected = normalise_sha256(request.sha256)?;
        let fetched = self.get(request.url, &request.headers)?;
        if fetched.status != 200 {
            return Err(Error::http(request.url, format!("HTTP {}", fetched.status)));
        }
        if let Some(length) = fetched.length
            && length > request.max_bytes
        {
            return Err(Error::refused(format!("{}: {length} bytes is over the limit of {}", request.url, request.max_bytes)));
        }
        let total = request.size.or(fetched.length);
        let part = dest.with_extension("part");
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
        let result = copy_hashed(fetched.body.into_reader(), &part, request, total, progress, cancel);
        let actual = match result {
            Ok(actual) => actual,
            Err(error) => {
                let _ = fs::remove_file(&part);
                return Err(error);
            }
        };
        if actual != expected {
            let _ = fs::remove_file(&part);
            return Err(Error::Hash { what: request.url.to_owned(), expected, actual });
        }
        fs::rename(&part, dest).map_err(|error| Error::io(dest, error))
    }
}

impl Fetched {
    pub fn read_limited(self, url: &str, limit: u64) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.body.into_reader().take(limit + 1).read_to_end(&mut bytes).map_err(|error| Error::http(url, error))?;
        if bytes.len() as u64 > limit {
            return Err(Error::refused(format!("{url}: larger than {limit} bytes")));
        }
        Ok(bytes)
    }
}

/// One file to fetch: where from, what it must hash to, and how large it may be.
pub struct Request<'a> {
    pub url: &'a str,
    pub sha256: &'a str,
    /// The expected size, when the source says it; also checked.
    pub size: Option<u64>,
    pub max_bytes: u64,
    pub headers: Vec<(&'a str, String)>,
}

fn copy_hashed(
    mut reader: impl Read,
    part: &Path,
    request: &Request<'_>,
    total: Option<u64>,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<String> {
    let mut file = fs::File::create(part).map_err(|error| Error::io(part, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::invalid(format!("{}: cancelled", request.url)));
        }
        let read = reader.read(&mut buffer).map_err(|error| Error::http(request.url, error))?;
        if read == 0 {
            break;
        }
        done += read as u64;
        if done > request.max_bytes {
            return Err(Error::refused(format!("{}: over the limit of {} bytes", request.url, request.max_bytes)));
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read]).map_err(|error| Error::io(part, error))?;
        progress(done, total);
    }
    file.sync_all().map_err(|error| Error::io(part, error))?;
    if let Some(size) = request.size
        && size != done
    {
        return Err(Error::invalid(format!("{}: {done} bytes, expected {size}", request.url)));
    }
    Ok(hex(&hasher.finalize()))
}

/// Lower-case hex of 64 digits; a `sha256:` prefix (GitHub's digest field) is accepted.
pub fn normalise_sha256(text: &str) -> Result<String> {
    let text = text.trim();
    let text = text.strip_prefix("sha256:").unwrap_or(text).to_ascii_lowercase();
    if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(text)
    } else {
        Err(Error::refused(format!("`{text}` is not a SHA-256 checksum")))
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The SHA-256 of a file on disk.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|error| Error::io(path, error))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|error| Error::io(path, error))?;
    Ok(hex(&hasher.finalize()))
}

#[cfg(test)]
mod tests;
