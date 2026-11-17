//! `mod.json`, the game's package manifest, read leniently: a broken one is shown with its error, not hidden.

use serde_json::{Map, Value};

pub const FILE: &str = "mod.json";
/// The keys the game knows (`src/data/modding/manifest.rs` of the game); `_comment` and other `_` keys are notes.
const KNOWN: [&str; 8] = ["id", "name", "version", "author", "depends", "wasm", "side", "aliases"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub depends: Vec<String>,
    /// A `.wasm` or `.wat` file: code that runs in the game's Wasm sandbox.
    pub wasm: Option<String>,
    /// `both`, `server` or `client`.
    pub side: String,
    pub aliases: Vec<(String, String)>,
    /// Keys the game would refuse.
    pub unknown_keys: Vec<String>,
}

impl Manifest {
    pub fn has_code(&self) -> bool {
        self.wasm.is_some()
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text).map_err(|error| format!("mod.json: {error}"))?;
        let Value::Object(map) = value else { return Err("mod.json: not a JSON object".to_owned()) };
        let id = text_key(&map, "id")?.ok_or("mod.json: missing `id`")?;
        if !valid_id(&id) {
            return Err(format!("mod.json: id `{id}` must be lowercase letters, digits or _"));
        }
        let side = text_key(&map, "side")?.unwrap_or_else(|| "both".to_owned());
        if !["both", "server", "client"].contains(&side.as_str()) {
            return Err(format!("mod.json: `side` must be both, server or client, got `{side}`"));
        }
        Ok(Self {
            name: text_key(&map, "name")?.ok_or("mod.json: missing `name`")?,
            version: text_key(&map, "version")?.ok_or("mod.json: missing `version`")?,
            author: text_key(&map, "author")?.unwrap_or_default(),
            depends: depends(map.get("depends"))?,
            wasm: text_key(&map, "wasm")?,
            aliases: aliases(map.get("aliases"))?,
            unknown_keys: map.keys().filter(|k| !k.starts_with('_') && !KNOWN.contains(&k.as_str())).cloned().collect(),
            side,
            id,
        })
    }
}

/// Lowercase letters, digits and `_`, as the game wants.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A string, or a number written as one (`"version": 2`).
fn text_key(map: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.trim().to_owned())),
        Some(Value::Number(number)) => Ok(Some(number.to_string())),
        Some(_) => Err(format!("mod.json: `{key}` must be text")),
    }
}

/// A list of ids, or one comma-separated text.
fn depends(value: Option<&Value>) -> Result<Vec<String>, String> {
    let list: Vec<String> = match value {
        None => Vec::new(),
        Some(Value::String(text)) => {
            text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect()
        }
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(|s| s.trim().to_owned()).ok_or("mod.json: `depends` must list ids as text"))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err("mod.json: `depends` must be a list of ids".to_owned()),
    };
    Ok(list)
}

/// An object of old id to new id.
fn aliases(value: Option<&Value>) -> Result<Vec<(String, String)>, String> {
    match value {
        None => Ok(Vec::new()),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(old, new)| match new {
                Value::String(new) => Ok((old.clone(), new.clone())),
                _ => Err(format!("mod.json: alias `{old}` must name an id")),
            })
            .collect(),
        Some(_) => Err("mod.json: `aliases` must be an object".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_manifest_parses() {
        let text = r#"{ "_comment": "x", "id": "embers", "name": "Embers", "version": 2, "author": "A",
            "depends": ["amber", "frost"], "wasm": "e.wasm", "side": "server",
            "aliases": { "embers:old": "embers:new" } }"#;
        let manifest = Manifest::parse(text).unwrap();
        assert_eq!(manifest.id, "embers");
        assert_eq!(manifest.version, "2");
        assert_eq!(manifest.depends, ["amber", "frost"]);
        assert!(manifest.has_code());
        assert_eq!(manifest.side, "server");
        assert_eq!(manifest.aliases, [("embers:old".to_owned(), "embers:new".to_owned())]);
        assert!(manifest.unknown_keys.is_empty());
    }

    #[test]
    fn comma_depends_and_defaults() {
        let manifest = Manifest::parse(r#"{"id":"a","name":"A","version":"1","depends":"b, c,"}"#).unwrap();
        assert_eq!(manifest.depends, ["b", "c"]);
        assert_eq!(manifest.side, "both");
        assert_eq!(manifest.author, "");
        assert!(!manifest.has_code());
    }

    #[test]
    fn errors_name_the_problem() {
        let cases = [
            ("[]", "not a JSON object"),
            (r#"{"name":"A","version":"1"}"#, "missing `id`"),
            (r#"{"id":"Bad-Id","name":"A","version":"1"}"#, "lowercase"),
            (r#"{"id":"a","version":"1"}"#, "missing `name`"),
            (r#"{"id":"a","name":"A","version":"1","side":"up"}"#, "`side`"),
            (r#"{"id":"a","name":"A","version":"1","depends":[1]}"#, "depends"),
            ("{", "mod.json"),
        ];
        for (text, needle) in cases {
            let error = Manifest::parse(text).unwrap_err();
            assert!(error.contains(needle), "{text}: {error}");
        }
    }

    #[test]
    fn keys_the_game_refuses_are_listed() {
        let manifest = Manifest::parse(r#"{"id":"a","name":"A","version":"1","game":">=0.1"}"#).unwrap();
        assert_eq!(manifest.unknown_keys, ["game"]);
    }
}
