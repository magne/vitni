//! Integration tests for the `present` capability and the `assisted-import` world (ADR 0017 §5),
//! exercised through the fixture component's `try-present` and `run-assisted` exports against a
//! scripted [`Presenter`]. Grant enforcement, the suspend/answer round-trip, cancellation-through-the
//! -response, and a presenter failure mapping onto `backend` are covered here; the contract parsing
//! and the session state machine are unit-tested in `genealogy-ui`.
//!
//! Requires the fixture component: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use genealogy_app::{AppDefaults, OperatorConfig, Session, Workspace, WorkspaceDefaults};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use genealogy_plugin_host::{Capability, Grants, PluginError, PresentError, Presenter, ResourceBudget};
use uuid::Uuid;

mod common;

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
            name: "genealogy-fixture-plugin".to_owned(),
            version: "0.1.0".to_owned(),
        },
        id: AgentId::from_uuid(Uuid::from_u128(7)),
        display: Some("Fixture".to_owned()),
    })
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

/// The full assisted-import grant set (ADR 0017 §9).
fn assisted_grants() -> Grants {
    Grants::none()
        .with(Capability::Log)
        .with(Capability::Query)
        .with(Capability::Commands)
        .with(Capability::Progress)
        .with(Capability::Net)
        .with(Capability::MediaStore)
        .with(Capability::Ai)
        .with(Capability::Present)
}

/// A presenter scripted with a fixed outcome that records every payload it was shown, so a test can
/// assert both the response the guest received and the payload the guest sent.
struct ScriptedPresenter {
    seen: Arc<Mutex<Vec<String>>>,
    outcome: Outcome,
}

/// What the scripted presenter does when shown a payload.
enum Outcome {
    /// Answer with this response string (the wizard's `submit`/`cancel` JSON).
    Reply(String),
    /// Fail as a dropped/unreachable frontend channel would (ADR 0017 §5 → `backend`).
    Fail(String),
}

impl ScriptedPresenter {
    fn new(outcome: Outcome) -> (Self, Arc<Mutex<Vec<String>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                seen: Arc::clone(&seen),
                outcome,
            },
            seen,
        )
    }
}

#[async_trait]
impl Presenter for ScriptedPresenter {
    async fn present(&mut self, payload: String) -> Result<String, PresentError> {
        self.seen.lock().expect("seen lock").push(payload);
        match &self.outcome {
            Outcome::Reply(response) => Ok(response.clone()),
            Outcome::Fail(message) => Err(PresentError::Backend(message.clone())),
        }
    }
}

fn assert_guest_error_contains(result: Result<(String, Workspace), PluginError>, needle: &str) {
    // `Workspace` is not `Debug`, so reduce the success value to its summary string before matching.
    let outcome = result.map(|(summary, _ws)| summary);
    let matched = matches!(&outcome, Err(PluginError::Guest(message)) if message.contains(needle));
    assert!(matched, "expected a guest error containing {needle:?}, got {outcome:?}");
}

// ----- present: grant enforcement + round-trip -----

#[tokio::test]
async fn present_is_denied_without_a_grant() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (presenter, seen) = ScriptedPresenter::new(Outcome::Reply(r#"{"kind":"cancel"}"#.to_owned()));
    let result = common::host()
        .fixture_try_present(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none(),
            ResourceBudget::default(),
            Box::new(presenter),
            r#"{"kind":"summary"}"#,
        )
        .await;
    assert_guest_error_contains(result, "Denied");
    assert!(
        seen.lock().expect("seen lock").is_empty(),
        "a denied present must never reach the presenter"
    );
}

#[tokio::test]
async fn present_submit_round_trips_the_payload_and_response() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let response = r#"{"kind":"submit","action":"import","values":{"row":"a"}}"#;
    let (presenter, seen) = ScriptedPresenter::new(Outcome::Reply(response.to_owned()));
    let payload = r#"{"kind":"records","source":{"title":"1920","url":"https://x/"},"records":[]}"#;
    let (returned, _ws) = common::host()
        .fixture_try_present(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Present),
            ResourceBudget::default(),
            Box::new(presenter),
            payload,
        )
        .await
        .expect("present succeeds");
    assert_eq!(
        returned, response,
        "the presenter's response reaches the guest verbatim"
    );
    let seen = seen.lock().expect("seen lock");
    assert_eq!(
        seen.as_slice(),
        [payload.to_owned()],
        "the guest's payload reaches the presenter"
    );
}

