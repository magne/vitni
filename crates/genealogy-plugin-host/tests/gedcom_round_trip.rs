//! GEDCOM round-trip integration test (roadmap Spike C / Phase 4): a GEDCOM file imports as personas
//! and a family with Software-agent provenance through the streaming bulk-import world, re-exports
//! through the bulk-export world, and re-imports identically — while progress is reported (ADR 0013).
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use genealogy_app::{
    AppDefaults, OperatorConfig, PersonSummary, Session, Workspace, WorkspaceDefaults, list_events, list_families,
    list_persons, list_places,
};
use genealogy_core::ids::AgentId;
use genealogy_plugin_host::{
    Capability, ExportTarget, Grants, Invocation, PluginHost, ProgressControl, ProgressUpdate, ResourceBudget,
};
use uuid::Uuid;

const SAMPLE: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
1 SEX M
1 BIRT
2 DATE 5 APR 1970
2 PLAC Mandal
0 @I2@ INDI
1 NAME Jane /Doe/
1 SEX F
0 @I3@ INDI
1 NAME Sam /Smith/
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 CHIL @I3@
0 TRLR
";

/// The same family as `SAMPLE`, but with the stable `_UID` MyHeritage/Gramps exports carry — the
/// identifier a re-import resolves records by, so importing this twice is a no-op the second time.
const SAMPLE_WITH_UID: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
1 _UID 11111111-1111-1111-1111-111111111111
1 BIRT
2 DATE 5 APR 1970
2 PLAC Mandal
0 @I2@ INDI
1 NAME Jane /Doe/
1 _UID 22222222-2222-2222-2222-222222222222
0 @I3@ INDI
1 NAME Sam /Smith/
1 _UID 33333333-3333-3333-3333-333333333333
0 @F1@ FAM
1 _UID FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF
1 HUSB @I1@
1 WIFE @I2@
1 CHIL @I3@
0 TRLR
";

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn software_session() -> Session {
    Session::software("genealogy-gedcom-import", "0.1.0")
}

fn import_grants() -> Grants {
    Grants::none()
        .with(Capability::Commands)
        .with(Capability::Log)
        .with(Capability::Progress)
        .with(Capability::ImportSource)
}

fn export_grants() -> Grants {
    Grants::none()
        .with(Capability::Query)
        .with(Capability::Log)
        .with(Capability::Progress)
        .with(Capability::ExportSink)
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

/// Writes `bytes` to a file under `dir` and returns its path.
fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write file");
    path
}

/// A progress sink that records every update, shareable across the `'static` closure boundary.
fn progress_collector() -> (
    Arc<Mutex<Vec<ProgressUpdate>>>,
    impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static,
) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let record = move |update: ProgressUpdate| {
        sink.lock().expect("progress lock").push(update);
        ProgressControl::Proceed
    };
    (log, record)
}

/// A comparable snapshot of a workspace's persons and family structure.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    persons: Vec<(String, Option<String>, Option<String>)>,
    families: Vec<(String, Vec<String>, Vec<String>)>,
}

async fn snapshot(workspace: &Workspace) -> Snapshot {
    let persons = list_persons(workspace)
        .await
        .expect("list persons")
        .into_iter()
        .map(|person: PersonSummary| (person.human_id, person.given, person.surname))
        .collect();
    let families = list_families(workspace)
        .await
        .expect("list families")
        .into_iter()
        .map(|family| (family.human_id, family.partners, family.children))
        .collect();
    Snapshot { persons, families }
}

/// Counts the rows in the event log directly — the proof that a re-import emitted no new events
/// (no use-case exposes the raw event count).
async fn event_count(root: &Path) -> i64 {
    let db = root.join("genealogy.sqlite3");
    let url = format!("sqlite://{}", db.display());
    let pool = sqlx::SqlitePool::connect(&url).await.expect("open events db");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
        .expect("count events");
    pool.close().await;
    count
}

/// Reads the events table directly and reports whether any event was recorded under a Software
/// operator (ADR 0011 §5) — no use-case exposes the operator kind.
async fn has_software_provenance(root: &Path) -> bool {
    let db = root.join("genealogy.sqlite3");
    let url = format!("sqlite://{}", db.display());
    let pool = sqlx::SqlitePool::connect(&url).await.expect("open events db");
    let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM events")
        .fetch_all(&pool)
        .await
        .expect("read events");
    pool.close().await;
    payloads.iter().any(|payload| payload.contains("Software"))
}

