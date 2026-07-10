//! Gramps XML round-trip integration test (Phase 4 group G, ADR 0018): a Gramps XML document imports
//! as persons and a family with Software-agent provenance through the streaming bulk-import world,
//! re-exports through the bulk-export world, and re-imports identically — and the owner-linked
//! records (events, places, sources, citations, notes) it carries survive the cycle.
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use genealogy_app::{
    AppDefaults, OperatorConfig, ParticipantRole, PersonSummary, Session, Workspace, WorkspaceDefaults, list_citations,
    list_events, list_families, list_media, list_notes, list_persons, list_places, list_sources,
};
use genealogy_core::ids::AgentId;
use genealogy_plugin_host::{
    Capability, ExportTarget, Grants, Invocation, PluginHost, ProgressControl, ProgressUpdate, ResourceBudget,
};
use uuid::Uuid;

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<gender>M</gender>
<name><first>John</first><surname>Smith</surname></name>
<eventref hlink="_e1"/>
<citationref hlink="_c1"/>
<noteref hlink="_n1"/>
<objref hlink="_o1"/>
</person>
<person handle="_p2" id="I0002">
<gender>F</gender>
<name><first>Jane</first><surname>Doe</surname></name>
</person>
<person handle="_p3" id="I0003">
<gender>U</gender>
<name><first>Sam</first><surname>Smith</surname></name>
</person>
</people>
<families>
<family handle="_f1" id="F0001">
<father hlink="_p1"/>
<mother hlink="_p2"/>
<childref hlink="_p3" mrel="Birth" frel="Adopted"/>
<eventref hlink="_e2"/>
</family>
</families>
<events>
<event handle="_e1" id="E0001"><type>Birth</type><dateval val="1850"/><place hlink="_pl1"/></event>
<event handle="_e2" id="E0002"><type>Marriage</type><dateval val="1848"/></event>
</events>
<places>
<placeobj handle="_pl1" id="P0001" type="City"><pname value="Bergen"/></placeobj>
</places>
<sources>
<source handle="_s1" id="S0001"><stitle>Census 1801</stitle></source>
</sources>
<citations>
<citation handle="_c1" id="C0001"><page>p. 5</page><confidence>2</confidence><sourceref hlink="_s1"/></citation>
</citations>
<notes>
<note handle="_n1" id="N0001"><text>A research note.</text></note>
</notes>
<objects>
<object handle="_o1" id="O0001"><file src="https://example.test/photo.jpg" mime="image/jpeg"/></object>
</objects>
</database>
"#;

/// Exercises the participation payload (ADR 0019): a witness whose person-side `<eventref>` carries a
/// `role`, an `"Age"` attribute, another attribute, and a `<noteref>` — data Gramps round-trips
/// (unlike GEDCOM, which has no slot for participation attributes).
const WITNESS_PAYLOAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<gender>M</gender>
<name><first>John</first><surname>Smith</surname></name>
<eventref hlink="_e1"/>
</person>
<person handle="_p2" id="I0002">
<gender>F</gender>
<name><first>Pat</first><surname>Vitne</surname></name>
<eventref hlink="_e1" role="Witness">
<attribute type="Age" value="45y"/>
<attribute type="Occupation" value="Clerk"/>
<noteref hlink="_n1"/>
</eventref>
</person>
</people>
<events>
<event handle="_e1" id="E0001"><type>Census</type><dateval val="1900"/></event>
</events>
<notes>
<note handle="_n1" id="N0001"><text>Witness note.</text></note>
</notes>
</database>
"#;

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn software_session() -> Session {
    Session::software("genealogy-gramps-import", "0.1.0")
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

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write file");
    path
}

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
        .map(|p: PersonSummary| (p.human_id, p.given, p.surname))
        .collect();
    let families = list_families(workspace)
        .await
        .expect("list families")
        .into_iter()
        .map(|f| {
            let partners = f.partners.into_iter().map(|p| p.human_id).collect();
            let children = f.children.into_iter().map(|c| c.human_id).collect();
            (f.human_id, partners, children)
        })
        .collect();
    Snapshot { persons, families }
}

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

