//! Plugin discovery integration test (ADR 0014 §2): scanning a directory of built **bundles**
//! (`<id>/plugin.toml` + `plugin.wasm` + optional `plugin.sig`), reading each manifest's declared
//! metadata (id, role, host-API, capabilities) and cross-checking it against what the component
//! genuinely imports/exports. [`PluginInfo::capabilities`] carries the manifest's declared set (the
//! authoritative grant-request); the component's actual imports were verified to be a subset of it.
//!
//! Requires the plugin bundles: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::PathBuf;

use genealogy_plugin_host::signing::PluginManifest;
use genealogy_plugin_host::{Capability, PluginRole, TrustRoots, TrustTier};

mod common;

#[test]
fn discover_finds_every_built_component_with_its_id() {
    let found = common::discovered();
    let mut ids: Vec<&str> = found.iter().map(|info| info.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![
            "digitalarkivet-import",
            "fixture",
            "gedcom-export",
            "gedcom-import",
            "gramps-export",
            "gramps-import",
            "ui-panel",
        ],
        "discovery must list every .wasm component in the directory, keyed by its file stem"
    );
}

#[test]
fn gedcom_import_declares_bulk_import_role_and_its_capabilities() {
    let found = common::discovered();
    let info = found
        .iter()
        .find(|info| info.id == "gedcom-import")
        .expect("gedcom-import present");

    assert_eq!(
        info.role,
        PluginRole::BulkImport,
        "run-import export maps to BulkImport"
    );
    assert!(
        info.capabilities.contains(&Capability::Log),
        "gedcom-import imports the log capability"
    );
    assert!(
        info.capabilities.contains(&Capability::Commands),
        "gedcom-import imports the commands capability"
    );
    assert!(
        info.capabilities.contains(&Capability::Progress),
        "gedcom-import imports the progress capability"
    );
    assert!(
        info.capabilities.contains(&Capability::ImportSource),
        "gedcom-import imports the import-source capability"
    );
    assert!(
        !info.capabilities.contains(&Capability::Query),
        "gedcom-import never imports query — it only writes"
    );
    assert!(
        !info.capabilities.contains(&Capability::ExportSink),
        "gedcom-import never imports export-sink"
    );
    assert!(
        info.host_api_version.starts_with("0."),
        "the host-api version read off the component should be a semver-ish string, got {:?}",
        info.host_api_version
    );
}

#[test]
fn gedcom_export_declares_bulk_export_role_and_its_capabilities() {
    let found = common::discovered();
    let info = found
        .iter()
        .find(|info| info.id == "gedcom-export")
        .expect("gedcom-export present");

    assert_eq!(
        info.role,
        PluginRole::BulkExport,
        "run-export export maps to BulkExport"
    );
    assert!(info.capabilities.contains(&Capability::Query));
    assert!(info.capabilities.contains(&Capability::ExportSink));
    assert!(!info.capabilities.contains(&Capability::Commands));
    assert!(!info.capabilities.contains(&Capability::ImportSource));
}

#[test]
fn ui_panel_declares_the_ui_panel_role_with_log_and_commands() {
    let found = common::discovered();
    let info = found
        .iter()
        .find(|info| info.id == "ui-panel")
        .expect("ui-panel present");

    assert_eq!(info.role, PluginRole::UiPanel);
    assert!(
        info.capabilities.contains(&Capability::Log),
        "the ui-panel component imports log for its render pass"
    );
    assert!(
        info.capabilities.contains(&Capability::Commands),
        "the ui-panel component imports commands for submission (ADR 0022)"
    );
}

#[test]
fn digitalarkivet_import_declares_the_assisted_import_role_and_its_capabilities() {
    let found = common::discovered();
    let info = found
        .iter()
        .find(|info| info.id == "digitalarkivet-import")
        .expect("digitalarkivet-import present");

    assert_eq!(
        info.role,
        PluginRole::AssistedImport,
        "run-assisted export maps to AssistedImport"
    );
    // Capabilities reflect the MANIFEST's declared grant-request (ADR 0014 §2), which includes `ai`
    // even though the component tree-shakes the unused `ai` import — the manifest may declare more
    // than the component imports (inspected ⊆ declared).
    for capability in [
        Capability::Log,
        Capability::Query,
        Capability::Commands,
        Capability::Progress,
        Capability::Net,
        Capability::MediaStore,
        Capability::Ai,
        Capability::Present,
    ] {
        assert!(
            info.capabilities.contains(&capability),
            "the assisted-import manifest declares {capability:?}"
        );
    }
    // `import-source` is a bulk-only capability the assisted manifest does not declare.
    assert!(
        !info.capabilities.contains(&Capability::ImportSource),
        "assisted import is not bulk"
    );
}

#[test]
fn fixture_declares_the_test_fixture_role() {
    let found = common::discovered();
    let info = found.iter().find(|info| info.id == "fixture").expect("fixture present");

    assert_eq!(info.role, PluginRole::TestFixture);
    assert!(info.capabilities.contains(&Capability::Commands));
}

#[test]
fn discover_on_a_missing_directory_is_an_error_not_a_panic() {
    let host = common::host();
    let result = host.discover(
        &PathBuf::from("/nonexistent/path/does-not-exist"),
        &TrustRoots::embedded(),
    );
    assert!(result.is_err(), "a missing plugins directory must be a typed error");
}

