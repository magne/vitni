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
    AiConfig, AppDefaults, OperatorConfig, ParticipantRole, PersonSummary, Session, Workspace, WorkspaceDefaults,
    list_citations, list_events, list_families, list_media, list_notes, list_persons, list_places, list_sources,
    list_tags,
};
use genealogy_core::ids::AgentId;
use genealogy_plugin_host::{
    Capability, ExportTarget, Grants, Invocation, NetPolicy, ProgressControl, ProgressUpdate, ResourceBudget,
};
use uuid::Uuid;

mod common;

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<gender>M</gender>
<name><first>John</first><surname>Smith</surname></name>
<eventref hlink="_e1"/>
<citationref hlink="_c1"/>
<noteref hlink="_n1"/>
<objref hlink="_o1"><region corner1_x="10" corner1_y="20" corner2_x="40" corner2_y="60"/></objref>
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
    let mut relationships: Vec<_> = child
        .relationships
        .iter()
        .map(|link| link.relationship.clone())
        .collect();
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
    let host = common::host();
    let importer = common::component("gramps-import");
    let exporter = common::component("gramps-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", SAMPLE.as_bytes());

    // 1. Import the sample.
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (progress, record) = progress_collector();
    let (count, workspace) = host
        .run_bulk_import(&importer, invocation(workspace, import_grants()), source, record)
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
    // The `<objref>` `<region>` crop plumbs through attach-person-media (WIT 0.18.0) to the
    // projection on import.
    let persons = list_persons(&workspace).await.expect("list persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(
        john.media.first().expect("one media ref").crop,
        Some(genealogy_app::Rect {
            left: 10,
            top: 20,
            width: 30,
            height: 40,
        }),
        "the <region> crop reached the projection"
    );
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
            invocation(workspace, export_grants()),
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
        .run_bulk_import(&importer, invocation(workspace2, import_grants()), exported, record)
        .await
        .expect("re-import");
    assert_eq!(count2, 4);
    assert_eq!(
        snapshot(&workspace2).await,
        original,
        "round-trip preserves persons and families"
    );
    assert_breadth(&workspace2).await;

    // The crop now survives the export→re-import leg too (STEP C item 2: `media-ref` carries it
    // out through `person-dto.media`, and `gramps-export` reconstructs a real `<region>`).
    let persons2 = list_persons(&workspace2).await.expect("list persons");
    let john2 = persons2.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(
        john2.media.first().expect("one media ref").crop,
        Some(genealogy_app::Rect {
            left: 10,
            top: 20,
            width: 30,
            height: 40,
        }),
        "the <region> crop survived export and re-import, not just the first import"
    );
}

