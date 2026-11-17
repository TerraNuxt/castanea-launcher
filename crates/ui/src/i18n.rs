//! UI strings from `lang/<code>.json`: flat objects of key to text, `{name}` marks a value to fill in.

use std::collections::HashMap;

pub const LANGUAGES: [(&str, &str); 2] = [("en", "English"), ("de", "Deutsch")];

fn source(code: &str) -> &'static str {
    match code {
        "de" => include_str!("../lang/de.json"),
        _ => include_str!("../lang/en.json"),
    }
}

pub struct Strings {
    map: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

fn parse(text: &str) -> HashMap<String, String> {
    serde_json::from_str(text).unwrap_or_default()
}

impl Strings {
    /// `code` is `en` or `de`; empty picks the system language from `LANG` (and `LANGUAGE` on Linux).
    pub fn load(code: &str) -> Self {
        let code = if code.is_empty() { system_language() } else { code.to_owned() };
        let code = if LANGUAGES.iter().any(|(c, _)| *c == code) { code } else { "en".to_owned() };
        Self { map: parse(source(&code)), fallback: parse(source("en")) }
    }

    /// The text for `key`; a missing key shows itself, so it is easy to spot.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.map.get(key).or_else(|| self.fallback.get(key)).map_or(key, String::as_str)
    }

    /// The text with `{name}` replaced by each value.
    pub fn fill(&self, key: &str, values: &[(&str, &str)]) -> String {
        let mut text = self.get(key).to_owned();
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

fn system_language() -> String {
    for name in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name)
            && !value.is_empty()
        {
            return value.chars().take(2).collect::<String>().to_lowercase();
        }
    }
    #[cfg(windows)]
    if let Some(code) = windows_language() {
        return code;
    }
    "en".to_owned()
}

/// The user's locale on Windows, from the registry.
#[cfg(windows)]
fn windows_language() -> Option<String> {
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Control Panel\International", "/v", "LocaleName"])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let value = text.lines().find(|l| l.contains("LocaleName"))?.split_whitespace().last()?.to_owned();
    Some(value.chars().take(2).collect::<String>().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_have_the_same_keys_and_no_empty_text() {
        let en = parse(source("en"));
        let de = parse(source("de"));
        assert!(en.len() > 100, "{} keys", en.len());
        let mut missing: Vec<&String> = en.keys().filter(|k| !de.contains_key(*k)).collect();
        missing.extend(de.keys().filter(|k| !en.contains_key(*k)));
        assert!(missing.is_empty(), "keys in one language only: {missing:?}");
        assert!(en.values().chain(de.values()).all(|v| !v.trim().is_empty()));
    }

    #[test]
    fn placeholders_match_between_languages() {
        let en = parse(source("en"));
        let de = parse(source("de"));
        let names = |text: &str| {
            let mut out: Vec<String> =
                text.split('{').skip(1).filter_map(|p| p.split_once('}').map(|(n, _)| n.to_owned())).collect();
            out.sort();
            out
        };
        for (key, text) in &en {
            assert_eq!(names(text), names(&de[key]), "{key}");
        }
    }

    #[test]
    fn fill_and_missing_keys() {
        let strings = Strings::load("de");
        assert_eq!(strings.get("play.play"), "Spielen");
        assert_eq!(strings.get("no.such.key"), "no.such.key");
        assert_eq!(Strings::load("xx").get("play.play"), "Play");
        let text = Strings::load("en").fill("log.exited", &[("code", "3"), ("time", "0:12")]);
        assert!(text.contains('3') && text.contains("0:12"), "{text}");
    }
}
