//! Resumable downloads: the partial file stays after a network drop or a restart and the next try continues it with an
//! HTTP `Range` request. The file is the build with the given SHA-256, whatever happens on the way.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::{Client, Fetched, Progress, Request, is_transient, normalise_sha256, sha256_file};
use crate::{Error, Result};

/// How one request ended, when it did not fail for good.
enum Step {
    /// The body ended and the file is as long as the server said.
    Complete,
    /// Worth another try after a wait: a drop, a 429 or a 5xx.
    Again { error: Error, wait: Option<Duration>, grew: bool },
    /// The partial file is not usable; start from zero.
    Restart,
}

/// `"<hex>"` or `W/"<hex>"` to `<hex>`.
fn etag_value(etag: &str) -> String {
    etag.trim().trim_start_matches("W/").trim_matches('"').to_ascii_lowercase()
}

/// `bytes 100-199/200` to (100, 200).
fn content_range(text: &str) -> Option<(u64, u64)> {
    let rest = text.trim().strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    Some((range.split_once('-')?.0.parse().ok()?, total.parse().ok()?))
}

fn length(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |m| m.len())
}

impl Client {
    /// Downloads to `dest` through `dest.part`, continuing a partial file that is already there. The server's `ETag`
    /// must be the SHA-256 (the release API's contract), so the bytes of a resumed file belong to one build; the
    /// finished file is hashed and a mismatch deletes it. A cancel keeps the partial file for the next start.
    pub fn download_resumable(
        &self,
        request: &Request<'_>,
        dest: &Path,
        progress: Progress<'_>,
        cancel: &AtomicBool,
    ) -> Result<()> {
        let expected = normalise_sha256(request.sha256)?;
        let part = dest.with_extension("part");
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
        if dest.is_file() {
            if sha256_file(dest)? == expected {
                return Ok(());
            }
            let _ = fs::remove_file(dest);
        }
        let mut failures = 0u32;
        let mut restarts = 0u32;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::invalid(format!("{}: cancelled", request.url)));
            }
            let have = length(&part);
            if have > request.max_bytes || request.size.is_some_and(|size| have > size) {
                let _ = fs::remove_file(&part);
                continue;
            }
            let step = if request.size == Some(have) && have > 0 {
                Step::Complete
            } else {
                self.fetch_part(request, &part, have, &expected, progress, cancel)?
            };
            match step {
                Step::Complete => break,
                Step::Restart => {
                    restarts += 1;
                    if restarts > 3 {
                        return Err(Error::http(request.url, "the server keeps answering with unusable ranges"));
                    }
                    let _ = fs::remove_file(&part);
                }
                Step::Again { error, wait, grew } => {
                    failures = if grew { 1 } else { failures + 1 };
                    if failures > self.retry.tries {
                        return Err(error);
                    }
                    self.pause(self.retry.wait(failures, wait), request.url, cancel)?;
                }
            }
        }
        let done = length(&part);
        let size_ok = request.size.is_none_or(|size| size == done);
        let actual = if size_ok { sha256_file(&part)? } else { String::new() };
        if !size_ok {
            let _ = fs::remove_file(&part);
            let size = request.size.unwrap_or(0);
            return Err(Error::invalid(format!("{}: {done} bytes, expected {size}", request.url)));
        }
        if actual != expected {
            let _ = fs::remove_file(&part);
            return Err(Error::Hash { what: request.url.to_owned(), expected, actual });
        }
        fs::rename(&part, dest).map_err(|error| Error::io(dest, error))
    }

    /// One request for the bytes from `have` on, appended to `part`.
    fn fetch_part(
        &self,
        request: &Request<'_>,
        part: &Path,
        have: u64,
        expected: &str,
        progress: Progress<'_>,
        cancel: &AtomicBool,
    ) -> Result<Step> {
        let mut headers = request.headers.clone();
        if have > 0 {
            headers.push(("Range", format!("bytes={have}-")));
            headers.push(("If-Range", format!("\"{expected}\"")));
        }
        let fetched = match self.get(request.url, &headers) {
            Ok(fetched) => fetched,
            Err(error @ Error::Http { .. }) => return Ok(Step::Again { error, wait: None, grew: false }),
            Err(error) => return Err(error),
        };
        let status = fetched.status;
        if is_transient(status) {
            let error = Error::http(request.url, format!("HTTP {status}"));
            return Ok(Step::Again { error, wait: fetched.retry_after, grew: false });
        }
        if status == 416 && have > 0 {
            return Ok(Step::Restart);
        }
        if status != 200 && status != 206 {
            return Err(Error::http(request.url, format!("HTTP {status}")));
        }
        if let Some(etag) = &fetched.etag
            && etag_value(etag) != expected
        {
            if have > 0 {
                let _ = fs::remove_file(part);
            }
            return Err(Error::refused(format!(
                "{}: the server's file is not the release's build (ETag {etag}); check again for updates",
                request.url
            )));
        }
        let start = match (status, fetched.content_range.as_deref().and_then(content_range)) {
            (206, Some((from, total))) if from == have && request.size.is_none_or(|size| size == total) => have,
            (206, _) => return Ok(Step::Restart),
            _ => 0,
        };
        let remaining = fetched.length;
        if let Some(remaining) = remaining
            && (start + remaining > request.max_bytes || request.size.is_some_and(|size| start + remaining != size))
        {
            return Err(Error::refused(format!(
                "{}: {} bytes, expected {:?} and at most {}",
                request.url,
                start + remaining,
                request.size,
                request.max_bytes
            )));
        }
        let total = request.size.or(remaining.map(|r| start + r));
        append(fetched, part, start, remaining, request, total, progress, cancel)
    }
}

/// Writes the body to `part` from byte `start` (truncating anything after it).
#[allow(clippy::too_many_arguments)]
fn append(
    fetched: Fetched,
    part: &Path,
    start: u64,
    remaining: Option<u64>,
    request: &Request<'_>,
    total: Option<u64>,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<Step> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(start == 0)
        .open(part)
        .map_err(|e| Error::io(part, e))?;
    file.set_len(start).map_err(|error| Error::io(part, error))?;
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(start)).map_err(|error| Error::io(part, error))?;
    let mut reader = fetched.into_reader();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut done = start;
    progress(done, total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::invalid(format!("{}: cancelled", request.url)));
        }
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => {
                let error = Error::http(request.url, error);
                return Ok(Step::Again { error, wait: None, grew: done > start });
            }
        };
        if read == 0 {
            break;
        }
        done += read as u64;
        if done > request.max_bytes {
            return Err(Error::refused(format!("{}: over the limit of {} bytes", request.url, request.max_bytes)));
        }
        file.write_all(&buffer[..read]).map_err(|error| Error::io(part, error))?;
        progress(done, total);
    }
    file.sync_all().map_err(|error| Error::io(part, error))?;
    let expected_end = remaining.map(|r| start + r).or(request.size);
    if expected_end.is_some_and(|end| done < end) {
        let error = Error::http(request.url, "the connection closed before the end of the file");
        return Ok(Step::Again { error, wait: None, grew: done > start });
    }
    Ok(Step::Complete)
}
