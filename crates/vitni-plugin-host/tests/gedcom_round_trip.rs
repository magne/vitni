//! GEDCOM round-trip integration test (roadmap Spike C / Phase 4): a GEDCOM file imports as personas
//! and a family with Software-agent provenance through the streaming bulk-import world, re-exports
//! through the bulk-export world, and re-imports identically — while progress is reported (ADR 0013).
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use uuid::Uuid;
use vitni_app::{
    AgeBound, AiConfig, AppDefaults, ChildParentRelationship, OperatorConfig, ParticipantRole, PersonSummary, Session,
    Workspace, WorkspaceDefaults, list_citations, list_events, list_families, list_media, list_notes, list_persons,
    list_places, list_repositories, list_sources,
};
use vitni_core::ids::AgentId;
use vitni_plugin_host::{
    Capability, ExportTarget, Grants, Invocation, NetPolicy, ProgressControl, ProgressUpdate, ResourceBudget,
};

mod common;

const SAMPLE: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
1 SEX M
1 BIRT
2 DATE 5 APR 1970
2 PLAC Mandal
1 SOUR @S1@
2 PAGE p. 5
1 OBJE
2 FILE https://example.test/photo.jpg
2 TITL Portrait
2 FORM image/jpeg
1 NOTE A research note.
0 @I2@ INDI
1 NAME Jane /Doe/
1 SEX F
0 @I3@ INDI
1 NAME Sam /Smith/
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 CHIL @I3@
2 _FREL Birth
2 _MREL Adopted
1 MARR
2 DATE 1848
0 @S1@ SOUR
1 TITL Census 1801
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
1 SOUR @S1@
2 PAGE p. 5
1 OBJE
2 FILE https://example.test/photo.jpg
2 TITL Portrait
2 FORM image/jpeg
1 NOTE A research note.
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
2 _FREL Birth
2 _MREL Adopted
1 MARR
2 DATE 1848
0 @S1@ SOUR
1 TITL Census 1801
0 TRLR
";

/// Exercises the F′ breadth: structured `NAME` sub-records, the full `DATE` grammar (`ABT`), an
/// `ADDR` on a residence event, an `OCCU` fact, and an `ASSO` association to a second person.
const RICH: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
2 TYPE birth
2 GIVN Johnny
2 SPFX van
2 SURN Smithson
2 NICK Jack
2 NPFX Dr
2 NSFX Jr
1 SEX M
1 BIRT
2 DATE ABT 1850
1 RESI
2 ADDR 12 Market Square
3 CITY Bergen
3 POST 5003
3 CTRY Norway
1 OCCU Carpenter
1 ASSO @I2@
2 ROLE WITN
0 @I2@ INDI
1 NAME Jane /Doe/
0 TRLR
";

/// Exercises the participation payload (ADR 0019): an `INDI` census with the participant's `AGE` and
/// an event-level `ASSO` witness (role, a cited `SOUR`, and a `NOTE`), plus a `FAM` marriage with
/// `HUSB`/`WIFE` ages.
const WITNESS_AND_AGES: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
1 SEX M
1 CENS
2 DATE 1900
2 AGE 45y
2 ASSO @I2@
3 ROLE WITN
3 SOUR @S1@
4 PAGE p. 3
3 NOTE Witnessed the census.
0 @I2@ INDI
1 NAME Pat /Vitne/
1 SEX F
0 @I3@ INDI
1 NAME Jane /Doe/
1 SEX F
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I3@
1 MARR
2 DATE 1890
2 HUSB
3 AGE 25y
2 WIFE
3 AGE < 24y 6m
0 @S1@ SOUR
1 TITL Parish register
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
    Session::software("vitni-gedcom-import", "0.1.0")
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

/// Builds a bulk-run [`Invocation`] for the fixture workspace: a Software session, the given grants,
/// the default budget, and no network access (bulk import/export never fetches).
fn invocation(workspace: Workspace, grants: Grants) -> Invocation {
    Invocation {
        workspace,
        session: software_session(),
        grants,
        budget: ResourceBudget::default(),
        net_policy: NetPolicy::deny_all(),
        ai_config: AiConfig::default(),
        provenance_confidence: None,
    }
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
    families: Vec<(String, Vec<String>, Vec<ChildSnapshot>)>,
}

