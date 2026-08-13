//! Host-mechanics integration tests (ADR 0011): capability gating (deny-by-default), the fuel
//! limit, and the memory cap — exercised through the test fixture component, independent of GEDCOM.
//!
//! These require the fixture component to be built first: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};

use uuid::Uuid;
use vitni_app::{AppDefaults, OperatorConfig, Session, Workspace, WorkspaceDefaults, list_persons};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};
use vitni_plugin_host::{Capability, Grants, PluginError, ResourceBudget};

mod common;

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

/// A `Session` whose operator is a Software agent — every plugin-authored change is audited under
/// it (ADR 0007 §7).
fn software_session() -> Session {
    Session::new(Agent {
        kind: AgentKind::Software {
            name: "vitni-fixture-plugin".to_owned(),
            version: "0.1.0".to_owned(),
        },
        id: AgentId::from_uuid(Uuid::from_u128(7)),
        display: Some("Fixture".to_owned()),
    })
}

/// Initializes a fresh workspace directory, returning its path and the temp-dir guard.
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
async fn commands_capability_is_denied_without_a_grant() {
    let (root, _dir) = init_workspace();
    let host = common::host();
    let component = common::component("fixture");

    let workspace = open_workspace(&root).await;
    let result = host
        .fixture_try_create(
            &component,
            workspace,
            software_session(),
            Grants::none(),
            ResourceBudget::default(),
            None,
        )
        .await;

    match result {
        Err(PluginError::Guest(message)) => assert!(
            message.contains("Denied"),
            "expected a denied capability error, got: {message}"
        ),
        Err(other) => panic!("expected a denied guest error, got {other:?}"),
        Ok(_) => panic!("expected a denied guest error, but the call succeeded"),
    }

    let workspace = open_workspace(&root).await;
    assert!(
        list_persons(&workspace).await.expect("list").is_empty(),
        "a denied command must not have created a person"
    );
}

#[tokio::test]
async fn commands_capability_succeeds_when_granted() {
    let (root, _dir) = init_workspace();
    let host = common::host();
    let component = common::component("fixture");

    let workspace = open_workspace(&root).await;
    let grants = Grants::none().with(Capability::Commands).with(Capability::Log);
    let (human_id, workspace) = host
        .fixture_try_create(
            &component,
            workspace,
            software_session(),
            grants,
            ResourceBudget::default(),
            None,
        )
        .await
        .expect("granted create");

    assert_eq!(human_id, "I0001");
    assert_eq!(list_persons(&workspace).await.expect("list").len(), 1);
}

#[tokio::test]
async fn fuel_limit_traps_a_runaway_guest() {
    let (root, _dir) = init_workspace();
    let host = common::host();
    let component = common::component("fixture");

    let workspace = open_workspace(&root).await;
    let budget = ResourceBudget {
        fuel: 50_000_000,
        ..ResourceBudget::default()
    };
    let result = host
        .fixture_busy_loop(&component, workspace, software_session(), Grants::none(), budget)
        .await;

    assert!(
        matches!(result, Err(PluginError::ResourceLimit(_))),
        "a runaway guest must be stopped by the fuel limit, got {result:?}"
    );
}

#[tokio::test]
async fn memory_cap_denies_an_oversized_allocation() {
    let (root, _dir) = init_workspace();
    let host = common::host();
    let component = common::component("fixture");

    // A generous cap admits a small allocation.
    let workspace = open_workspace(&root).await;
    let (report, _workspace) = host
        .fixture_allocate(
            &component,
            workspace,
            software_session(),
            Grants::none(),
            ResourceBudget::default(),
            8,
        )
        .await
        .expect("allocate under cap");
    assert_eq!(report, 1, "an 8 MiB allocation should fit the default cap");

    // A tight cap denies a large allocation — the limiter refuses the growth (no trap).
    let workspace = open_workspace(&root).await;
    let tight = ResourceBudget {
        memory_bytes: 8 * 1024 * 1024,
        ..ResourceBudget::default()
    };
    let (report, _workspace) = host
        .fixture_allocate(&component, workspace, software_session(), Grants::none(), tight, 64)
        .await
        .expect("allocate over cap returns gracefully");
    assert_eq!(report, 0, "a 64 MiB allocation must be denied by an 8 MiB cap");
}