#[tokio::test]
async fn gedcom_imports_with_software_provenance_then_round_trips() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gedcom-import")).expect("load import");
    let exporter = host.load(&plugin_path("gedcom-export")).expect("load export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE.as_bytes());

    // 1. Import the sample GEDCOM from the host-opened source, collecting progress.
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (progress, record) = progress_collector();
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            Invocation {
                workspace,
                session: software_session(),
                grants: import_grants(),
                budget: ResourceBudget::default(),
            },
            source,
            record,
        )
        .await
        .expect("import");
    assert_eq!(count, 4, "3 individuals + 1 family");
    assert!(
        !progress.lock().expect("progress").is_empty(),
        "the import must report progress (ADR 0013)"
    );

    // 2. The persons and family landed as expected.
    let original = snapshot(&workspace).await;
    assert_eq!(
        original.persons,
        vec![
            ("I0001".to_owned(), Some("John".to_owned()), Some("Smith".to_owned())),
            ("I0002".to_owned(), Some("Jane".to_owned()), Some("Doe".to_owned())),
            ("I0003".to_owned(), Some("Sam".to_owned()), Some("Smith".to_owned())),
        ]
    );
    assert_eq!(
        original.families,
        vec![(
            "F0001".to_owned(),
            vec!["I0001".to_owned(), "I0002".to_owned()],
            vec!["I0003".to_owned()],
        )]
    );

    // 2b. SEX imported as an assertion on the person.
    let persons = list_persons(&workspace).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(john.sex, Some(genealogy_app::Sex::Male), "SEX M imported");
    let jane = persons.iter().find(|p| p.human_id == "I0002").expect("I0002");
    assert_eq!(jane.sex, Some(genealogy_app::Sex::Female), "SEX F imported");

    // 2c. The birth event and its place imported as their own aggregates.
    assert_eq!(
        list_events(&workspace).await.expect("list events").len(),
        1,
        "BIRT event created"
    );
    assert_eq!(
        list_places(&workspace).await.expect("list places").len(),
        1,
        "PLAC place created"
    );

    // 3. The import was attributed to a Software operator.
    assert!(
        has_software_provenance(&root).await,
        "imported events must carry AgentKind::Software provenance"
    );

    // 4. Export to a host-resolved file.
    let exported = io_dir.path().join("out.ged");
    let (_, record) = progress_collector();
    let (exported_count, workspace) = host
        .run_bulk_export(
            &exporter,
            Invocation {
                workspace,
                session: software_session(),
                grants: export_grants(),
                budget: ResourceBudget::default(),
            },
            ExportTarget::File(exported.clone()),
            record,
        )
        .await
        .expect("export");
    drop(workspace);
    assert_eq!(exported_count, 4, "3 individuals + 1 family exported");
    let bytes = std::fs::read(&exported).expect("read exported document");
    assert!(!bytes.is_empty(), "export produced a document");

    // 5. Re-import the exported document into a fresh workspace — structure is identical.
    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, record) = progress_collector();
    let (count2, workspace2) = host
        .run_bulk_import(
            &importer,
            Invocation {
                workspace: workspace2,
                session: software_session(),
                grants: import_grants(),
                budget: ResourceBudget::default(),
            },
            exported,
            record,
        )
        .await
        .expect("re-import");
    assert_eq!(count2, 4);
    assert_eq!(
        snapshot(&workspace2).await,
        original,
        "round-trip must preserve persons and families"
    );
}

#[tokio::test]
async fn re_importing_the_same_file_into_one_workspace_emits_no_new_events() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gedcom-import")).expect("load import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE_WITH_UID.as_bytes());

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;

    // First import populates the workspace.
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            Invocation {
                workspace,
                session: software_session(),
                grants: import_grants(),
                budget: ResourceBudget::default(),
            },
            source.clone(),
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("first import");
    assert_eq!(count, 4, "3 individuals + 1 family");
    let first_snapshot = snapshot(&workspace).await;
    let events_after_first = event_count(&root).await;
    assert!(events_after_first > 0, "the first import recorded events");
    // The birth event and place were created on first import.
    assert_eq!(list_events(&workspace).await.expect("events").len(), 1, "one event");
    assert_eq!(list_places(&workspace).await.expect("places").len(), 1, "one place");

    // Re-import the identical file into the SAME workspace: every record resolves to its existing
    // aggregate, so no new events are written and the projection is unchanged.
    let (_, workspace) = host
        .run_bulk_import(
            &importer,
            Invocation {
                workspace,
                session: software_session(),
                grants: import_grants(),
                budget: ResourceBudget::default(),
            },
            source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("second import");

    assert_eq!(
        event_count(&root).await,
        events_after_first,
        "re-importing an identical file must emit no new events"
    );
    assert_eq!(
        snapshot(&workspace).await,
        first_snapshot,
        "re-import must not change the projection"
    );
    // The owned event and place were not duplicated (created only on first import).
    assert_eq!(
        list_events(&workspace).await.expect("events").len(),
        1,
        "event not duplicated"
    );
    assert_eq!(
        list_places(&workspace).await.expect("places").len(),
        1,
        "place not duplicated"
    );
}

#[tokio::test]
async fn import_is_denied_without_the_commands_capability() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gedcom-import")).expect("load import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE.as_bytes());

    // The plugin may read the source and report progress, but not submit commands.
    let grants = Grants::none()
        .with(Capability::Log)
        .with(Capability::Progress)
        .with(Capability::ImportSource);
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, record) = progress_collector();
    let result = host
        .run_bulk_import(
            &importer,
            Invocation {
                workspace,
                session: software_session(),
                grants,
                budget: ResourceBudget::default(),
            },
            source,
            record,
        )
        .await;

    assert!(result.is_err(), "import without the commands grant must fail");
    let workspace = open_workspace(&root).await;
    assert!(
        list_persons(&workspace).await.expect("list").is_empty(),
        "a denied import must not have created any person"
    );
}

#[tokio::test]
async fn import_stops_when_progress_reports_cancel() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gedcom-import")).expect("load import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE.as_bytes());

    // Cancel at the first progress report: the importer should stop after the first person.
    let cancel_after_first = |_: ProgressUpdate| ProgressControl::Cancel;

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            Invocation {
                workspace,
                session: software_session(),
                grants: import_grants(),
                budget: ResourceBudget::default(),
            },
            source,
            cancel_after_first,
        )
        .await
        .expect("import");

    assert_eq!(count, 1, "cancel after the first report stops the import at one record");
    assert_eq!(
        list_persons(&workspace).await.expect("list").len(),
        1,
        "only the records imported before cancellation are persisted"
    );
}
