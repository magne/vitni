//! End-to-end integration test for the Digitalarkivet assisted-import plugin (ADR 0017): the
//! `digitalarkivet-import` component drives a full `run-assisted` session against a local `wiremock`
//! server and a scripted [`Presenter`] that answers each `present` payload. The server serves the
//! `vitni-digitalarkivet` page fixtures (a census person page, a residence page and a scan-viewer
//! page, all invented per ADR 0042) plus a scan JPEG. Asserts the created aggregates — the person,
//! its census and birth events, their places and the household's family, each under its origin —
//! the crop, re-run idempotence, a church-book record's event, cancellation, and denied-capability
//! behaviour.
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
use serde_json::{Value, json};
use uuid::Uuid;
use vitni_app::{
    AiConfig, AppDefaults, ChosenDataset, Confidence, DatasetId, EventSummary, ExternalId, IdentityDecision,
    ImportRunStatus, MatchQuestion, MatchReply, NewPerson, OperatorConfig, PairAnswer, PairDecision, PersonNameParts,
    Provenance, Rect, Session, Workspace, WorkspaceDefaults, change_log_for_person, create_person, list_citations,
    list_events, list_families, list_import_runs, list_media, list_persons, list_places, list_repositories,
    list_sources,
};
use vitni_core::date::{DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::enums::EvidenceLevel;
use vitni_core::enums::{EventType, ParticipantRole, PlaceType};
use vitni_core::ids::AgentId;
use vitni_core::matching::MatchableKind;
use vitni_core::provenance::{Agent, AgentKind};
use vitni_plugin_host::{
    Capability, Grants, HostPattern, ImportRunSpec, Invocation, NetPolicy, PluginError, PresentError, Presenter,
    ProgressControl, ResourceBudget, RunDataset,
};
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
            name: "vitni-digitalarkivet-import".to_owned(),
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
        import: Some(ImportRunSpec {
            operator: Session::new(Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(1)),
                display: Some("Tester".to_owned()),
            }),
            dataset: RunDataset::Chosen(ChosenDataset {
                id: DatasetId::global("digitalarkivet"),
                label: "digitalarkivet".to_owned(),
            }),
            source_label: "census person".to_owned(),
            plugin: PLUGIN.to_owned(),
            plugin_version: "0.1.0".to_owned(),
            reviewer: Box::new(vitni_plugin_host::DeferMatches),
        }),
    }
}

/// Reads a `vitni-digitalarkivet` page fixture and rewrites its absolute Digitalarkivet URLs to the mock base, so every
/// in-page link (record URL, scan viewer, permanent image) resolves to wiremock.
fn fixture(kind: &str, name: &str, base: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../vitni-digitalarkivet/tests/fixtures")
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

/// SHA-256 of [`SCAN_JPEG`], in the `media-store` checksum form.
const SCAN_JPEG_CHECKSUM: &str = "sha256:32461d5bd1773012acef0ba15636752949bd7c2ce50f9172159d9f56cf0dd9af";

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

/// The wizard's answer to the host's match stage.
type MatchAnswer = Box<dyn FnMut(&MatchQuestion) -> MatchReply + Send>;

/// A presenter scripted by a reply closure over the payload's `kind`, recording every payload it saw,
/// and answering every match question *Decide later* unless told otherwise.
struct ScriptedPresenter {
    seen: Arc<Mutex<Vec<String>>>,
    reply: Reply,
    questions: Arc<Mutex<Vec<MatchQuestion>>>,
    answer: MatchAnswer,
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
                questions: Arc::new(Mutex::new(Vec::new())),
                answer: Box::new(|_| MatchReply::Pair(Box::new(PairAnswer::Later))),
            },
            seen,
        )
    }

    /// This presenter, answering each match question with `answer`; returns the questions it is asked.
    fn answering(
        mut self,
        answer: impl FnMut(&MatchQuestion) -> MatchReply + Send + 'static,
    ) -> (Self, Arc<Mutex<Vec<MatchQuestion>>>) {
        self.answer = Box::new(answer);
        let questions = Arc::clone(&self.questions);
        (self, questions)
    }
}

