//! Import runs and record origins (ADR 0037 §1, §5): an import with a run spec writes one run
//! operated by the invoking human, and every assertion the importer writes names the record it came
//! from and the run that wrote it.
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use uuid::Uuid;
use vitni_app::{
    AbandonReason, AgentKind, AiConfig, AppDefaults, ChosenDataset, DatasetChoice, DatasetError, DatasetId,
    DatasetProposal, DatasetScope, DatasetSpec, ImportRunStatus, ImportRunSummary, OperatorConfig, Session, Workspace,
    WorkspaceDefaults, list_import_runs, undo_assertion, workspace_counts,
};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, EventContext};
use vitni_plugin_host::{
    Capability, ExportTarget, Grants, ImportRunSpec, Invocation, NetPolicy, PluginError, ProgressControl, ProgressStep,
    ProgressUpdate, ResourceBudget, RunDataset,
};

mod common;

const GEDCOM: &str = "\
0 HEAD
1 SOUR test
1 FILE tree.ged
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
2 FORM image/jpeg
1 NOTE A research note.
0 @I2@ INDI
1 NAME Jane /Doe/
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 MARR
2 DATE 1848
0 @S1@ SOUR
1 TITL Census 1801
0 TRLR
";

const GRAMPS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<gender>M</gender>
<name><first>John</first><surname>Smith</surname></name>
<eventref hlink="_e1"/>
<citationref hlink="_c1"/>
</person>
</people>
<events>
<event handle="_e1" id="E0001"><type>Birth</type><dateval val="1850"/><place hlink="_pl1"/></event>
</events>
<places>
<placeobj handle="_pl1" id="P0001" type="City"><pname value="Bergen"/></placeobj>
</places>
<sources>
<source handle="_s1" id="S0001"><stitle>Census 1801</stitle></source>
</sources>
<citations>
<citation handle="_c1" id="C0001"><page>p. 5</page><sourceref hlink="_s1"/></citation>
</citations>
</database>
"#;

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn human() -> Session {
    Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
    })
}

fn spec(plugin: &str, dataset: DatasetId, source_label: &str) -> ImportRunSpec {
    ImportRunSpec {
        operator: human(),
        dataset: RunDataset::Chosen(ChosenDataset {
            id: dataset,
            label: source_label.to_owned(),
        }),
        source_label: source_label.to_owned(),
        plugin: plugin.to_owned(),
        plugin_version: "0.1.0".to_owned(),
    }
}

fn invocation(workspace: Workspace, import: Option<ImportRunSpec>) -> Invocation {
    Invocation {
        workspace,
        session: Session::software("gedcom-import", "0.1.0"),
        grants: Grants::none()
            .with(Capability::Commands)
            .with(Capability::Log)
            .with(Capability::Progress)
            .with(Capability::ImportSource),
        budget: ResourceBudget::default(),
        net_policy: NetPolicy::deny_all(),
        ai_config: AiConfig::default(),
        provenance_confidence: None,
        import,
    }
}