#[tokio::test]
async fn re_importing_the_same_gramps_file_emits_no_new_events() {
    let host = common::host();
    let importer = common::component("gramps-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", SAMPLE.as_bytes());

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_bulk_import(
            &importer,
            invocation(workspace, import_grants()),
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
            invocation(open_workspace(&root).await, import_grants()),
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
    let host = common::host();
    let importer = common::component("gramps-import");
    let exporter = common::component("gramps-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "witness.gramps", WITNESS_PAYLOAD.as_bytes());

    // 1. Import and assert the witness participation payload landed.
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (_, record) = progress_collector();
    let (_, workspace) = host
        .run_bulk_import(&importer, invocation(workspace, import_grants()), source, record)
        .await
        .expect("import");
    assert_witness_payload(&workspace).await;

    // 2. Export and re-import into a fresh workspace.
    let exported = io_dir.path().join("witness-out.gramps");
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

    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (_, record) = progress_collector();
    let (_, workspace2) = host
        .run_bulk_import(&importer, invocation(workspace2, import_grants()), exported, record)
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

/// A tag applied to a person and a family (PR4 Step A: Gramps `<tagref>`).
const TAGGED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<name><first>John</first><surname>Smith</surname></name>
<tagref hlink="_t1"/>
</person>
<person handle="_p2" id="I0002">
<name><first>Jane</first><surname>Doe</surname></name>
</person>
</people>
<families>
<family handle="_f1" id="F0001">
<father hlink="_p1"/>
<mother hlink="_p2"/>
<tagref hlink="_t1"/>
</family>
</families>
<tags>
<tag handle="_t1" name="Direct line"/>
</tags>
</database>
"#;

#[tokio::test]
async fn gramps_imports_a_tag_applied_to_a_person_and_a_family() {
    let host = common::host();
    let importer = common::component("gramps-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", TAGGED.as_bytes());
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
    assert_eq!(count, 3, "2 people + 1 family");

    let tags = list_tags(&workspace).await.expect("tags");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name.as_deref(), Some("Direct line"));

    let persons = list_persons(&workspace).await.expect("persons");
    let john = persons.iter().find(|p| p.human_id == "I0001").expect("I0001");
    assert_eq!(
        john.tags,
        vec![tags[0].id.clone()],
        "the person-side <tagref> applied the tag"
    );

    let families = list_families(&workspace).await.expect("families");
    assert_eq!(families[0].tags.len(), 1, "the family-side <tagref> applied the tag");
    assert_eq!(families[0].tags[0].name, "Direct line");
}

/// A source's `<sabbrev>` (PR4 Step B), on a source cited by a person (a source is created lazily
/// off its first citation).
const SOURCE_ABBREVIATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<name><first>John</first><surname>Smith</surname></name>
<citationref hlink="_c1"/>
</person>
</people>
<sources>
<source handle="_s1" id="S0001"><stitle>Census 1801</stitle><sabbrev>1801 Census</sabbrev></source>
</sources>
<citations>
<citation handle="_c1" id="C0001"><sourceref hlink="_s1"/></citation>
</citations>
</database>
"#;

#[tokio::test]
async fn gramps_imports_a_source_abbreviation() {
    let host = common::host();
    let importer = common::component("gramps-import");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", SOURCE_ABBREVIATION.as_bytes());
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
    assert_eq!(count, 1, "1 person");

    let sources = list_sources(&workspace).await.expect("sources");
    assert_eq!(
        sources[0].abbrev.as_deref(),
        Some("1801 Census"),
        "<sabbrev> round-trips into the app's Source.abbrev"
    );
}

/// A `<reporef>` call number and medium (PR4 Step C item 3, the last).
const REPOSITORY_CALL_NUMBER_AND_MEDIUM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<name><first>John</first><surname>Smith</surname></name>
<citationref hlink="_c1"/>
</person>
</people>
<sources>
<source handle="_s1" id="S0001">
<stitle>Death certificate</stitle>
<reporef hlink="_r1" callno="6Mi5202" medium="Film"/>
</source>
</sources>
<citations>
<citation handle="_c1" id="C0001"><sourceref hlink="_s1"/></citation>
</citations>
<repositories>
<repository handle="_r1" id="R0001"><rname>Country Archives of New York</rname></repository>
</repositories>
</database>
"#;

#[tokio::test]
async fn gramps_imports_and_exports_a_repository_call_number_and_medium() {
    let host = common::host();
    let importer = common::component("gramps-import");
    let exporter = common::component("gramps-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", REPOSITORY_CALL_NUMBER_AND_MEDIUM.as_bytes());
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
    assert_eq!(count, 1, "1 person");

    let sources = list_sources(&workspace).await.expect("sources");
    assert_eq!(
        sources[0].repositories[0].call_number.as_deref(),
        Some("6Mi5202"),
        "callno imported"
    );
    assert_eq!(
        sources[0].repositories[0].media_type,
        genealogy_app::SourceMediaType::Film,
        "medium imported"
    );

    // Export and re-import: both must survive the WIT `repository-ref` boundary too.
    let exported = io_dir.path().join("out.gramps");
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
        "callno survived export and re-import"
    );
    assert_eq!(
        sources2[0].repositories[0].media_type,
        genealogy_app::SourceMediaType::Film,
        "medium survived export and re-import"
    );
}

/// A second `<name>` (PR4 Step C item 1): must be kept, not silently clobber the first.
const TWO_NAMES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<name><first>Jane</first><surname>Smith</surname></name>
<name alt="1"><first>Jane</first><surname>Doe</surname></name>
</person>
</people>
</database>
"#;

#[tokio::test]
async fn gramps_imports_and_exports_a_second_name_without_clobbering_the_first() {
    let host = common::host();
    let importer = common::component("gramps-import");
    let exporter = common::component("gramps-export");

    let io_dir = tempfile::tempdir().expect("io dir");
    let source = write_file(io_dir.path(), "in.gramps", TWO_NAMES.as_bytes());
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
    assert_eq!(count, 1, "1 person");

    let persons = list_persons(&workspace).await.expect("list persons");
    assert_eq!(
        persons[0].names.len(),
        2,
        "both <name> elements kept on import, not just the last"
    );

    // Export and re-import: the second name must survive the WIT `person-dto.names` boundary too.
    let exported = io_dir.path().join("out.gramps");
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

#[tokio::test]
async fn import_is_denied_without_the_commands_capability() {
    let host = common::host();
    let importer = common::component("gramps-import");

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
        .run_bulk_import(&importer, invocation(workspace, grants), source, record)
        .await;

    assert!(result.is_err(), "import without the commands grant must fail");
    let workspace = open_workspace(&root).await;
    assert!(
        list_persons(&workspace).await.expect("list").is_empty(),
        "a denied import must not have created any person"
    );
}