#[async_trait]
impl Presenter for ScriptedPresenter {
    async fn present(&mut self, payload: String) -> Result<String, PresentError> {
        self.seen.lock().expect("seen lock").push(payload.clone());
        (self.reply)(&payload)
    }

    async fn review_match(&mut self, question: MatchQuestion) -> Result<MatchReply, PresentError> {
        let reply = (self.answer)(&question);
        self.questions.lock().expect("questions lock").push(question);
        Ok(reply)
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
    format!("{base}/census/person/pf01099901000101")
}

async fn run(
    component_workspace: (Workspace, Grants),
    server: &MockServer,
    page: &str,
    presenter: ScriptedPresenter,
) -> Result<String, PluginError> {
    let base = format!("http://localhost:{}", server.address().port());
    let url = match page {
        "census-residence" => format!("{base}/census/rural-residence/bf01099901000100"),
        "churchbook-record" => format!("{base}/view/999/pd00000099901001"),
        _ => person_url(&base),
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
    let (presenter, questions) = presenter.answering(|_| MatchReply::Cancel);

    let summary = run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-person",
        presenter,
    )
    .await
    .expect("assisted import runs");

    assert!(
        questions.lock().expect("questions").is_empty(),
        "a record with no possible match asks nothing"
    );
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

/// Asserts the session wrote one finished run into the global dataset, and the citation names the
/// census record it was read from (ADR 0037).
async fn assert_census_run(root: &Path, workspace: &Workspace) {
    let runs = list_import_runs(workspace).await.expect("runs");
    assert_eq!(runs.len(), 1, "the session wrote one import run");
    assert_eq!(runs[0].status, ImportRunStatus::Finished);
    assert_eq!(runs[0].dataset, DatasetId::global("digitalarkivet"));
    assert!(
        events_contain(root, r#""item":"citation","record":"pf01099901000101""#).await,
        "the citation names the census record it was read from"
    );
}

/// Asserts the aggregates a single census-person import produces: the person (with the edited name,
/// the attached citation, and the cropped media), the source/repository/citation, the media object,
/// the scan on disk, and the Software-agent `digitalarkivet` `ExternalId` in the event store.
async fn assert_census_import(root: &Path) {
    let workspace = open_workspace(root).await;
    assert_census_run(root, &workspace).await;
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
    assert_every_assertion_is_low(&workspace, &person.human_id).await;

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
    assert_eq!(
        media[0].checksum.as_deref(),
        Some(SCAN_JPEG_CHECKSUM),
        "the stored scan's digest reached the media object (#359)"
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

/// The invocation's `Low` template reaches every assertion on the imported person, the creation and
/// its key included (#390).
async fn assert_every_assertion_is_low(workspace: &Workspace, human_id: &str) {
    let log = change_log_for_person(workspace, human_id)
        .await
        .expect("person change log");
    assert!(
        log.iter().any(|entry| entry.event_type == "PersonCreated"),
        "the change log covers the creation: {log:?}"
    );
    for entry in &log {
        assert_eq!(
            entry.confidence,
            Some(Confidence::Low),
            "{} carries the assisted template",
            entry.event_type
        );
    }
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
    assert_eq!(
        list_events(&workspace).await.expect("events").len(),
        2,
        "no duplicate census or birth event"
    );
    assert_eq!(
        list_places(&workspace).await.expect("places").len(),
        2,
        "no duplicate residence or municipality"
    );
    assert_eq!(
        list_families(&workspace).await.expect("families").len(),
        1,
        "no duplicate household"
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

/// Serves `person.html` for the household's second member too, under its own record id, so a
/// household import can pick two distinct records.
async fn mount_second_member(server: &MockServer) {
    let base = format!("http://localhost:{}", server.address().port());
    let html = fixture("census", "person.html", &base).replace(
        &format!(r#"content="{base}/census/person/pf01099901000101""#),
        &format!(r#"content="{base}/census/person/pf01099901000102""#),
    );
    Mock::given(method("GET"))
        .and(path_regex(r"^/census/person/pf01099901000102$"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(html),
        )
        .with_priority(1)
        .mount(server)
        .await;
}

/// Selects the record with id `row` from a records payload.
fn select(row: &str) -> String {
    json!({ "kind": "submit", "action": "select", "values": { "row": row } }).to_string()
}

/// A residence reply that picks each of `rows` in turn, then finishes.
fn household_reply(rows: &'static [&'static str]) -> impl FnMut(&str) -> Result<String, PresentError> + Send {
    let mut next = rows.iter();
    move |payload: &str| {
        Ok(match kind_of(payload).as_str() {
            "records" => next.next().map_or_else(done, |row| select(row)),
            "confirm-record" => import_response(),
            "save-scan" => save_response(payload),
            _ => done(),
        })
    }
}

/// Asserts the workspace holds `persons` persons, each citing the one source and carrying the one
/// scan, held by the one repository.
async fn assert_one_source_repository_and_scan(root: &Path, persons: usize) {
    let workspace = open_workspace(root).await;
    let listed = list_persons(&workspace).await.expect("persons");
    assert_eq!(listed.len(), persons, "every picked record was imported");
    for person in &listed {
        assert_eq!(person.citations.len(), 1, "{} cites the record", person.human_id);
        assert_eq!(person.media.len(), 1, "{} carries the scan", person.human_id);
    }
    assert_eq!(list_sources(&workspace).await.expect("sources").len(), 1, "one source");
    assert_eq!(
        list_repositories(&workspace).await.expect("repos").len(),
        1,
        "one repository"
    );
    assert_eq!(list_media(&workspace).await.expect("media").len(), 1, "one scan");
    assert_eq!(
        list_citations(&workspace).await.expect("citations").len(),
        persons,
        "a citation per record"
    );
}

#[tokio::test]
async fn two_records_of_one_household_share_its_source_repository_and_scan() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    mount_second_member(&server).await;
    let (presenter, _seen) = ScriptedPresenter::new(household_reply(&["pf01099901000101", "pf01099901000102"]));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-residence",
        presenter,
    )
    .await
    .expect("assisted import runs");

    assert_one_source_repository_and_scan(&root, 2).await;
}

#[tokio::test]
async fn a_later_session_reuses_the_source_repository_and_scan_an_earlier_one_made() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    mount_second_member(&server).await;

    let sessions: [&'static [&'static str]; 2] = [&["pf01099901000101"], &["pf01099901000102"]];
    for rows in sessions {
        let (presenter, _seen) = ScriptedPresenter::new(household_reply(rows));
        run(
            (open_workspace(&root).await, grants(&[])),
            &server,
            "census-residence",
            presenter,
        )
        .await
        .expect("assisted import runs");
    }

    assert_one_source_repository_and_scan(&root, 2).await;
}

#[tokio::test]
async fn a_record_of_a_person_another_dataset_made_is_a_persona_merged_into_it() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let workspace = open_workspace(&root).await;
    let human = Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
    });
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some("Ola".to_owned()),
            Some("Fjellstue".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: vec![ExternalId {
            authority: "digitalarkivet".to_owned(),
            value: "pf01099901000101".to_owned(),
            kind: None,
            url: None,
        }],
    };
    let existing = create_person(&workspace, &human, new, Provenance::default(), &[])
        .await
        .expect("person");
    let (presenter, _seen) = ScriptedPresenter::new(|payload: &str| {
        Ok(match kind_of(payload).as_str() {
            "confirm-record" => json!({
                "kind": "submit",
                "action": "import",
                "values": {
                    "fields": [
                        { "key": "name", "value": "Edited Name" },
                        { "key": "occupation", "value": "Gårdbruker S." }
                    ],
                    "region": { "left": 4, "top": 47, "width": 92, "height": 9 },
                    "confidence": "low"
                }
            })
            .to_string(),
            "save-scan" => save_response(payload),
            _ => done(),
        })
    });

    let summary = run((workspace, grants(&[])), &server, "census-person", presenter)
        .await
        .expect("assisted import runs");

    let workspace = open_workspace(&root).await;
    let records = workspace.store().list_persons().await.expect("person records");
    let persona = records
        .iter()
        .filter_map(|view| view.human_id())
        .map(|human_id| human_id.as_str().to_owned())
        .find(|human_id| *human_id != existing)
        .expect("the record's own persona");
    assert!(summary.contains(&persona), "the summary names the persona: {summary}");
    assert_eq!(
        vitni_app::pair_decision(&workspace, &existing, &persona)
            .await
            .expect("decision"),
        Some(PairDecision::SameCluster),
        "merged into the person the id names"
    );
    let persons = list_persons(&workspace).await.expect("persons");
    assert_eq!(persons.len(), 1, "one person, two records");
    assert!(
        !persons[0].citations.is_empty(),
        "the record's citation reaches the person"
    );
    assert!(
        events_contain(&root, "Gårdbruker").await,
        "the record's occupation is kept"
    );
    assert!(
        !list_events(&workspace).await.expect("events").is_empty(),
        "the record's events are kept"
    );
    assert_eq!(list_sources(&workspace).await.expect("sources").len(), 1);
    let stored = workspace
        .store()
        .find_person(&existing)
        .await
        .expect("find")
        .expect("existing person");
    assert!(
        stored.facts().is_empty(),
        "the other dataset's record keeps its own contents"
    );
}

// ----- what a record says beyond the person: events, places, the household -----

/// The event of `event_type` in `events`.
fn event_of<'a>(events: &'a [EventSummary], event_type: &EventType) -> &'a EventSummary {
    events
        .iter()
        .find(|event| event.event_type.as_ref() == Some(event_type))
        .expect("an event of the type")
}

