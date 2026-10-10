//! Import-run use-cases (ADR 0037 §3, §5): a run's lifecycle, and the datasets projection and
//! dataset choice built over runs.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AbandonReason, AppDefaults, AppError, DatasetChoice, DatasetError, DatasetId, DatasetScope, DatasetSpec,
    EntityFields, Fingerprint, ImportCounts, ImportRunStatus, NewImportRun, OperatorConfig, RecordGraph,
    ResolutionDecision, ResolvedItem, Session, StagedEntity, StagedFamily, StagedPerson, Workspace, WorkspaceDefaults,
    abandon_import_run, choose_dataset, dataset_required, find_import_run, finish_import_run, list_datasets,
    list_import_runs, propose_dataset, start_import_run,
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
        source_path: None,
        file_asserted_at: None,
        unreadable_file_date: None,
        dataset_hint: None,
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
async fn the_first_lineage_import_needs_no_choice_but_a_later_one_is_proposed_one() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    let first = choose_dataset(&workspace, &session, &gedcom(), DatasetChoice::Unspecified, "tree.ged")
        .await
        .expect("nothing to choose between")
        .expect("a new dataset");
    assert_eq!(first.id.scheme(), "gedcom");
    assert_eq!(first.label, "tree.ged");
    start_import_run(&workspace, &session, run_over(first.id.clone(), "tree.ged"))
        .await
        .expect("start");

    let unchosen = choose_dataset(&workspace, &session, &gedcom(), DatasetChoice::Unspecified, "tree.ged")
        .await
        .expect("no error");
    assert_eq!(
        unchosen, None,
        "an unflagged import over an existing dataset is left to a proposal"
    );
    let proposal = propose_dataset(&workspace, &gedcom(), None, &file(&["I1"]))
        .await
        .expect("proposal");
    let DatasetError::Required { candidates, .. } = dataset_required(&gedcom(), &proposal) else {
        panic!("not a refusal");
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
    .expect("by label")
    .expect("chosen");
    assert_eq!(by_label, first);
    let by_id = choose_dataset(
        &workspace,
        &session,
        &gedcom(),
        DatasetChoice::Existing(first.id.to_string()),
        "x.ged",
    )
    .await
    .expect("by id")
    .expect("chosen");
    assert_eq!(by_id, first);
    let fresh = choose_dataset(&workspace, &session, &gedcom(), DatasetChoice::New, "x.ged")
        .await
        .expect("new")
        .expect("chosen");
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
        .expect("global")
        .expect("chosen");
    assert_eq!(chosen.id, DatasetId::global("digitalarkivet"));
    let refused = choose_dataset(&workspace, &session, &archive, DatasetChoice::New, "pf01").await;
    assert!(
        matches!(refused, Err(AppError::Dataset(DatasetError::Global { .. }))),
        "{refused:?}"
    );
}

/// Records a finished run over `dataset`, fingerprinted `hint`, that resolved each of `persons`.
async fn earlier_run(workspace: &Workspace, dataset: &DatasetId, hint: Option<&str>, persons: &[&str]) {
    let session = session();
    let run = NewImportRun {
        dataset_hint: hint.map(str::to_owned),
        ..run_over(dataset.clone(), "tree.ged")
    };
    let run = start_import_run(workspace, &session, run).await.expect("start");
    let mut resolved = Vec::new();
    for record in persons {
        resolved.push(ResolvedItem {
            record: (*record).to_owned(),
            item: None,
            kind: "person".to_owned(),
            aggregate_id: Uuid::from_u128(9),
            decision: ResolutionDecision::ExternalId,
        });
    }
    finish_import_run(workspace, &session, run, resolved, ImportCounts::default())
        .await
        .expect("finish");
}

/// A file of one person graph per record id, plus one family graph `F1`.
fn file(persons: &[&str]) -> Vec<RecordGraph> {
    let mut graphs = Vec::new();
    for record in persons {
        graphs.push(RecordGraph {
            record: (*record).to_owned(),
            entities: vec![StagedEntity {
                local_id: 0,
                item: None,
                fields: EntityFields::Person(StagedPerson::default()),
            }],
            links: Vec::new(),
        });
    }
    graphs.push(RecordGraph {
        record: "F1".to_owned(),
        entities: vec![StagedEntity {
            local_id: 0,
            item: None,
            fields: EntityFields::Family(StagedFamily::default()),
        }],
        links: Vec::new(),
    });
    graphs
}

