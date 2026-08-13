//! End-to-end check that a publisher key pinned in the client-scope `[plugin_trust]` config
//! (ADR 0014 §3, ADR 0015) round-trips through `vitni-app`'s `ConfigStore` and classifies a
//! bundle signed by that key as [`TrustTier::UserTrusted`]. This is the one place both the app config
//! surface and the host's `classify` are in scope (the host sits above the app), so the whole path is
//! exercised together.

use std::collections::BTreeMap;

use vitni_app::{ConfigStore, FileConfigStore, PluginTrustConfig, resolve_trust_pins};
use vitni_plugin_host::signing::{self, SigningKey};
use vitni_plugin_host::{TrustTier, classify, resolve_trust_roots};

/// Lowercase hex encoding of `bytes` (a public key, for the config's pin value).
fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

#[test]
fn a_config_pinned_publisher_key_classifies_its_bundle_as_user_trusted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileConfigStore::new(dir.path().join("config.toml"), None);
    store.load_or_bootstrap_config().expect("bootstrap config");

    // Pin a (non-dev) publisher key in the client-scope trust store and persist it.
    let publisher_key = SigningKey::from_bytes(&[42u8; 32]);
    let mut publishers = BTreeMap::new();
    publishers.insert(
        "acme-genealogy".to_owned(),
        hex_encode(&publisher_key.verifying_key().to_bytes()),
    );
    store
        .store_plugin_trust(&PluginTrustConfig { publishers })
        .expect("store pinned publisher");

    // Read it back, decode the pin, and build the trust roots (embedded + this pin).
    let trust = store.load_plugin_trust().expect("reload trust store");
    let pins = resolve_trust_pins(&trust).expect("decode pins");
    let roots = resolve_trust_roots(&pins).expect("build trust roots");

    // A bundle signed by the pinned key classifies as UserTrusted.
    let manifest = b"id = 'acme-plugin'\n".to_vec();
    let wasm = b"\0asm-acme-component".to_vec();
    let signature = signing::signature_to_bytes(&signing::sign(
        &publisher_key,
        &signing::bundle_digest(&manifest, &wasm),
    ));
    let tier = classify(&roots, &manifest, &wasm, Some(&signature)).expect("classify");
    assert_eq!(tier, TrustTier::UserTrusted);
}

#[test]
fn a_bad_hex_pin_is_a_clear_config_error() {
    let mut publishers = BTreeMap::new();
    publishers.insert("broken".to_owned(), "not-hex".to_owned());
    let trust = PluginTrustConfig { publishers };
    let error = resolve_trust_pins(&trust).expect_err("a bad pin must error");
    assert!(
        error.to_string().contains("broken"),
        "the error must name the offending publisher, got {error}"
    );
}