async fn workspace(dir: &Path) -> Workspace {
    let root = dir.join("ws");
    Workspace::init(&root, &operator(), &AppDefaults::default(), None).expect("init");
    Workspace::open(&root, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace")
}

fn write_file(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write file");
    path
}

fn proceed(_: ProgressUpdate) -> ProgressControl {
    ProgressControl::Proceed
}

#[derive(serde::Deserialize)]
struct Header {
    context: EventContext,
}

/// Every event in the log with its aggregate kind, event type and provenance envelope.
async fn log(workspace: &Workspace) -> Vec<(String, String, EventContext)> {
    let events = workspace.store().read_recent_events(10_000).await.expect("read log");
    let mut out = Vec::with_capacity(events.len());
    for event in events {
        let header: Header = serde_json::from_str(&event.payload).expect("decode envelope");
        out.push((event.aggregate_type, event.event_type, header.context));
    }
    out
}

/// The `(kind, event type, record, item)` of every imported event.
async fn origin_keys(workspace: &Workspace) -> BTreeSet<(String, String, String, Option<String>)> {
    let mut keys = BTreeSet::new();
    for (kind, event_type, context) in log(workspace).await {
        if let Some(origin) = context.origin {
            keys.insert((kind, event_type, origin.record, origin.item));
        }
    }
    keys
}

async fn only_run(workspace: &Workspace) -> ImportRunSummary {
    let mut runs = list_import_runs(workspace).await.expect("runs");
    assert_eq!(runs.len(), 1, "{runs:?}");
    runs.remove(0)
}

#[tokio::test]
async fn a_gedcom_import_writes_one_run_and_stamps_every_assertion_with_its_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = workspace(dir.path()).await;
    let source = write_file(dir.path(), "tree.ged", GEDCOM);
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let (count, workspace) = common::host()
        .run_bulk_import(
            &common::component("gedcom-import"),
            invocation(workspace, Some(spec("gedcom-import", dataset.clone(), "tree.ged"))),
            source,
            proceed,
        )
        .await
        .expect("import");

    let run = only_run(&workspace).await;
    assert_eq!(run.status, ImportRunStatus::Finished);
    assert_eq!(run.operator_display.as_deref(), Some("Tester"));
    assert_eq!(
        (run.dataset.clone(), run.source_label.as_str()),
        (dataset.clone(), "tree.ged")
    );
    assert_eq!(run.counts.records, Some(count));
    assert_eq!(run.counts.created.get("person"), Some(&2));
    assert_eq!(run.counts.created.get("family"), Some(&1));
    assert_eq!(run.counts.created.get("place"), Some(&1));

    for (kind, event_type, context) in log(&workspace).await {
        if kind == "import_run" {
            assert_eq!(context.operator.kind, AgentKind::Human, "the run is the human's");
            assert_eq!(context.origin, None);
            continue;
        }
        let origin = context
            .origin
            .unwrap_or_else(|| panic!("{kind} {event_type} carries no origin"));
        assert_eq!((&origin.dataset, origin.run), (&dataset, run.id), "{kind} {event_type}");
        assert!(
            matches!(context.operator.kind, AgentKind::Software { .. }),
            "imported claims keep the importer's agent"
        );
    }

    let keys = origin_keys(&workspace).await;
    for expected in [
        ("person", "PersonCreated", "I1", None),
        ("person", "SexAsserted", "I1", None),
        ("event", "EventCreated", "I1", Some("event:BIRT:0")),
        ("place", "PlaceCreated", "plac:Mandal", None),
        ("source", "SourceCreated", "S1", None),
        ("citation", "CitationCreated", "I1", Some("citation:0")),
        ("note", "NoteCreated", "I1", Some("note:0")),
        ("media", "MediaCreated", "file:https://example.test/photo.jpg", None),
        ("family", "FamilyCreated", "F1", None),
        ("event", "EventCreated", "F1", Some("event:MARR:0")),
    ] {
        let (kind, event_type, record, item) = expected;
        let key = (
            kind.to_owned(),
            event_type.to_owned(),
            record.to_owned(),
            item.map(ToOwned::to_owned),
        );
        assert!(keys.contains(&key), "missing {key:?} in {keys:#?}");
    }
}

#[tokio::test]
async fn gedcom_and_gramps_item_keys_are_stable_across_runs_of_the_same_file() {
    for (plugin, name, text) in [
        ("gedcom-import", "tree.ged", GEDCOM),
        ("gramps-import", "tree.gramps", GRAMPS),
    ] {
        let mut runs = Vec::new();
        for _ in 0..2 {
            let dir = tempfile::tempdir().expect("tempdir");
            let workspace = workspace(dir.path()).await;
            let source = write_file(dir.path(), name, text);
            let dataset = DatasetId::lineage(plugin, Uuid::from_u128(5));
            let (_, workspace) = common::host()
                .run_bulk_import(
                    &common::component(plugin),
                    invocation(workspace, Some(spec(plugin, dataset, name))),
                    source,
                    proceed,
                )
                .await
                .expect("import");
            runs.push(origin_keys(&workspace).await);
        }
        assert!(!runs[0].is_empty(), "{plugin} stamped no origins");
        assert_eq!(runs[0], runs[1], "{plugin} keyed the same file differently");
    }
}

