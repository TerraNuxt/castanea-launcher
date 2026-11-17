//! The data of the release API (docs/release-api.md): platforms, channels and one release.

use serde::Deserialize;

use crate::download::normalise_sha256;
use crate::{Error, Result};

/// The platforms the release server has builds for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    LinuxX86_64,
    WindowsX86_64,
}

impl Platform {
    pub const ALL: [Self; 2] = [Self::LinuxX86_64, Self::WindowsX86_64];

    /// The platform this launcher runs on; an error says what is supported when there is none.
    pub fn detect() -> Result<Self> {
        Self::from_parts(std::env::consts::OS, std::env::consts::ARCH)
    }

    pub fn from_parts(os: &str, arch: &str) -> Result<Self> {
        match (os, arch) {
            ("linux", "x86_64") => Ok(Self::LinuxX86_64),
            ("windows", "x86_64") => Ok(Self::WindowsX86_64),
            _ => {
                let supported: Vec<&str> = Self::ALL.iter().map(|p| p.as_str()).collect();
                Err(Error::invalid(format!(
                    "There is no castanea game build for {os} on {arch} yet (supported: {}).",
                    supported.join(", ")
                )))
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "linux-x86_64",
            Self::WindowsX86_64 => "windows-x86_64",
        }
    }

    /// The game executable in the archive.
    pub fn binary(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "castanea",
            Self::WindowsX86_64 => "castanea.exe",
        }
    }

    /// The launcher executable at the root of a launcher archive.
    pub fn launcher_binary(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "castanea-launcher",
            Self::WindowsX86_64 => "castanea-launcher.exe",
        }
    }
}

/// What a release is a build of; the server's `product` parameter (the game when it is left out).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Product {
    #[default]
    Game,
    Launcher,
}

impl Product {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Launcher => "launcher",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

/// One release as the API describes it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Release {
    /// `game` or `launcher`; empty when the server does not say.
    #[serde(default)]
    pub product: String,
    pub version: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub released_at: String,
    /// The zip's size in bytes.
    pub size: u64,
    /// The zip's SHA-256, lower-case hex.
    pub sha256: String,
    /// `{base}/v1/releases/<version>/<platform>/download`.
    pub url: String,
    /// What changed, as markdown.
    #[serde(default)]
    pub notes: String,
    /// Base64 ed25519 signature over the release's fields (docs/release-api.md, "Signatures").
    #[serde(default)]
    pub signature: Option<String>,
    /// Hex id of the key that signed it.
    #[serde(default)]
    pub key_id: Option<String>,
}

impl Release {
    /// Parses one release object and checks the parts the launcher relies on.
    pub fn parse(value: serde_json::Value) -> Result<Self> {
        let mut release: Self = serde_json::from_value(value).map_err(|error| Error::json("release", error))?;
        release.version = release.version.trim().to_owned();
        if release.version.is_empty() || release.version.len() > 64 || release.version.chars().any(char::is_control) {
            return Err(Error::invalid(format!("release: `{}` is not a version", release.version)));
        }
        release.sha256 = normalise_sha256(&release.sha256)?;
        if release.size == 0 {
            return Err(Error::invalid(format!("release {}: the size is 0", release.version)));
        }
        Ok(release)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms_come_from_os_and_arch() {
        assert_eq!(Platform::from_parts("linux", "x86_64").unwrap(), Platform::LinuxX86_64);
        assert_eq!(Platform::from_parts("windows", "x86_64").unwrap().binary(), "castanea.exe");
        let error = Platform::from_parts("macos", "aarch64").unwrap_err().to_string();
        assert!(
            error.contains("macos") && error.contains("linux-x86_64") && error.contains("windows-x86_64"),
            "{error}"
        );
        assert!(Platform::from_parts("linux", "aarch64").is_err());
    }

    #[test]
    fn a_release_needs_a_version_a_hash_and_a_size() {
        let good = serde_json::json!({"version": "0.2.0", "size": 5, "sha256": "AB".repeat(32), "url": "u"});
        let release = Release::parse(good).unwrap();
        assert_eq!(release.sha256, "ab".repeat(32));
        assert_eq!(release.notes, "");
        assert!(
            Release::parse(serde_json::json!({"version": "", "size": 5, "sha256": "ab".repeat(32), "url": "u"}))
                .is_err()
        );
        assert!(Release::parse(serde_json::json!({"version": "1", "size": 5, "sha256": "x", "url": "u"})).is_err());
        assert!(
            Release::parse(serde_json::json!({"version": "1", "size": 0, "sha256": "ab".repeat(32), "url": "u"}))
                .is_err()
        );
    }
}