/// The structured date `date` holds, when it holds one.
fn modifier(date: Option<&GenealogicalDate>) -> Option<(&DateModifier, DateQuality)> {
    let date = date?;
    match &date.modifier {
        GenealogicalDateBody::Structured(modifier) => Some((modifier, date.quality)),
        GenealogicalDateBody::TextOnly { .. } => None,
    }
}

fn point(year: i32, month: Option<u8>, day: Option<u8>) -> DatePoint {
    DatePoint {
        year: Some(year),
        month,
        day,
    }
}

#[tokio::test]
async fn a_census_person_imports_its_census_its_birth_their_places_and_its_household() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let (presenter, seen) = ScriptedPresenter::new(|payload| Ok(single_person_reply(payload)));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-person",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let confirm = seen
        .lock()
        .expect("seen")
        .iter()
        .find(|payload| kind_of(payload) == "confirm-record")
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
        .expect("a confirm payload");
    let preview = &confirm["record"]["provenance"];
    assert_eq!(preview["event"], "Folketelling 1920 for 9901 Eksempelvik herred");
    assert_eq!(preview["places"], json!(["Fjellstue", "Eksempelvik"]));
    assert_eq!(
        preview["household"],
        json!({ "position": "partner", "residence": "Fjellstue" })
    );

    let workspace = open_workspace(&root).await;
    let events = list_events(&workspace).await.expect("events");
    assert_eq!(events.len(), 2, "the census and the birth: {events:#?}");

    let census = event_of(&events, &EventType::Census);
    assert_eq!(
        modifier(census.date.as_ref()),
        Some((&DateModifier::None(point(1920, None, None)), DateQuality::Normal)),
        "the census is dated by its year"
    );
    assert_eq!(
        census.place.as_ref().and_then(|place| place.name.as_deref()),
        Some("Fjellstue"),
        "the census took place at the residence"
    );
    assert_eq!(census.participants.len(), 1, "the person took part in the census");
    let participant = &census.participants[0];
    assert_eq!(participant.role, ParticipantRole::Primary);
    assert!(
        participant
            .attributes
            .iter()
            .any(|attribute| attribute.attribute_type == "Familiestilling" && attribute.value == "hp"),
        "the family position is kept on the participation: {participant:?}"
    );

    let birth = event_of(&events, &EventType::Birth);
    assert_eq!(
        modifier(birth.date.as_ref()),
        Some((&DateModifier::None(point(1887, Some(3), Some(14))), DateQuality::Normal)),
        "the birth carries the transcribed date"
    );
    assert_eq!(
        birth.place.as_ref().and_then(|place| place.name.as_deref()),
        Some("Eksempelvik"),
        "the birthplace is the municipality of the same name"
    );
    assert_eq!(birth.participants.len(), 1);
    assert_eq!(birth.participants[0].role, ParticipantRole::Primary);

    let places = list_places(&workspace).await.expect("places");
    assert_eq!(places.len(), 2, "the residence and its municipality: {places:#?}");
    let residence = places
        .iter()
        .find(|place| place.resolved_name.as_deref() == Some("Fjellstue"))
        .expect("the residence");
    assert_eq!(
        residence.place_type,
        Some(PlaceType::Farm),
        "a rural residence is a farm"
    );
    assert!(
        residence
            .enclosing
            .iter()
            .any(|enclosing| enclosing.name.as_deref() == Some("Eksempelvik")
                && enclosing.place_type == Some(PlaceType::Municipality)),
        "the residence lies in the census municipality: {residence:#?}"
    );

    let families = list_families(&workspace).await.expect("families");
    assert_eq!(families.len(), 1, "the household's family");
    assert_eq!(families[0].partners.len(), 1, "the head is a partner");
    assert!(families[0].children.is_empty(), "the head alone is no child");

    for origin in [
        r#""record":"census:bf01099901000100""#,
        r#""record":"residence:bf01099901000100""#,
        r#""record":"municipality:9901""#,
        r#""record":"household:bf01099901000100:01""#,
        r#""item":"event:BIRT","record":"pf01099901000101""#,
    ] {
        assert!(events_contain(&root, origin).await, "an assertion carries {origin}");
    }
}

