//! Golden vectors and refusals. The vector key is the TEST key of docs/releases.md in castanea-backend: seed 00 01 .. 1f.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer, SigningKey};

use super::*;
use crate::trust;

pub const VECTOR_PUBLIC: &str = "A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg=";
const VECTOR_SIGNATURE: &str =
    "h89f3ne86da3z4QcHROSigIsmGfV5bLhxA0Tfq4KjNR/yCiJDrOjOl8RE5TXyMEgFkeCvc/YMso7f56bZ/nAAg==";
const SHA: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// The TEST signing key (its seed is public): for tests only, never embedded.
pub fn test_key() -> SigningKey {
    SigningKey::from_bytes(&std::array::from_fn(|i| i as u8))
}

pub fn test_store() -> TrustStore {
    TrustStore::from_base64([VECTOR_PUBLIC]).unwrap()
}

pub fn test_key_id() -> String {
    trust::key_id(&test_key().verifying_key().to_bytes())
}

/// A signature of the fields by the test key.
pub fn sign(f: &Fields<'_>) -> String {
    STANDARD.encode(test_key().sign(message(f).as_bytes()).to_bytes())
}

fn vector() -> Fields<'static> {
    Fields { product: "game", version: "0.3.0", platform: "linux-x86_64", channel: "stable", sha256: SHA, size: 3 }
}

fn check(f: &Fields<'_>, signature: &str) -> Result<()> {
    verify(&test_store(), f, Some(signature), Some(&test_key_id()))
}

#[test]
fn the_message_has_the_exact_bytes() {
    let want = format!("castanea-release-v1\ngame\n0.3.0\nlinux-x86_64\nstable\n{SHA}\n3");
    assert_eq!(message(&vector()), want);
    assert!(!message(&vector()).ends_with('\n'));
    let launcher = Fields { product: "launcher", platform: "windows-x86_64", channel: "beta", ..vector() };
    assert!(message(&launcher).starts_with("castanea-release-v1\nlauncher\n0.3.0\nwindows-x86_64\nbeta\n"));
}

#[test]
fn the_golden_vector_verifies() {
    assert_eq!(STANDARD.encode(test_key().verifying_key().to_bytes()), VECTOR_PUBLIC);
    assert_eq!(test_key_id(), "56475aa75463474c");
    assert_eq!(sign(&vector()), VECTOR_SIGNATURE, "ed25519 is deterministic");
    check(&vector(), VECTOR_SIGNATURE).unwrap();
}

#[test]
fn every_tampered_field_breaks_it() {
    let good = sign(&vector());
    check(&vector(), &good).unwrap();
    let tampered = [
        Fields { size: 4, ..vector() },
        Fields { channel: "beta", ..vector() },
        Fields { product: "launcher", ..vector() },
        Fields { version: "0.3.1", ..vector() },
        Fields { platform: "windows-x86_64", ..vector() },
        Fields { sha256: &SHA.replace('b', "c"), ..vector() },
    ];
    for fields in &tampered {
        let error = check(fields, &good).unwrap_err().to_string();
        assert!(error.contains("not signed by castanea"), "{error}");
    }
}

#[test]
fn a_missing_malformed_or_foreign_signature_is_refused() {
    let store = test_store();
    let good = sign(&vector());
    let id = test_key_id();
    assert!(verify(&store, &vector(), None, Some(&id)).is_err());
    assert!(verify(&store, &vector(), Some(" "), Some(&id)).is_err());
    assert!(verify(&store, &vector(), Some(&good), None).is_err());
    assert!(verify(&store, &vector(), Some(&good[..good.len() - 4]), Some(&id)).is_err(), "truncated base64");
    assert!(verify(&store, &vector(), Some(&good[..20]), Some(&id)).is_err());
    assert!(verify(&store, &vector(), Some("!!!"), Some(&id)).is_err());
    assert!(verify(&store, &vector(), Some(&STANDARD.encode([0u8; 10])), Some(&id)).is_err(), "wrong length");
    assert!(verify(&store, &vector(), Some(&STANDARD.encode([0u8; 64])), Some(&id)).is_err(), "zero signature");
    assert!(verify(&store, &vector(), Some(&good), Some("0123456789abcdef")).is_err(), "unknown key id");
    let other = SigningKey::from_bytes(&[7u8; 32]);
    let forged = STANDARD.encode(other.sign(message(&vector()).as_bytes()).to_bytes());
    let error = check(&vector(), &forged).unwrap_err().to_string();
    assert!(error.contains("does not match"), "wrong key: {error}");
}

#[test]
fn a_rotated_list_accepts_either_key() {
    let other = SigningKey::from_bytes(&[7u8; 32]);
    let other_public = STANDARD.encode(other.verifying_key().to_bytes());
    let store = TrustStore::from_base64([other_public.as_str(), VECTOR_PUBLIC]).unwrap();
    let from_other = STANDARD.encode(other.sign(message(&vector()).as_bytes()).to_bytes());
    let other_id = trust::key_id(&other.verifying_key().to_bytes());
    verify(&store, &vector(), Some(&from_other), Some(&other_id)).unwrap();
    verify(&store, &vector(), Some(&sign(&vector())), Some(&test_key_id())).unwrap();
    assert!(verify(&store, &vector(), Some(&from_other), Some(&test_key_id())).is_err(), "the id picks the key");
}
