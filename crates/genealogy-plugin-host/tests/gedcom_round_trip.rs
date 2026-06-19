//! GEDCOM round-trip integration test (roadmap Spike C exit criteria): a GEDCOM file imports as
//! personas + a family with Software-agent provenance, re-exports, and re-imports identically.
//!
//! Requires the plugin components: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};

use genealogy_app::{
    AppDefaults, OperatorConfig, PersonSummary, Session, Workspace, WorkspaceDefaults, list_families, list_persons,
};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use genealogy_plugin_host::{Capability, Grants, PluginHost, ResourceBudget};
use uuid::Uuid;

const SAMPLE: &str = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
0 @I2@ INDI
1 NAME Jane /Doe/
0 @I3@ INDI
1 NAME Sam /Smith/
0 @F1@ FAM
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
    Session::new(Agent {
        kind: AgentKind::Software {
            name: "genealogy-gedcom-import".to_owned(),
            version: "0.1.0".to_owned(),
        },
        id: AgentId::from_uuid(Uuid::from_u128(9)),
        display: Some("GEDCOM import".to_owned()),
    })
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
    Workspace::init(&root, &operator(), &AppDefaults::default()).expect("init");
    (root, dir)
}

async fn open_workspace(root: &Path) -> Workspace {
    Workspace::open(root, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace")
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

    let import_grants = || Grants::none().with(Capability::Commands).with(Capability::Log);
    let export_grants = || Grants::none().with(Capability::Query).with(Capability::Log);

    // 1. Import the sample GEDCOM.
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let (count, workspace) = host
        .run_gedcom_import(
            &importer,
            workspace,
            software_session(),
            import_grants(),
            SAMPLE.as_bytes(),
            ResourceBudget::default(),
        )
        .await
        .expect("import");
    assert_eq!(count, 4, "3 individuals + 1 family");

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

    // 3. The import was attributed to a Software operator.
    assert!(
        has_software_provenance(&root).await,
        "imported events must carry AgentKind::Software provenance"
    );

    // 4. Export to GEDCOM.
    let (bytes, workspace) = host
        .run_gedcom_export(
            &exporter,
            workspace,
            software_session(),
            export_grants(),
            ResourceBudget::default(),
        )
        .await
        .expect("export");
    drop(workspace);
    assert!(!bytes.is_empty(), "export produced a document");

    // 5. Re-import the exported document into a fresh workspace — structure is identical.
    let (root2, _dir2) = init_workspace();
    let workspace2 = open_workspace(&root2).await;
    let (count2, workspace2) = host
        .run_gedcom_import(
            &importer,
            workspace2,
            software_session(),
            import_grants(),
            &bytes,
            ResourceBudget::default(),
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
async fn import_is_denied_without_the_commands_capability() {
    let host = PluginHost::new().expect("host");
    let importer = host.load(&plugin_path("gedcom-import")).expect("load import");

    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let result = host
        .run_gedcom_import(
            &importer,
            workspace,
            software_session(),
            Grants::none().with(Capability::Log),
            SAMPLE.as_bytes(),
            ResourceBudget::default(),
        )
        .await;

    assert!(result.is_err(), "import without the commands grant must fail");
    let workspace = open_workspace(&root).await;
    assert!(
        list_persons(&workspace).await.expect("list").is_empty(),
        "a denied import must not have created any person"
    );
}
