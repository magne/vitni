//! Plugin bundle signing primitive and bundle-manifest type (ADR 0014 §1, §2).
//!
//! A plugin bundle is a `plugin.toml` manifest plus a `plugin.wasm` component. It is signed by
//! computing a canonical SHA-256 [`bundle_digest`] over **both** (length-prefixed so neither field
//! can be swapped for the other) and producing a 64-byte ed25519 detached signature stored beside
//! the component as `plugin.sig`. Verification checks that signature against a trust root.
//!
//! This module is the pure signing/verification core (plus the manifest type and the dev/release
//! key resolution). It does **not** wire verification into the loader — that is a later sub-PR
//! (ADR 0014 §3). `xtask build-plugins` reuses [`bundle_digest`], [`sign`], [`PluginManifest`], and
//! [`resolve_signing_key`] to emit the signature and manifest sidecars.

use ed25519_dalek::{Signer, Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use ed25519_dalek::{Signature, SignatureError, SigningKey, VerifyingKey};

/// The declared metadata of a plugin bundle (`plugin.toml`, ADR 0014 §2). This is the authoritative
/// grant-request the capability-grant UX surfaces; a later sub-PR cross-checks it against what the
/// component actually imports/exports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// The stable id the host loads the plugin under.
    pub id: String,
    /// The plugin's own semver version (distinct from the host-API version it pins).
    pub version: String,
    /// The publisher identity a signature attributes the bundle to.
    pub publisher: String,
    /// The `genealogy:host-api` package version the plugin was built against.
    pub host_api: String,
    /// The plugin role: `bulk-import`, `bulk-export`, `assisted-import`, `ui-panel`, or
    /// `test-fixture`.
    pub role: String,
    /// The host capabilities the plugin declares it needs (the interface names from the WIT world it
    /// implements, e.g. `log`, `commands`).
    pub capabilities: Vec<String>,
}

/// Domain separator mixed into every bundle digest so a digest can never be confused with a hash
/// computed for another purpose.
const BUNDLE_DOMAIN: &[u8] = b"genealogy-plugin-bundle-v1";

/// The environment variable holding the release signing key: a 32-byte ed25519 seed, hex-encoded
/// (64 hex characters). When absent, [`resolve_signing_key`] falls back to the deterministic dev key.
pub const SIGNING_KEY_ENV: &str = "GENEALOGY_PLUGIN_SIGNING_KEY";

/// DEV-ONLY fixed ed25519 seed. Local and CI builds sign with the keypair derived from this seed
/// when [`SIGNING_KEY_ENV`] is unset, so the dev inner loop needs no release secret and produces
/// reproducible signatures. **This is never a release key**: real bundles are signed with the
/// private key held as a release secret and provided through [`SIGNING_KEY_ENV`].
const DEV_SEED: [u8; 32] = *b"genealogy-dev-signing-key-seed!!";

/// The verifying-key bytes of the [`DEV_SEED`] keypair, hardcoded so a later sub-PR can embed it as a
/// dev trust root without deriving it at runtime. A test asserts it matches
/// `dev_signing_key().verifying_key()`.
pub const DEV_PUBLIC_KEY: [u8; 32] = [
    0xa5, 0xe2, 0x72, 0x24, 0xef, 0x4b, 0xeb, 0xf9, 0xa7, 0x91, 0x21, 0xbf, 0x2c, 0x44, 0xf5, 0x2d, 0xe9, 0x4c, 0x59,
    0x66, 0x6c, 0xb9, 0x83, 0x6c, 0x0f, 0xfe, 0x09, 0xd4, 0xde, 0xf2, 0x68, 0x4e,
];

/// A failure resolving the signing key from [`SIGNING_KEY_ENV`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SigningError {
    /// The hex seed was not exactly 64 hex characters (32 bytes).
    #[error("{SIGNING_KEY_ENV} must be 64 hex characters (a 32-byte ed25519 seed), got {0}")]
    SeedLength(usize),
    /// The hex seed contained a non-hexadecimal character at the given byte offset.
    #[error("{SIGNING_KEY_ENV} is not valid hexadecimal at offset {0}")]
    SeedHex(usize),
}

/// The deterministic DEV-ONLY signing key (see [`DEV_SEED`]).
#[must_use]
pub fn dev_signing_key() -> SigningKey {
    SigningKey::from_bytes(&DEV_SEED)
}

