//! The news feed of the home page (docs/release-api.md, "News"): fetched from the release server, kept on disk so it
//! shows at once and offline. Everything from the server is untrusted data: sizes are limited, text is cleaned, and
//! only `https://` links survive.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::download::{Client, UrlPolicy};
use crate::install::release_api::loopback;
use crate::{Error, Result, json_file};

pub mod text;

/// The largest answer or cache file that is read.
pub const MAX_BYTES: u64 = 256 << 10;
/// Items asked for, and the most that are kept.
pub const LIMIT: usize = 10;
pub const MAX_ITEMS: usize = 20;
pub const MAX_TITLE: usize = 120;
pub const MAX_BODY: usize = 1500;
pub const MAX_ID: usize = 64;
pub const MAX_TAG: usize = 24;
pub const MAX_DATE: usize = 32;
/// The request, answer included, is dropped after this long.
pub const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    /// Plain text with `**bold**` and `[text](https://…)` only; see `text::lines`.
    pub body: String,
    pub date: String,
    pub tag: String,
    pub url: Option<String>,
}

impl Item {
    /// `2026-11-01` from a date or a date and time; anything else as it is.
    pub fn day(&self) -> &str {
        let head = self.date.get(..10).unwrap_or(&self.date);
        let shaped = head.len() == 10 && head.bytes().enumerate().all(|(i, b)| matches!(i, 4 | 7) == (b == b'-'));
        if shaped { head } else { &self.date }
    }
}

/// What is on disk: the items and the ETag they came with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cache {
    pub etag: Option<String>,
    pub items: Vec<Item>,
}

/// Text without control characters (a newline stays when `lines`), cut to `max` characters.
fn clean(value: Option<&Value>, max: usize, lines: bool) -> String {
    let raw = value.and_then(Value::as_str).unwrap_or("");
    let spoofing = |c: char| matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200b}'..='\u{200f}');
    let kept = raw.chars().filter(|c| !spoofing(*c)).map(|c| if c == '\t' { ' ' } else { c });
    let kept = kept.filter(|c| (*c == '\n' && lines) || !c.is_control());
    let text: String = kept.take(max).collect();
    text.trim().to_owned()
}

/// The items of `{"items":[…]}`: the usable ones, cleaned and limited. An item without a title is dropped.
pub fn items_from(value: &Value) -> Result<Vec<Item>> {
    let list = value.get("items").and_then(Value::as_array).ok_or_else(|| Error::invalid("news: no `items` list"))?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (n, entry) in list.iter().enumerate() {
        let title = clean(entry.get("title"), MAX_TITLE, false);
        if title.is_empty() {
            continue;
        }
        let id = Some(clean(entry.get("id"), MAX_ID, false)).filter(|id| !id.is_empty()).unwrap_or(format!("#{n}"));
        if !seen.insert(id.clone()) {
            continue;
        }
        out.push(Item {
            id,
            title,
            body: clean(entry.get("body"), MAX_BODY, true),
            date: clean(entry.get("date"), MAX_DATE, false),
            tag: clean(entry.get("tag"), MAX_TAG, false),
            url: entry.get("url").and_then(Value::as_str).and_then(text::safe_url),
        });
        if out.len() == MAX_ITEMS {
            break;
        }
    }
    Ok(out)
}

/// The items of a server answer; broken JSON or a wrong shape is an error.
pub fn parse(bytes: &[u8]) -> Result<Vec<Item>> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| Error::json("news", error))?;
    items_from(&value)
}

fn language(lang: &str) -> &'static str {
    if lang == "de" { "de" } else { "en" }
}

fn cache_file(dir: &Path, lang: &str) -> PathBuf {
    dir.join(format!("{}.json", language(lang)))
}

/// The saved feed for `lang`; nothing, a file that is too large or one that does not read gives an empty cache.
pub fn load(dir: &Path, lang: &str) -> Cache {
    let path = cache_file(dir, lang);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 2 * MAX_BYTES) {
        return Cache::default();
    }
    let Ok(text) = std::fs::read_to_string(&path) else { return Cache::default() };
    let Ok(value) = serde_json::from_str::<Value>(&text) else { return Cache::default() };
    let etag = value.get("etag").and_then(Value::as_str).and_then(valid_etag);
    Cache { etag, items: items_from(&value).unwrap_or_default() }
}

fn save(dir: &Path, lang: &str, cache: &Cache) -> Result<()> {
    let value = serde_json::json!({ "etag": cache.etag, "items": cache.items });
    json_file::save(&cache_file(dir, lang), &value)
}

/// An ETag that is safe to send back in a header.
fn valid_etag(etag: &str) -> Option<String> {
    let fine = !etag.is_empty() && etag.len() <= 128 && etag.bytes().all(|b| (0x20..0x7f).contains(&b));
    fine.then(|| etag.to_owned())
}

/// How the server answered.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Fresh(Cache),
    /// 304: the cached items are current.
    Unchanged,
}

/// Asks the server once (no retry, `TIMEOUT`); `cached.etag` goes out as `If-None-Match`.
pub fn request(base: &str, lang: &str, cached: &Cache) -> Result<Answer> {
    let base = base.trim().trim_end_matches('/');
    let client = Client::with_deadline(UrlPolicy { allow_loopback_http: loopback(base) }, TIMEOUT);
    let url = format!("{base}/v1/news?lang={}&limit={LIMIT}", language(lang));
    let mut headers = vec![("Accept", "application/json".to_owned())];
    if let Some(etag) = cached.etag.as_ref().filter(|_| !cached.items.is_empty()) {
        headers.push(("If-None-Match", etag.clone()));
    }
    let fetched = client.get(&url, &headers)?;
    match fetched.status {
        304 => Ok(Answer::Unchanged),
        200 => {
            let etag = fetched.etag.as_deref().and_then(valid_etag);
            let items = parse(&fetched.read_limited(&url, MAX_BYTES)?)?;
            Ok(Answer::Fresh(Cache { etag, items }))
        }
        code => Err(Error::http(&url, format!("HTTP {code}"))),
    }
}

/// Brings the saved feed up to date and returns its items. An error leaves the saved feed as it was.
pub fn sync(dir: &Path, base: &str, lang: &str) -> Result<Vec<Item>> {
    let cached = load(dir, lang);
    match request(base, lang, &cached)? {
        Answer::Unchanged => Ok(cached.items),
        Answer::Fresh(fresh) => {
            // A cache that cannot be written only costs the next start a request.
            let _ = save(dir, lang, &fresh);
            Ok(fresh.items)
        }
    }
}

#[cfg(test)]
mod tests;