/// A child's `human_id` and its per-partner relationships, so the round-trip preserves `_FREL`/`_MREL`.
type ChildSnapshot = (String, Vec<(String, ChildParentRelationship)>);

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
        .map(|family| {
            let partners = family.partners.into_iter().map(|p| p.human_id).collect();
            let children = family
                .children
                .into_iter()
                .map(|c| {
                    // Per-partner links are separate assertions now (ADR 0021); flatten them back to
                    // the `(partner, relationship)` tuples the snapshot compares — the data is the same.
                    let relationships = c
                        .relationships
                        .into_iter()
                        .map(|link| (link.partner_human_id, link.relationship))
                        .collect();
                    (c.human_id, relationships)
                })
                .collect();
            (family.human_id, partners, children)
        })
        .collect();
    Snapshot { persons, families }
}

/// Counts the rows in the event log directly — the proof that a re-import emitted no new events
/// (no use-case exposes the raw event count).
async fn event_count(root: &Path) -> i64 {
    let db = root.join("vitni.sqlite3");
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
    let db = root.join("vitni.sqlite3");
    let url = format!("sqlite://{}", db.display());
    let pool = sqlx::SqlitePool::connect(&url).await.expect("open events db");
    let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM events")
        .fetch_all(&pool)
        .await
        .expect("read events");
    pool.close().await;
    payloads.iter().any(|payload| payload.contains("Software"))
}

/// Reads every event payload as raw JSON (used to assert a claim type was recorded — no use-case
/// exposes facts, associations, or event addresses yet).
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

/// Asserts the GEDCOM 7 breadth the `SAMPLE` import produces: John's and Jane's sex, and exactly one
/// event, place, source, and citation (group F).
async fn assert_sample_breadth(workspace: &Workspace) {
    let persons = list_persons(workspace).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(john.sex, Some(vitni_app::Sex::Male), "SEX M imported");
    let jane = persons.iter().find(|p| p.human_id == "I0002").expect("I0002");
    assert_eq!(jane.sex, Some(vitni_app::Sex::Female), "SEX F imported");
    assert_eq!(
        list_events(workspace).await.expect("events").len(),
        2,
        "BIRT + MARR events created"
    );
    let families = list_families(workspace).await.expect("families");
    assert_eq!(
        families.first().expect("one family").events.len(),
        1,
        "the FAM MARR round-tripped as an explicit FamilyEventLinked"
    );
    assert_eq!(
        list_places(workspace).await.expect("places").len(),
        1,
        "PLAC place created"
    );
    assert_eq!(
        list_sources(workspace).await.expect("sources").len(),
        1,
        "SOUR source created"
    );
    assert_eq!(
        list_citations(workspace).await.expect("citations").len(),
        1,
        "SOUR citation created"
    );
    let media = list_media(workspace).await.expect("media");
    assert_eq!(media.len(), 1, "OBJE media created");
    assert_eq!(
        media[0].mime.as_deref(),
        Some("image/jpeg"),
        "OBJE.FILE.FORM imported as the media MIME"
    );
    assert_eq!(
        list_notes(workspace).await.expect("notes").len(),
        1,
        "NOTE note created"
    );
}

#[tokio::test]
async fn gedcom_imports_with_software_provenance_then_round_trips() {
    let host = common::host();
    let importer = common::component("gedcom-import");
    let exporter = common::component("gedcom-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE.as_bytes());

    // 1. Import the sample GEDCOM from the host-opened source, collecting progress.
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (progress, record) = progress_collector();
    let (count, workspace) = host
        .run_bulk_import(&importer, invocation(workspace, import_grants()), source, record)
        .await
        .expect("import");
    assert_eq!(count, 4, "3 individuals + 1 family");
    assert!(
        !progress.lock().expect("progress").is_empty(),
        "the import reports progress (ADR 0013)"
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
            vec![(
                "I0003".to_owned(),
                vec![
                    ("I0001".to_owned(), ChildParentRelationship::Birth),
                    ("I0002".to_owned(), ChildParentRelationship::Adopted),
                ],
            )],
        )]
    );

    // 2b. The richer GEDCOM 7 records (sex, event, place, source, citation) imported.
    assert_sample_breadth(&workspace).await;

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
            invocation(workspace, export_grants()),
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
        .run_bulk_import(&importer, invocation(workspace2, import_grants()), exported, record)
        .await
        .expect("re-import");
    assert_eq!(count2, 4);
    assert_eq!(
        snapshot(&workspace2).await,
        original,
        "round-trip must preserve persons and families"
    );
    // The owner-linked source, citation, media, and note survived the round-trip (ADR 0018):
    // gedcom-export now emits INDI.SOUR/OBJE/NOTE from the projected attachments.
    assert_sample_breadth(&workspace2).await;
}

