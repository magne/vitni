//! Import runs and record origins (ADR 0037 §1, §5): an import with a run spec writes one run
//! operated by the invoking human, and every assertion the importer writes names the record it came
//! from and the run that wrote it.
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use uuid::Uuid;
use vitni_app::{
    AbandonReason, AgentKind, AiConfig, AppDefaults, DatasetId, ImportRunStatus, ImportRunSummary, OperatorConfig,
    Session, Workspace, WorkspaceDefaults, list_import_runs,
};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, EventContext};
use vitni_plugin_host::{
    Capability, Grants, ImportRunSpec, Invocation, NetPolicy, PluginError, ProgressControl, ProgressUpdate,
    ResourceBudget,
};

mod common;

const GEDCOM: &str = "\
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
        dataset,
        dataset_label: source_label.to_owned(),
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

#[tokio::test]
async fn a_reimport_into_the_same_dataset_records_what_it_resolved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut workspace = workspace(dir.path()).await;
    let source = write_file(dir.path(), "tree.ged", GEDCOM);
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    for _ in 0..2 {
        let (_, reopened) = common::host()
            .run_bulk_import(
                &common::component("gedcom-import"),
                invocation(workspace, Some(spec("gedcom-import", dataset.clone(), "tree.ged"))),
                source.clone(),
                proceed,
            )
            .await
            .expect("import");
        workspace = reopened;
    }
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
}

#[tokio::test]
async fn a_cancelled_bulk_import_abandons_its_run() {
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
    let run = only_run(&workspace).await;
    assert_eq!(
        run.status,
        ImportRunStatus::Abandoned {
            reason: AbandonReason::Cancelled
        }
    );
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
    assert!(list_import_runs(&workspace).await.expect("runs").is_empty());
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
    assert!(list_import_runs(&workspace).await.expect("runs").is_empty());
    assert!(origin_keys(&workspace).await.is_empty());
}
