//! Verifies the signature + manifest sidecars that `cargo xtask build-plugins` emits beside each
//! `<id>.wasm` (ADR 0014 §2): every bundle's `<id>.sig` must verify against the embedded dev trust
//! root over the canonical digest of its `<id>.plugin.toml` and `<id>.wasm`. Requires the plugins to
//! be built first (CI runs `build-plugins` before the tests, as the other host integration tests
//! already assume).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::fs;
use std::path::PathBuf;

use genealogy_plugin_host::signing::{self, DEV_PUBLIC_KEY};

/// Every first-party plugin id `build-plugins` produces a bundle for.
const EXPECTED: &[&str] = &[
    "gedcom-import",
    "gedcom-export",
    "gramps-import",
    "gramps-export",
    "digitalarkivet-import",
    "ui-panel",
    "fixture",
];

fn plugins_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins");
    assert!(
        dir.is_dir(),
        "missing {} — run `cargo xtask build-plugins` first",
        dir.display()
    );
    dir
}

fn read_bundle(id: &str) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let dir = plugins_dir();
    let read = |suffix: &str| fs::read(dir.join(format!("{id}.{suffix}")));
    let manifest = read("plugin.toml").expect("reading the bundle manifest; run `cargo xtask build-plugins`");
    let wasm = read("wasm").expect("reading the bundle component; run `cargo xtask build-plugins`");
    let signature = read("sig").expect("reading the bundle signature; run `cargo xtask build-plugins`");
    (manifest, wasm, signature)
}

#[test]
fn emitted_bundles_verify_against_the_dev_trust_root() {
    let verifying_key = signing::verifying_key_from_bytes(&DEV_PUBLIC_KEY).expect("dev public key decodes");
    for id in EXPECTED {
        let (manifest, wasm, signature_bytes) = read_bundle(id);
        assert_eq!(signature_bytes.len(), 64, "{id}.sig is not 64 raw bytes");
        let signature = signing::signature_from_bytes(&signature_bytes).expect("signature decodes");
        let digest = signing::bundle_digest(&manifest, &wasm);
        assert!(
            signing::verify(&verifying_key, &digest, &signature).is_ok(),
            "{id} bundle signature did not verify against the dev trust root"
        );
    }
}

#[test]
fn tampering_with_a_bundle_component_breaks_verification() {
    let verifying_key = signing::verifying_key_from_bytes(&DEV_PUBLIC_KEY).expect("dev public key decodes");
    let (manifest, mut wasm, signature_bytes) = read_bundle("gedcom-import");
    let signature = signing::signature_from_bytes(&signature_bytes).expect("signature decodes");

    assert!(!wasm.is_empty(), "gedcom-import.wasm is empty");
    wasm[0] ^= 0xff;
    let tampered_digest = signing::bundle_digest(&manifest, &wasm);
    assert!(
        signing::verify(&verifying_key, &tampered_digest, &signature).is_err(),
        "a tampered component must not verify against the original signature"
    );
}