/// Serves the household's second member as a `role` (`Familiestilling`), under its own record id.
async fn mount_member_as(server: &MockServer, role: &str) {
    mount_member_with(
        server,
        r#"<div class="ssp-semibold">hp</div>"#,
        &format!(r#"<div class="ssp-semibold">{role}</div>"#),
    )
    .await;
}

#[tokio::test]
async fn two_members_of_a_household_share_its_census_and_its_family() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    mount_member_as(&server, "s").await;
    let (presenter, _seen) = ScriptedPresenter::new(household_reply(&["pf01099901000101", "pf01099901000102"]));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-residence",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let workspace = open_workspace(&root).await;
    let events = list_events(&workspace).await.expect("events");
    assert_eq!(events.len(), 3, "one census and a birth each: {events:#?}");
    assert_eq!(
        event_of(&events, &EventType::Census).participants.len(),
        2,
        "both members took part in the one census"
    );
    assert_eq!(
        list_places(&workspace).await.expect("places").len(),
        2,
        "the places are shared"
    );
    let families = list_families(&workspace).await.expect("families");
    assert_eq!(families.len(), 1, "one household family");
    assert_eq!(families[0].partners.len(), 1, "the head is the partner");
    assert_eq!(families[0].children.len(), 1, "the son is the child");
    assert_ne!(families[0].partners[0].id, families[0].children[0].id);
}

