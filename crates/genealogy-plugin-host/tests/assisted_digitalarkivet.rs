//! End-to-end integration test for the Digitalarkivet assisted-import plugin (ADR 0017): the
//! `digitalarkivet-import` component drives a full `run-assisted` session against a local `wiremock`
//! server serving the PR3 fixtures (a census person page, a residence page, a scan-viewer page, and a
//! scan JPEG) and a scripted [`Presenter`] that answers each `present` payload. Asserts the created
//! aggregates, the crop, re-run idempotence, cancellation, and denied-capability behaviour.
//!
//! The fixtures' absolute Digitalarkivet URLs are rewritten to the mock host so every fetch hits
//! wiremock; the request carries an explicit `page` hint so the flow routes without the
//! host-restricted `classify_url` (unit-tested in the crate) rejecting the mock host.
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use genealogy_app::{
    AiConfig, AppDefaults, Confidence, OperatorConfig, Rect, Session, Workspace, WorkspaceDefaults, list_citations,
    list_media, list_persons, list_repositories, list_sources,
};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use genealogy_plugin_host::{
    Capability, Grants, HostPattern, Invocation, NetPolicy, PluginError, PresentError, Presenter, ProgressControl,
    ResourceBudget,
};
use serde_json::{Value, json};
use uuid::Uuid;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;

const PLUGIN: &str = "digitalarkivet-import";

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
            name: "genealogy-digitalarkivet-import".to_owned(),
            version: "0.1.0".to_owned(),
        },
        id: AgentId::from_uuid(Uuid::from_u128(7)),
        display: Some("Digitalarkivet".to_owned()),
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

/// The assisted-import grant set (ADR 0017 §9), minus any capability in `without`.
fn grants(without: &[Capability]) -> Grants {
    let all = [
        Capability::Log,
        Capability::Query,
        Capability::Commands,
        Capability::Progress,
        Capability::Net,
        Capability::MediaStore,
        Capability::Ai,
        Capability::Present,
    ];
    let mut grants = Grants::none();
    for capability in all {
        if !without.contains(&capability) {
            grants = grants.with(capability);
        }
    }
    grants
}

/// A policy that reaches the local mock server over plain HTTP.
fn localhost_policy() -> NetPolicy {
    NetPolicy {
        allowed_hosts: vec![HostPattern::parse("localhost")],
        require_https: false,
        ..NetPolicy::deny_all()
    }
}

fn invocation(workspace: Workspace, grants: Grants) -> Invocation {
    Invocation {
        workspace,
        session: software_session(),
        grants,
        budget: ResourceBudget::assisted(),
        net_policy: localhost_policy(),
        ai_config: AiConfig::default(),
        provenance_confidence: Some(Confidence::Low),
    }
}

/// Reads a PR3 fixture and rewrites its absolute Digitalarkivet URLs to the mock base, so every
/// in-page link (record URL, scan viewer, permanent image) resolves to wiremock.
fn fixture(kind: &str, name: &str, base: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../genealogy-digitalarkivet/tests/fixtures")
        .join(kind)
        .join(name);
    let mut html = std::fs::read_to_string(&path).expect("reading a fixture");
    for host in [
        "https://www.digitalarkivet.no",
        "https://media.digitalarkivet.no",
        "https://urn.digitalarkivet.no",
        "https://nye.digitalarkivet.no",
        "https://digitalarkivet.no",
    ] {
        html = html.replace(host, base);
    }
    html
}

/// Starts a mock server serving the census fixtures: the person page, the residence page, the
/// scan-viewer page, and a small JPEG for the permanent image.
async fn census_server() -> MockServer {
    let server = MockServer::start().await;
    let base = format!("http://localhost:{}", server.address().port());
    mount(&server, r"^/census/person/.*", fixture("census", "person.html", &base)).await;
    mount(
        &server,
        r"^/census/(rural|urban)-residence/.*",
        fixture("census", "bosted.html", &base),
    )
    .await;
    mount(&server, r"^/fs\d+.*", fixture("census", "viewer.html", &base)).await;
    Mock::given(method("GET"))
        .and(path_regex(r".*\.jpg$"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/jpeg")
                .set_body_bytes(SCAN_JPEG),
        )
        .mount(&server)
        .await;
    server
}

/// A minimal valid JPEG body (SOI + EOI markers) — enough for `media-store` to store and checksum.
const SCAN_JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xD9];

