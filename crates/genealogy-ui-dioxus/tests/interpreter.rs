//! End-to-end interpreter test (ADR 0012): run the `ui-panel` plugin through the host, parse its
//! JSON with `genealogy-ui`, and render it through the Dioxus vocabulary interpreter to an HTML
//! string. Proves the full host → plugin → vocabulary → RSX path without opening a window.
//!
//! Requires the component to be built first: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use genealogy_app::{AppDefaults, OperatorConfig, Session, Workspace};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use genealogy_plugin_host::{Capability, Grants, PluginHost, ResourceBudget};
use genealogy_ui::Form;
use genealogy_ui_dioxus::vocabulary_render::FormView;
use uuid::Uuid;

thread_local! {
    /// The form under test, handed to the no-argument render root (which `VirtualDom::new` requires).
    static FORM: RefCell<Option<Form>> = const { RefCell::new(None) };
}

fn root() -> Element {
    let form = FORM.with(|cell| cell.borrow().clone()).expect("form set before render");
    rsx! { FormView { form } }
}

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn software_session() -> Session {
    Session::new(Agent {
        kind: AgentKind::Software {
            name: "genealogy-ui-panel-plugin".to_owned(),
            version: "0.1.0".to_owned(),
        },
        id: AgentId::from_uuid(Uuid::from_u128(9)),
        display: Some("UI panel".to_owned()),
    })
}

fn plugin_path() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins/ui-panel.wasm");
    assert!(
        path.is_file(),
        "missing plugin component {} — run `cargo xtask build-plugins` first",
        path.display()
    );
    path
}

async fn open_workspace(root: &Path) -> Workspace {
    Workspace::open(root, &operator(), &genealogy_app::WorkspaceDefaults::default())
        .await
        .expect("open workspace")
}

/// Runs the plugin and returns the JSON form it emitted.
async fn run_plugin() -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws_dir = dir.path().join("ws");
    Workspace::init(&ws_dir, &operator(), &AppDefaults::default()).expect("init");
    let host = PluginHost::new().expect("host");
    let component = host.load(&plugin_path()).expect("load ui-panel");
    let workspace = open_workspace(&ws_dir).await;
    let (json, _workspace) = host
        .run_ui_panel(
            &component,
            workspace,
            software_session(),
            Grants::none().with(Capability::Log),
            ResourceBudget::default(),
        )
        .await
        .expect("run ui-panel");
    json
}

/// The `ui-panel` plugin's shipped catalogue directory, collected next to the component.
fn catalogue_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins/ui-panel/i18n")
}

#[tokio::test]
async fn renders_a_real_plugin_form_to_html() {
    let json = run_plugin().await;
    let form = genealogy_ui::parse(&json).expect("the plugin emitted a schema-conformant form");
    // The plugin returns label ids; resolve them against its catalogue (ADR 0012 §5).
    let form = genealogy_ui::resolve_form(&form, &catalogue_dir(), "ui-panel", &["en".parse().expect("tag")]);

    FORM.with(|cell| *cell.borrow_mut() = Some(form));
    let mut vdom = VirtualDom::new(root);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    // The resolved title, every field label, the select, and the submit button all render.
    for needle in [
        "Add research note",
        "Title",
        "Detail",
        "Year",
        "Private",
        "Confidence",
        "Save note",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in rendered HTML:\n{html}");
    }
    assert!(html.contains("<select"), "the select field renders a <select>:\n{html}");
    assert!(
        html.contains("type=\"checkbox\""),
        "the checkbox field renders:\n{html}"
    );
}

#[tokio::test]
async fn resolves_plugin_form_to_norwegian() {
    let json = run_plugin().await;
    let form = genealogy_ui::parse(&json).expect("parse");
    let form = genealogy_ui::resolve_form(&form, &catalogue_dir(), "ui-panel", &["nb-NO".parse().expect("tag")]);
    assert_eq!(
        form.title, "Legg til forskningsnotat",
        "nb-NO negotiates to the no catalogue"
    );
    assert_eq!(form.submit, "Lagre notat");
}
