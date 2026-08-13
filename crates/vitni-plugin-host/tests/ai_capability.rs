//! Integration tests for the `ai` host capability (ADR 0017 §4), exercised through the fixture
//! component's `try-interpret` export. The command-kind tests spawn a checked-in argv-echo stub to
//! prove that a hostile prompt is passed as a single argv element with no shell; the vision-api tests
//! drive a local `wiremock` server, asserting the OpenAI-compatible request shape and the env-var API
//! key. The provenance tests confirm the invocation-level `Confidence::Low` template (ADR 0017 §7)
//! lands on the emitted events.
//!
//! Requires the fixture component: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;
use uuid::Uuid;
use vitni_app::{AiConfig, AiProvider, AppDefaults, Confidence, OperatorConfig, Session, Workspace, WorkspaceDefaults};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};
use vitni_plugin_host::{Capability, Grants, HostPattern, NetPolicy, PluginError, ResourceBudget};
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
            name: "vitni-fixture-plugin".to_owned(),
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

fn ai_grant() -> Grants {
    Grants::none().with(Capability::Ai)
}

/// The absolute path to a checked-in test-support script.
fn support(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/support")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// An `[ai]` config with a single `command` provider named `stub` (the default).
fn command_config(command: &str, args: &[&str], timeout_secs: u64) -> AiConfig {
    let mut providers = BTreeMap::new();
    providers.insert(
        "stub".to_owned(),
        AiProvider::Command {
            command: command.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            timeout_secs,
        },
    );
    AiConfig {
        default: Some("stub".to_owned()),
        providers,
    }
}

/// An `[ai]` config with a single `vision-api` provider named `vision` (the default).
fn vision_config(base_url: &str, api_key_env: &str) -> AiConfig {
    let mut providers = BTreeMap::new();
    providers.insert(
        "vision".to_owned(),
        AiProvider::VisionApi {
            url: base_url.to_owned(),
            model: "vision-model".to_owned(),
            api_key_env: api_key_env.to_owned(),
            timeout_secs: 30,
        },
    );
    AiConfig {
        default: Some("vision".to_owned()),
        providers,
    }
}

/// A policy that reaches `localhost` over plain HTTP so the vision-api tests hit a local mock server
/// (`require_https = false`); `command` providers ignore it.
fn localhost_policy() -> NetPolicy {
    NetPolicy {
        allowed_hosts: vec![HostPattern::parse("localhost")],
        require_https: false,
        ..NetPolicy::deny_all()
    }
}

fn base_url(server: &MockServer) -> String {
    format!("http://localhost:{}", server.address().port())
}

/// Asserts the fixture call was refused for lack of the `ai` grant.
fn assert_denied(result: Result<(String, Workspace), PluginError>) {
    let outcome = result.map(|(text, _ws)| text);
    let denied = matches!(&outcome, Err(PluginError::Guest(message)) if message.contains("Denied"));
    assert!(denied, "expected a denied capability error, got {outcome:?}");
}

/// Asserts the fixture call was refused by a caller-fault check (mapped to `invalid-input`).
fn assert_invalid_input(result: Result<(String, Workspace), PluginError>) {
    let outcome = result.map(|(text, _ws)| text);
    let rejected = matches!(&outcome, Err(PluginError::Guest(message)) if message.contains("InvalidInput"));
    assert!(rejected, "expected an invalid-input rejection, got {outcome:?}");
}

// ----- grant + provider resolution -----

#[tokio::test]
async fn ai_is_denied_without_a_grant() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none(),
            ResourceBudget::default(),
            command_config(&support("argv-echo"), &["{prompt}"], 30),
            NetPolicy::deny_all(),
            None,
            "media/scan.jpg",
            "read this",
        )
        .await;
    assert_denied(result);
}

#[tokio::test]
async fn an_unknown_provider_is_invalid_input() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            command_config(&support("argv-echo"), &["{prompt}"], 30),
            NetPolicy::deny_all(),
            Some("does-not-exist"),
            "media/scan.jpg",
            "read this",
        )
        .await;
    // The error names the requested provider.
    match result.map(|(text, _ws)| text) {
        Err(PluginError::Guest(message)) => {
            assert!(
                message.contains("InvalidInput"),
                "expected invalid-input, got: {message}"
            );
            assert!(
                message.contains("does-not-exist"),
                "the message names the provider: {message}"
            );
        }
        other => panic!("expected an invalid-input guest error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_missing_default_is_invalid_input() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    // A config with a provider but no default; asking for `None` cannot resolve one.
    let mut config = command_config(&support("argv-echo"), &["{prompt}"], 30);
    config.default = None;
    let result = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            config,
            NetPolicy::deny_all(),
            None,
            "media/scan.jpg",
            "read this",
        )
        .await;
    assert_invalid_input(result);
}

// ----- command kind: argv, no shell -----

