//! Integration tests for the `net` and `media-store` host capabilities (ADR 0017 §2, §3),
//! exercised through the fixture component's `try-fetch`/`try-store`/`try-fetch-store` exports and a
//! local `wiremock` HTTP server. Pure policy/path logic is unit-tested in the crate modules; these
//! tests cover grant enforcement and the real transport (redirect chains, size caps, timeouts,
//! content-type sniffing, checksums, dedup).
//!
//! Requires the fixture component: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;
use vitni_app::{AppDefaults, OperatorConfig, Session, Workspace, WorkspaceDefaults};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};
use vitni_plugin_host::{Capability, Grants, HostPattern, NetPolicy, PluginError, ResourceBudget};
use wiremock::matchers::{method, path};
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

/// A policy that reaches `localhost` over plain HTTP (so the tests can hit a local mock server),
/// with the given field overrides applied on top of the deny-all defaults.
fn localhost_policy() -> NetPolicy {
    NetPolicy {
        allowed_hosts: vec![HostPattern::parse("localhost")],
        require_https: false,
        ..NetPolicy::deny_all()
    }
}

/// The `localhost` base URL for a running mock server, using the hostname (not the `127.0.0.1`
/// literal, which the policy refuses).
fn base_url(server: &MockServer) -> String {
    format!("http://localhost:{}", server.address().port())
}

// ----- net: grant + policy enforcement (no network reached) -----

#[tokio::test]
async fn net_is_denied_without_a_grant() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none(),
            ResourceBudget::default(),
            localhost_policy(),
            "https://www.digitalarkivet.no/",
        )
        .await;
    assert_denied(result);
}

#[tokio::test]
async fn net_rejects_a_host_not_on_the_allowlist() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    // Allowlist is `localhost` only; a different host is refused before any connection is made.
    let result = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Net),
            ResourceBudget::default(),
            localhost_policy(),
            "http://example.com/",
        )
        .await;
    assert_policy_rejection(result);
}

#[tokio::test]
async fn net_rejects_plain_http_when_https_is_required() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let policy = NetPolicy::allow(vec![HostPattern::parse("www.digitalarkivet.no")]);
    let result = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Net),
            ResourceBudget::default(),
            policy,
            "http://www.digitalarkivet.no/",
        )
        .await;
    assert_policy_rejection(result);
}

#[tokio::test]
async fn net_rejects_an_ip_literal_host() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let policy = NetPolicy {
        allowed_hosts: vec![HostPattern::parse("127.0.0.1")],
        require_https: false,
        ..NetPolicy::deny_all()
    };
    let result = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Net),
            ResourceBudget::default(),
            policy,
            "http://127.0.0.1:8080/",
        )
        .await;
    assert_policy_rejection(result);
}

#[tokio::test]
async fn net_rejects_a_userinfo_url() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Net),
            ResourceBudget::default(),
            NetPolicy::allow(vec![HostPattern::parse("www.digitalarkivet.no")]),
            "https://user:pass@www.digitalarkivet.no/",
        )
        .await;
    assert_policy_rejection(result);
}

// ----- net: real transport over wiremock -----

#[tokio::test]
async fn net_follows_redirects_and_reports_the_final_url() {
    let server = MockServer::start().await;
    let base = base_url(&server);
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", format!("{base}/final")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/final"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (summary, _ws) = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Net),
            ResourceBudget::default(),
            localhost_policy(),
            &format!("{base}/start"),
        )
        .await
        .expect("fetch follows the redirect");

    // Summary is "status final-url body-len".
    assert_eq!(summary, format!("200 {base}/final 5"));
}

#[tokio::test]
async fn net_aborts_a_body_over_the_size_cap() {
    let server = MockServer::start().await;
    let base = base_url(&server);
    Mock::given(method("GET"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 100]))
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let policy = NetPolicy {
        max_response_bytes: 8,
        ..localhost_policy()
    };
    let result = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Net),
            ResourceBudget::default(),
            policy,
            &format!("{base}/big"),
        )
        .await;
    assert_policy_rejection(result);
}

#[tokio::test]
async fn net_times_out_a_slow_response() {
    let server = MockServer::start().await;
    let base = base_url(&server);
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(600))
                .set_body_bytes(b"late".to_vec()),
        )
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let policy = NetPolicy {
        timeout: Duration::from_millis(100),
        ..localhost_policy()
    };
    let result = common::host()
        .fixture_try_fetch(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::Net),
            ResourceBudget::default(),
            policy,
            &format!("{base}/slow"),
        )
        .await;
    match result.map(|(summary, _ws)| summary) {
        Err(PluginError::Guest(message)) => {
            assert!(
                message.contains("Backend"),
                "a timeout should surface as a backend error, got: {message}"
            );
        }
        Ok(summary) => panic!("expected a timeout, but the fetch succeeded: {summary}"),
        Err(other) => panic!("expected a backend (timeout) guest error, got {other:?}"),
    }
}

// ----- media-store -----

/// Parses the fixture's `"relative-path checksum mime size existed"` summary.
fn parse_stored(summary: &str) -> (String, String, String, u64, bool) {
    let parts: Vec<&str> = summary.split(' ').collect();
    assert_eq!(parts.len(), 5, "stored-media summary has five fields, got: {summary}");
    let size = parts[3].parse::<u64>().expect("size is a number");
    let existed = parts[4].parse::<bool>().expect("existed is a bool");
    (
        parts[0].to_owned(),
        parts[1].to_owned(),
        parts[2].to_owned(),
        size,
        existed,
    )
}

#[tokio::test]
async fn media_store_is_denied_without_a_grant() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = common::host()
        .fixture_try_store(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none(),
            ResourceBudget::default(),
            b"content",
            "scan.jpg",
        )
        .await;
    assert_denied(result);
}