/// Asserts the owner-linked breadth the `SAMPLE` import produces.
async fn assert_breadth(workspace: &Workspace) {
    let persons = list_persons(workspace).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(john.sex, Some(genealogy_app::Sex::Male), "gender M imported");
    assert_eq!(
        john.citations.len(),
        1,
        "INDI citation attached and round-tripped to the read DTO"
    );
    assert_eq!(john.notes.len(), 1, "INDI note attached");
    assert_eq!(
        list_events(workspace).await.expect("events").len(),
        2,
        "birth + marriage events created"
    );
    assert_eq!(list_places(workspace).await.expect("places").len(), 1, "place created");
    assert_eq!(
        list_sources(workspace).await.expect("sources").len(),
        1,
        "source created"
    );
    assert_eq!(
        list_citations(workspace).await.expect("citations").len(),
        1,
        "citation created"
    );
    assert_eq!(list_notes(workspace).await.expect("notes").len(), 1, "note created");
    let families = list_families(workspace).await.expect("families");
    let family = families.first().expect("one family");
    let child = family.children.first().expect("one child");
    let mut relationships: Vec<_> = child.relationships.iter().map(|(_, rel)| rel.clone()).collect();
    relationships.sort_by_key(|rel| format!("{rel:?}"));
    assert_eq!(
        relationships,
        vec![
            genealogy_app::ChildParentRelationship::Adopted,
            genealogy_app::ChildParentRelationship::Birth,
        ],
        "childref frel/mrel round-tripped into per-partner relationships"
    );
    assert_eq!(
        family.events.len(),
        1,
        "the family <eventref> round-tripped as an explicit FamilyEventLinked"
    );
    assert_eq!(john.media.len(), 1, "INDI media attached");
    let media = list_media(workspace).await.expect("media");
    assert_eq!(media.len(), 1, "object media created");
    assert_eq!(
        media[0].mime.as_deref(),
        Some("image/jpeg"),
        "<file mime> imported as the media MIME"
    );
}

#[tokio::test]
async fn gramps_imports_with_software_provenance_then_round_trips() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gramps-import")).expect("load import");
    let exporter = host.load(&plugin_path("gramps-export")).expect("load export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", SAMPLE.as_bytes());

    // 1. Import the sample.
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
    assert_eq!(count, 4, "3 people + 1 family");
    assert!(
        !progress.lock().expect("progress").is_empty(),
        "the import reports progress"
    );

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
    assert_breadth(&workspace).await;
    assert!(
        has_software_provenance(&root).await,
        "imported events carry Software provenance"
    );

    // 2. Export to a host-resolved file.
    let exported = io_dir.path().join("out.gramps");
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
    assert_eq!(exported_count, 4);
    let bytes = std::fs::read(&exported).expect("read exported document");
    assert!(!bytes.is_empty(), "export produced a document");

    // 3. Re-import the exported document into a fresh workspace — structure is identical, and the
    // owner-linked records survived the cycle.
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
        "round-trip preserves persons and families"
    );
    assert_breadth(&workspace2).await;
}

#[tokio::test]
async fn re_importing_the_same_gramps_file_emits_no_new_events() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gramps-import")).expect("load import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", SAMPLE.as_bytes());

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
            source.clone(),
            |_: ProgressUpdate| ProgressControl::Proceed,
        )
        .await
        .expect("first import");
    assert_eq!(count, 4);
    let first = snapshot(&workspace).await;
    let events_after_first = event_count(&root).await;
    drop(workspace);

    let (_, workspace) = host
        .run_bulk_import(
            &importer,
            Invocation {
                workspace: open_workspace(&root).await,
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
        "re-import emits no new events"
    );
    assert_eq!(
        snapshot(&workspace).await,
        first,
        "re-import does not change the projection"
    );
}

#[tokio::test]
async fn gramps_round_trips_eventref_role_age_attributes_and_note() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gramps-import")).expect("load import");
    let exporter = host.load(&plugin_path("gramps-export")).expect("load export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "witness.gramps", WITNESS_PAYLOAD.as_bytes());

    // 1. Import and assert the witness participation payload landed.
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, record) = progress_collector();
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
            record,
        )
        .await
        .expect("import");
    assert_witness_payload(&workspace).await;

    // 2. Export and re-import into a fresh workspace.
    let exported = io_dir.path().join("witness-out.gramps");
    let (_, record) = progress_collector();
    let (_, workspace) = host
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

    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, record) = progress_collector();
    let (_, workspace2) = host
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

    // 3. The role, age, attribute, and note survived the round-trip (Gramps carries all of them).
    assert_witness_payload(&workspace2).await;
}

/// Asserts the `WITNESS_PAYLOAD` participation: I0001 a plain census primary, I0002 a witness with
/// age 45y, the `Occupation` attribute, and one note.
async fn assert_witness_payload(workspace: &Workspace) {
    let persons = list_persons(workspace).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    let john_census = john.participations.first().expect("primary participation");
    assert_eq!(
        john_census.role,
        ParticipantRole::Primary,
        "plain participant is primary"
    );
    assert!(john_census.age.is_none(), "the primary carries no age");

    let pat = persons.iter().find(|p| p.human_id == "I0002").expect("I0002");
    let witness = pat.participations.first().expect("witness participation");
    assert_eq!(witness.role, ParticipantRole::Witness, "eventref role Witness");
    assert_eq!(
        witness.age.as_ref().and_then(|age| age.years),
        Some(45),
        "the Age eventref attribute"
    );
    assert!(
        witness
            .attributes
            .iter()
            .any(|attribute| attribute.attribute_type == "Occupation" && attribute.value == "Clerk"),
        "the Occupation eventref attribute round-tripped, got {:?}",
        witness.attributes
    );
    assert_eq!(witness.notes.len(), 1, "the eventref noteref round-tripped");
}

#[tokio::test]
async fn import_is_denied_without_the_commands_capability() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gramps-import")).expect("load import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", SAMPLE.as_bytes());

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