#[tokio::test]
async fn command_passes_a_hostile_prompt_as_one_argv_element() {
    let hostile = r#""; rm -rf /tmp/x; echo pwned"#;
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (text, _ws) = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            command_config(&support("argv-echo"), &["{prompt}"], 30),
            NetPolicy::deny_all(),
            None,
            "media/scan.jpg",
            hostile,
        )
        .await
        .expect("command runs");
    // argv-echo prints each argument on its own line — exactly one line proves the whole hostile
    // prompt arrived as a single argument (no shell splitting, no argument injection).
    assert_eq!(
        text.lines().count(),
        1,
        "the prompt is a single argv element, got: {text:?}"
    );
    assert_eq!(text.trim_end(), hostile);
}

#[tokio::test]
async fn command_does_not_evaluate_shell_substitutions_in_the_prompt() {
    let substitution = "$(echo INJECTED)";
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (text, _ws) = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            command_config(&support("argv-echo"), &["{prompt}"], 30),
            NetPolicy::deny_all(),
            None,
            "media/scan.jpg",
            substitution,
        )
        .await
        .expect("command runs");
    // No shell means no command substitution: the literal text survives (a shell would print
    // "INJECTED" instead).
    assert_eq!(text.trim_end(), substitution);
}

#[tokio::test]
async fn command_media_placeholder_resolves_relative_to_the_workspace() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (text, _ws) = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            command_config(&support("argv-echo"), &["{media}"], 30),
            NetPolicy::deny_all(),
            None,
            "media/kirkebok/1801_scan.jpg",
            "ignored",
        )
        .await
        .expect("command runs");
    assert_eq!(text.trim_end(), "media/kirkebok/1801_scan.jpg");
}

#[tokio::test]
async fn command_timeout_kills_a_slow_provider() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            command_config("/bin/sleep", &["30"], 1),
            NetPolicy::deny_all(),
            None,
            "media/scan.jpg",
            "read this",
        )
        .await;
    match result.map(|(text, _ws)| text) {
        Err(PluginError::Guest(message)) => {
            assert!(
                message.contains("Backend"),
                "a timeout is a backend error, got: {message}"
            );
            assert!(
                message.contains("timed out"),
                "the message says it timed out: {message}"
            );
        }
        other => panic!("expected a timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn command_non_zero_exit_is_a_backend_error_with_stderr() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            command_config(&support("ai-fail"), &[], 30),
            NetPolicy::deny_all(),
            None,
            "media/scan.jpg",
            "read this",
        )
        .await;
    match result.map(|(text, _ws)| text) {
        Err(PluginError::Guest(message)) => {
            assert!(
                message.contains("Backend"),
                "a non-zero exit is a backend error, got: {message}"
            );
            assert!(
                message.contains("stderr-boom"),
                "the stderr excerpt is included: {message}"
            );
        }
        other => panic!("expected a backend error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_media_path_that_escapes_the_media_root_is_rejected() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_interpret(
            &common::component("fixture"),
            workspace,
            software_session(),
            ai_grant(),
            ResourceBudget::default(),
            command_config(&support("argv-echo"), &["{media}"], 30),
            NetPolicy::deny_all(),
            None,
            "../escape.jpg",
            "read this",
        )
        .await;
    assert_invalid_input(result);
}

// ----- vision-api kind over wiremock -----

/// Writes a small media file under the workspace `media/` root for the vision-api tests to read.
fn write_media(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("media dir");
    }
    std::fs::write(path, bytes).expect("write media");
}

#[tokio::test]
async fn vision_api_sends_the_openai_request_shape_and_reads_string_content() {
    const KEY_ENV: &str = "VITNI_TEST_AI_KEY_STRING";
    let server = MockServer::start().await;
    let base = base_url(&server);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer secret-key-123"))
        .and(body_string_contains("\"model\":\"vision-model\""))
        .and(body_string_contains("data:image/png;base64,"))
        .and(body_string_contains("read this scan"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "content": "the transcribed text" } }]
        })))
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    write_media(&root, "media/scan.png", b"\x89PNG fake bytes");
    let workspace = open_workspace(&root).await;

    let result = Box::pin(temp_env::async_with_vars([(KEY_ENV, Some("secret-key-123"))], async {
        common::host()
            .fixture_try_interpret(
                &common::component("fixture"),
                workspace,
                software_session(),
                ai_grant(),
                ResourceBudget::default(),
                vision_config(&base, KEY_ENV),
                localhost_policy(),
                None,
                "media/scan.png",
                "read this scan",
            )
            .await
    }))
    .await;

    let (text, _ws) = result.expect("vision-api call");
    assert_eq!(text, "the transcribed text");
}