#[tokio::test]
async fn media_store_checksum_matches_a_known_vector() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_rel, checksum, _mime, size, existed) = {
        let (summary, _ws) = common::host()
            .fixture_try_store(
                &common::component("fixture"),
                workspace,
                software_session(),
                Grants::none().with(Capability::MediaStore),
                ResourceBudget::default(),
                b"abc",
                "docs/test.bin",
            )
            .await
            .expect("store");
        parse_stored(&summary)
    };
    assert_eq!(
        checksum,
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(size, 3);
    assert!(!existed);
}

#[tokio::test]
async fn media_store_rejects_path_traversal() {
    for unsafe_path in ["../escape.jpg", "/etc/passwd", "kirkebok\\evil.jpg"] {
        let (root, _dir) = init_workspace();
        let workspace = open_workspace(&root).await;
        let result = common::host()
            .fixture_try_store(
                &common::component("fixture"),
                workspace,
                software_session(),
                Grants::none().with(Capability::MediaStore),
                ResourceBudget::default(),
                b"content",
                unsafe_path,
            )
            .await;
        assert_policy_rejection(result);
    }
}

#[tokio::test]
async fn media_store_dedups_an_identical_re_store() {
    let (root, _dir) = init_workspace();

    let workspace = open_workspace(&root).await;
    let (first, _ws) = common::host()
        .fixture_try_store(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::MediaStore),
            ResourceBudget::default(),
            b"abc",
            "kirkebok/scan.jpg",
        )
        .await
        .expect("first store");
    let (first_rel, _c, _m, _s, first_existed) = parse_stored(&first);
    assert!(!first_existed);

    let workspace = open_workspace(&root).await;
    let (again, _ws) = common::host()
        .fixture_try_store(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::MediaStore),
            ResourceBudget::default(),
            b"abc",
            "kirkebok/scan.jpg",
        )
        .await
        .expect("second store");
    let (again_rel, _c, _m, _s, again_existed) = parse_stored(&again);
    assert!(again_existed, "an identical re-store is a dedup hit");
    assert_eq!(again_rel, first_rel, "the dedup hit returns the same path");
}

#[tokio::test]
async fn media_store_uniquifies_a_different_file_at_a_taken_path() {
    let (root, _dir) = init_workspace();

    let workspace = open_workspace(&root).await;
    common::host()
        .fixture_try_store(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::MediaStore),
            ResourceBudget::default(),
            b"content-a",
            "kirkebok/scan.jpg",
        )
        .await
        .expect("first store");

    let workspace = open_workspace(&root).await;
    let (collision, _ws) = common::host()
        .fixture_try_store(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::MediaStore),
            ResourceBudget::default(),
            b"content-b",
            "kirkebok/scan.jpg",
        )
        .await
        .expect("colliding store");
    let (rel, _c, _m, _s, existed) = parse_stored(&collision);
    assert!(!existed);
    assert_eq!(rel, "media/kirkebok/scan-2.jpg");
}

#[tokio::test]
async fn media_store_writes_bytes_under_the_media_root() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (summary, _ws) = common::host()
        .fixture_try_store(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::MediaStore),
            ResourceBudget::default(),
            b"real-bytes",
            "sub/photo.png",
        )
        .await
        .expect("store");
    let (rel, _c, mime, _s, _e) = parse_stored(&summary);
    assert_eq!(rel, "media/sub/photo.png");
    assert_eq!(mime, "image/png", "mime falls back to the path extension");

    let on_disk = root.join("media/sub/photo.png");
    assert_eq!(std::fs::read(&on_disk).expect("read stored file"), b"real-bytes");
}

#[tokio::test]
async fn media_fetch_and_store_uses_the_response_content_type() {
    let server = MockServer::start().await;
    let base = base_url(&server);
    Mock::given(method("GET"))
        .and(path("/scan"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/jpeg")
                .set_body_bytes(b"\xff\xd8\xff\xe0jpeg-bytes".to_vec()),
        )
        .mount(&server)
        .await;

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    // The suggested path ends in `.bin`, so a header MIME proves precedence over the extension.
    let (summary, _ws) = common::host()
        .fixture_try_fetch_store(
            &common::component("fixture"),
            workspace,
            software_session(),
            Grants::none().with(Capability::MediaStore),
            ResourceBudget::default(),
            localhost_policy(),
            &format!("{base}/scan"),
            "kirkebok/1801_scan.bin",
        )
        .await
        .expect("fetch-and-store");
    let (rel, _c, mime, size, existed) = parse_stored(&summary);
    assert_eq!(rel, "media/kirkebok/1801_scan.bin");
    assert_eq!(
        mime, "image/jpeg",
        "the response content-type wins over the .bin extension"
    );
    assert_eq!(size, 14);
    assert!(!existed);
    assert!(root.join("media/kirkebok/1801_scan.bin").is_file());
}

// ----- shared assertions -----

/// Asserts the fixture call was refused for lack of a capability grant. (`Workspace` is not `Debug`,
/// so the success value is reduced to its summary string before matching.)
fn assert_denied(result: Result<(String, Workspace), PluginError>) {
    let outcome = result.map(|(summary, _ws)| summary);
    let denied = matches!(&outcome, Err(PluginError::Guest(message)) if message.contains("Denied"));
    assert!(denied, "expected a denied capability error, got {outcome:?}");
}

/// Asserts the fixture call was refused by a policy/path check (mapped to `invalid-input`).
fn assert_policy_rejection(result: Result<(String, Workspace), PluginError>) {
    let outcome = result.map(|(summary, _ws)| summary);
    let rejected = matches!(&outcome, Err(PluginError::Guest(message)) if message.contains("InvalidInput"));
    assert!(rejected, "expected a policy rejection (invalid-input), got {outcome:?}");
}