async fn mount(server: &MockServer, regex: &str, body: String) {
    Mock::given(method("GET"))
        .and(path_regex(regex))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(body),
        )
        .mount(server)
        .await;
}

/// A reply closure: the wizard's answer to a presented payload.
type Reply = Box<dyn FnMut(&str) -> Result<String, PresentError> + Send>;

/// A presenter scripted by a reply closure over the payload's `kind`, recording every payload it saw.
struct ScriptedPresenter {
    seen: Arc<Mutex<Vec<String>>>,
    reply: Reply,
}

impl ScriptedPresenter {
    fn new(
        reply: impl FnMut(&str) -> Result<String, PresentError> + Send + 'static,
    ) -> (Self, Arc<Mutex<Vec<String>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                seen: Arc::clone(&seen),
                reply: Box::new(reply),
            },
            seen,
        )
    }
}

#[async_trait]
impl Presenter for ScriptedPresenter {
    async fn present(&mut self, payload: String) -> Result<String, PresentError> {
        self.seen.lock().expect("seen lock").push(payload.clone());
        (self.reply)(&payload)
    }
}

/// The `kind` discriminator of a payload.
fn kind_of(payload: &str) -> String {
    serde_json::from_str::<Value>(payload)
        .ok()
        .and_then(|value| value.get("kind").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

/// The confirm-stage import response: an edited name, a crop region, and a `low` confidence.
fn import_response() -> String {
    json!({
        "kind": "submit",
        "action": "import",
        "values": {
            "fields": [{ "key": "name", "value": "Edited Name" }],
            "region": { "left": 4, "top": 47, "width": 92, "height": 9 },
            "confidence": "low"
        }
    })
    .to_string()
}

/// The save-scan response: echoes the payload's suggested filing target verbatim.
fn save_response(payload: &str) -> String {
    let value: Value = serde_json::from_str(payload).unwrap_or_default();
    let suggested = value.get("suggested").cloned().unwrap_or(json!({}));
    json!({ "kind": "submit", "action": "save", "values": { "save": suggested } }).to_string()
}

/// Selects the first row of a records payload.
fn select_first(payload: &str) -> String {
    let value: Value = serde_json::from_str(payload).unwrap_or_default();
    let row = value
        .get("records")
        .and_then(Value::as_array)
        .and_then(|records| records.first())
        .and_then(|record| record.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    json!({ "kind": "submit", "action": "select", "values": { "row": row } }).to_string()
}

fn done() -> String {
    json!({ "kind": "submit", "action": "done" }).to_string()
}

fn cancel() -> String {
    json!({ "kind": "cancel" }).to_string()
}

/// The single-census-person reply: import the confirm, save the scan, finish the summary.
fn single_person_reply(payload: &str) -> String {
    match kind_of(payload).as_str() {
        "confirm-record" => import_response(),
        "save-scan" => save_response(payload),
        _ => done(),
    }
}

fn request(url: &str, page: &str) -> String {
    json!({ "kind": "url", "url": url, "page": page }).to_string()
}

fn person_url(base: &str) -> String {
    format!("{base}/census/person/pf01073902000464")
}

async fn run(
    component_workspace: (Workspace, Grants),
    server: &MockServer,
    page: &str,
    presenter: ScriptedPresenter,
) -> Result<String, PluginError> {
    let base = format!("http://localhost:{}", server.address().port());
    let url = if page == "census-residence" {
        format!("{base}/census/rural-residence/bf01052209001842")
    } else {
        person_url(&base)
    };
    let (workspace, grants) = component_workspace;
    common::host()
        .run_assisted_import(
            &common::component(PLUGIN),
            invocation(workspace, grants),
            &request(&url, page),
            Box::new(presenter),
            |_update| ProgressControl::Proceed,
        )
        .await
        .map(|(summary, _workspace)| summary)
}

// ----- the happy path: a census person imported end to end -----

#[tokio::test]
async fn imports_a_census_person_with_source_citation_and_cropped_media() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let (presenter, seen) = ScriptedPresenter::new(|payload| Ok(single_person_reply(payload)));

    let summary = run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-person",
        presenter,
    )
    .await
    .expect("assisted import runs");

    // The plugin presented confirm → save-scan → summary.
    let kinds: Vec<String> = seen.lock().expect("seen").iter().map(|p| kind_of(p)).collect();
    assert_eq!(
        kinds,
        ["confirm-record", "save-scan", "summary"],
        "the wizard saw each stage"
    );
    assert!(
        summary.contains("\"skipped\":0"),
        "summary reports nothing skipped: {summary}"
    );

    assert_census_import(&root).await;
}

/// Asserts the aggregates a single census-person import produces: the person (with the edited name,
/// the attached citation, and the cropped media), the source/repository/citation, the media object,
/// the scan on disk, and the Software-agent `digitalarkivet` `ExternalId` in the event store.
async fn assert_census_import(root: &Path) {
    let workspace = open_workspace(root).await;
    let persons = list_persons(&workspace).await.expect("persons");
    assert_eq!(persons.len(), 1, "one person created");
    let person = &persons[0];
    assert_eq!(
        person.given.as_deref(),
        Some("Edited"),
        "the edited given name was imported"
    );
    assert_eq!(
        person.surname.as_deref(),
        Some("Name"),
        "the edited surname was imported"
    );
    assert_eq!(person.citations.len(), 1, "the citation is attached to the person");
    assert_eq!(person.media.len(), 1, "the scan media is attached to the person");
    assert_eq!(
        person.media[0].crop,
        Some(Rect {
            left: 4,
            top: 47,
            width: 92,
            height: 9
        }),
        "the user's line region became the media ref crop"
    );

    assert_eq!(
        list_sources(&workspace).await.expect("sources").len(),
        1,
        "one source created"
    );
    assert_eq!(
        list_repositories(&workspace).await.expect("repos").len(),
        1,
        "one repository created"
    );
    let citations = list_citations(&workspace).await.expect("citations");
    assert_eq!(citations.len(), 1, "one citation created");
    assert_eq!(
        citations[0].confidence,
        Some(Confidence::Low),
        "the citation confidence is Low"
    );
    assert!(
        citations[0]
            .page
            .as_deref()
            .is_some_and(|page| page.contains("URN:NBN:")),
        "the citation locator carries the scan URN: {:?}",
        citations[0].page
    );

    let media = list_media(&workspace).await.expect("media");
    assert_eq!(media.len(), 1, "one media object created");
    let media_path = media[0]
        .path
        .clone()
        .or_else(|| media[0].file_path.clone())
        .expect("a media path");
    assert_eq!(
        media[0].mime.as_deref(),
        Some("image/jpeg"),
        "the scan MIME was sniffed"
    );
    // The stored path is media-root-relative (`02_…/x.jpg`), NOT the `media/`-prefixed workspace path
    // `media-store` returns; persisting the prefix doubles the segment and the GUI asset handler 404s.
    assert!(
        !media_path.starts_with("media/"),
        "stored media path must be media-root-relative, not doubled: {media_path}"
    );
    let on_disk = root.join("media").join(&media_path);
    assert!(
        on_disk.is_file(),
        "the scan was written under the media root: {}",
        on_disk.display()
    );
    assert_eq!(
        std::fs::read(&on_disk).expect("read scan"),
        SCAN_JPEG,
        "the stored bytes match the download"
    );

    assert!(
        events_contain(root, "digitalarkivet").await,
        "an ExternalId under `digitalarkivet` was recorded"
    );
    assert!(
        events_contain(root, "Software").await,
        "the operator is a Software agent"
    );
}

// ----- re-run idempotence -----

#[tokio::test]
async fn re_running_the_same_url_imports_no_duplicates() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;

    for _ in 0..2 {
        let (presenter, _seen) = ScriptedPresenter::new(|payload| Ok(single_person_reply(payload)));
        run(
            (open_workspace(&root).await, grants(&[])),
            &server,
            "census-person",
            presenter,
        )
        .await
        .expect("assisted import runs");
    }

    let workspace = open_workspace(&root).await;
    assert_eq!(
        list_persons(&workspace).await.expect("persons").len(),
        1,
        "the person resolved (created=false)"
    );
    assert_eq!(
        list_sources(&workspace).await.expect("sources").len(),
        1,
        "no duplicate source"
    );
    assert_eq!(
        list_citations(&workspace).await.expect("citations").len(),
        1,
        "no duplicate citation"
    );
    assert_eq!(
        list_media(&workspace).await.expect("media").len(),
        1,
        "no duplicate media (existed=true)"
    );
    assert_eq!(
        list_repositories(&workspace).await.expect("repos").len(),
        1,
        "no duplicate repository"
    );
}

