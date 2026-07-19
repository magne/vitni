//! Plugin discovery integration test (PR21): scanning a directory of built components and reading
//! their genuinely declared metadata — role (from the exported entry point), the host-API version
//! the component was compiled against (from its imported interfaces), and declared capabilities
//! (from which `genealogy:host-api/<capability>` interfaces it imports).
//!
//! No plugin-owned id/version/capability manifest exists yet (that is ADR 0014, deferred); this test
//! asserts only what [`PluginHost::discover`] can read from the component itself.
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

use std::path::PathBuf;

use genealogy_plugin_host::{Capability, PluginRole};

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
    // Discovery reflects the capabilities the component actually imports (wit-bindgen tree-shakes the
    // rest): the flow drives net, media-store, present, query, commands, progress, and log.
    for capability in [
        Capability::Log,
        Capability::Query,
        Capability::Commands,
        Capability::Progress,
        Capability::Net,
        Capability::MediaStore,
        Capability::Present,
    ] {
        assert!(
            info.capabilities.contains(&capability),
            "the assisted-import component imports {capability:?}"
        );
    }
    // `ai` is granted at the call site but never invoked in this flow (ADR 0017 §4), so it is
    // tree-shaken from the component; `import-source` is a bulk-only capability.
    assert!(
        !info.capabilities.contains(&Capability::Ai),
        "ai is unused and tree-shaken"
    );
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
    let result = host.discover(&PathBuf::from("/nonexistent/path/does-not-exist"));
    assert!(result.is_err(), "a missing plugins directory must be a typed error");
}

#[test]
fn discover_skips_non_wasm_files_in_the_directory() {
    let host = common::host();
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("README.md"), b"not a plugin").expect("write");
    let found = host.discover(dir.path()).expect("discover an otherwise-empty dir");
    assert!(found.is_empty(), "non-.wasm files must be ignored, not fail discovery");
}