#[tokio::test]
async fn a_workspace_without_a_dataset_of_the_scheme_proposes_nothing() {
    let (workspace, _dir) = workspace().await;
    let proposal = propose_dataset(&workspace, &gedcom(), Some("GRAMPS|tree.ged"), &file(&["I1"]))
        .await
        .expect("proposal");
    assert!(proposal.candidates.is_empty(), "{proposal:?}");
    assert_eq!(proposal.proposed, None);
    assert_eq!(proposal.keys, 2, "one person and one family");
}

#[tokio::test]
async fn the_dataset_holding_the_files_records_under_its_fingerprint_is_proposed() {
    let (workspace, _dir) = workspace().await;
    let tree = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let other = DatasetId::lineage("gedcom", Uuid::from_u128(6));
    earlier_run(&workspace, &tree, Some("GRAMPS|tree.ged"), &["I1", "I2"]).await;
    earlier_run(&workspace, &other, Some("GRAMPS|other.ged"), &["I1", "I2", "I3"]).await;

    let proposal = propose_dataset(
        &workspace,
        &gedcom(),
        Some("GRAMPS|tree.ged"),
        &file(&["I1", "I2", "I3"]),
    )
    .await
    .expect("proposal");
    let candidates: Vec<(DatasetId, usize, Fingerprint)> = proposal
        .candidates
        .iter()
        .map(|candidate| (candidate.id.clone(), candidate.shared, candidate.fingerprint))
        .collect();
    assert_eq!(
        candidates,
        vec![(tree.clone(), 2, Fingerprint::Same), (other, 3, Fingerprint::Different)]
    );
    assert_eq!(
        proposal.proposed,
        Some(tree),
        "the fingerprint outweighs a larger overlap"
    );
}

#[tokio::test]
async fn colliding_record_ids_under_another_fingerprint_propose_nothing() {
    let (workspace, _dir) = workspace().await;
    let tree = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    earlier_run(&workspace, &tree, Some("GRAMPS|tree.ged"), &["I1", "I2"]).await;
    let proposal = propose_dataset(
        &workspace,
        &gedcom(),
        Some("Legacy|unrelated.ged"),
        &file(&["I1", "I2"]),
    )
    .await
    .expect("proposal");
    assert_eq!(proposal.candidates.len(), 1);
    assert_eq!(proposal.proposed, None, "unrelated GEDCOM files share xrefs like I1");
}

#[tokio::test]
async fn without_a_fingerprint_to_compare_nothing_is_proposed() {
    let (workspace, _dir) = workspace().await;
    let tree = DatasetId::lineage("gramps", Uuid::from_u128(5));
    earlier_run(&workspace, &tree, None, &["_a", "_b"]).await;
    let gramps = DatasetSpec {
        scheme: "gramps".to_owned(),
        scope: DatasetScope::Lineage,
    };
    for hint in [Some("Kari Hansen"), None] {
        let proposal = propose_dataset(&workspace, &gramps, hint, &file(&["_a", "_b", "_c"]))
            .await
            .expect("proposal");
        assert_eq!(proposal.candidates[0].fingerprint, Fingerprint::Unknown);
        assert_eq!(proposal.candidates[0].shared, 2);
        assert_eq!(proposal.proposed, None, "shared ids alone are no evidence: {hint:?}");
    }
}

#[tokio::test]
async fn a_global_importer_is_proposed_nothing() {
    let (workspace, _dir) = workspace().await;
    let archive = DatasetSpec {
        scheme: "digitalarkivet".to_owned(),
        scope: DatasetScope::Global,
    };
    earlier_run(&workspace, &DatasetId::global("digitalarkivet"), None, &["pf01"]).await;
    let proposal = propose_dataset(&workspace, &archive, None, &file(&["pf01"]))
        .await
        .expect("proposal");
    assert!(proposal.candidates.is_empty(), "{proposal:?}");
}
