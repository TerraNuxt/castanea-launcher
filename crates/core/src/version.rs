//! Game and mod versions: lenient semver parsing and range checks.

use semver::{Version, VersionReq};

/// How a mod's game range fits a game build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compat {
    Yes,
    No,
    /// The build's version or the range is not known or not semver.
    Unknown,
}

/// Parses `1`, `1.2`, `v1.2.3` and full semver; anything else is `None`.
pub fn parse(text: &str) -> Option<Version> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    if let Ok(version) = Version::parse(text) {
        return Some(version);
    }
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().map_or(Some(0), |p| p.parse().ok())?;
    let patch = parts.next().map_or(Some(0), |p| p.parse().ok())?;
    parts.next().is_none().then(|| Version::new(major, minor, patch))
}

/// Whether `game` (a build's version text) satisfies `range` (a semver requirement such as `>=0.1, <0.3`).
pub fn compat(range: Option<&str>, game: Option<&str>) -> Compat {
    let (Some(range), Some(game)) = (range, game) else { return Compat::Unknown };
    match (VersionReq::parse(range.trim()), parse(game)) {
        (Ok(req), Some(version)) => {
            if req.matches(&version) {
                Compat::Yes
            } else {
                Compat::No
            }
        }
        _ => Compat::Unknown,
    }
}

/// `a` is newer than `b`; texts that are not versions compare as plain text.
pub fn newer(a: &str, b: &str) -> bool {
    match (parse(a), parse(b)) {
        (Some(a), Some(b)) => a > b,
        _ => a > b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_and_prefixed_versions_parse() {
        assert_eq!(parse("1"), Some(Version::new(1, 0, 0)));
        assert_eq!(parse("v0.2"), Some(Version::new(0, 2, 0)));
        assert_eq!(parse("0.1.0-rc.1").unwrap().pre.as_str(), "rc.1");
        assert_eq!(parse("local"), None);
        assert_eq!(parse("1.2.3.4"), None);
    }

    #[test]
    fn ranges_match_or_say_unknown() {
        assert_eq!(compat(Some(">=0.1, <0.3"), Some("0.2.5")), Compat::Yes);
        assert_eq!(compat(Some(">=0.3"), Some("v0.2")), Compat::No);
        assert_eq!(compat(Some(">=0.3"), Some("local")), Compat::Unknown);
        assert_eq!(compat(None, Some("0.1.0")), Compat::Unknown);
        assert_eq!(compat(Some("what"), Some("0.1.0")), Compat::Unknown);
    }

    #[test]
    fn newer_compares_numbers_not_text() {
        assert!(newer("0.10.0", "0.9.0"));
        assert!(!newer("0.9", "0.10"));
        assert!(!newer("1.0.0", "1.0.0"));
    }
}