/// Imports `text` (as `name`) with `plugin` into `workspace`, under `dataset`.
async fn import(
    workspace: Workspace,
    plugin: &str,
    dataset: &DatasetId,
    dir: &Path,
    name: &str,
    text: &str,
) -> Workspace {
    let source = write_file(dir, name, text);
    let (_, workspace) = common::host()
        .run_bulk_import(
            &common::component(plugin),
            invocation(workspace, Some(spec(plugin, dataset.clone(), name))),
            source,
            proceed,
        )
        .await
        .expect("import");
    workspace
}

async fn event_count(workspace: &Workspace) -> u64 {
    workspace.store().event_count().await.expect("event count")
}

#[tokio::test]
async fn re_importing_an_unchanged_file_into_its_dataset_writes_nothing() {
    for (plugin, name, text) in [
        ("gedcom-import", "tree.ged", GEDCOM),
        ("gramps-import", "tree.gramps", GRAMPS),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let dataset = DatasetId::lineage(plugin, Uuid::from_u128(5));
        let workspace = import(workspace(dir.path()).await, plugin, &dataset, dir.path(), name, text).await;
        let (events, counts) = (
            event_count(&workspace).await,
            workspace_counts(&workspace).await.expect("counts"),
        );

        let workspace = import(workspace, plugin, &dataset, dir.path(), name, text).await;
        assert_eq!(
            event_count(&workspace).await,
            events,
            "{plugin}: the re-import wrote events"
        );
        assert_eq!(workspace_counts(&workspace).await.expect("counts"), counts, "{plugin}");
        assert_eq!(
            list_import_runs(&workspace).await.expect("runs").len(),
            1,
            "{plugin}: no second run"
        );
    }
}

#[tokio::test]
async fn unrelated_files_sharing_record_ids_import_as_separate_people() {
    let other_gedcom = "0 HEAD\n1 SOUR test\n0 @I1@ INDI\n1 NAME Kari /Nordmann/\n1 SEX F\n0 TRLR\n";
    let other_gramps = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_other" id="I0001">
<gender>F</gender>
<name><first>Kari</first><surname>Nordmann</surname></name>
</person>
</people>
</database>
"#;
    for (plugin, name, first, second) in [
        ("gedcom-import", "tree.ged", GEDCOM, other_gedcom),
        ("gramps-import", "tree.gramps", GRAMPS, other_gramps),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = import_as(workspace(dir.path()).await, plugin, 5, dir.path(), name, first).await;
        let before = workspace_counts(&workspace).await.expect("counts").person;
        let workspace = import_as(workspace, plugin, 6, dir.path(), name, second).await;
        assert_eq!(
            workspace_counts(&workspace).await.expect("counts").person,
            before + 1,
            "{plugin}: the second file's first person is a new person"
        );
    }
}

/// Imports `text` with `plugin` into its own dataset, `gedcom:<n>` / `gramps:<n>`.
async fn import_as(workspace: Workspace, plugin: &str, n: u128, dir: &Path, name: &str, text: &str) -> Workspace {
    let scheme = plugin.trim_end_matches("-import");
    let dataset = DatasetId::lineage(scheme, Uuid::from_u128(n));
    import(workspace, plugin, &dataset, dir, name, text).await
}

#[tokio::test]
async fn a_reimport_into_another_dataset_records_what_it_resolved_and_then_resolves_by_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let second = DatasetId::lineage("gedcom", Uuid::from_u128(6));
    let text = GEDCOM
        .replace("0 @I1@ INDI\n", "0 @I1@ INDI\n1 _UID U-I1\n")
        .replace("0 @I2@ INDI\n", "0 @I2@ INDI\n1 _UID U-I2\n")
        .replace("0 @F1@ FAM\n", "0 @F1@ FAM\n1 _UID U-F1\n");
    let workspace = import(
        workspace(dir.path()).await,
        "gedcom-import",
        &first,
        dir.path(),
        "tree.ged",
        &text,
    )
    .await;
    let workspace = import(workspace, "gedcom-import", &second, dir.path(), "tree.ged", &text).await;
    let runs = list_import_runs(&workspace).await.expect("runs");
    assert_eq!(runs.len(), 2);
    assert_eq!(
        runs[1].counts.resolved, 3,
        "two persons and a family resolved by external id"
    );
    let resolutions = log(&workspace)
        .await
        .into_iter()
        .filter(|(_, event_type, _)| event_type == "ItemResolved")
        .count();
    assert_eq!(resolutions, 3);
    assert_eq!(workspace_counts(&workspace).await.expect("counts").person, 2);

    let events = event_count(&workspace).await;
    let workspace = import(workspace, "gedcom-import", &second, dir.path(), "tree.ged", &text).await;
    assert_eq!(
        event_count(&workspace).await,
        events,
        "the recorded resolutions resolve the re-run"
    );
}

