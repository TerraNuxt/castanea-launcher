//! A local release server for tests, built to the contract in docs/release-api.md: latest, list, a zip download with
//! `Content-Length`, `ETag` and `Range`, 404 `{"error":"no release"}`, 429 with `Retry-After`, and a connection that
//! drops in the middle of a body.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer, SigningKey};

use crate::download::sha256_hex;
use crate::signature::tests::{sign, test_key_id};
use crate::signature::{Fields, message};
use crate::version;

/// What the server says about a release's signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sig {
    Valid,
    Missing,
    Garbage,
    /// Signed by a key that is not in the trust list, with its real key id.
    WrongKey,
    /// A good signature naming a key id nobody trusts.
    UnknownId,
}

pub struct Entry {
    pub product: String,
    pub sig: Sig,
    pub version: String,
    pub channel: String,
    pub platform: String,
    pub notes: String,
    pub zip: Vec<u8>,
    /// The hash the server claims (and sends as ETag); the real one when `None`.
    pub claimed_sha: Option<String>,
}

impl Entry {
    pub fn sha(&self) -> String {
        self.claimed_sha.clone().unwrap_or_else(|| sha256_hex(&self.zip))
    }
}

/// What a request looked like.
#[derive(Clone, Debug)]
pub struct Seen {
    pub path: String,
    pub range: Option<String>,
    pub if_range: Option<String>,
}

#[derive(Default)]
pub struct State {
    pub entries: Vec<Entry>,
    /// Each download request takes one value and sends only that many body bytes before the connection drops.
    pub truncate: VecDeque<usize>,
    /// Download requests answered with 429 and `Retry-After: 1` first.
    pub rate_limited: u32,
    /// Latest requests answered with 503 first.
    pub unavailable: u32,
    pub ignore_range: bool,
    /// Sent as the ETag of downloads instead of the SHA-256.
    pub etag: Option<String>,
    pub seen: Vec<Seen>,
}

pub struct ReleaseServer {
    pub base: String,
    pub state: Arc<Mutex<State>>,
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ReleaseServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let base = format!("http://127.0.0.1:{port}");
        let state: Arc<Mutex<State>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (state, stop, base) = (state.clone(), stop.clone(), base.clone());
            thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let (state, base) = (state.clone(), base.clone());
                    thread::spawn(move || {
                        let _ = handle(stream, &state, &base);
                    });
                }
            })
        };
        Self { base, state, port, stop, thread: Some(thread) }
    }

    pub fn publish(&self, version: &str, channel: &str, platform: &str, notes: &str, zip: Vec<u8>) {
        self.publish_as("game", Sig::Valid, version, channel, platform, notes, zip);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish_as(
        &self,
        product: &str,
        sig: Sig,
        version: &str,
        channel: &str,
        platform: &str,
        notes: &str,
        zip: Vec<u8>,
    ) {
        let entry = Entry {
            product: product.into(),
            sig,
            version: version.into(),
            channel: channel.into(),
            platform: platform.into(),
            notes: notes.into(),
            zip,
            claimed_sha: None,
        };
        self.state.lock().unwrap().entries.push(entry);
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.state.lock().unwrap().seen.clone()
    }

    /// The download requests so far.
    pub fn downloads(&self) -> Vec<Seen> {
        self.seen().into_iter().filter(|s| s.path.split('?').next().is_some_and(|p| p.ends_with("/download"))).collect()
    }
}

impl Drop for ReleaseServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<(String, Vec<(String, String)>)> {
    let mut data = Vec::new();
    let mut byte = [0u8; 1];
    while !data.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).ok()?;
        data.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&data).into_owned();
    let mut lines = text.lines();
    let path = lines.next()?.split(' ').nth(1)?.to_owned();
    let headers = lines
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned())))
        .collect();
    Some((path, headers))
}

