//! The mod index format, version 1 (docs/mod-index.md).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::download::{UrlPolicy, normalise_sha256};
use crate::mods::manifest::valid_id;
use crate::version::{self, Compat};
use crate::{Error, Result};

pub const FORMAT: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub format: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub news: Vec<News>,
    #[serde(default)]
    pub mods: Vec<IndexMod>,
    /// Planned: an Ed25519 signature over the index without this field. Not checked yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct News {
    pub title: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexMod {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    /// `both`, `server` or `client`, as in mod.json.
    #[serde(default = "both")]
    pub side: String,
    /// The package carries Wasm code.
    #[serde(default)]
    pub has_code: bool,
    pub versions: Vec<IndexVersion>,
}

fn both() -> String {
    "both".to_owned()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexVersion {
    pub version: String,
    /// A semver range of game versions, such as `>=0.1, <0.3`; missing means unknown.
    #[serde(default)]
    pub game: Option<String>,
    /// Mod id to a semver range of its versions (`*` for any).
    #[serde(default)]
    pub depends: BTreeMap<String, String>,
    #[serde(default)]
    pub published: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub changelog: String,
}

impl Index {
    /// Parses and checks an index: format, ids, checksums and download URLs.
    pub fn parse(bytes: &[u8], what: &str, policy: UrlPolicy) -> Result<Self> {
        let mut index: Self = serde_json::from_slice(bytes).map_err(|error| Error::json(what, error))?;
        if index.format != FORMAT {
            return Err(Error::invalid(format!("{what}: index format {} is not supported (this launcher reads {FORMAT})", index.format)));
        }
        for item in &mut index.mods {
            if !valid_id(&item.id) {
                return Err(Error::invalid(format!("{what}: mod id `{}` is not valid", item.id)));
            }
            for version in &mut item.versions {
                version.sha256 = normalise_sha256(&version.sha256)
                    .map_err(|_| Error::invalid(format!("{what}: {} {}: bad sha256", item.id, version.version)))?;
                if policy.check(&version.url).is_err() {
                    return Err(Error::invalid(format!("{what}: {} {}: the url must be https://", item.id, version.version)));
                }
            }
            item.versions.sort_by(|a, b| {
                if version::newer(&a.version, &b.version) {
                    std::cmp::Ordering::Less
                } else if version::newer(&b.version, &a.version) {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });
        }
        let mut seen = std::collections::HashSet::new();
        if let Some(dup) = index.mods.iter().find(|m| !seen.insert(m.id.clone())) {
            return Err(Error::invalid(format!("{what}: mod `{}` is listed twice", dup.id)));
        }
        Ok(index)
    }

    pub fn get(&self, id: &str) -> Option<&IndexMod> {
        self.mods.iter().find(|m| m.id == id)
    }

    /// All tags with how many mods carry each, most used first.
    pub fn tags(&self) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for tag in self.mods.iter().flat_map(|m| &m.tags) {
            *counts.entry(tag).or_default() += 1;
        }
        let mut out: Vec<(String, usize)> = counts.into_iter().map(|(t, n)| (t.to_owned(), n)).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }

    /// Adds the mods and news of `other` that this index lacks (the first index wins on an id).
    pub fn merge(&mut self, other: Index) {
        for item in other.mods {
            if self.get(&item.id).is_none() {
                self.mods.push(item);
            }
        }
        self.news.extend(other.news);
    }
}

impl IndexMod {
    /// The newest version for `game`: the newest compatible one, or the newest overall when nothing says no.
    pub fn best_version(&self, game: Option<&str>) -> Option<&IndexVersion> {
        let fits = |v: &&IndexVersion| version::compat(v.game.as_deref(), game);
        self.versions
            .iter()
            .find(|v| fits(v) == Compat::Yes)
            .or_else(|| self.versions.iter().find(|v| fits(v) == Compat::Unknown))
    }

    pub fn compat(&self, game: Option<&str>) -> Compat {
        match self.best_version(game) {
            None => Compat::No,
            Some(v) => version::compat(v.game.as_deref(), game),
        }
    }

    /// True when the query (any case) is in the id, name, summary, authors or tags.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        [&self.id, &self.name, &self.summary].iter().any(|t| t.to_lowercase().contains(&query))
            || self.authors.iter().chain(&self.tags).any(|t| t.to_lowercase().contains(&query))
    }
}
