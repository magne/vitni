//! Place, source, citation, repository, note and media clusters (ADR 0039 §1, §4, §5): identity
//! decisions on each kind, judged between clusters, and a merged cluster that reads as one record —
//! listed once, its rows attributed to the record they came from — until its merge is undone.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeMap;

use uuid::Uuid;
use vitni_app::{
    ActivityDetail, AppDefaults, AppError, IdentityDecision, MatchBand, MatchableKind, MutationMeta, NewCitation,
    NewMedia, NewNote, NewPlace, NewRepository, NewSource, OperatorConfig, PairDecision, Provenance, Session, Url,
    Workspace, WorkspaceCounts, WorkspaceDefaults, add_citation_attribute, add_media_attribute, add_note_translation,
    add_place_name, add_repository_url, add_source_attribute, change_log_for_citation, change_log_for_media,
    change_log_for_note, change_log_for_place, change_log_for_repository, change_log_for_source,
    citation_pair_decision, create_citation, create_media, create_note, create_place, create_repository, create_source,
    distinguish_citations, distinguish_media, distinguish_notes, distinguish_places, distinguish_repositories,
    distinguish_sources, list_citations, list_media, list_notes, list_places, list_repositories, list_sources,
    media_pair_decision, merge_citations, merge_media, merge_notes, merge_places, merge_repositories, merge_sources,
    note_pair_decision, place_pair_decision, repository_pair_decision, show_citation, show_media, show_note,
    show_place, show_repository, show_source, similar_pairs, source_pair_decision, undo_citation_assertion,
    undo_citation_distinction_and_merge, undo_media_assertion, undo_media_distinction_and_merge, undo_note_assertion,
    undo_note_distinction_and_merge, undo_place_assertion, undo_place_distinction_and_merge, undo_repository_assertion,
    undo_repository_distinction_and_merge, undo_source_assertion, undo_source_distinction_and_merge, workspace_counts,
};
use vitni_core::citation::CitationError;
use vitni_core::enums::PlaceType;
use vitni_core::ids::AgentId;
use vitni_core::matching::{CultureId, EngineVersion, MatchEvidence};
use vitni_core::media::MediaError;
use vitni_core::note::NoteError;
use vitni_core::place::PlaceError;
use vitni_core::provenance::{Agent, AgentKind};
use vitni_core::repository::RepositoryError;
use vitni_core::source::SourceError;

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

fn evidence() -> MatchEvidence {
    MatchEvidence {
        score_bp: 9300,
        band: MatchBand::Probable,
        engine: EngineVersion(4),
        cultures: vec![CultureId::new("universal")],
        features: Vec::new(),
    }
}

/// A record read back through its kind's show use-case: the `human_id` it resolved to, the records
/// merged into it, and the owner of each row a member supplied.
struct Shown {
    human_id: String,
    merged: Vec<String>,
    claim_owners: BTreeMap<String, String>,
    rows: Vec<String>,
}

async fn new_place(ws: &Workspace) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some("Haugen".to_owned()),
    };
    create_place(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create place")
}

async fn place_row(ws: &Workspace, human_id: &str) {
    add_place_name(ws, &session(), human_id, "Hougen".to_owned(), MutationMeta::default())
        .await
        .expect("name");
}

async fn shown_place(ws: &Workspace, human_id: &str) -> Shown {
    let place = show_place(ws, human_id).await.expect("show").expect("place");
    Shown {
        human_id: place.human_id,
        merged: place.merged.into_iter().map(|m| m.human_id).collect(),
        claim_owners: place.claim_owners,
        rows: place.names.into_iter().map(|n| n.assertion_id).collect(),
    }
}

async fn listed_places(ws: &Workspace) -> Vec<String> {
    list_places(ws)
        .await
        .expect("list")
        .into_iter()
        .map(|p| p.human_id)
        .collect()
}

