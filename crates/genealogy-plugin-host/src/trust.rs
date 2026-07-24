//! Plugin trust-tier classification (ADR 0014 §3).
//!
//! Each discovered bundle is classified by verifying its `plugin.sig` (the A1 detached signature over
//! the canonical [`bundle_digest`](crate::signing::bundle_digest) of `plugin.toml` + `plugin.wasm`)
//! against a set of [`TrustRoots`]:
//!
//! - no signature at all → [`TrustTier::Untrusted`] (unsigned — still loadable, ADR 0007 §9);
//! - verifies against an embedded **sanctioned** project key → [`TrustTier::Sanctioned`];
//! - verifies against a **user-pinned** publisher key → [`TrustTier::UserTrusted`];
//! - present but verifies against no trusted key → a hard [`PluginError::Signature`] (fails closed).
//!
//! Trust never widens the sandbox (ADR 0014 §3): a tier only says *who* the host believes signed a
//! bundle, never *what* it may do. Wiring the tier into loading/grants is a later sub-PR.

use crate::error::PluginError;
use crate::signing::{self, VerifyingKey};

/// The trust tier a bundle's signature places it in (ADR 0014 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustTier {
    /// Signed by an embedded sanctioned project key — every declared capability is grantable.
    Sanctioned,
    /// Signed by a publisher key the user pinned in their client-scope trust store — trusted like
    /// sanctioned, for this user only.
    UserTrusted,
    /// Unsigned, or signed by a key the host does not trust. Loadable, but never auto-granted.
    Untrusted,
}

/// The trust roots a bundle signature is verified against: the embedded sanctioned project key(s)
/// baked into the binary, plus the user's pinned publisher keys (ADR 0014 §3).
pub struct TrustRoots {
    sanctioned: Vec<VerifyingKey>,
    user_pinned: Vec<(String, VerifyingKey)>,
}

/// The environment variable holding the release sanctioned public key (64-hex ed25519), read at
/// **compile time** so release binaries carry the project key without a runtime file. Wired at
/// packaging time (workstream C); when unset, release builds trust no sanctioned key.
#[cfg(not(debug_assertions))]
const PROJECT_PUBLIC_KEY_ENV: Option<&str> = option_env!("GENEALOGY_PROJECT_PUBLIC_KEY");

impl TrustRoots {
    /// The sanctioned roots baked into this binary (ADR 0014 §3, §6).
    ///
    /// **Debug/CI builds** include [`signing::DEV_PUBLIC_KEY`] so the first-party fleet
    /// `cargo xtask build-plugins` (dev-)signs classifies as [`TrustTier::Sanctioned`] during local
    /// and CI testing. **Release builds** read the real project key from the compile-time
    /// `GENEALOGY_PROJECT_PUBLIC_KEY` (64-hex); when it is unset the sanctioned set is empty (the
    /// release signing key is wired at packaging time, workstream C). The dev key is **never**
    /// trusted in a release build.
    #[must_use]
    pub fn embedded() -> Self {
        Self {
            sanctioned: embedded_sanctioned_keys(),
            user_pinned: Vec::new(),
        }
    }

    /// Adds the user's pinned publisher keys to the trust roots (builder style).
    #[must_use]
    pub fn with_user_keys(mut self, pins: impl IntoIterator<Item = (String, VerifyingKey)>) -> Self {
        self.user_pinned.extend(pins);
        self
    }
}

/// The embedded sanctioned key(s) for a **debug/CI** build: the deterministic dev key, so locally
/// (dev-)signed bundles classify as sanctioned. Never compiled into a release build.
#[cfg(debug_assertions)]
fn embedded_sanctioned_keys() -> Vec<VerifyingKey> {
    match signing::verifying_key_from_bytes(&signing::DEV_PUBLIC_KEY) {
        Ok(key) => vec![key],
        Err(_) => Vec::new(),
    }
}

/// The embedded sanctioned key(s) for a **release** build: the compile-time project key, or none.
#[cfg(not(debug_assertions))]
fn embedded_sanctioned_keys() -> Vec<VerifyingKey> {
    match PROJECT_PUBLIC_KEY_ENV.and_then(signing::verifying_key_from_hex) {
        Some(key) => vec![key],
        None => Vec::new(),
    }
}

/// Builds the trust roots for classification: the embedded sanctioned key(s) plus the user's pinned
/// publisher keys, decoded from their raw 32-byte public-key encodings (ADR 0014 §3).
///
/// The pins come from `genealogy-app`'s client-scope trust store (hex-decoded there); this crate owns
/// the [`VerifyingKey`] decode because `ed25519-dalek` is its dependency, not the app's.
///
/// # Errors
///
/// [`PluginError::Signature`] if a pinned entry's bytes are not a valid ed25519 public key.
pub fn resolve_trust_roots(pins: &[(String, [u8; 32])]) -> Result<TrustRoots, PluginError> {
    let mut keys = Vec::with_capacity(pins.len());
    for (publisher, bytes) in pins {
        let key = signing::verifying_key_from_bytes(bytes).map_err(|error| {
            PluginError::Signature(format!(
                "pinned publisher {publisher:?} has an invalid public key: {error}"
            ))
        })?;
        keys.push((publisher.clone(), key));
    }
    Ok(TrustRoots::embedded().with_user_keys(keys))
}

