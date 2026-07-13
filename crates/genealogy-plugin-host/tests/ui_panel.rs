//! Plugin-UI integration test (ADR 0012): the host runs the `ui-panel` plugin and returns the form
//! it emitted as an opaque JSON string. The host does not parse the payload — this test parses it
//! only to confirm the plugin produced a well-formed form against the vocabulary schema; the renderer
//! (`genealogy-ui-dioxus`) asserts full `genealogy_ui::Form` conformance and rendering.
//!
//! Requires the component to be built first: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};

use genealogy_app::{AppDefaults, OperatorConfig, Session, Workspace, WorkspaceDefaults};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use genealogy_plugin_host::{Capability, Grants, PluginHost, ResourceBudget};
use uuid::Uuid;

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

fn plugin_path(id: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/plugins")
        .join(format!("{id}.wasm"));
    assert!(
        path.is_file(),
        "missing plugin component {} — run `cargo xtask build-plugins` first",
        path.display()
    );
    path
}

fn init_workspace() -> (PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("ws");
    Workspace::init(&root, &operator(), &AppDefaults::default(), None).expect("init");
    (root, dir)
}

async fn open_workspace(root: &Path) -> Workspace {
    Workspace::open(root, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace")
}

#[tokio::test]
async fn ui_panel_plugin_returns_a_wellformed_form() {
    let (root, _dir) = init_workspace();
    let host = PluginHost::new().expect("host");
    let component = host.load(&plugin_path("ui-panel")).expect("load ui-panel");

    let workspace = open_workspace(&root).await;
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

    assert!(!json.is_empty(), "the plugin must emit a non-empty form payload");

    // The host carries the payload opaquely; parse it here only to confirm it is a well-formed panel.
    let panel: serde_json::Value = serde_json::from_str(&json).expect("plugin emitted valid JSON");
    assert_eq!(panel["kind"], "form", "the demo panel is a form (ADR 0022)");
    assert!(panel["title"].is_string(), "form has a title");
    let fields = panel["fields"].as_array().expect("form has fields");
    assert!(!fields.is_empty(), "form has at least one field");
    // Every field is internally tagged with a `kind` discriminator (ADR 0012).
    for field in fields {
        assert!(field["kind"].is_string(), "each field carries a kind: {field}");
    }
    // The single submit label is replaced by one or more actions (ADR 0022 §1).
    let actions = panel["actions"].as_array().expect("form has actions");
    assert!(!actions.is_empty(), "form has at least one action");
    for action in actions {
        assert!(action["id"].is_string(), "each action carries an id: {action}");
        assert!(action["label"].is_string(), "each action carries a label: {action}");
    }
    assert!(
        panel.get("submit").is_none(),
        "the bare submit label is gone (ADR 0022)"
    );
}

#[tokio::test]
async fn ui_panel_plugin_emits_label_ids_not_display_text() {
    let (root, _dir) = init_workspace();
    let host = PluginHost::new().expect("host");
    let component = host.load(&plugin_path("ui-panel")).expect("load ui-panel");

    let workspace = open_workspace(&root).await;
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

    // Labels are Fluent message ids resolved by the frontend (ADR 0012 §5); the host stays opaque.
    let form: serde_json::Value = serde_json::from_str(&json).expect("plugin emitted valid JSON");
    assert_eq!(form["title"], "form-title", "title is a message id, not display text");
}

#[tokio::test]
async fn handle_action_with_commands_grant_creates_a_note() {
    let (root, _dir) = init_workspace();
    let host = PluginHost::new().expect("host");
    let component = host.load(&plugin_path("ui-panel")).expect("load ui-panel");
    let workspace = open_workspace(&root).await;

    let (json, workspace) = host
        .run_ui_panel_action(
            &component,
            workspace,
            software_session(),
            Grants::none().with(Capability::Log).with(Capability::Commands),
            ResourceBudget::default(),
            "save",
            r#"{"title":"A parish record","notes":"Found in the 1801 census","year":1801,
                "private":false,"confidence":"low","detail":"","when":""}"#,
        )
        .await
        .expect("handle-action succeeds when commands is granted");

    // Validation/confirmation rides the submit-result payload (ADR 0022 §2), which the host carries
    // opaquely; parse it here only to assert the outcome.
    let result: serde_json::Value = serde_json::from_str(&json).expect("valid submit-result JSON");
    assert_eq!(result["kind"], "success", "a granted save succeeds: {json}");

    // The note actually landed in the mutated workspace (audited as a Software operator).
    let counts = genealogy_app::workspace_counts(&workspace).await.expect("counts");
    assert_eq!(counts.note, 1, "the save created exactly one note");
}

#[tokio::test]
async fn handle_action_without_commands_grant_is_denied() {
    let (root, _dir) = init_workspace();
    let host = PluginHost::new().expect("host");
    let component = host.load(&plugin_path("ui-panel")).expect("load ui-panel");
    let workspace = open_workspace(&root).await;

    // Only `log` is granted: the guest's create-note call is denied by construction (ADR 0011 §2,
    // ADR 0022 §3) and the plugin surfaces the denial as a technical `err`.
    let outcome = host
        .run_ui_panel_action(
            &component,
            workspace,
            software_session(),
            Grants::none().with(Capability::Log),
            ResourceBudget::default(),
            "save",
            r#"{"title":"A parish record"}"#,
        )
        .await;

    assert!(
        outcome.is_err(),
        "a save without the commands grant must be denied (deny-by-default)"
    );
}

#[tokio::test]
async fn preview_action_returns_a_table_panel() {
    let (root, _dir) = init_workspace();
    let host = PluginHost::new().expect("host");
    let component = host.load(&plugin_path("ui-panel")).expect("load ui-panel");
    let workspace = open_workspace(&root).await;

    let (json, _workspace) = host
        .run_ui_panel_action(
            &component,
            workspace,
            software_session(),
            Grants::none().with(Capability::Log).with(Capability::Commands),
            ResourceBudget::default(),
            "preview",
            r#"{"title":"A parish record","year":1801}"#,
        )
        .await
        .expect("handle-action succeeds");

    let result: serde_json::Value = serde_json::from_str(&json).expect("valid submit-result JSON");
    assert_eq!(result["kind"], "success", "preview succeeds: {json}");
    assert_eq!(
        result["panel"]["kind"], "table",
        "preview returns a table panel: {json}"
    );
    assert_eq!(
        result["panel"]["title"], "preview-title",
        "the table title is a message id"
    );
}
