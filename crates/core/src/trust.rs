//! The public keys the launcher trusts for release signatures (docs/release-api.md, "Signatures").

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::VerifyingKey;

use crate::{Error, Result};

/// Base64 of raw 32-byte ed25519 public keys; several may be listed so a key can be rotated. A release verifies when
/// one of them signed it.
///
/// PLACEHOLDER: the first entry is a key nobody holds the private half of, so the launcher refuses every update until
/// the real release key from castanea-backend `docs/releases.md` replaces it (see docs/packaging.md).
pub const TRUST_KEYS: &[&str] = &["Fzl9p9ttARoPPt9fwKD2ZMFNNl2KmCGoICogJN82Y20="]; // PLACEHOLDER-NOT-A-RELEASE-KEY

// A launcher without any key could never accept an update; refuse to build one.
const _: () = assert!(!TRUST_KEYS.is_empty(), "trust.rs: TRUST_KEYS must not be empty");

/// Names a file with extra keys (one base64 key per line) that replaces the embedded list. Debug builds and tests only.
pub const KEYS_FILE_ENV: &str = "CASTANEA_TRUST_KEYS_FILE";

/// Whether this build lets `CASTANEA_TRUST_KEYS_FILE` redirect trust: never in a release build.
pub const fn env_override_allowed() -> bool {
    cfg!(debug_assertions)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustStore {
    keys: Vec<(String, VerifyingKey)>,
}

impl TrustStore {
    /// The keys compiled into the launcher, or those of `CASTANEA_TRUST_KEYS_FILE` in a debug build.
    pub fn current() -> Result<Self> {
        let file = std::env::var(KEYS_FILE_ENV).ok().filter(|v| !v.trim().is_empty());
        Self::load(file.as_deref(), env_override_allowed())
    }

    /// `file` is honoured only when `allow_override` is true.
    pub fn load(file: Option<&str>, allow_override: bool) -> Result<Self> {
        match file {
            Some(path) if allow_override => {
                let text = std::fs::read_to_string(path).map_err(|e| Error::io(std::path::Path::new(path), e))?;
                Self::parse(&text)
            }
            _ => Self::embedded(),
        }
    }

    pub fn embedded() -> Result<Self> {
        Self::from_base64(TRUST_KEYS.iter().copied())
    }

    /// One base64 key per line; blank lines and `#` comments are skipped.
    pub fn parse(text: &str) -> Result<Self> {
        Self::from_base64(text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')))
    }

    pub fn from_base64<'a>(keys: impl IntoIterator<Item = &'a str>) -> Result<Self> {
        let mut out = Vec::new();
        for text in keys {
            let raw = STANDARD.decode(text.trim()).map_err(|_| Error::invalid("a trusted key is not base64"))?;
            let raw: [u8; 32] = raw.try_into().map_err(|_| Error::invalid("a trusted key is not 32 bytes"))?;
            let key = VerifyingKey::from_bytes(&raw).map_err(|_| Error::invalid("a trusted key is not valid"))?;
            out.push((key_id(&raw), key));
        }
        if out.is_empty() {
            return Err(Error::invalid("no trusted release keys"));
        }
        Ok(Self { keys: out })
    }

    pub fn key(&self, key_id: &str) -> Option<&VerifyingKey> {
        let wanted = key_id.trim().to_ascii_lowercase();
        self.keys.iter().find(|(id, _)| *id == wanted).map(|(_, key)| key)
    }

    pub fn ids(&self) -> Vec<&str> {
        self.keys.iter().map(|(id, _)| id.as_str()).collect()
    }
}

/// Hex of the first 8 bytes of the SHA-256 of the raw public key.
pub fn key_id(raw: &[u8; 32]) -> String {
    crate::download::sha256_hex(raw)[..16].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_list_parses_and_is_not_empty() {
        let store = TrustStore::embedded().unwrap();
        assert_eq!(store.ids().len(), TRUST_KEYS.len());
        assert!(store.ids().iter().all(|id| id.len() == 16));
    }

    #[test]
    fn a_release_build_ignores_the_override() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("keys");
        std::fs::write(&file, "# test\nA6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg=\n").unwrap();
        let path = file.to_str().unwrap();
        let allowed = TrustStore::load(Some(path), true).unwrap();
        assert_eq!(allowed.ids().len(), 1);
        assert_ne!(allowed, TrustStore::embedded().unwrap());
        assert_eq!(TrustStore::load(Some(path), false).unwrap(), TrustStore::embedded().unwrap());
        assert_eq!(env_override_allowed(), cfg!(debug_assertions));
    }

    #[test]
    fn bad_keys_are_refused() {
        assert!(TrustStore::parse("").is_err());
        assert!(TrustStore::parse("not base64!").is_err());
        assert!(TrustStore::parse(&STANDARD.encode([1u8; 31])).is_err());
    }
}