/// Resolves the signing key: the release key parsed from [`SIGNING_KEY_ENV`] if that variable holds
/// a non-empty value, otherwise the deterministic [`dev_signing_key`].
///
/// # Errors
///
/// [`SigningError`] if [`SIGNING_KEY_ENV`] is set to a value that is not a 32-byte hex seed.
pub fn resolve_signing_key() -> Result<SigningKey, SigningError> {
    match std::env::var(SIGNING_KEY_ENV) {
        Ok(hex) if !hex.trim().is_empty() => signing_key_from_seed_hex(hex.trim()),
        Ok(_) | Err(_) => Ok(dev_signing_key()),
    }
}

/// Parses a 32-byte ed25519 seed from its 64-character hex encoding into a [`SigningKey`].
fn signing_key_from_seed_hex(hex: &str) -> Result<SigningKey, SigningError> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return Err(SigningError::SeedLength(bytes.len()));
    }
    let mut seed = [0u8; 32];
    for (index, slot) in seed.iter_mut().enumerate() {
        let high = hex_nibble(bytes[2 * index]).ok_or(SigningError::SeedHex(2 * index))?;
        let low = hex_nibble(bytes[2 * index + 1]).ok_or(SigningError::SeedHex(2 * index + 1))?;
        *slot = (high << 4) | low;
    }
    Ok(SigningKey::from_bytes(&seed))
}

/// Decodes one hex digit to its nibble value, or `None` for a non-hex byte.
fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Computes the canonical SHA-256 digest over a bundle's manifest and component (ADR 0014 §1).
///
/// Both fields are length-prefixed (little-endian `u64`) under a fixed domain separator so the
/// manifest and the component cannot be swapped or their boundary shifted without changing the
/// digest — signing this digest binds the declared capabilities to the exact code that runs.
#[must_use]
pub fn bundle_digest(manifest_toml_bytes: &[u8], wasm_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(BUNDLE_DOMAIN);
    hasher.update((manifest_toml_bytes.len() as u64).to_le_bytes());
    hasher.update(manifest_toml_bytes);
    hasher.update((wasm_bytes.len() as u64).to_le_bytes());
    hasher.update(wasm_bytes);
    let output = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&output);
    digest
}

/// Signs a [`bundle_digest`] with `signing_key`, producing the 64-byte detached signature.
#[must_use]
pub fn sign(signing_key: &SigningKey, digest: &[u8; 32]) -> Signature {
    signing_key.sign(digest)
}

/// Verifies a detached bundle signature against `verifying_key`.
///
/// # Errors
///
/// [`SignatureError`] if the signature does not verify against the key and digest.
pub fn verify(verifying_key: &VerifyingKey, digest: &[u8; 32], signature: &Signature) -> Result<(), SignatureError> {
    verifying_key.verify(digest, signature)
}

/// The on-disk `plugin.sig` encoding: the 64 raw signature bytes.
#[must_use]
pub fn signature_to_bytes(signature: &Signature) -> [u8; 64] {
    signature.to_bytes()
}

/// Decodes a detached signature from its raw on-disk bytes.
///
/// # Errors
///
/// [`SignatureError`] if `bytes` is not exactly 64 bytes long.
pub fn signature_from_bytes(bytes: &[u8]) -> Result<Signature, SignatureError> {
    Signature::from_slice(bytes)
}

/// Decodes a verifying (public) key from its 32 raw bytes.
///
/// # Errors
///
/// [`SignatureError`] if `bytes` is not a valid ed25519 public key encoding.
pub fn verifying_key_from_bytes(bytes: &[u8; 32]) -> Result<VerifyingKey, SignatureError> {
    VerifyingKey::from_bytes(bytes)
}

