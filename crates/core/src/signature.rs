//! Release signatures (docs/release-api.md, "Signatures"): ed25519 over a canonical text of the release's fields.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::Signature;

use crate::trust::TrustStore;
use crate::{Error, Result};

/// First line of the signed text; another format gets another word.
pub const DOMAIN: &str = "castanea-release-v1";

/// What a signature covers. `sha256` and `size` are those of the downloaded file, the rest comes from the API.
#[derive(Clone, Copy, Debug)]
pub struct Fields<'a> {
    pub product: &'a str,
    pub version: &'a str,
    pub platform: &'a str,
    pub channel: &'a str,
    pub sha256: &'a str,
    pub size: u64,
}

/// The exact text that is signed, with no trailing newline.
pub fn message(f: &Fields<'_>) -> String {
    format!("{DOMAIN}\n{}\n{}\n{}\n{}\n{}\n{}", f.product, f.version, f.platform, f.channel, f.sha256, f.size)
}

fn not_signed(reason: impl std::fmt::Display) -> Error {
    Error::Signature(reason.to_string())
}

/// Checks the signature (base64, standard, padded) from the key named by `key_id` against the fields.
pub fn verify(store: &TrustStore, fields: &Fields<'_>, signature: Option<&str>, key_id: Option<&str>) -> Result<()> {
    let signature = signature.map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| not_signed("no signature"))?;
    let raw = STANDARD.decode(signature).map_err(|_| not_signed("the signature is not valid base64"))?;
    let raw: [u8; 64] = raw.try_into().map_err(|_| not_signed("the signature is not 64 bytes"))?;
    let id = key_id.map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| not_signed("no key id"))?;
    let key = store.key(id).ok_or_else(|| not_signed(format!("unknown key {id}")))?;
    key.verify_strict(message(fields).as_bytes(), &Signature::from_bytes(&raw))
        .map_err(|_| not_signed("the signature does not match this file"))
}

#[cfg(test)]
pub(crate) mod tests;