async fn new_source(ws: &Workspace) -> String {
    let new = NewSource {
        human_id: None,
        title: Some("Ministerialbok for Hof".to_owned()),
    };
    create_source(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create source")
}

async fn source_row(ws: &Workspace, human_id: &str) {
    add_source_attribute(
        ws,
        &session(),
        human_id,
        "film".to_owned(),
        "8123".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("attribute");
}

async fn shown_source(ws: &Workspace, human_id: &str) -> Shown {
    let source = show_source(ws, human_id).await.expect("show").expect("source");
    Shown {
        human_id: source.human_id,
        merged: source.merged.into_iter().map(|m| m.human_id).collect(),
        claim_owners: source.claim_owners,
        rows: source.attributes.into_iter().map(|a| a.assertion_id).collect(),
    }
}

async fn listed_sources(ws: &Workspace) -> Vec<String> {
    list_sources(ws)
        .await
        .expect("list")
        .into_iter()
        .map(|s| s.human_id)
        .collect()
}

async fn new_citation(ws: &Workspace) -> String {
    let new = NewSource {
        human_id: None,
        title: Some("Folketelling 1865".to_owned()),
    };
    let source = create_source(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create source");
    let new = NewCitation {
        human_id: None,
        source,
        page: Some("p. 12".to_owned()),
    };
    create_citation(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create citation")
}

async fn citation_row(ws: &Workspace, human_id: &str) {
    add_citation_attribute(
        ws,
        &session(),
        human_id,
        "line".to_owned(),
        "4".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("attribute");
}

async fn shown_citation(ws: &Workspace, human_id: &str) -> Shown {
    let citation = show_citation(ws, human_id).await.expect("show").expect("citation");
    Shown {
        human_id: citation.human_id,
        merged: citation.merged.into_iter().map(|m| m.human_id).collect(),
        claim_owners: citation.claim_owners,
        rows: citation.attributes.into_iter().map(|a| a.assertion_id).collect(),
    }
}

async fn listed_citations(ws: &Workspace) -> Vec<String> {
    list_citations(ws)
        .await
        .expect("list")
        .into_iter()
        .map(|c| c.human_id)
        .collect()
}

async fn new_repository(ws: &Workspace) -> String {
    let new = NewRepository {
        human_id: None,
        name: Some("Arkivverket".to_owned()),
    };
    create_repository(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create repository")
}

async fn repository_row(ws: &Workspace, human_id: &str) {
    let url = Url {
        url_type: None,
        href: "https://example.org/archive".to_owned(),
        description: None,
    };
    add_repository_url(ws, &session(), human_id, url, MutationMeta::default())
        .await
        .expect("url");
}

async fn shown_repository(ws: &Workspace, human_id: &str) -> Shown {
    let repository = show_repository(ws, human_id).await.expect("show").expect("repository");
    Shown {
        human_id: repository.human_id,
        merged: repository.merged.into_iter().map(|m| m.human_id).collect(),
        claim_owners: repository.claim_owners,
        rows: repository.urls.into_iter().map(|u| u.assertion_id).collect(),
    }
}

async fn listed_repositories(ws: &Workspace) -> Vec<String> {
    list_repositories(ws)
        .await
        .expect("list")
        .into_iter()
        .map(|r| r.human_id)
        .collect()
}

async fn new_note(ws: &Workspace) -> String {
    let new = NewNote {
        human_id: None,
        text: Some("Born at the farm".to_owned()),
    };
    create_note(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create note")
}

async fn note_row(ws: &Workspace, human_id: &str) {
    add_note_translation(
        ws,
        &session(),
        human_id,
        "nb".to_owned(),
        "Født på garden".to_owned(),
        None,
        MutationMeta::default(),
    )
    .await
    .expect("translation");
}

async fn shown_note(ws: &Workspace, human_id: &str) -> Shown {
    let note = show_note(ws, human_id).await.expect("show").expect("note");
    Shown {
        human_id: note.human_id,
        merged: note.merged.into_iter().map(|m| m.human_id).collect(),
        claim_owners: note.claim_owners,
        rows: note.translations.into_iter().map(|t| t.assertion_id).collect(),
    }
}

async fn listed_notes(ws: &Workspace) -> Vec<String> {
    list_notes(ws)
        .await
        .expect("list")
        .into_iter()
        .map(|n| n.human_id)
        .collect()
}

async fn new_media(ws: &Workspace) -> String {
    let new = NewMedia {
        human_id: None,
        path: Some("https://example.org/scan.jpg".to_owned()),
    };
    create_media(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create media")
}

async fn media_row(ws: &Workspace, human_id: &str) {
    add_media_attribute(
        ws,
        &session(),
        human_id,
        "scanner".to_owned(),
        "flatbed".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("attribute");
}

async fn shown_media(ws: &Workspace, human_id: &str) -> Shown {
    let media = show_media(ws, human_id).await.expect("show").expect("media");
    Shown {
        human_id: media.human_id,
        merged: media.merged.into_iter().map(|m| m.human_id).collect(),
        claim_owners: media.claim_owners,
        rows: media.attributes.into_iter().map(|a| a.assertion_id).collect(),
    }
}

async fn listed_media(ws: &Workspace) -> Vec<String> {
    list_media(ws)
        .await
        .expect("list")
        .into_iter()
        .map(|m| m.human_id)
        .collect()
}

/// The same five behaviours for one record kind, over that kind's use-cases.
macro_rules! cluster_tests {
    (
        $module:ident,
        new: $new:ident,
        row: $row:ident,
        shown: $shown:ident,
        listed: $listed:ident,
        merge: $merge:ident,
        distinguish: $distinguish:ident,
        undo_and_merge: $undo_and_merge:ident,
        pair: $pair:ident,
        log: $log:ident,
        undo: $undo:ident,
        merged_event: $merged_event:literal,
        distinguished_event: $distinguished_event:literal,
        count: $count:ident,
        decided: $decided:pat,
        conflict: $conflict:pat,
        not_found: $not_found:pat $(,)?
    ) => {
        mod $module {
            use super::*;

            async fn merge(ws: &Workspace, surviving: &str, merged: &str) {
                $merge(ws, &session(), surviving, merged, IdentityDecision::default())
                    .await
                    .expect("merge");
            }

            #[tokio::test]
            async fn a_merged_record_reads_as_one_with_its_rows_attributed() {
                let (ws, _dir) = workspace().await;
                let (a, b) = ($new(&ws).await, $new(&ws).await);
                $row(&ws, &b).await;
                merge(&ws, &a, &b).await;

                assert_eq!(
                    $listed(&ws).await,
                    vec![a.clone()],
                    "the member is listed behind its root"
                );
                let root = $shown(&ws, &a).await;
                assert_eq!(root.merged, vec![b.clone()]);
                let member_row = root.rows.last().expect("the member's row is composed").clone();
                assert_eq!(
                    root.claim_owners.get(&member_row),
                    Some(&b),
                    "{:?}",
                    root.claim_owners
                );
                assert_eq!(
                    $shown(&ws, &b).await.human_id,
                    a,
                    "the member reads as its cluster"
                );

                let counts: WorkspaceCounts = workspace_counts(&ws).await.expect("counts");
                let listed = u64::try_from($listed(&ws).await.len()).expect("count");
                assert_eq!(counts.$count, listed, "the dashboard counts the cluster once");
            }

            #[tokio::test]
            async fn decisions_are_judged_between_clusters() {
                let (ws, _dir) = workspace().await;
                let (a, b, c) = ($new(&ws).await, $new(&ws).await, $new(&ws).await);
                merge(&ws, &a, &b).await;
                let again = $merge(&ws, &session(), &b, &a, IdentityDecision::default()).await;
                assert!(matches!(again, Err($decided)), "{again:?}");
                assert_eq!(
                    $pair(&ws, &a, &b).await.expect("pair"),
                    Some(PairDecision::SameCluster)
                );

                $distinguish(&ws, &session(), &c, &b, IdentityDecision::default())
                    .await
                    .expect("distinguish");
                assert_eq!(
                    $pair(&ws, &a, &c).await.expect("pair"),
                    Some(PairDecision::Distinct)
                );
                let blocked = $merge(&ws, &session(), &a, &c, IdentityDecision::default()).await;
                assert!(matches!(blocked, Err($decided)), "{blocked:?}");

                let merged = $undo_and_merge(&ws, &session(), &a, &c, IdentityDecision::default())
                    .await
                    .expect("undo and merge");
                assert_eq!(merged.survivor.human_id, a);
                assert_eq!(merged.merged_human_id, c);
                assert_eq!($listed(&ws).await, vec![a.clone()]);
            }

            #[tokio::test]
            async fn a_record_cannot_be_merged_with_itself_or_an_unknown_one() {
                let (ws, _dir) = workspace().await;
                let a = $new(&ws).await;
                let itself = $merge(&ws, &session(), &a, &a, IdentityDecision::default()).await;
                assert!(matches!(itself, Err($conflict)), "{itself:?}");
                let unknown = $merge(&ws, &session(), &a, "X9999", IdentityDecision::default()).await;
                assert!(matches!(unknown, Err($not_found)), "{unknown:?}");
            }

            #[tokio::test]
            async fn undoing_the_merge_splits_the_cluster() {
                let (ws, _dir) = workspace().await;
                let (a, b) = ($new(&ws).await, $new(&ws).await);
                merge(&ws, &a, &b).await;
                let entry = $log(&ws, &a)
                    .await
                    .expect("log")
                    .into_iter()
                    .find(|entry| entry.event_type == $merged_event)
                    .expect("merge logged");
                $undo(&ws, &session(), &a, &entry.assertion_id, None)
                    .await
                    .expect("undo");
                let mut listed = $listed(&ws).await;
                listed.sort();
                let mut both = vec![a.clone(), b.clone()];
                both.sort();
                assert_eq!(listed, both);
                assert_eq!($pair(&ws, &a, &b).await.expect("pair"), None);
                assert!($shown(&ws, &a).await.merged.is_empty());
            }

            #[tokio::test]
            async fn a_decision_records_the_assessment_it_was_made_on() {
                let (ws, _dir) = workspace().await;
                let (a, b) = ($new(&ws).await, $new(&ws).await);
                let decision = IdentityDecision {
                    provenance: Provenance::default(),
                    assessment: Some(evidence()),
                };
                $distinguish(&ws, &session(), &a, &b, decision)
                    .await
                    .expect("distinguish");
                let entry = $log(&ws, &a)
                    .await
                    .expect("log")
                    .into_iter()
                    .find(|entry| entry.event_type == $distinguished_event)
                    .expect("distinction logged");
                assert_eq!(
                    entry.detail,
                    Some(ActivityDetail::IdentityDecision {
                        assessment: evidence()
                    })
                );
                assert!(entry.can_undo);
            }
        }
    };
}

cluster_tests!(
    places,
    new: new_place,
    row: place_row,
    shown: shown_place,
    listed: listed_places,
    merge: merge_places,
    distinguish: distinguish_places,
    undo_and_merge: undo_place_distinction_and_merge,
    pair: place_pair_decision,
    log: change_log_for_place,
    undo: undo_place_assertion,
    merged_event: "PlacesMerged",
    distinguished_event: "PlacesDistinguished",
    count: place,
    decided: AppError::PlaceDomain(PlaceError::IdentityDecided { .. }),
    conflict: AppError::PlaceDomain(PlaceError::MergeConflict { .. }),
    not_found: AppError::PlaceNotFound(_),
);

cluster_tests!(
    sources,
    new: new_source,
    row: source_row,
    shown: shown_source,
    listed: listed_sources,
    merge: merge_sources,
    distinguish: distinguish_sources,
    undo_and_merge: undo_source_distinction_and_merge,
    pair: source_pair_decision,
    log: change_log_for_source,
    undo: undo_source_assertion,
    merged_event: "SourcesMerged",
    distinguished_event: "SourcesDistinguished",
    count: source,
    decided: AppError::SourceDomain(SourceError::IdentityDecided { .. }),
    conflict: AppError::SourceDomain(SourceError::MergeConflict { .. }),
    not_found: AppError::SourceNotFound(_),
);

cluster_tests!(
    citations,
    new: new_citation,
    row: citation_row,
    shown: shown_citation,
    listed: listed_citations,
    merge: merge_citations,
    distinguish: distinguish_citations,
    undo_and_merge: undo_citation_distinction_and_merge,
    pair: citation_pair_decision,
    log: change_log_for_citation,
    undo: undo_citation_assertion,
    merged_event: "CitationsMerged",
    distinguished_event: "CitationsDistinguished",
    count: citation,
    decided: AppError::CitationDomain(CitationError::IdentityDecided { .. }),
    conflict: AppError::CitationDomain(CitationError::MergeConflict { .. }),
    not_found: AppError::CitationNotFound(_),
);

cluster_tests!(
    repositories,
    new: new_repository,
    row: repository_row,
    shown: shown_repository,
    listed: listed_repositories,
    merge: merge_repositories,
    distinguish: distinguish_repositories,
    undo_and_merge: undo_repository_distinction_and_merge,
    pair: repository_pair_decision,
    log: change_log_for_repository,
    undo: undo_repository_assertion,
    merged_event: "RepositoriesMerged",
    distinguished_event: "RepositoriesDistinguished",
    count: repository,
    decided: AppError::RepositoryDomain(RepositoryError::IdentityDecided { .. }),
    conflict: AppError::RepositoryDomain(RepositoryError::MergeConflict { .. }),
    not_found: AppError::RepositoryNotFound(_),
);

cluster_tests!(
    notes,
    new: new_note,
    row: note_row,
    shown: shown_note,
    listed: listed_notes,
    merge: merge_notes,
    distinguish: distinguish_notes,
    undo_and_merge: undo_note_distinction_and_merge,
    pair: note_pair_decision,
    log: change_log_for_note,
    undo: undo_note_assertion,
    merged_event: "NotesMerged",
    distinguished_event: "NotesDistinguished",
    count: note,
    decided: AppError::NoteDomain(NoteError::IdentityDecided { .. }),
    conflict: AppError::NoteDomain(NoteError::MergeConflict { .. }),
    not_found: AppError::NoteNotFound(_),
);

cluster_tests!(
    media,
    new: new_media,
    row: media_row,
    shown: shown_media,
    listed: listed_media,
    merge: merge_media,
    distinguish: distinguish_media,
    undo_and_merge: undo_media_distinction_and_merge,
    pair: media_pair_decision,
    log: change_log_for_media,
    undo: undo_media_assertion,
    merged_event: "MediaMerged",
    distinguished_event: "MediaDistinguished",
    count: media,
    decided: AppError::MediaDomain(MediaError::IdentityDecided { .. }),
    conflict: AppError::MediaDomain(MediaError::MergeConflict { .. }),
    not_found: AppError::MediaNotFound(_),
);

#[tokio::test]
async fn a_decided_place_pair_is_not_proposed_as_a_duplicate() {
    let (ws, _dir) = workspace().await;
    let (a, b, c) = (new_place(&ws).await, new_place(&ws).await, new_place(&ws).await);
    let proposed = async || -> usize {
        similar_pairs(&ws, MatchableKind::Place, MatchBand::Possible)
            .await
            .expect("similar pairs")
            .len()
    };
    assert_eq!(proposed().await, 3, "three farms of one name pair up");

    merge_places(&ws, &session(), &a, &b, IdentityDecision::default())
        .await
        .expect("merge");
    distinguish_places(&ws, &session(), &c, &b, IdentityDecision::default())
        .await
        .expect("distinguish");
    assert_eq!(
        proposed().await,
        0,
        "A and B are one cluster, and C is distinct from it"
    );
}