/// Decodes a verifying (public) key from its 64-character hex encoding (32 bytes).
///
/// Returns `None` if `hex` is not exactly 64 hex characters or does not encode a valid ed25519
/// point — the release sanctioned-key resolution and the user trust store both parse keys this way.
#[must_use]
pub fn verifying_key_from_hex(hex: &str) -> Option<VerifyingKey> {
    let raw = hex.as_bytes();
    if raw.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let high = hex_nibble(raw[2 * index])?;
        let low = hex_nibble(raw[2 * index + 1])?;
        *slot = (high << 4) | low;
    }
    verifying_key_from_bytes(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        DEV_PUBLIC_KEY, PluginManifest, SigningError, SigningKey, bundle_digest, dev_signing_key, sign,
        signature_from_bytes, signature_to_bytes, signing_key_from_seed_hex, verify, verifying_key_from_bytes,
    };

    fn sample_manifest() -> Vec<u8> {
        let manifest = PluginManifest {
            id: "gedcom-import".to_owned(),
            version: "0.1.0".to_owned(),
            publisher: "genealogy-project".to_owned(),
            host_api: "0.21.0".to_owned(),
            role: "bulk-import".to_owned(),
            capabilities: vec!["log".to_owned(), "commands".to_owned()],
        };
        toml::to_string(&manifest)
            .expect("serialize sample manifest")
            .into_bytes()
    }

    #[test]
    fn sign_then_verify_round_trips() {
        let key = dev_signing_key();
        let digest = bundle_digest(&sample_manifest(), b"\0asm-component-bytes");
        let signature = sign(&key, &digest);
        assert!(verify(&key.verifying_key(), &digest, &signature).is_ok());
    }

    #[test]
    fn tampered_manifest_fails_verification() {
        let key = dev_signing_key();
        let wasm = b"\0asm-component-bytes";
        let signature = sign(&key, &bundle_digest(&sample_manifest(), wasm));

        let mut tampered = sample_manifest();
        tampered.extend_from_slice(b"\n# sneaky extra capability");
        let tampered_digest = bundle_digest(&tampered, wasm);
        assert!(verify(&key.verifying_key(), &tampered_digest, &signature).is_err());
    }

    #[test]
    fn tampered_wasm_fails_verification() {
        let key = dev_signing_key();
        let manifest = sample_manifest();
        let signature = sign(&key, &bundle_digest(&manifest, b"\0asm-component-bytes"));

        let tampered_digest = bundle_digest(&manifest, b"\0asm-different-bytes!");
        assert!(verify(&key.verifying_key(), &tampered_digest, &signature).is_err());
    }

    #[test]
    fn wrong_key_fails_verification() {
        let signer = dev_signing_key();
        let other = SigningKey::from_bytes(&[7u8; 32]);
        let digest = bundle_digest(&sample_manifest(), b"\0asm-component-bytes");
        let signature = sign(&signer, &digest);
        assert!(verify(&other.verifying_key(), &digest, &signature).is_err());
    }

    #[test]
    fn malformed_signature_bytes_error_without_panic() {
        assert!(signature_from_bytes(&[0u8; 10]).is_err());
        assert!(signature_from_bytes(&[]).is_err());
        assert!(signature_from_bytes(&[0u8; 65]).is_err());
    }

    #[test]
    fn digest_is_deterministic_and_order_sensitive() {
        let manifest = sample_manifest();
        let wasm = b"\0asm-component-bytes".to_vec();
        assert_eq!(bundle_digest(&manifest, &wasm), bundle_digest(&manifest, &wasm));
        assert_ne!(bundle_digest(&manifest, &wasm), bundle_digest(&wasm, &manifest));
    }

    #[test]
    fn signature_bytes_round_trip() {
        let key = dev_signing_key();
        let digest = bundle_digest(&sample_manifest(), b"wasm");
        let signature = sign(&key, &digest);
        let decoded = signature_from_bytes(&signature_to_bytes(&signature)).expect("decode signature");
        assert!(verify(&key.verifying_key(), &digest, &decoded).is_ok());
    }

    #[test]
    fn manifest_serde_round_trips() {
        let bytes = sample_manifest();
        let text = String::from_utf8(bytes).expect("manifest is utf-8");
        let manifest: PluginManifest = toml::from_str(&text).expect("deserialize manifest");
        assert_eq!(manifest.id, "gedcom-import");
        assert_eq!(manifest.capabilities, vec!["log".to_owned(), "commands".to_owned()]);
    }

    #[test]
    fn dev_public_key_matches_dev_seed() {
        assert_eq!(dev_signing_key().verifying_key().to_bytes(), DEV_PUBLIC_KEY);
    }

    #[test]
    fn verifying_key_bytes_round_trip() {
        let key = dev_signing_key();
        let decoded = verifying_key_from_bytes(&key.verifying_key().to_bytes()).expect("decode verifying key");
        assert_eq!(decoded, key.verifying_key());
    }

    #[test]
    fn seed_hex_parses_and_rejects_bad_input() {
        let hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        assert!(signing_key_from_seed_hex(hex).is_ok());
        assert_eq!(signing_key_from_seed_hex("abcd"), Err(SigningError::SeedLength(4)));
        let mut bad = String::from(hex);
        bad.replace_range(0..1, "z");
        assert_eq!(signing_key_from_seed_hex(&bad), Err(SigningError::SeedHex(0)));
    }
}