#[tokio::test]
async fn present_cancel_response_passes_through() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (presenter, _seen) = ScriptedPresenter::new(Outcome::Reply(r#"{"kind":"cancel"}"#.to_owned()));
    let (returned, _ws) = common::host()
        .fixture_try_present(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Present),
            ResourceBudget::default(),
            Box::new(presenter),
            r#"{"kind":"summary","imported":[],"skipped":0}"#,
        )
        .await
        .expect("present succeeds");
    assert_eq!(returned, r#"{"kind":"cancel"}"#);
}

#[tokio::test]
async fn a_presenter_failure_surfaces_as_backend() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    // A dropped/unreachable frontend channel is a presenter failure; the host maps it onto `backend`.
    let (presenter, _seen) = ScriptedPresenter::new(Outcome::Fail("channel dropped".to_owned()));
    let result = common::host()
        .fixture_try_present(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Present),
            ResourceBudget::default(),
            Box::new(presenter),
            r#"{"kind":"summary","imported":[],"skipped":0}"#,
        )
        .await;
    assert_guest_error_contains(result, "Backend");
}

// ----- run_assisted_import: the entry point + grant matrix -----

fn assisted_invocation(workspace: Workspace, grants: Grants) -> genealogy_plugin_host::Invocation {
    genealogy_plugin_host::Invocation {
        workspace,
        session: software_session(),
        grants,
        budget: ResourceBudget::assisted(),
        net_policy: genealogy_plugin_host::NetPolicy::deny_all(),
        ai_config: genealogy_app::AiConfig::default(),
        provenance_confidence: Some(genealogy_app::Confidence::Low),
    }
}

#[tokio::test]
async fn run_assisted_import_drives_a_session_to_its_summary() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let summary = r#"{"kind":"summary","imported":[{"human_id":"I1","label":"Ola"}],"skipped":0}"#;
    let (presenter, seen) = ScriptedPresenter::new(Outcome::Reply(summary.to_owned()));
    let request = r#"{"kind":"url","url":"https://www.digitalarkivet.no/census/person/1"}"#;
    let (returned, _ws) = common::host()
        .run_assisted_import(
            &common::component("fixture"),
            assisted_invocation(workspace, assisted_grants()),
            request,
            Box::new(presenter),
            |_update| genealogy_plugin_host::ProgressControl::Proceed,
        )
        .await
        .expect("assisted import runs");
    assert_eq!(
        returned, summary,
        "the fixture returns the presenter's response as the summary"
    );
    let seen = seen.lock().expect("seen lock");
    assert_eq!(seen.as_slice(), [request.to_owned()], "the guest presents the request");
}

#[tokio::test]
async fn run_assisted_import_is_denied_without_the_present_grant() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    // Every capability except `present`: the fixture's single `present.show` is denied, so the guest's
    // `run-assisted` returns an error.
    let grants = Grants::none()
        .with(Capability::Log)
        .with(Capability::Query)
        .with(Capability::Commands)
        .with(Capability::Progress)
        .with(Capability::Net)
        .with(Capability::MediaStore)
        .with(Capability::Ai);
    let (presenter, seen) = ScriptedPresenter::new(Outcome::Reply(r#"{"kind":"cancel"}"#.to_owned()));
    let result = common::host()
        .run_assisted_import(
            &common::component("fixture"),
            assisted_invocation(workspace, grants),
            r#"{"kind":"url","url":"https://x/"}"#,
            Box::new(presenter),
            |_update| genealogy_plugin_host::ProgressControl::Proceed,
        )
        .await;
    assert_guest_error_contains(result, "Denied");
    assert!(seen.lock().expect("seen lock").is_empty());
}