#[tokio::test]
async fn a_new_record_citing_an_imported_source_and_place_reuses_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let workspace = import(
        workspace(dir.path()).await,
        "gedcom-import",
        &dataset,
        dir.path(),
        "tree.ged",
        GEDCOM,
    )
    .await;
    let grown = GEDCOM.replace(
        "0 @S1@ SOUR",
        "0 @I3@ INDI\n1 NAME Ola /Smith/\n1 BIRT\n2 DATE 1972\n2 PLAC Mandal\n1 SOUR @S1@\n2 PAGE p. 6\n0 @S1@ SOUR",
    );
    let workspace = import(workspace, "gedcom-import", &dataset, dir.path(), "tree.ged", &grown).await;
    let counts = workspace_counts(&workspace).await.expect("counts");
    assert_eq!(
        (counts.person, counts.event),
        (3, 3),
        "the new person and its birth landed"
    );
    assert_eq!(
        (counts.source, counts.place),
        (1, 1),
        "the source and place were resolved, not duplicated"
    );
    assert_eq!(counts.citation, 2);
}

#[tokio::test]
async fn a_new_fact_on_an_imported_person_lands_on_reimport() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let workspace = import(
        workspace(dir.path()).await,
        "gedcom-import",
        &dataset,
        dir.path(),
        "tree.ged",
        GEDCOM,
    )
    .await;
    let grown = GEDCOM.replace("1 SEX M\n", "1 SEX M\n1 OCCU Farmer\n");
    let workspace = import(workspace, "gedcom-import", &dataset, dir.path(), "tree.ged", &grown).await;
    let person = workspace
        .store()
        .find_person("I0001")
        .await
        .expect("find")
        .expect("person");
    assert_eq!(
        person.facts().len(),
        1,
        "the occupation added in the file reached the person"
    );
}

#[tokio::test]
async fn an_occupation_the_user_retracted_stays_retracted_on_reimport() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let text = GEDCOM.replace("1 SEX M\n", "1 SEX M\n1 OCCU Farmer\n");
    let workspace = import(
        workspace(dir.path()).await,
        "gedcom-import",
        &dataset,
        dir.path(),
        "tree.ged",
        &text,
    )
    .await;
    let person = workspace
        .store()
        .find_person("I0001")
        .await
        .expect("find")
        .expect("person");
    let [fact] = person.facts_with_assertions() else {
        panic!("expected the occupation: {:?}", person.facts_with_assertions());
    };
    undo_assertion(&workspace, &human(), "I0001", &fact.assertion_id.to_string(), None)
        .await
        .expect("retract");
    let events = event_count(&workspace).await;

    let workspace = import(workspace, "gedcom-import", &dataset, dir.path(), "tree.ged", &text).await;
    let person = workspace
        .store()
        .find_person("I0001")
        .await
        .expect("find")
        .expect("person");
    assert!(person.facts().is_empty(), "the retracted occupation came back");
    assert_eq!(event_count(&workspace).await, events, "the re-import wrote nothing");
    assert_eq!(list_import_runs(&workspace).await.expect("runs").len(), 1);
}

/// The GEDCOM fixture with a header export date and John's birth on `birth`.
fn dated(export: &str, birth: &str) -> String {
    GEDCOM
        .replace("1 SOUR test\n", &format!("1 SOUR test\n1 DATE {export}\n"))
        .replace("2 DATE 5 APR 1970", &format!("2 DATE {birth}"))
}

async fn birth_date(workspace: &Workspace) -> String {
    let events = workspace.store().list_events().await.expect("events");
    let birth = events
        .iter()
        .find(|event| event.event_type() == Some(&vitni_core::enums::EventType::Birth))
        .expect("birth");
    format!("{:?}", birth.date())
}