/// Classifies a bundle into a [`TrustTier`] by verifying its detached signature against `roots`
/// (ADR 0014 §3).
///
/// `signature_bytes` is the raw `plugin.sig` (`None` when the bundle carries no signature). The
/// digest is recomputed over `manifest_toml_bytes` + `wasm_bytes`, so a tampered manifest or
/// component no longer verifies.
///
/// # Errors
///
/// [`PluginError::Signature`] if the signature bytes are malformed, or a present signature verifies
/// against no sanctioned or user-trusted key — a present-but-unverifiable signature **fails closed**,
/// never silently downgraded to [`TrustTier::Untrusted`].
pub fn classify(
    roots: &TrustRoots,
    manifest_toml_bytes: &[u8],
    wasm_bytes: &[u8],
    signature_bytes: Option<&[u8]>,
) -> Result<TrustTier, PluginError> {
    let Some(signature_bytes) = signature_bytes else {
        return Ok(TrustTier::Untrusted);
    };
    let signature = signing::signature_from_bytes(signature_bytes)
        .map_err(|error| PluginError::Signature(format!("malformed detached signature: {error}")))?;
    let digest = signing::bundle_digest(manifest_toml_bytes, wasm_bytes);
    for key in &roots.sanctioned {
        if signing::verify(key, &digest, &signature).is_ok() {
            return Ok(TrustTier::Sanctioned);
        }
    }
    for (_publisher, key) in &roots.user_pinned {
        if signing::verify(key, &digest, &signature).is_ok() {
            return Ok(TrustTier::UserTrusted);
        }
    }
    Err(PluginError::Signature(
        "the bundle signature verifies against no sanctioned or user-trusted key (fails closed, ADR 0014 §3)"
            .to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{TrustRoots, TrustTier, classify, resolve_trust_roots};
    use crate::signing::{self, SigningKey, dev_signing_key};

    fn sample_bundle() -> (Vec<u8>, Vec<u8>) {
        (b"id = 'sample'\n".to_vec(), b"\0asm-sample-component".to_vec())
    }

    fn sign_bundle(key: &SigningKey, manifest: &[u8], wasm: &[u8]) -> [u8; 64] {
        signing::signature_to_bytes(&signing::sign(key, &signing::bundle_digest(manifest, wasm)))
    }

    #[test]
    fn an_unsigned_bundle_is_untrusted() {
        let (manifest, wasm) = sample_bundle();
        let tier = classify(&TrustRoots::embedded(), &manifest, &wasm, None).expect("classify");
        assert_eq!(tier, TrustTier::Untrusted);
    }

    #[test]
    fn a_dev_signed_bundle_is_sanctioned_and_embedded_debug_carries_the_dev_key() {
        // In debug/CI, `TrustRoots::embedded()` carries `DEV_PUBLIC_KEY`, so a bundle signed with the
        // dev key (as `cargo xtask build-plugins` produces) classifies as Sanctioned.
        let (manifest, wasm) = sample_bundle();
        let signature = sign_bundle(&dev_signing_key(), &manifest, &wasm);
        let tier = classify(&TrustRoots::embedded(), &manifest, &wasm, Some(&signature)).expect("classify");
        assert_eq!(tier, TrustTier::Sanctioned);
    }

    #[test]
    fn an_unknown_key_fails_closed() {
        let (manifest, wasm) = sample_bundle();
        let stranger = SigningKey::from_bytes(&[9u8; 32]);
        let signature = sign_bundle(&stranger, &manifest, &wasm);
        assert!(
            classify(&TrustRoots::embedded(), &manifest, &wasm, Some(&signature)).is_err(),
            "a present signature from an untrusted key must be a hard error, not Untrusted"
        );
    }

    #[test]
    fn a_user_pinned_key_is_user_trusted() {
        let (manifest, wasm) = sample_bundle();
        let pinned = SigningKey::from_bytes(&[3u8; 32]);
        let signature = sign_bundle(&pinned, &manifest, &wasm);
        let roots = resolve_trust_roots(&[("acme".to_owned(), pinned.verifying_key().to_bytes())]).expect("roots");
        let tier = classify(&roots, &manifest, &wasm, Some(&signature)).expect("classify");
        assert_eq!(tier, TrustTier::UserTrusted);
    }

    #[test]
    fn a_tampered_bundle_with_a_valid_format_signature_errors() {
        let (manifest, wasm) = sample_bundle();
        let signature = sign_bundle(&dev_signing_key(), &manifest, &wasm);
        let mut tampered = wasm.clone();
        tampered.push(0x99);
        assert!(
            classify(&TrustRoots::embedded(), &manifest, &tampered, Some(&signature)).is_err(),
            "a tampered component must not silently verify"
        );
    }

    #[test]
    fn a_malformed_signature_is_an_error_not_a_panic() {
        let (manifest, wasm) = sample_bundle();
        assert!(classify(&TrustRoots::embedded(), &manifest, &wasm, Some(&[0u8; 10])).is_err());
    }
}