#[tokio::test]
async fn re_importing_the_same_file_into_one_workspace_emits_no_new_events() {
    let host = common::host();
    let importer = common::component("gedcom-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE_WITH_UID.as_bytes());

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;

    // First import populates the workspace.
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            source.clone(),
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("first import");
    assert_eq!(count, 4, "3 individuals + 1 family");
    let first_snapshot = snapshot(&workspace).await;
    let events_after_first = event_count(&root).await;
    assert!(events_after_first > 0, "the first import recorded events");
    // The birth + marriage events, place, source, and citation were created on first import.
    assert_eq!(list_events(&workspace).await.expect("events").len(), 2, "two events");
    assert_eq!(list_places(&workspace).await.expect("places").len(), 1, "one place");
    assert_eq!(list_sources(&workspace).await.expect("sources").len(), 1, "one source");
    assert_eq!(
        list_citations(&workspace).await.expect("citations").len(),
        1,
        "one citation"
    );
    let media = list_media(&workspace).await.expect("media");
    assert_eq!(media.len(), 1, "one media");
    assert_eq!(
        media[0].mime.as_deref(),
        Some("image/jpeg"),
        "media MIME survived the export → re-import round-trip (OBJE.FILE.FORM)"
    );
    assert_eq!(list_notes(&workspace).await.expect("notes").len(), 1, "one note");

    // Re-import the identical file into the SAME workspace: every record resolves to its existing
    // aggregate, so no new events are written and the projection is unchanged.
    let (_, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
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
    // The owned events and place were not duplicated (created only on first import).
    assert_eq!(
        list_events(&workspace).await.expect("events").len(),
        2,
        "events not duplicated"
    );
    assert_eq!(
        list_places(&workspace).await.expect("places").len(),
        1,
        "place not duplicated"
    );
    assert_eq!(
        list_sources(&workspace).await.expect("sources").len(),
        1,
        "source not duplicated"
    );
    assert_eq!(
        list_citations(&workspace).await.expect("citations").len(),
        1,
        "citation not duplicated"
    );
    assert_eq!(
        list_media(&workspace).await.expect("media").len(),
        1,
        "media not duplicated"
    );
    assert_eq!(
        list_notes(&workspace).await.expect("notes").len(),
        1,
        "note not duplicated"
    );
}

/// Exercises the round-trip-gap group (PR4 Step A): a top-level `REPO` linked from `SOUR.REPO`,
/// `FAM`-level `SOUR`/`OBJE`/`NOTE`, and an `OBJE.CAPT` caption.
const REPO_AND_FAM_ATTACHMENTS: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
0 @I2@ INDI
1 NAME Jane /Doe/
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 SOUR @S1@
2 PAGE p. 2
1 OBJE
2 FILE https://example.test/marriage.jpg
2 CAPT The wedding day
1 NOTE A family note.
0 @S1@ SOUR
1 TITL Parish register
1 REPO @R1@
0 @R1@ REPO
1 NAME National Archive
0 TRLR
";

#[tokio::test]
async fn gedcom_imports_repositories_fam_level_attachments_and_media_caption() {
    let host = common::host();
    let importer = common::component("gedcom-import");
    let exporter = common::component("gedcom-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", REPO_AND_FAM_ATTACHMENTS.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("import");
    assert_eq!(count, 3, "2 individuals + 1 family");

    let repositories = list_repositories(&workspace).await.expect("repositories");
    assert_eq!(repositories.len(), 1);
    assert_eq!(repositories[0].name.as_deref(), Some("National Archive"));

    let sources = list_sources(&workspace).await.expect("sources");
    assert_eq!(sources[0].repositories.len(), 1, "the source links to its repository");
    assert_eq!(
        sources[0].repositories[0].name.as_deref(),
        Some("National Archive"),
        "SOUR.REPO round-trips into the app's Source→Repository link"
    );

    let families = list_families(&workspace).await.expect("families");
    let family = &families[0];
    assert_eq!(family.citations.len(), 1, "FAM.SOUR attached");
    assert_eq!(family.media.len(), 1, "FAM.OBJE attached");
    assert_eq!(
        family.media[0].caption.as_deref(),
        Some("The wedding day"),
        "OBJE.CAPT carries through to the attached media's caption"
    );
    assert_eq!(family.notes.len(), 1, "FAM.NOTE attached");

    // The caption now survives the export→re-import leg too (STEP C item 2: `media-ref` carries it
    // out through `family-dto.media`, replacing the exporter's previous always-`None` caption).
    let exported = io_dir.path().join("out.ged");
    let (_, workspace) = host
        .run_bulk_export(
            &exporter,
            invocation(workspace, export_grants()),
            ExportTarget::File(exported.clone()),
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("export");
    drop(workspace);

    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, workspace2) = host
        .run_bulk_import(
            &importer,
            invocation(workspace2, import_grants()),
            exported,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("re-import");

    let families2 = list_families(&workspace2).await.expect("families");
    assert_eq!(
        families2[0].media[0].caption.as_deref(),
        Some("The wedding day"),
        "OBJE.CAPT survived export and re-import, not just the first import"
    );

    // The SOUR.REPO link itself must also survive: `source-dto.repositories` had been threading the
    // repository's internal UUID (`id`) instead of its human id, so the re-exported REPO pointer
    // didn't match any REPO record's xref — an incidental pre-existing bug this broadened round-trip
    // coverage caught, fixed alongside (state.rs's `list_sources` repository mapping).
    let sources2 = list_sources(&workspace2).await.expect("sources");
    assert_eq!(
        sources2[0].repositories[0].name.as_deref(),
        Some("National Archive"),
        "SOUR.REPO survived export and re-import too"
    );
}

/// Exercises the round-trip-gap group (PR4 Step B): a top-level `SOUR.ABBR`, on a source cited by
/// an individual (a source is created lazily off its first citation).
const SOURCE_ABBREVIATION: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
1 SOUR @S1@
0 @S1@ SOUR
1 TITL Census 1801
1 ABBR 1801 Census
0 TRLR
";

#[tokio::test]
async fn gedcom_imports_a_source_abbreviation() {
    let host = common::host();
    let importer = common::component("gedcom-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SOURCE_ABBREVIATION.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("import");
    assert_eq!(count, 1, "1 individual");

    let sources = list_sources(&workspace).await.expect("sources");
    assert_eq!(
        sources[0].abbrev.as_deref(),
        Some("1801 Census"),
        "SOUR.ABBR round-trips into the app's Source.abbrev"
    );
}

/// Exercises the round-trip-gap group (PR4 Step C item 3, the last): a repository citation's call
/// number and medium (`SOUR.REPO.CALN`/`.MEDI`).
const REPOSITORY_CALL_NUMBER_AND_MEDIUM: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
1 SOUR @S1@
0 @S1@ SOUR
1 TITL Death certificate
1 REPO @R1@
2 CALN 6Mi5202
3 MEDI FILM
0 @R1@ REPO
1 NAME Country Archives of New York
0 TRLR
";

#[tokio::test]
async fn gedcom_imports_and_exports_a_repository_call_number_and_medium() {
    let host = common::host();
    let importer = common::component("gedcom-import");
    let exporter = common::component("gedcom-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", REPOSITORY_CALL_NUMBER_AND_MEDIUM.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("import");
    assert_eq!(count, 1, "1 individual");

    let sources = list_sources(&workspace).await.expect("sources");
    assert_eq!(
        sources[0].repositories[0].call_number.as_deref(),
        Some("6Mi5202"),
        "CALN imported"
    );
    assert_eq!(
        sources[0].repositories[0].media_type,
        vitni_app::SourceMediaType::Film,
        "MEDI imported"
    );

    // Export and re-import: both must survive the WIT `repository-ref` boundary too.
    let exported = io_dir.path().join("out.ged");
    let (_, workspace) = host
        .run_bulk_export(
            &exporter,
            invocation(workspace, export_grants()),
            ExportTarget::File(exported.clone()),
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("export");
    drop(workspace);

    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, workspace2) = host
        .run_bulk_import(
            &importer,
            invocation(workspace2, import_grants()),
            exported,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("re-import");

    let sources2 = list_sources(&workspace2).await.expect("sources");
    assert_eq!(
        sources2[0].repositories[0].call_number.as_deref(),
        Some("6Mi5202"),
        "CALN survived export and re-import"
    );
    assert_eq!(
        sources2[0].repositories[0].media_type,
        vitni_app::SourceMediaType::Film,
        "MEDI survived export and re-import"
    );
}

/// Exercises the round-trip-gap group (PR4 Step C item 1): a second `NAME` record must be kept, not
/// silently clobber the first.
const TWO_NAMES: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME Jane /Smith/
2 TYPE birth
1 NAME Jane /Doe/
2 TYPE married
0 TRLR
";

#[tokio::test]
async fn gedcom_imports_and_exports_a_second_name_without_clobbering_the_first() {
    let host = common::host();
    let importer = common::component("gedcom-import");
    let exporter = common::component("gedcom-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", TWO_NAMES.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("import");
    assert_eq!(count, 1, "1 individual");

    let persons = list_persons(&workspace).await.expect("list persons");
    assert_eq!(
        persons[0].names.len(),
        2,
        "both NAME records kept on import, not just the last"
    );

    // Export and re-import: the second name must survive the WIT `person-dto.names` boundary too.
    let exported = io_dir.path().join("out.ged");
    let (_, workspace) = host
        .run_bulk_export(
            &exporter,
            invocation(workspace, export_grants()),
            ExportTarget::File(exported.clone()),
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("export");
    drop(workspace);

    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, workspace2) = host
        .run_bulk_import(
            &importer,
            invocation(workspace2, import_grants()),
            exported,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("re-import");

    let persons = list_persons(&workspace2).await.expect("list persons");
    let names = &persons[0].names;
    assert_eq!(names.len(), 2, "both names survive export and re-import");
    assert_eq!(
        names[0].name.surnames.first().map(|s| s.surname.as_str()),
        Some("Smith")
    );
    assert_eq!(names[1].name.surnames.first().map(|s| s.surname.as_str()), Some("Doe"));
}

/// The `_UID` a re-import resolves the sample person by.
const RECONCILE_UID: &str = "1 _UID 11111111-1111-1111-1111-111111111111\n";

/// A minimal one-person GEDCOM document naming `sex` and, when `head_date` is given, a `HEAD.1 DATE`
/// (the file's own export date, ADR 0029 §2).
fn reconcile_doc(head_date: Option<&str>, sex: &str) -> String {
    let head_date_line = head_date.map(|date| format!("1 DATE {date}\n")).unwrap_or_default();
    format!(
        "0 HEAD\n1 SOUR test\n{head_date_line}0 @I1@ INDI\n1 NAME John /Smith/\n{RECONCILE_UID}1 SEX {sex}\n0 TRLR\n"
    )
}

#[tokio::test]
async fn reimport_reconciles_sex_only_when_the_files_export_date_is_at_least_as_recent() {
    let host = common::host();
    let importer = common::component("gedcom-import");
    let io_dir = tempfile::tempdir().expect("io dir");

    // 1. First import: no HEAD date, sex Male. The live sex assertion's `occurred_at` is "now".
    let source = write_file(io_dir.path(), "in1.ged", reconcile_doc(None, "M").as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("first import");
    let persons = list_persons(&workspace).await.expect("persons");
    assert_eq!(persons[0].sex, Some(vitni_app::Sex::Male), "first import asserts Male");

    // 2. Re-import the same person with a changed sex (F) and an OLDER export date: the file is
    // stale relative to the live assertion just made, so the workspace's value must not change
    // (ADR 0029 §1) — even though the person resolves to the existing aggregate (not created), and
    // `assert-sex` is now called on every re-import (ADR 0029's plugin control-flow change).
    let stale_source = write_file(
        io_dir.path(),
        "in2.ged",
        reconcile_doc(Some("1 JAN 2000"), "F").as_bytes(),
    );
    let (_, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            stale_source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("stale re-import");
    let persons = list_persons(&workspace).await.expect("persons");
    assert_eq!(
        persons[0].sex,
        Some(vitni_app::Sex::Male),
        "a stale (older) export date must not override the live sex"
    );

    // 3. Re-import the same changed sex (F) with a NEWER export date: the file is at least as
    // current as the live assertion, so the workspace's value is superseded (ADR 0029 §1).
    let fresh_source = write_file(
        io_dir.path(),
        "in3.ged",
        reconcile_doc(Some("1 JAN 2100"), "F").as_bytes(),
    );
    let (_, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            fresh_source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("fresh re-import");
    let persons = list_persons(&workspace).await.expect("persons");
    assert_eq!(
        persons[0].sex,
        Some(vitni_app::Sex::Female),
        "a fresher (newer) export date supersedes the live sex"
    );
}

#[tokio::test]
async fn rich_gedcom_imports_structured_name_dates_address_fact_and_association() {
    use vitni_app::{DateModifier, GenealogicalDateBody};

    let host = common::host();
    let importer = common::component("gedcom-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "rich.ged", RICH.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, record) = progress_collector();
    let (count, workspace) = host
        .run_bulk_import(&importer, invocation(workspace, import_grants()), source, record)
        .await
        .expect("import");
    assert_eq!(count, 2, "two individuals");

    // 1. The structured NAME sub-records landed on the projection.
    let persons = list_persons(&workspace).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(john.given.as_deref(), Some("Johnny"), "GIVN overrides the slash form");
    assert_eq!(
        john.surname.as_deref(),
        Some("Smithson"),
        "SURN overrides the slash form"
    );
    assert_eq!(john.surname_prefix.as_deref(), Some("van"), "SPFX");
    assert_eq!(john.nickname.as_deref(), Some("Jack"), "NICK");
    assert_eq!(john.name_prefix.as_deref(), Some("Dr"), "NPFX");
    assert_eq!(john.name_suffix.as_deref(), Some("Jr"), "NSFX");

    // 2. The birth event carries the `ABT 1850` modifier (the full date grammar).
    let events = list_events(&workspace).await.expect("events");
    assert_eq!(events.len(), 2, "BIRT + RESI");
    let birth = events
        .iter()
        .find(|e| e.event_type == Some(vitni_app::EventType::Birth))
        .expect("birth event");
    let modifier = match birth.date.as_ref().expect("birth date").modifier.clone() {
        GenealogicalDateBody::Structured(modifier) => modifier,
        GenealogicalDateBody::TextOnly { text } => panic!("expected a structured date, got {text:?}"),
    };
    assert!(
        matches!(modifier, DateModifier::About(point) if point.year == Some(1850)),
        "ABT 1850 parsed as About(1850), got {modifier:?}"
    );

    // 3. The address, fact, and association were recorded as their respective events.
    let payloads = event_payloads(&root).await;
    assert!(
        payloads.iter().any(|p| p.contains("AddressAdded")),
        "RESI ADDR → AddressAdded"
    );
    assert!(
        payloads.iter().any(|p| p.contains("FactAsserted")),
        "OCCU → FactAsserted"
    );
    assert!(
        payloads.iter().any(|p| p.contains("AssociationAsserted")),
        "ASSO → AssociationAsserted"
    );
}

#[tokio::test]
async fn rich_gedcom_round_trips_structured_name_date_address_fact_and_association_through_export() {
    use vitni_app::{DateModifier, GenealogicalDateBody};

    let host = common::host();
    let importer = common::component("gedcom-import");
    let exporter = common::component("gedcom-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "rich.ged", RICH.as_bytes());

    // 1. Import the rich fixture.
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, record) = progress_collector();
    let (_, workspace) = host
        .run_bulk_import(&importer, invocation(workspace, import_grants()), source, record)
        .await
        .expect("import");

    // 2. Export to a host-resolved file.
    let exported = io_dir.path().join("rich-out.ged");
    let (_, record) = progress_collector();
    let (_, workspace) = host
        .run_bulk_export(
            &exporter,
            invocation(workspace, export_grants()),
            ExportTarget::File(exported.clone()),
            record,
        )
        .await
        .expect("export");
    drop(workspace);

    // 3. Re-import the exported document into a fresh workspace.
    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, record) = progress_collector();
    let (_, workspace2) = host
        .run_bulk_import(&importer, invocation(workspace2, import_grants()), exported, record)
        .await
        .expect("re-import");

    // 4. The structured name, sex, and dated event survived the round-trip.
    let persons = list_persons(&workspace2).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(john.given.as_deref(), Some("Johnny"), "GIVN round-tripped");
    assert_eq!(john.surname.as_deref(), Some("Smithson"), "SURN round-tripped");
    assert_eq!(john.surname_prefix.as_deref(), Some("van"), "SPFX round-tripped");
    assert_eq!(john.nickname.as_deref(), Some("Jack"), "NICK round-tripped");
    assert_eq!(john.name_prefix.as_deref(), Some("Dr"), "NPFX round-tripped");
    assert_eq!(john.name_suffix.as_deref(), Some("Jr"), "NSFX round-tripped");
    assert_eq!(john.sex, Some(vitni_app::Sex::Male), "SEX round-tripped");

    let events = list_events(&workspace2).await.expect("events");
    assert_eq!(events.len(), 2, "BIRT + RESI round-tripped");
    let birth = events
        .iter()
        .find(|e| e.event_type == Some(vitni_app::EventType::Birth))
        .expect("birth event");
    let modifier = match birth.date.as_ref().expect("birth date").modifier.clone() {
        GenealogicalDateBody::Structured(modifier) => modifier,
        GenealogicalDateBody::TextOnly { text } => panic!("expected a structured date, got {text:?}"),
    };
    assert!(
        matches!(modifier, DateModifier::About(point) if point.year == Some(1850)),
        "ABT 1850 round-tripped as About(1850), got {modifier:?}"
    );

    // 5. The address, fact, and association survived as their respective events.
    let payloads = event_payloads(&root2).await;
    assert!(
        payloads.iter().any(|p| p.contains("AddressAdded")),
        "RESI ADDR round-tripped to AddressAdded"
    );
    assert!(
        payloads.iter().any(|p| p.contains("FactAsserted")),
        "OCCU round-tripped to FactAsserted"
    );
    assert!(
        payloads.iter().any(|p| p.contains("AssociationAsserted")),
        "ASSO round-tripped to AssociationAsserted"
    );
}

#[tokio::test]
async fn gedcom_imports_participation_age_and_event_witness() {
    let host = common::host();
    let importer = common::component("gedcom-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "witness.ged", WITNESS_AND_AGES.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, record) = progress_collector();
    let (_, workspace) = host
        .run_bulk_import(&importer, invocation(workspace, import_grants()), source, record)
        .await
        .expect("import");

    let persons = list_persons(&workspace).await.expect("list persons");

    // John (I0001) is the census participant (age 45y) and a marriage partner (age 25y).
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    let john_ages: Vec<u16> = john
        .participations
        .iter()
        .filter_map(|p| p.age.as_ref().and_then(|age| age.years))
        .collect();
    assert!(john_ages.contains(&45), "census AGE 45y imported, got {john_ages:?}");
    assert!(john_ages.contains(&25), "HUSB AGE 25y imported, got {john_ages:?}");

    // Pat (I0002) is only the census witness: role Witness, one note, one backing citation.
    let pat = persons.iter().find(|p| p.human_id == "I0002").expect("I0002");
    assert_eq!(pat.participations.len(), 1, "witness has one participation");
    let witness = &pat.participations[0];
    assert_eq!(witness.role, ParticipantRole::Witness, "event ASSO ROLE WITN → Witness");
    assert_eq!(witness.notes.len(), 1, "the ASSO NOTE imported as a participation note");
    assert_eq!(
        witness.source_count, 1,
        "the ASSO SOUR imported to the assertion envelope"
    );

    // Jane (I0003) is the wife: age `< 24y 6m`.
    let jane = persons.iter().find(|p| p.human_id == "I0003").expect("I0003");
    let jane_age = jane
        .participations
        .iter()
        .find_map(|p| p.age.as_ref())
        .expect("wife age");
    assert_eq!(jane_age.bound, Some(AgeBound::LessThan), "WIFE AGE < bound");
    assert_eq!(jane_age.years, Some(24));
    assert_eq!(jane_age.months, Some(6));
}

#[tokio::test]
async fn participation_age_and_witness_round_trip_through_export() {
    let host = common::host();
    let importer = common::component("gedcom-import");
    let exporter = common::component("gedcom-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "witness.ged", WITNESS_AND_AGES.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, record) = progress_collector();
    let (_, workspace) = host
        .run_bulk_import(&importer, invocation(workspace, import_grants()), source, record)
        .await
        .expect("import");

    // Export and check the serialized text carries the participant ages and the event witness.
    let exported = io_dir.path().join("witness-out.ged");
    let (_, record) = progress_collector();
    let (_, workspace) = host
        .run_bulk_export(
            &exporter,
            invocation(workspace, export_grants()),
            ExportTarget::File(exported.clone()),
            record,
        )
        .await
        .expect("export");
    drop(workspace);
    let text = String::from_utf8(std::fs::read(&exported).expect("read export")).expect("utf8");
    assert!(text.contains("2 AGE 45y"), "census AGE emitted: {text}");
    assert!(text.contains("2 ASSO @I0002@"), "event witness emitted as ASSO: {text}");
    assert!(text.contains("3 ROLE WITN"), "witness role emitted: {text}");
    assert!(text.contains("3 AGE 25y"), "HUSB AGE emitted: {text}");

    // Re-import: the ages and the witness survive; the witness's source count does not (participation
    // citations are import-only — they ride the envelope but are not re-emitted, ADR 0019/0020).
    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, record) = progress_collector();
    let (_, workspace2) = host
        .run_bulk_import(&importer, invocation(workspace2, import_grants()), exported, record)
        .await
        .expect("re-import");

    let persons = list_persons(&workspace2).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    let john_ages: Vec<u16> = john
        .participations
        .iter()
        .filter_map(|p| p.age.as_ref().and_then(|age| age.years))
        .collect();
    assert!(
        john_ages.contains(&45) && john_ages.contains(&25),
        "ages survived, got {john_ages:?}"
    );
    let pat = persons.iter().find(|p| p.human_id == "I0002").expect("I0002");
    let witness = pat.participations.first().expect("witness participation");
    assert_eq!(witness.role, ParticipantRole::Witness, "witness role survived");
    assert_eq!(witness.notes.len(), 1, "witness note survived");
    assert_eq!(
        witness.source_count, 0,
        "witness source count is not re-exported (one-way)"
    );
}

#[tokio::test]
async fn import_is_denied_without_the_commands_capability() {
    let host = common::host();
    let importer = common::component("gedcom-import");

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
        .run_bulk_import(&importer, invocation(workspace, grants), source, record)
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
    let host = common::host();
    let importer = common::component("gedcom-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE.as_bytes());

    // Cancel at the first progress report: the importer should stop after the first person.
    let cancel_after_first = |_: ProgressUpdate| ProgressControl::Cancel;

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
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

/// The GUI's default destination is a directory (`<workspace>/exports`), not a pinned file: the host
/// resolves the leaf from the plugin's suggested name, and the document must land there and re-import
/// unchanged. Every other export test pins an `ExportTarget::File`, so this is the only cover for the
/// directory path.
#[tokio::test]
async fn export_to_a_directory_lands_under_the_plugins_suggested_name() {
    let host = common::host();
    let importer = common::component("gedcom-import");
    let exporter = common::component("gedcom-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.ged", SAMPLE.as_bytes());
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
            source,
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("import");
    let original = snapshot(&workspace).await;

    // The wizard hands the host a directory that does not exist yet, exactly as `<workspace>/exports`
    // does on a fresh workspace.
    let out_dir = io_dir.path().join("exports");
    let (log, record) = progress_collector();
    let (count, workspace) = host
        .run_bulk_export(
            &exporter,
            invocation(workspace, export_grants()),
            ExportTarget::Directory(out_dir.clone()),
            record,
        )
        .await
        .expect("export to a directory");
    drop(workspace);
    assert_eq!(count, 4, "3 individuals + 1 family exported");
    assert!(
        !log.lock().expect("progress lock").is_empty(),
        "a directory export reports progress like a file export"
    );

    // The plugin's suggested name decides the leaf; nothing else is written into the directory.
    let written: Vec<PathBuf> = std::fs::read_dir(&out_dir)
        .expect("the host created the export directory")
        .map(|entry| entry.expect("dir entry").path())
        .collect();
    assert_eq!(written, vec![out_dir.join("export.ged")], "one file, plugin-named");

    // The document itself is a real export: it re-imports to the same structure.
    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (recount, workspace2) = host
        .run_bulk_import(
            &importer,
            invocation(workspace2, import_grants()),
            out_dir.join("export.ged"),
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("re-import");
    assert_eq!(recount, 4);
    assert_eq!(
        snapshot(&workspace2).await,
        original,
        "a directory export round-trips like a file export"
    );
}