#[tokio::test]
async fn a_changed_date_supersedes_the_imported_one_only_when_the_file_is_newer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let first = dated("1 JAN 2000", "5 APR 1970");
    let workspace = import(
        workspace(dir.path()).await,
        "gedcom-import",
        &dataset,
        dir.path(),
        "tree.ged",
        &first,
    )
    .await;
    let imported = birth_date(&workspace).await;

    let stale = dated("1 JAN 2001", "6 APR 1970");
    let events = event_count(&workspace).await;
    let workspace = import(workspace, "gedcom-import", &dataset, dir.path(), "tree.ged", &stale).await;
    assert_eq!(
        event_count(&workspace).await,
        events,
        "a file older than the import changes nothing"
    );
    assert_eq!(birth_date(&workspace).await, imported);

    let newer = dated("1 JAN 2100", "6 APR 1970");
    let workspace = import(workspace, "gedcom-import", &dataset, dir.path(), "tree.ged", &newer).await;
    assert_ne!(
        birth_date(&workspace).await,
        imported,
        "the newer file's date replaced it"
    );
    let superseded = log(&workspace)
        .await
        .into_iter()
        .filter(|(kind, event_type, _)| kind == "event" && event_type == "AssertionSuperseded")
        .count();
    assert_eq!(superseded, 1, "the imported date was superseded, not overwritten");
}

/// Cancels a bulk import at the commit's first progress report past its start.
fn cancel_mid_commit(update: &ProgressUpdate) -> ProgressControl {
    if update.step == ProgressStep::Writing && update.processed > 0 {
        ProgressControl::Cancel
    } else {
        ProgressControl::Proceed
    }
}

#[tokio::test]
async fn a_bulk_import_cancelled_while_writing_abandons_its_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = workspace(dir.path()).await;
    let source = write_file(dir.path(), "tree.ged", GEDCOM);
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let (_, workspace) = common::host()
        .run_bulk_import(
            &common::component("gedcom-import"),
            invocation(workspace, Some(spec("gedcom-import", dataset, "tree.ged"))),
            source,
            |update| cancel_mid_commit(&update),
        )
        .await
        .expect("a cancelled import returns normally");
    let run = only_run(&workspace).await;
    assert_eq!(
        run.status,
        ImportRunStatus::Abandoned {
            reason: AbandonReason::Cancelled
        }
    );
}

#[tokio::test]
async fn a_bulk_import_cancelled_while_reading_writes_nothing_and_leaves_no_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = workspace(dir.path()).await;
    let source = write_file(dir.path(), "tree.ged", GEDCOM);
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let (_, workspace) = common::host()
        .run_bulk_import(
            &common::component("gedcom-import"),
            invocation(workspace, Some(spec("gedcom-import", dataset, "tree.ged"))),
            source,
            |_| ProgressControl::Cancel,
        )
        .await
        .expect("a cancelled import returns normally");
    assert_eq!(event_count(&workspace).await, 0);
    let runs = list_import_runs(&workspace).await.expect("runs");
    assert!(runs.is_empty(), "{runs:?}");
}

#[tokio::test]
async fn an_interrupted_bulk_commit_finishes_on_re_run_without_duplicates() {
    for (plugin, name, text) in [
        ("gedcom-import", "tree.ged", GEDCOM),
        ("gramps-import", "tree.gramps", GRAMPS),
    ] {
        let dataset = DatasetId::lineage(plugin, Uuid::from_u128(5));
        let clean_dir = tempfile::tempdir().expect("tempdir");
        let clean = workspace(clean_dir.path()).await;
        let clean = import(clean, plugin, &dataset, clean_dir.path(), name, text).await;
        let expected = workspace_counts(&clean).await.expect("counts");
        let (clean_events, clean_keys) = (event_count(&clean).await, origin_keys(&clean).await);

        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let source = write_file(dir.path(), name, text);
        let (_, workspace) = common::host()
            .run_bulk_import(
                &common::component(plugin),
                invocation(workspace, Some(spec(plugin, dataset.clone(), name))),
                source,
                |update| cancel_mid_commit(&update),
            )
            .await
            .expect("interrupted");
        assert!(
            event_count(&workspace).await < clean_events,
            "{plugin} stopped part-way"
        );

        let workspace = import(workspace, plugin, &dataset, dir.path(), name, text).await;
        assert_eq!(
            workspace_counts(&workspace).await.expect("counts"),
            expected,
            "{plugin}"
        );
        assert_eq!(origin_keys(&workspace).await, clean_keys, "{plugin}: every claim, once");
        let runs = list_import_runs(&workspace).await.expect("runs");
        let statuses: Vec<ImportRunStatus> = runs.into_iter().map(|run| run.status).collect();
        assert!(statuses.contains(&ImportRunStatus::Finished), "{plugin}: {statuses:?}");

        let before = event_count(&workspace).await;
        let workspace = import(workspace, plugin, &dataset, dir.path(), name, text).await;
        assert_eq!(
            event_count(&workspace).await,
            before,
            "{plugin}: a third run writes nothing"
        );
    }
}