#[test]
fn discover_skips_non_bundle_entries_in_the_directory() {
    let host = common::host();
    let dir = tempfile::tempdir().expect("tempdir");
    // A loose file and a subdirectory without a `plugin.toml` are both not bundles.
    std::fs::write(dir.path().join("README.md"), b"not a plugin").expect("write file");
    std::fs::create_dir(dir.path().join("not-a-bundle")).expect("stray dir");
    let found = host
        .discover(dir.path(), &TrustRoots::embedded())
        .expect("discover an otherwise-empty dir");
    assert!(
        found.is_empty(),
        "non-bundle entries must be ignored, not fail discovery"
    );
}

#[test]
fn every_built_bundle_classifies_as_sanctioned_under_the_dev_trust_root() {
    // `build-plugins` dev-signs every first-party bundle; in a debug/CI build the embedded roots
    // carry the dev key, so each classifies as Sanctioned (ADR 0014 §3).
    for info in common::discovered() {
        assert_eq!(
            info.trust,
            TrustTier::Sanctioned,
            "{} should be Sanctioned under the embedded dev trust root",
            info.id
        );
    }
}

/// Copies the real `fixture` bundle (`plugin.toml` + `plugin.wasm` + `plugin.sig`) into a fresh
/// `<root>/fixture/` bundle directory, returning the temp dir handle (kept alive by the caller) and
/// the created bundle directory. The signature is copied only when `with_signature`.
fn copy_fixture_bundle(with_signature: bool) -> (tempfile::TempDir, PathBuf) {
    let src = common::plugins_dir().join("fixture");
    let tmp = tempfile::tempdir().expect("tempdir");
    let bundle = tmp.path().join("fixture");
    std::fs::create_dir(&bundle).expect("bundle dir");
    std::fs::copy(src.join("plugin.toml"), bundle.join("plugin.toml")).expect("copy manifest");
    std::fs::copy(src.join("plugin.wasm"), bundle.join("plugin.wasm")).expect("copy component");
    if with_signature {
        std::fs::copy(src.join("plugin.sig"), bundle.join("plugin.sig")).expect("copy signature");
    }
    (tmp, bundle)
}

#[test]
fn an_unsigned_bundle_is_untrusted() {
    // A bundle with a manifest + component but no `plugin.sig` is unsigned — loadable but Untrusted.
    let host = common::host();
    let (_tmp, bundle) = copy_fixture_bundle(false);
    let info = host
        .discover_bundle(&bundle, &TrustRoots::embedded())
        .expect("discover");
    assert_eq!(
        info.trust,
        TrustTier::Untrusted,
        "an unsigned bundle is Untrusted, not an error"
    );
}

#[test]
fn a_tampered_bundle_fails_discovery_closed() {
    // Copy a real dev-signed bundle, then append a comment to its manifest: the manifest still
    // parses and cross-checks, but its digest changes, so the signature no longer verifies and
    // classification fails closed (a hard error, not Untrusted).
    let host = common::host();
    let (_tmp, bundle) = copy_fixture_bundle(true);
    let manifest_path = bundle.join("plugin.toml");
    let mut manifest = std::fs::read(&manifest_path).expect("read manifest");
    manifest.extend_from_slice(b"\n# tampered, but still valid TOML\n");
    std::fs::write(&manifest_path, &manifest).expect("write tampered manifest");

    let result = host.discover_bundle(&bundle, &TrustRoots::embedded());
    assert!(result.is_err(), "a tampered signed bundle must fail discovery closed");
}

/// Rewrites the (unsigned) fixture bundle's manifest with `capabilities`, re-serializing so the file
/// stays valid TOML regardless of the array formatting.
fn rewrite_fixture_capabilities(bundle: &std::path::Path, capabilities: Vec<String>) {
    let manifest_path = bundle.join("plugin.toml");
    let text = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let mut manifest: PluginManifest = toml::from_str(&text).expect("parse manifest");
    manifest.capabilities = capabilities;
    let rewritten = toml::to_string(&manifest).expect("serialize manifest");
    std::fs::write(&manifest_path, rewritten).expect("write rewritten manifest");
}

#[test]
fn a_manifest_under_declaring_a_capability_fails_the_cross_check() {
    // The fixture component imports `commands`. Rewrite its (unsigned) manifest to drop `commands`
    // from the declared set: the component now imports a capability the manifest does not declare,
    // so the cross-check rejects it (ADR 0014 §2). Unsigned so the cross-check, not the signature,
    // is the failing gate.
    let host = common::host();
    let (_tmp, bundle) = copy_fixture_bundle(false);
    rewrite_fixture_capabilities(
        &bundle,
        vec![
            "log".to_owned(),
            "net".to_owned(),
            "media-store".to_owned(),
            "ai".to_owned(),
            "present".to_owned(),
        ],
    );

    let result = host.discover_bundle(&bundle, &TrustRoots::embedded());
    assert!(
        result.is_err(),
        "a manifest that under-declares an imported capability must fail the cross-check"
    );
}

#[test]
fn a_manifest_over_declaring_a_capability_is_accepted() {
    // The manifest may declare MORE than the component imports (wit-bindgen tree-shakes unused
    // imports, so inspected ⊆ declared). `export-sink` is not imported by the fixture; adding it to
    // the (unsigned) manifest must still discover cleanly.
    let host = common::host();
    let (_tmp, bundle) = copy_fixture_bundle(false);
    rewrite_fixture_capabilities(
        &bundle,
        vec![
            "log".to_owned(),
            "commands".to_owned(),
            "net".to_owned(),
            "media-store".to_owned(),
            "ai".to_owned(),
            "present".to_owned(),
            "export-sink".to_owned(),
        ],
    );

    let info = host
        .discover_bundle(&bundle, &TrustRoots::embedded())
        .expect("over-declaring is legitimate");
    assert!(
        info.capabilities.contains(&Capability::ExportSink),
        "the declared (over-declared) capability is reported"
    );
}