#[tokio::test]
async fn vision_api_reads_content_parts() {
    const KEY_ENV: &str = "VITNI_TEST_AI_KEY_PARTS";
    let server = MockServer::start().await;
    let base = base_url(&server);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "content": [
                { "type": "text", "text": "part one " },
                { "type": "text", "text": "part two" }
            ] } }]
        })))
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    write_media(&root, "media/scan.png", b"bytes");
    let workspace = open_workspace(&root).await;

    let result = Box::pin(temp_env::async_with_vars([(KEY_ENV, Some("k"))], async {
        common::host()
            .fixture_try_interpret(
                &common::component("fixture"),
                workspace,
                software_session(),
                ai_grant(),
                ResourceBudget::default(),
                vision_config(&base, KEY_ENV),
                localhost_policy(),
                None,
                "media/scan.png",
                "read this",
            )
            .await
    }))
    .await;

    let (text, _ws) = result.expect("vision-api call");
    assert_eq!(text, "part one part two");
}

#[tokio::test]
async fn vision_api_error_status_is_a_backend_error_without_the_key() {
    const KEY_ENV: &str = "VITNI_TEST_AI_KEY_ERROR";
    let server = MockServer::start().await;
    let base = base_url(&server);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"invalid api key"}"#))
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    write_media(&root, "media/scan.png", b"bytes");
    let workspace = open_workspace(&root).await;

    let result = Box::pin(temp_env::async_with_vars(
        [(KEY_ENV, Some("super-secret-value"))],
        async {
            common::host()
                .fixture_try_interpret(
                    &common::component("fixture"),
                    workspace,
                    software_session(),
                    ai_grant(),
                    ResourceBudget::default(),
                    vision_config(&base, KEY_ENV),
                    localhost_policy(),
                    None,
                    "media/scan.png",
                    "read this",
                )
                .await
        },
    ))
    .await;

    match result.map(|(text, _ws)| text) {
        Err(PluginError::Guest(message)) => {
            assert!(
                message.contains("Backend"),
                "an API error is a backend error, got: {message}"
            );
            assert!(message.contains("401"), "the status is reported: {message}");
            assert!(
                !message.contains("super-secret-value"),
                "the API key must never appear in an error: {message}"
            );
        }
        other => panic!("expected a backend error, got {other:?}"),
    }
}

#[tokio::test]
async fn vision_api_missing_env_var_is_invalid_input_naming_the_var() {
    const KEY_ENV: &str = "VITNI_TEST_AI_KEY_ABSENT";
    let server = MockServer::start().await;
    let base = base_url(&server);
    // The request must never be made — the missing key fails before any call.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .expect(0)
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    write_media(&root, "media/scan.png", b"bytes");
    let workspace = open_workspace(&root).await;

    // Ensure the env var is unset for the duration of the call.
    let result = Box::pin(temp_env::async_with_vars([(KEY_ENV, None::<&str>)], async {
        common::host()
            .fixture_try_interpret(
                &common::component("fixture"),
                workspace,
                software_session(),
                ai_grant(),
                ResourceBudget::default(),
                vision_config(&base, KEY_ENV),
                localhost_policy(),
                None,
                "media/scan.png",
                "read this",
            )
            .await
    }))
    .await;

    match result.map(|(text, _ws)| text) {
        Err(PluginError::Guest(message)) => {
            assert!(
                message.contains("InvalidInput"),
                "a missing key is invalid-input, got: {message}"
            );
            assert!(message.contains(KEY_ENV), "the message names the env var: {message}");
        }
        other => panic!("expected an invalid-input error, got {other:?}"),
    }
}

// ----- provenance confidence template (ADR 0017 §7) -----

/// Reads every event payload from the workspace's SQLite event store.
async fn event_payloads(root: &Path) -> Vec<String> {
    let db = root.join("vitni.sqlite3");
    let url = format!("sqlite://{}", db.display());
    let pool = sqlx::SqlitePool::connect(&url).await.expect("open events db");
    let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM events")
        .fetch_all(&pool)
        .await
        .expect("read events");
    pool.close().await;
    payloads
}

#[tokio::test]
async fn provenance_confidence_low_lands_on_created_assertions() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    common::host()
        .fixture_try_create(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Commands).with(Capability::Log),
            ResourceBudget::default(),
            Some(Confidence::Low),
        )
        .await
        .expect("create");

    let payloads = event_payloads(&root).await;
    assert!(
        payloads.iter().any(|payload| payload.contains(r#""confidence":"Low""#)),
        "an invocation-level Confidence::Low stamps the created assertion's EventContext"
    );
}

#[tokio::test]
async fn no_provenance_confidence_records_no_surety() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    common::host()
        .fixture_try_create(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Commands).with(Capability::Log),
            ResourceBudget::default(),
            None,
        )
        .await
        .expect("create");

    let payloads = event_payloads(&root).await;
    assert!(
        payloads
            .iter()
            .all(|payload| !payload.contains(r#""confidence":"Low""#)),
        "with no template, no assertion is stamped Low"
    );
    assert!(
        payloads.iter().any(|payload| payload.contains(r#""confidence":null"#)),
        "the default records no surety judgment (confidence: null)"
    );
}
