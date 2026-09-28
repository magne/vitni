//! Import-run use-cases (ADR 0037 §3, §5): a run's lifecycle, and the datasets projection and
//! dataset choice built over runs.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AbandonReason, AppDefaults, AppError, DatasetChoice, DatasetError, DatasetId, DatasetScope, DatasetSpec,
    ImportCounts, ImportRunStatus, NewImportRun, OperatorConfig, Session, Workspace, WorkspaceDefaults,
    abandon_import_run, choose_dataset, find_import_run, finish_import_run, list_datasets, list_import_runs,
    start_import_run,
};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn session() -> Session {
    Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
    })
}

async fn workspace() -> (Workspace, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, dir)
}

fn gedcom() -> DatasetSpec {
    DatasetSpec {
        scheme: "gedcom".to_owned(),
        scope: DatasetScope::Lineage,
    }
}

fn run_over(dataset: DatasetId, label: &str) -> NewImportRun {
    NewImportRun {
        plugin: "gedcom-import".to_owned(),
        plugin_version: "0.3.0".to_owned(),
        dataset,
        dataset_label: label.to_owned(),
        source_label: label.to_owned(),
        file_asserted_at: None,
    }
}

#[tokio::test]
async fn a_finished_run_is_listed_with_its_operator_and_counts() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let run = start_import_run(&workspace, &session, run_over(dataset.clone(), "tree.ged"))
        .await
        .expect("start");
    let counts = ImportCounts {
        records: Some(3),
        ..ImportCounts::default()
    };
    finish_import_run(&workspace, &session, run, Vec::new(), counts.clone())
        .await
        .expect("finish");

    let runs = list_import_runs(&workspace).await.expect("list");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, run);
    assert_eq!(runs[0].dataset, dataset);
    assert_eq!(runs[0].operator_display.as_deref(), Some("Tester"));
    assert_eq!(runs[0].status, ImportRunStatus::Finished);
    assert_eq!(runs[0].counts, counts);
    assert_eq!(
        find_import_run(&workspace, run).await.expect("find"),
        Some(runs[0].clone())
    );
}

#[tokio::test]
async fn an_ended_run_refuses_a_second_end() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let run = start_import_run(&workspace, &session, run_over(dataset, "tree.ged"))
        .await
        .expect("start");
    abandon_import_run(
        &workspace,
        &session,
        run,
        Vec::new(),
        ImportCounts::default(),
        AbandonReason::Cancelled,
    )
    .await
    .expect("abandon");
    let again = finish_import_run(&workspace, &session, run, Vec::new(), ImportCounts::default()).await;
    assert!(matches!(again, Err(AppError::ImportRunDomain(_))), "{again:?}");
}

#[tokio::test]
async fn datasets_group_runs_and_take_the_first_runs_label() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    let tree = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let other = DatasetId::lineage("gedcom", Uuid::from_u128(6));
    for (dataset, label) in [(&tree, "tree.ged"), (&other, "other.ged"), (&tree, "tree-2026.ged")] {
        start_import_run(&workspace, &session, run_over(dataset.clone(), label))
            .await
            .expect("start");
    }
    let datasets = list_datasets(&workspace).await.expect("datasets");
    let summary: Vec<(DatasetId, &str, usize)> = datasets
        .iter()
        .map(|dataset| (dataset.id.clone(), dataset.label.as_str(), dataset.runs))
        .collect();
    assert_eq!(summary, vec![(tree, "tree.ged", 2), (other, "other.ged", 1)]);
}

#[tokio::test]
async fn the_first_lineage_import_needs_no_choice_but_a_later_one_does() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    let first = choose_dataset(&workspace, &session, &gedcom(), DatasetChoice::Unspecified, "tree.ged")
        .await
        .expect("nothing to choose between");
    assert_eq!(first.id.scheme(), "gedcom");
    assert_eq!(first.label, "tree.ged");
    start_import_run(&workspace, &session, run_over(first.id.clone(), "tree.ged"))
        .await
        .expect("start");

    let refused = choose_dataset(&workspace, &session, &gedcom(), DatasetChoice::Unspecified, "tree.ged").await;
    let Err(AppError::Dataset(DatasetError::Required { candidates, .. })) = refused else {
        panic!("an unflagged import over an existing dataset is refused, got {refused:?}");
    };
    assert_eq!(candidates, vec![format!("tree.ged ({})", first.id)]);

    let by_label = choose_dataset(
        &workspace,
        &session,
        &gedcom(),
        DatasetChoice::Existing("tree.ged".to_owned()),
        "x.ged",
    )
    .await
    .expect("by label");
    assert_eq!(by_label, first);
    let by_id = choose_dataset(
        &workspace,
        &session,
        &gedcom(),
        DatasetChoice::Existing(first.id.to_string()),
        "x.ged",
    )
    .await
    .expect("by id");
    assert_eq!(by_id, first);
    let fresh = choose_dataset(&workspace, &session, &gedcom(), DatasetChoice::New, "x.ged")
        .await
        .expect("new");
    assert_ne!(fresh.id, first.id);
    assert_eq!(fresh.label, "x.ged");
}

#[tokio::test]
async fn a_label_shared_by_two_datasets_is_ambiguous_and_an_unknown_one_is_not_found() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    for lineage in [5, 6] {
        let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(lineage));
        start_import_run(&workspace, &session, run_over(dataset, "tree.ged"))
            .await
            .expect("start");
    }
    let ambiguous = choose_dataset(
        &workspace,
        &session,
        &gedcom(),
        DatasetChoice::Existing("tree.ged".to_owned()),
        "tree.ged",
    )
    .await;
    assert!(
        matches!(ambiguous, Err(AppError::Dataset(DatasetError::Ambiguous { .. }))),
        "{ambiguous:?}"
    );
    let unknown = choose_dataset(
        &workspace,
        &session,
        &gedcom(),
        DatasetChoice::Existing("nope.ged".to_owned()),
        "tree.ged",
    )
    .await;
    assert!(
        matches!(unknown, Err(AppError::Dataset(DatasetError::NotFound { .. }))),
        "{unknown:?}"
    );
}

#[tokio::test]
async fn a_global_importer_always_writes_its_one_dataset() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    let archive = DatasetSpec {
        scheme: "digitalarkivet".to_owned(),
        scope: DatasetScope::Global,
    };
    let chosen = choose_dataset(&workspace, &session, &archive, DatasetChoice::Unspecified, "pf01")
        .await
        .expect("global");
    assert_eq!(chosen.id, DatasetId::global("digitalarkivet"));
    let refused = choose_dataset(&workspace, &session, &archive, DatasetChoice::New, "pf01").await;
    assert!(
        matches!(refused, Err(AppError::Dataset(DatasetError::Global { .. }))),
        "{refused:?}"
    );
}