#[tokio::test]
async fn an_import_that_fails_before_writing_leaves_no_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = workspace(dir.path()).await;
    let source = write_file(dir.path(), "broken.gramps", "<database");
    let dataset = DatasetId::lineage("gramps", Uuid::from_u128(5));
    let root = dir.path().join("ws");
    let result = common::host()
        .run_bulk_import(
            &common::component("gramps-import"),
            invocation(workspace, Some(spec("gramps-import", dataset, "broken.gramps"))),
            source,
            proceed,
        )
        .await;
    let Err(error) = result else {
        panic!("a malformed document fails the import");
    };
    assert!(matches!(error, PluginError::Guest(_)), "{error:?}");
    let workspace = Workspace::open(&root, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("reopen");
    let checked = list_import_runs(&workspace).await.expect("runs");
    assert!(checked.is_empty(), "{checked:?}");
}

#[tokio::test]
async fn an_import_without_a_run_stamps_no_origin() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = workspace(dir.path()).await;
    let source = write_file(dir.path(), "tree.ged", GEDCOM);
    let (_, workspace) = common::host()
        .run_bulk_import(
            &common::component("gedcom-import"),
            invocation(workspace, None),
            source,
            proceed,
        )
        .await
        .expect("import");
    let checked = list_import_runs(&workspace).await.expect("runs");
    assert!(checked.is_empty(), "{checked:?}");
    assert!(origin_keys(&workspace).await.is_empty());
}

#[tokio::test]
async fn exports_carry_no_record_origin() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = workspace(dir.path()).await;
    let source = write_file(dir.path(), "tree.ged", GEDCOM);
    let lineage = Uuid::from_u128(0x5eed);
    let dataset = DatasetId::lineage("gedcom", lineage);
    let (_, mut workspace) = common::host()
        .run_bulk_import(
            &common::component("gedcom-import"),
            invocation(workspace, Some(spec("gedcom-import", dataset, "tree.ged"))),
            source,
            proceed,
        )
        .await
        .expect("import");
    let run = only_run(&workspace).await.id;

    for exporter in ["gedcom-export", "gramps-export"] {
        let target = dir.path().join(format!("{exporter}.out"));
        let export = Invocation {
            grants: Grants::none()
                .with(Capability::Query)
                .with(Capability::Log)
                .with(Capability::Progress)
                .with(Capability::ExportSink),
            ..invocation(workspace, None)
        };
        let (_, reopened) = common::host()
            .run_bulk_export(
                &common::component(exporter),
                export,
                ExportTarget::File(target.clone()),
                proceed,
            )
            .await
            .expect("export");
        workspace = reopened;
        let written = std::fs::read(&target).expect("read export");
        let written = String::from_utf8_lossy(&written);
        assert!(written.contains("John"), "{exporter} exported the person");
        for leaked in [lineage.to_string(), run.to_string(), "plac:Mandal".to_owned()] {
            assert!(!written.contains(&leaked), "{exporter} leaked {leaked:?} (ADR 0037 §2)");
        }
    }
}

/// The proposals an import showed its operator.
type Shown = Arc<Mutex<Vec<DatasetProposal>>>;

