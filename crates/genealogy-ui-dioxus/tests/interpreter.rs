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
use genealogy_ui::Panel;
use genealogy_ui_dioxus::vocabulary_render::PanelView;
use uuid::Uuid;

thread_local! {
    /// The panel under test, handed to the no-argument render root (which `VirtualDom::new` requires).
    static PANEL: RefCell<Option<Panel>> = const { RefCell::new(None) };
}

fn root() -> Element {
    let panel = PANEL
        .with(|cell| cell.borrow().clone())
        .expect("panel set before render");
    rsx! { PanelView { panel } }
}

/// Renders a panel through the interpreter to an HTML string.
fn render_panel(panel: Panel) -> String {
    PANEL.with(|cell| *cell.borrow_mut() = Some(panel));
    let mut vdom = VirtualDom::new(root);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
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
    Workspace::init(&ws_dir, &operator(), &AppDefaults::default(), None).expect("init");
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
    let panel = genealogy_ui::parse(&json).expect("the plugin emitted a schema-conformant panel");
    // The plugin returns label ids; resolve them against its catalogue (ADR 0012 §5).
    let panel = genealogy_ui::resolve_panel(&panel, &catalogue_dir(), "ui-panel", &["en".parse().expect("tag")]);

    let html = render_panel(panel);

    // The resolved title, every field label, and both action buttons render.
    for needle in [
        "Add research note",
        "Title",
        "Detail",
        "Notes",
        "Year",
        "Date",
        "Private",
        "Confidence",
        "Save note",
        "Preview",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in rendered HTML:\n{html}");
    }
    assert!(html.contains("<select"), "the select field renders a <select>:\n{html}");
    assert!(
        html.contains("<textarea"),
        "the textarea field renders a <textarea>:\n{html}"
    );
    assert!(
        html.contains("type=\"date\""),
        "the date field renders a date input:\n{html}"
    );
    assert!(
        html.contains("type=\"checkbox\""),
        "the checkbox field renders:\n{html}"
    );
}

#[tokio::test]
async fn resolves_plugin_form_to_norwegian() {
    let json = run_plugin().await;
    let panel = genealogy_ui::parse(&json).expect("parse");
    let panel = genealogy_ui::resolve_panel(&panel, &catalogue_dir(), "ui-panel", &["nb-NO".parse().expect("tag")]);
    assert!(matches!(panel, Panel::Form(_)), "the ui-panel plugin emits a form");
    let Panel::Form(form) = panel else {
        return;
    };
    assert_eq!(
        form.title, "Legg til forskningsnotat",
        "nb-NO negotiates to the no catalogue"
    );
    assert_eq!(form.actions[0].label, "Lagre notat");
}

/// A pure-SSR test: a `Table` panel renders its localized columns and literal row cells (ADR 0022).
#[test]
fn renders_a_table_panel() {
    use genealogy_ui::Table;

    let panel = Panel::Table(Table {
        title: "Submitted values".to_owned(),
        columns: vec!["Field".to_owned(), "Value".to_owned()],
        rows: vec![
            vec!["title".to_owned(), "Hello".to_owned()],
            vec!["year".to_owned(), "1900".to_owned()],
        ],
    });
    let html = render_panel(panel);

    assert!(html.contains("<table"), "a table renders a <table>:\n{html}");
    for needle in ["Submitted values", "Field", "Value", "title", "Hello", "year", "1900"] {
        assert!(html.contains(needle), "expected {needle:?} in rendered HTML:\n{html}");
    }
}