fn respond(stream: &mut TcpStream, status: &str, headers: &[(&str, String)], body: &[u8], cut: Option<usize>) {
    let mut head = format!("HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let sent = cut.map_or(body.len(), |n| n.min(body.len()));
    let _ = stream.write_all(&body[..sent]);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn json(stream: &mut TcpStream, status: &str, value: &serde_json::Value) {
    let headers = [("Content-Type", "application/json".to_owned())];
    respond(stream, status, &headers, value.to_string().as_bytes(), None);
}

fn query(path: &str, key: &str) -> Option<String> {
    let (_, q) = path.split_once('?')?;
    q.split('&').find_map(|pair| pair.strip_prefix(&format!("{key}=")).map(str::to_owned))
}

fn describe(base: &str, e: &Entry) -> serde_json::Value {
    let suffix = if e.product == "game" { String::new() } else { format!("?product={}", e.product) };
    let mut value = serde_json::json!({
        "product": e.product, "version": e.version, "channel": e.channel, "platform": e.platform,
        "released_at": "2026-11-01T00:00:00Z", "size": e.zip.len(), "sha256": e.sha(), "notes": e.notes,
        "url": format!("{base}/v1/releases/{}/{}/download{suffix}", e.version, e.platform),
    });
    let fields = Fields {
        product: &e.product,
        version: &e.version,
        platform: &e.platform,
        channel: &e.channel,
        sha256: &sha256_hex(&e.zip),
        size: e.zip.len() as u64,
    };
    let (signature, key_id) = match e.sig {
        Sig::Valid => (Some(sign(&fields)), Some(test_key_id())),
        Sig::Missing => (None, None),
        Sig::Garbage => (Some("AAAA".to_owned()), Some(test_key_id())),
        Sig::WrongKey => {
            let other = SigningKey::from_bytes(&[9u8; 32]);
            let signature = STANDARD.encode(other.sign(message(&fields).as_bytes()).to_bytes());
            (Some(signature), Some(crate::trust::key_id(&other.verifying_key().to_bytes())))
        }
        Sig::UnknownId => (Some(sign(&fields)), Some("0123456789abcdef".to_owned())),
    };
    value["signature"] = signature.into();
    value["key_id"] = key_id.into();
    value
}

fn handle(mut stream: TcpStream, state: &Mutex<State>, base: &str) -> Option<()> {
    let (path, headers) = read_request(&mut stream)?;
    let header = |name: &str| headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    let mut state = state.lock().unwrap();
    state.seen.push(Seen { path: path.clone(), range: header("range"), if_range: header("if-range") });
    let route = path.split('?').next().unwrap_or("");
    let is_download = route.ends_with("/download");
    let unavailable = route == "/v1/releases/latest" && state.unavailable > 0;
    if unavailable {
        state.unavailable -= 1;
    }
    let limited = is_download && state.rate_limited > 0;
    if limited {
        state.rate_limited -= 1;
    }
    let cut = if is_download && !limited { state.truncate.pop_front() } else { None };
    let (platform, channel) = (query(&path, "platform"), query(&path, "channel"));
    let product = query(&path, "product").unwrap_or_else(|| "game".to_owned());
    let mut matching: Vec<&Entry> = state
        .entries
        .iter()
        .filter(|e| {
            e.product == product
                && platform.as_ref().is_none_or(|p| *p == e.platform)
                && channel.as_ref().is_none_or(|c| *c == e.channel)
        })
        .collect();
    matching.sort_by_key(|e| std::cmp::Reverse(version::parse(&e.version)));
    if route == "/v1/releases/latest" {
        if unavailable {
            json(&mut stream, "503 Service Unavailable", &serde_json::json!({"error": "unavailable"}));
        } else if let Some(entry) = matching.first() {
            json(&mut stream, "200 OK", &describe(base, entry));
        } else {
            json(&mut stream, "404 Not Found", &serde_json::json!({"error": "no release"}));
        }
        return Some(());
    }
    if route == "/v1/releases" {
        let list: Vec<_> = matching.iter().map(|e| describe(base, e)).collect();
        json(&mut stream, "200 OK", &serde_json::json!({ "releases": list }));
        return Some(());
    }
    let parts: Vec<&str> = route.trim_start_matches('/').split('/').collect();
    let [_, _, version, platform, "download"] = parts[..] else {
        json(&mut stream, "404 Not Found", &serde_json::json!({"error": "not found"}));
        return Some(());
    };
    let Some(entry) =
        state.entries.iter().find(|e| e.version == version && e.platform == platform && e.product == product)
    else {
        json(&mut stream, "404 Not Found", &serde_json::json!({"error": "not found"}));
        return Some(());
    };
    let etag = format!("\"{}\"", state.etag.clone().unwrap_or_else(|| entry.sha()));
    let total = entry.zip.len();
    let zip = entry.zip.clone();
    if limited {
        respond(&mut stream, "429 Too Many Requests", &[("Retry-After", "1".to_owned())], b"slow down", None);
        return Some(());
    }
    let from = header("range")
        .and_then(|r| r.strip_prefix("bytes=")?.strip_suffix('-')?.parse::<usize>().ok())
        .filter(|_| !state.ignore_range && header("if-range").is_none_or(|v| v == etag));
    let mut out = vec![("ETag", etag), ("Content-Type", "application/zip".to_owned())];
    match from {
        Some(from) if from >= total => respond(&mut stream, "416 Range Not Satisfiable", &out, b"", None),
        Some(from) => {
            out.push(("Content-Range", format!("bytes {from}-{}/{total}", total - 1)));
            respond(&mut stream, "206 Partial Content", &out, &zip[from..], cut);
        }
        None => respond(&mut stream, "200 OK", &out, &zip, cut),
    }
    Some(())
}