/// A run that names no dataset: the host proposes one once the file is read, records the proposal in
/// `shown`, and takes `answer`'s reply as the operator's.
fn proposing(
    plugin: &str,
    source_label: &str,
    shown: &Shown,
    answer: impl FnOnce(&DatasetProposal) -> Option<DatasetChoice> + Send + 'static,
) -> ImportRunSpec {
    let scheme = plugin.trim_end_matches("-import").to_owned();
    let shown = Arc::clone(shown);
    ImportRunSpec {
        dataset: RunDataset::Propose {
            spec: DatasetSpec {
                scheme,
                scope: DatasetScope::Lineage,
            },
            confirm: Box::new(move |proposal: DatasetProposal| {
                let reply = answer(&proposal);
                shown.lock().expect("lock").push(proposal);
                Box::pin(std::future::ready(reply))
            }),
        },
        ..spec(plugin, DatasetId::new("unused"), source_label)
    }
}

/// Imports `text` with `plugin` under a [`proposing`] run.
async fn import_proposing(
    workspace: Workspace,
    plugin: &str,
    dir: &Path,
    (name, text): (&str, &str),
    run: ImportRunSpec,
) -> Result<Workspace, PluginError> {
    let source = write_file(dir, name, text);
    let (_, workspace) = common::host()
        .run_bulk_import(
            &common::component(plugin),
            invocation(workspace, Some(run)),
            source,
            proceed,
        )
        .await?;
    Ok(workspace)
}

fn accept_the_proposal(proposal: &DatasetProposal) -> Option<DatasetChoice> {
    proposal
        .proposed
        .as_ref()
        .map(|id| DatasetChoice::Existing(id.to_string()))
}

#[tokio::test]
async fn a_re_export_naming_no_dataset_is_proposed_the_one_it_came_from() {
    for (plugin, name, text, hint) in [
        ("gedcom-import", "tree.ged", GEDCOM, "test|tree.ged"),
        ("gramps-import", "tree.gramps", RERUN[1].2, "Ingrid Strand"),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let shown = Shown::default();
        let first = proposing(plugin, name, &shown, |_| panic!("nothing to propose between"));
        let workspace = import_proposing(workspace(dir.path()).await, plugin, dir.path(), (name, text), first)
            .await
            .expect("first import");
        let run = only_run(&workspace).await;
        assert_eq!(
            run.dataset_hint.as_deref(),
            Some(hint),
            "{plugin} declared its fingerprint"
        );
        let events = event_count(&workspace).await;

        let again = proposing(plugin, name, &shown, accept_the_proposal);
        let workspace = import_proposing(workspace, plugin, dir.path(), (name, text), again)
            .await
            .expect("re-import");
        let proposals = shown.lock().expect("lock").clone();
        assert_eq!(proposals.len(), 1, "{plugin}");
        assert_eq!(
            proposals[0].proposed.as_ref(),
            Some(&run.dataset),
            "{plugin}: {proposals:?}"
        );
        assert_eq!(
            event_count(&workspace).await,
            events,
            "{plugin}: the confirmed re-import wrote events"
        );
    }
}

#[tokio::test]
async fn a_file_sharing_xrefs_under_another_fingerprint_is_proposed_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = import_as(
        workspace(dir.path()).await,
        "gedcom-import",
        5,
        dir.path(),
        "tree.ged",
        GEDCOM,
    )
    .await;
    let other = "0 HEAD\n1 SOUR test\n1 FILE other.ged\n0 @I1@ INDI\n1 NAME Kari /Nordmann/\n0 TRLR\n";
    let shown = Shown::default();
    let run = proposing("gedcom-import", "other.ged", &shown, |_| Some(DatasetChoice::New));
    let workspace = import_proposing(workspace, "gedcom-import", dir.path(), ("other.ged", other), run)
        .await
        .expect("import");
    let proposals = shown.lock().expect("lock").clone();
    assert_eq!(proposals[0].candidates[0].shared, 1, "@I1@ collides");
    assert_eq!(proposals[0].proposed, None);
    assert_eq!(
        list_import_runs(&workspace).await.expect("runs").len(),
        2,
        "a new dataset"
    );
}