/// Serves the household's second member with `from` replaced by `to` in its page.
async fn mount_member_with(server: &MockServer, from: &str, to: &str) {
    let base = format!("http://localhost:{}", server.address().port());
    let html = fixture("census", "person.html", &base)
        .replace(
            &format!(r#"content="{base}/census/person/pf01099901000101""#),
            &format!(r#"content="{base}/census/person/pf01099901000102""#),
        )
        .replace(from, to);
    Mock::given(method("GET"))
        .and(path_regex(r"^/census/person/pf01099901000102$"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(html),
        )
        .with_priority(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn two_households_of_one_residence_found_two_families() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    mount_member_with(
        &server,
        r#"<div class="ssp-semibold">01</div>"#,
        r#"<div class="ssp-semibold">02</div>"#,
    )
    .await;
    let (presenter, _seen) = ScriptedPresenter::new(household_reply(&["pf01099901000101", "pf01099901000102"]));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-residence",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let workspace = open_workspace(&root).await;
    let families = list_families(&workspace).await.expect("families");
    assert_eq!(families.len(), 2, "a family per household number: {families:#?}");
    let events = list_events(&workspace).await.expect("events");
    assert_eq!(
        event_of(&events, &EventType::Census).participants.len(),
        2,
        "both households took part in the residence's one census"
    );
}

#[tokio::test]
async fn a_birthplace_is_one_place_only_within_its_census_municipality() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    mount_member_with(
        &server,
        r#"<div class="ssp-semibold">Eksempelvik</div>"#,
        r#"<div class="ssp-semibold">Nes</div>"#,
    )
    .await;
    let (presenter, _seen) = ScriptedPresenter::new(household_reply(&["pf01099901000102"]));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-residence",
        presenter,
    )
    .await
    .expect("assisted import runs");

    assert!(
        events_contain(&root, r#""record":"birthplace:municipality:9901:Nes""#).await,
        "the birthplace is keyed within the census municipality"
    );
}

#[tokio::test]
async fn a_servant_takes_part_in_the_census_but_not_the_family() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    mount_member_as(&server, "tj").await;
    let (presenter, _seen) = ScriptedPresenter::new(household_reply(&["pf01099901000102"]));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-residence",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let workspace = open_workspace(&root).await;
    let events = list_events(&workspace).await.expect("events");
    assert_eq!(event_of(&events, &EventType::Census).participants.len(), 1);
    assert!(
        list_families(&workspace).await.expect("families").is_empty(),
        "a servant joins no family"
    );
}

#[tokio::test]
async fn a_head_living_alone_founds_no_family() {
    let (root, _dir) = init_workspace();
    let server = MockServer::start().await;
    let base = format!("http://localhost:{}", server.address().port());
    let alone = fixture("census", "person.html", &base).replace(r#"<div class="data-item">"#, "<div>");
    mount(&server, r"^/census/person/.*", alone).await;
    let (presenter, _seen) = ScriptedPresenter::new(|payload| Ok(single_person_reply(payload)));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "census-person",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let workspace = open_workspace(&root).await;
    assert_eq!(list_persons(&workspace).await.expect("persons").len(), 1);
    assert_eq!(
        list_events(&workspace).await.expect("events").len(),
        2,
        "the census and the birth"
    );
    assert!(
        list_families(&workspace).await.expect("families").is_empty(),
        "a household of one is no family"
    );
}

/// Starts a mock server serving the church-book record page, its `Rolle` replaced by `role`. Its scan
/// link points at a host the policy denies, so the record imports without a scan.
async fn churchbook_server(role: &str) -> MockServer {
    let server = MockServer::start().await;
    let base = format!("http://localhost:{}", server.address().port());
    let html = fixture("churchbook", "person.html", &base).replace(
        r#"<div class="ssp-semibold">far</div>"#,
        &format!(r#"<div class="ssp-semibold">{role}</div>"#),
    );
    mount(&server, r"^/view/999/pd.*", html).await;
    server
}

/// The church-book reply: import the confirm, finish the summary (no scan resolves to save).
fn churchbook_reply(payload: &str) -> String {
    match kind_of(payload).as_str() {
        "confirm-record" => {
            json!({ "kind": "submit", "action": "import", "values": { "confidence": "low" } }).to_string()
        }
        _ => done(),
    }
}

#[tokio::test]
async fn a_church_book_record_imports_its_event_with_the_participant_by_role() {
    let (root, _dir) = init_workspace();
    let server = churchbook_server("far").await;
    let (presenter, seen) = ScriptedPresenter::new(|payload| Ok(churchbook_reply(payload)));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "churchbook-record",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let confirm = seen
        .lock()
        .expect("seen")
        .iter()
        .find(|payload| kind_of(payload) == "confirm-record")
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
        .expect("a confirm payload");
    assert_eq!(confirm["record"]["provenance"]["event"], "Fødte og døpte: 1925-02-15");
    assert_eq!(
        confirm["record"]["provenance"].get("household"),
        None,
        "no household in a church book"
    );

    let workspace = open_workspace(&root).await;
    let events = list_events(&workspace).await.expect("events");
    let baptism = event_of(&events, &EventType::Baptism);
    assert_eq!(
        modifier(baptism.date.as_ref()),
        Some((&DateModifier::None(point(1925, Some(2), Some(15))), DateQuality::Normal)),
        "the baptism carries the heading's date"
    );
    assert_eq!(baptism.participants.len(), 1);
    assert_eq!(
        baptism.participants[0].role,
        ParticipantRole::Father,
        "the father by his role"
    );
    let birth = event_of(&events, &EventType::Birth);
    assert_eq!(
        modifier(birth.date.as_ref()),
        Some((&DateModifier::None(point(1887, None, None)), DateQuality::Normal)),
        "the father's birth year"
    );
    assert!(
        list_families(&workspace).await.expect("families").is_empty(),
        "a church-book event founds no family"
    );
    assert!(
        events_contain(&root, r#""record":"churchbook-event:hd00000099901000""#).await,
        "the event carries its record's origin"
    );
}

#[tokio::test]
async fn a_church_book_age_dates_the_birth_from_the_event_not_the_book() {
    let (root, _dir) = init_workspace();
    let server = MockServer::start().await;
    let base = format!("http://localhost:{}", server.address().port());
    let html = fixture("churchbook", "person.html", &base).replace(
        r#"<div class="ssp-semibold">1887</div>"#,
        r#"<div class="ssp-semibold">38</div>"#,
    );
    mount(&server, r"^/view/999/pd.*", html).await;
    let (presenter, _seen) = ScriptedPresenter::new(|payload| Ok(churchbook_reply(payload)));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "churchbook-record",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let events = list_events(&open_workspace(&root).await).await.expect("events");
    assert_eq!(
        modifier(event_of(&events, &EventType::Birth).date.as_ref()),
        Some((&DateModifier::About(point(1887, None, None)), DateQuality::Calculated)),
        "the 1925 baptism less 38 years, not the book's 1904"
    );
}

#[tokio::test]
async fn a_church_book_role_no_rule_reads_asserts_no_participation() {
    let (root, _dir) = init_workspace();
    let server = churchbook_server("husbonde").await;
    let (presenter, _seen) = ScriptedPresenter::new(|payload| Ok(churchbook_reply(payload)));

    run(
        (open_workspace(&root).await, grants(&[])),
        &server,
        "churchbook-record",
        presenter,
    )
    .await
    .expect("assisted import runs");

    let workspace = open_workspace(&root).await;
    assert_eq!(list_persons(&workspace).await.expect("persons").len(), 1);
    let events = list_events(&workspace).await.expect("events");
    assert!(
        event_of(&events, &EventType::Baptism).participants.is_empty(),
        "no role is guessed"
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
    let url = format!("sqlite://{}", root.join("vitni.sqlite3").display());
    let pool = sqlx::SqlitePool::connect(&url).await.expect("open events db");
    let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM events")
        .fetch_all(&pool)
        .await
        .expect("read events");
    pool.close().await;
    payloads.iter().any(|payload| payload.contains(needle))
}

// ----- the host's match stage (ADR 0040 §4) -----

/// The census person's confirm, unedited, then the scan saved and the summary closed.
fn unedited_reply(payload: &str) -> String {
    match kind_of(payload).as_str() {
        "confirm-record" => json!({ "kind": "submit", "action": "import", "values": {} }).to_string(),
        "save-scan" => save_response(payload),
        _ => done(),
    }
}

/// A person entered by hand with the census person's name, and no external id: a possible match.
async fn stored_namesake(workspace: &Workspace) -> String {
    let human = Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
    });
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some("Ola Eksempelsen".to_owned()),
            Some("Fjellstue".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(workspace, &human, new, Provenance::default(), &[])
        .await
        .expect("person")
}

/// The census person the import wrote, by its Digitalarkivet id.
async fn imported_person(workspace: &Workspace) -> Option<String> {
    let view = workspace
        .store()
        .find_person_by_external_id("digitalarkivet", "pf01099901000101")
        .await
        .expect("lookup")?;
    view.human_id().map(|id| id.as_str().to_owned())
}

#[tokio::test]
async fn a_possible_match_is_asked_about_and_same_merges_the_record_into_it() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let workspace = open_workspace(&root).await;
    let stored = stored_namesake(&workspace).await;
    let (presenter, _seen) = ScriptedPresenter::new(|payload| Ok(unedited_reply(payload)));
    let (presenter, questions) = presenter.answering(|question| {
        let answer = if question.kind == MatchableKind::Person {
            PairAnswer::Same(IdentityDecision::default())
        } else {
            PairAnswer::Later
        };
        MatchReply::Pair(Box::new(answer))
    });

    run((workspace, grants(&[])), &server, "census-person", presenter)
        .await
        .expect("assisted import runs");

    let asked = questions.lock().expect("questions").clone();
    let person = asked
        .iter()
        .find(|question| question.kind == MatchableKind::Person)
        .expect("asked about the person");
    assert_eq!(person.candidate.human_id, stored);
    let workspace = open_workspace(&root).await;
    let imported = imported_person(&workspace).await.expect("the record was imported");
    assert_ne!(imported, stored, "the record keeps its own persona");
    assert_eq!(
        vitni_app::pair_decision(&workspace, &stored, &imported)
            .await
            .expect("decision"),
        Some(PairDecision::SameCluster)
    );
}

#[tokio::test]
async fn skipping_at_the_match_stage_writes_nothing_of_the_record() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let workspace = open_workspace(&root).await;
    stored_namesake(&workspace).await;
    let (presenter, _seen) = ScriptedPresenter::new(|payload| Ok(unedited_reply(payload)));
    let (presenter, questions) = presenter.answering(|_| MatchReply::Skip);

    let summary = run((workspace, grants(&[])), &server, "census-person", presenter)
        .await
        .expect("assisted import runs");

    assert_eq!(
        questions.lock().expect("questions").len(),
        1,
        "asked once, then skipped"
    );
    assert!(
        summary.contains("\"skipped\":1"),
        "the record counts as skipped: {summary}"
    );
    let workspace = open_workspace(&root).await;
    assert_eq!(imported_person(&workspace).await, None);
    assert_eq!(list_persons(&workspace).await.expect("persons").len(), 1);
    assert_eq!(
        list_sources(&workspace).await.expect("sources").len(),
        0,
        "no source written"
    );
}

#[tokio::test]
async fn cancelling_at_the_match_stage_ends_the_session_writing_nothing() {
    let (root, _dir) = init_workspace();
    let server = census_server().await;
    let workspace = open_workspace(&root).await;
    stored_namesake(&workspace).await;
    let (presenter, _seen) = ScriptedPresenter::new(|payload| Ok(unedited_reply(payload)));
    let (presenter, _questions) = presenter.answering(|_| MatchReply::Cancel);

    let summary = run((workspace, grants(&[])), &server, "census-person", presenter)
        .await
        .expect("assisted import runs");

    assert!(summary.contains("\"imported\":[]"), "nothing imported: {summary}");
    let workspace = open_workspace(&root).await;
    assert_eq!(list_persons(&workspace).await.expect("persons").len(), 1);
    assert_eq!(
        list_sources(&workspace).await.expect("sources").len(),
        0,
        "no source written"
    );
}