// ----- residence flow: records-pick -----

#[tokio::test]
async fn residence_presents_a_records_list_and_imports_a_pick() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    // Pick the first record once, then finish on the next records list (which ends the session).
    let picked = Arc::new(Mutex::new(false));
    let (presenter, seen) = ScriptedPresenter::new(move |payload: &str| {
        Ok(match kind_of(payload).as_str() {
            "records" => {
                let mut picked = picked.lock().expect("lock");
                if *picked {
                    done()
                } else {
                    *picked = true;
                    select_first(payload)
                }
            }
            "confirm-record" => import_response(),
            "save-scan" => save_response(payload),
            _ => done(),
        })
    });

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-residence",
        presenter,
    )
    .await
    .expect("assisted import runs");

    assert!(
        seen.lock().expect("seen").iter().any(|p| kind_of(p) == "records"),
        "a records list was presented"
    );
    let workspace = open_workspace(&root).await;
    assert_eq!(
        list_persons(&workspace).await.expect("persons").len(),
        1,
        "the picked record was imported"
    );
}

// ----- cancellation -----

#[tokio::test]
async fn cancelling_the_confirm_writes_nothing() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let (presenter, _seen) = ScriptedPresenter::new(|payload: &str| {
        Ok(match kind_of(payload).as_str() {
            "confirm-record" => cancel(),
            _ => done(),
        })
    });

    let summary = run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-person",
        presenter,
    )
    .await
    .expect("assisted import runs");

    assert!(
        summary.contains("\"imported\":[]"),
        "nothing imported after cancel: {summary}"
    );
    let workspace = open_workspace(&root).await;
    assert!(
        list_persons(&workspace).await.expect("persons").is_empty(),
        "no person created after cancel"
    );
    assert!(
        list_sources(&workspace).await.expect("sources").is_empty(),
        "no source created after cancel"
    );
}

// ----- denied capability -----

#[tokio::test]
async fn a_missing_net_grant_fails_the_session() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let (presenter, seen) = ScriptedPresenter::new(|payload| Ok(single_person_reply(payload)));

    let result = run(
        (open_workspace(&root).await, grants(&[Capability::Net])),
        &server,
        "census-person",
        presenter,
    )
    .await;

    assert!(
        matches!(&result, Err(PluginError::Guest(message)) if message.contains("Denied") || message.contains("denied")),
        "a missing Net grant is a guest failure, got {result:?}"
    );
    assert!(
        seen.lock().expect("seen").is_empty(),
        "nothing is presented when the first fetch is denied"
    );
}

/// Whether any event payload in the workspace's event store contains `needle`.
async fn events_contain(root: &Path, needle: &str) -> bool {
    let url = format!("sqlite://{}", root.join("genealogy.sqlite3").display());
    let pool = sqlx::SqlitePool::connect(&url).await.expect("open events db");
    let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM events")
        .fetch_all(&pool)
        .await
        .expect("read events");
    pool.close().await;
    payloads.iter().any(|payload| payload.contains(needle))
}