#[tokio::test]
async fn a_gedcom_file_naming_no_file_in_its_header_is_proposed_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let anonymous = GEDCOM.replace("1 FILE tree.ged\n", "");
    let workspace = import_as(
        workspace(dir.path()).await,
        "gedcom-import",
        5,
        dir.path(),
        "a.ged",
        &anonymous,
    )
    .await;
    let shown = Shown::default();
    let run = proposing("gedcom-import", "a.ged", &shown, |_| Some(DatasetChoice::New));
    import_proposing(workspace, "gedcom-import", dir.path(), ("a.ged", &anonymous), run)
        .await
        .expect("import");
    let proposals = shown.lock().expect("lock").clone();
    assert_eq!(proposals[0].candidates[0].shared, 3, "every xref collides");
    assert_eq!(
        proposals[0].proposed, None,
        "a writer's name alone fingerprints no tree"
    );
}

#[tokio::test]
async fn declining_the_proposal_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = import_as(
        workspace(dir.path()).await,
        "gedcom-import",
        5,
        dir.path(),
        "tree.ged",
        GEDCOM,
    )
    .await;
    let events = event_count(&workspace).await;
    let changed = GEDCOM.replace("Jane /Doe/", "Jane /Berg/");
    let run = proposing("gedcom-import", "tree.ged", &Shown::default(), |_| None);
    let workspace = import_proposing(workspace, "gedcom-import", dir.path(), ("tree.ged", &changed), run)
        .await
        .expect("a declined import is no failure");
    assert_eq!(event_count(&workspace).await, events);
    assert_eq!(
        list_import_runs(&workspace).await.expect("runs").len(),
        1,
        "no second run"
    );
}

#[tokio::test]
async fn naming_no_dataset_when_asked_is_refused_with_the_candidates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = import_as(
        workspace(dir.path()).await,
        "gedcom-import",
        5,
        dir.path(),
        "tree.ged",
        GEDCOM,
    )
    .await;
    let run = proposing("gedcom-import", "tree.ged", &Shown::default(), |_| {
        Some(DatasetChoice::Unspecified)
    });
    let refused = import_proposing(workspace, "gedcom-import", dir.path(), ("tree.ged", GEDCOM), run).await;
    let Err(PluginError::Dataset(DatasetError::Required { candidates, .. })) = refused else {
        panic!("expected a refusal, got {:?}", refused.map(|_| ()));
    };
    assert_eq!(candidates.len(), 1, "{candidates:?}");
}

/// The re-run fixtures (#408): one invented tree exported once per lineage importer.
const RERUN: [(&str, &str, &str); 2] = [
    ("gedcom-import", "tree.ged", include_str!("fixtures/rerun/tree.ged")),
    (
        "gramps-import",
        "tree.gramps",
        include_str!("fixtures/rerun/tree.gramps"),
    ),
];

#[tokio::test]
async fn each_rerun_fixture_keys_every_item_alike_and_its_re_run_is_proposed_back_and_writes_nothing() {
    for (plugin, name, text) in RERUN {
        let mut keys = Vec::new();
        let mut last = None;
        for _ in 0..2 {
            let dir = tempfile::tempdir().expect("tempdir");
            let run = proposing(plugin, name, &Shown::default(), |_| {
                panic!("a fresh workspace has no dataset")
            });
            let workspace = import_proposing(workspace(dir.path()).await, plugin, dir.path(), (name, text), run)
                .await
                .expect("import");
            keys.push(origin_keys(&workspace).await);
            last = Some((workspace, dir));
        }
        let kinds: BTreeSet<&str> = keys[0].iter().map(|(kind, ..)| kind.as_str()).collect();
        assert!(kinds.len() >= 6, "{plugin} keyed too few kinds: {kinds:?}");
        assert_eq!(keys[0], keys[1], "{plugin} keyed the same file differently");

        let (workspace, dir) = last.expect("imported");
        let events = event_count(&workspace).await;
        let shown = Shown::default();
        let run = proposing(plugin, name, &shown, accept_the_proposal);
        let workspace = import_proposing(workspace, plugin, dir.path(), (name, text), run)
            .await
            .expect("re-run");
        let dataset = only_run(&workspace).await.dataset;
        let proposals = shown.lock().expect("lock").clone();
        assert_eq!(
            proposals[0].proposed.as_ref(),
            Some(&dataset),
            "{plugin}: {proposals:?}"
        );
        assert_eq!(
            event_count(&workspace).await,
            events,
            "{plugin}: the re-run wrote events"
        );
    }
}
