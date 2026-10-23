//! Place temporal-resolution and succession integration tests (ADR 0026): the transitive
//! hierarchy walk, the date-aware resolution rule, and place succession — end to end against a
//! temp workspace.
//!
//! The app-layer `assert_place_enclosed_by` use-case does not yet accept a date (a pre-existing
//! gap, not part of this ADR's scope), so the dated-enclosure setup here executes a raw
//! `PlaceCommand::AssertEnclosedBy` through the public `Store` directly — everything under test
//! (the resolution rule, the walk, succession) still runs through the public `vitni_app` API.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, AppError, MutationMeta, NewPlace, OperatorConfig, PlaceError, PlaceSuccessionInput, Provenance,
    Session, SuccessionKind, Workspace, WorkspaceDefaults, assert_place_enclosed_by, assert_place_succession,
    create_place, show_place, show_place_as_of,
};
use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::ids::AgentId;
use vitni_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use vitni_core::place_ref::PlaceRef;
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

/// A minimal exact-year `GenealogicalDate`, with `sort_value` set directly (the resolution rule
/// reads only this field — data-model §7.1).
fn year(value: i32) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(value),
            month: None,
            day: None,
        })),
        time: None,
        new_year_begins: None,
        sort_value: i64::from(value),
        original_text: None,
    }
}

async fn create(ws: &Workspace, session: &Session, name: &str, place_type: vitni_app::PlaceTypeStatement) -> String {
    create_place(
        ws,
        session,
        NewPlace {
            human_id: None,
            place_type,
            name: Some(name.to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place")
}

/// Directly asserts a dated `AssertEnclosedBy` through the public `Store` (see module docs).
async fn assert_dated_enclosure(
    ws: &Workspace,
    session: &Session,
    human_id: &str,
    enclosing_human_id: &str,
    date: Option<GenealogicalDate>,
) {
    let store = ws.store();
    let place_id = store
        .find_place(human_id)
        .await
        .expect("find")
        .expect("place exists")
        .place_id()
        .expect("place id");
    let enclosing_id = store
        .find_place(enclosing_human_id)
        .await
        .expect("find")
        .expect("place exists")
        .place_id()
        .expect("place id");
    let envelope = PlaceCommandEnvelope {
        meta: session.new_meta(Provenance::default(), Vec::new()),
        command: PlaceCommand::AssertEnclosedBy {
            place_id,
            enclosed_by: PlaceRef {
                place_id: enclosing_id,
                date,
            },
        },
    };
    store
        .execute_place(&place_id.to_string(), envelope)
        .await
        .expect("enclosure");
}

#[tokio::test]
async fn hierarchy_walk_resolves_the_full_transitive_chain_and_title() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let farm = create(
        &ws,
        &session,
        "Nordgarden",
        vitni_app::std_statement("farm").unwrap_or_default(),
    )
    .await;
    let parish = create(
        &ws,
        &session,
        "Vågå",
        vitni_app::std_statement("parish").unwrap_or_default(),
    )
    .await;
    let county = create(
        &ws,
        &session,
        "Innlandet",
        vitni_app::std_statement("county").unwrap_or_default(),
    )
    .await;
    let country = create(
        &ws,
        &session,
        "Norway",
        vitni_app::std_statement("country").unwrap_or_default(),
    )
    .await;

    assert_place_enclosed_by(&ws, &session, &farm, &parish, MutationMeta::default())
        .await
        .expect("enclose farm");
    assert_place_enclosed_by(&ws, &session, &parish, &county, MutationMeta::default())
        .await
        .expect("enclose parish");
    assert_place_enclosed_by(&ws, &session, &county, &country, MutationMeta::default())
        .await
        .expect("enclose county");

    let summary = show_place(&ws, &farm).await.expect("show").expect("found");
    let chain: Vec<_> = summary.enclosing.iter().map(|e| e.human_id.clone()).collect();
    assert_eq!(chain, vec![parish, county, country]);
    assert_eq!(summary.generated_title, "Nordgarden, Vågå, Innlandet, Norway");
}

#[tokio::test]
async fn a_cycle_in_the_enclosure_chain_terminates_the_walk() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let a = create(&ws, &session, "A", vitni_app::std_statement("farm").unwrap_or_default()).await;
    let b = create(
        &ws,
        &session,
        "B",
        vitni_app::std_statement("parish").unwrap_or_default(),
    )
    .await;
    assert_place_enclosed_by(&ws, &session, &a, &b, MutationMeta::default())
        .await
        .expect("enclose a in b");
    assert_place_enclosed_by(&ws, &session, &b, &a, MutationMeta::default())
        .await
        .expect("enclose b in a");

    // The walk must terminate (the cycle guard), not hang or panic.
    let summary = show_place(&ws, &a).await.expect("show").expect("found");
    assert_eq!(
        summary.enclosing.len(),
        1,
        "the cycle stops the walk after the first repeat"
    );
}

#[tokio::test]
async fn date_aware_resolution_picks_the_enclosure_in_effect_at_the_query_year() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let farm = create(
        &ws,
        &session,
        "Nedre Vågå",
        vitni_app::std_statement("farm").unwrap_or_default(),
    )
    .await;
    let old_parish = create(
        &ws,
        &session,
        "Vågå prestegjeld",
        vitni_app::std_statement("parish").unwrap_or_default(),
    )
    .await;
    let new_parish = create(
        &ws,
        &session,
        "Lom prestegjeld",
        vitni_app::std_statement("parish").unwrap_or_default(),
    )
    .await;

    // Two dated jurisdictions: valid from 1801, then reassigned from 1900 — no undated fallback.
    assert_dated_enclosure(&ws, &session, &farm, &old_parish, Some(year(1801))).await;
    assert_dated_enclosure(&ws, &session, &farm, &new_parish, Some(year(1900))).await;

    let as_of_1850 = show_place_as_of(&ws, &farm, year(1850))
        .await
        .expect("show")
        .expect("found");
    assert_eq!(as_of_1850.enclosing[0].human_id, old_parish);

    let as_of_1950 = show_place_as_of(&ws, &farm, year(1950))
        .await
        .expect("show")
        .expect("found");
    assert_eq!(as_of_1950.enclosing[0].human_id, new_parish);

    // No date context: the primary (first-asserted) link — the 1801 assignment, asserted first.
    let primary = show_place(&ws, &farm).await.expect("show").expect("found");
    assert_eq!(primary.enclosing[0].human_id, old_parish);
}

#[tokio::test]
async fn a_merge_is_reachable_from_both_the_survivor_and_each_merged_place() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let aker = create(
        &ws,
        &session,
        "Aker",
        vitni_app::std_statement("municipality").unwrap_or_default(),
    )
    .await;
    let kristiania = create(
        &ws,
        &session,
        "Kristiania",
        vitni_app::std_statement("municipality").unwrap_or_default(),
    )
    .await;
    let oslo = create(
        &ws,
        &session,
        "Oslo",
        vitni_app::std_statement("municipality").unwrap_or_default(),
    )
    .await;

    assert_place_succession(
        &ws,
        &session,
        &aker,
        PlaceSuccessionInput {
            from_human_ids: vec![aker.clone(), kristiania.clone()],
            to_human_ids: vec![oslo.clone()],
            kind: SuccessionKind::Merged,
            date: Some(year(1948)),
        },
        MutationMeta::default(),
    )
    .await
    .expect("merge");

    let oslo_summary = show_place(&ws, &oslo).await.expect("show").expect("found");
    let mut predecessors: Vec<_> = oslo_summary.predecessors.iter().map(|p| p.human_id.clone()).collect();
    predecessors.sort();
    let mut expected = vec![aker.clone(), kristiania.clone()];
    expected.sort();
    assert_eq!(predecessors, expected);
    assert_eq!(oslo_summary.predecessors[0].kind, SuccessionKind::Merged);

    let aker_summary = show_place(&ws, &aker).await.expect("show").expect("found");
    assert_eq!(aker_summary.successors[0].human_id, oslo);

    // Kristiania is not the anchor (Aker is), but is still reachable — the symmetric projection.
    let kristiania_summary = show_place(&ws, &kristiania).await.expect("show").expect("found");
    assert_eq!(kristiania_summary.successors[0].human_id, oslo);
}

#[tokio::test]
async fn a_split_is_reachable_from_every_resulting_place() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let county = create(
        &ws,
        &session,
        "Old County",
        vitni_app::std_statement("county").unwrap_or_default(),
    )
    .await;
    let north = create(
        &ws,
        &session,
        "North County",
        vitni_app::std_statement("county").unwrap_or_default(),
    )
    .await;
    let south = create(
        &ws,
        &session,
        "South County",
        vitni_app::std_statement("county").unwrap_or_default(),
    )
    .await;

    assert_place_succession(
        &ws,
        &session,
        &county,
        PlaceSuccessionInput {
            from_human_ids: vec![county.clone()],
            to_human_ids: vec![north.clone(), south.clone()],
            kind: SuccessionKind::Split,
            date: None,
        },
        MutationMeta::default(),
    )
    .await
    .expect("split");

    let county_summary = show_place(&ws, &county).await.expect("show").expect("found");
    let mut successors: Vec<_> = county_summary.successors.iter().map(|s| s.human_id.clone()).collect();
    successors.sort();
    let mut expected = vec![north.clone(), south.clone()];
    expected.sort();
    assert_eq!(successors, expected);

    let north_summary = show_place(&ws, &north).await.expect("show").expect("found");
    assert_eq!(north_summary.predecessors[0].human_id, county);
}

#[tokio::test]
async fn an_unknown_referenced_place_is_a_domain_error() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(
        &ws,
        &session,
        "Somewhere",
        vitni_app::std_statement("village").unwrap_or_default(),
    )
    .await;

    let err = assert_place_succession(
        &ws,
        &session,
        &place,
        PlaceSuccessionInput {
            from_human_ids: vec![place.clone()],
            to_human_ids: vec!["P9999".to_owned()],
            kind: SuccessionKind::Renamed,
            date: None,
        },
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(err, Err(AppError::PlaceNotFound(_))));
}

#[tokio::test]
async fn an_anchor_not_among_the_from_places_is_a_domain_error() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let a = create(
        &ws,
        &session,
        "A",
        vitni_app::std_statement("village").unwrap_or_default(),
    )
    .await;
    let b = create(
        &ws,
        &session,
        "B",
        vitni_app::std_statement("village").unwrap_or_default(),
    )
    .await;
    let c = create(
        &ws,
        &session,
        "C",
        vitni_app::std_statement("village").unwrap_or_default(),
    )
    .await;

    let err = assert_place_succession(
        &ws,
        &session,
        &a,
        PlaceSuccessionInput {
            from_human_ids: vec![b.clone()],
            to_human_ids: vec![c.clone()],
            kind: SuccessionKind::Merged,
            date: None,
        },
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(
        err,
        Err(AppError::PlaceDomain(PlaceError::SuccessionAnchorMismatch(_)))
    ));
}

#[tokio::test]
async fn empty_succession_endpoints_are_a_domain_error() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let a = create(
        &ws,
        &session,
        "A",
        vitni_app::std_statement("village").unwrap_or_default(),
    )
    .await;

    let err = assert_place_succession(
        &ws,
        &session,
        &a,
        PlaceSuccessionInput {
            from_human_ids: vec![a.clone()],
            to_human_ids: Vec::new(),
            kind: SuccessionKind::Renamed,
            date: None,
        },
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(
        err,
        Err(AppError::PlaceDomain(PlaceError::EmptySuccessionEndpoints))
    ));
}

// --- Same-date retype and creation-aware supersession (ADR 0054) ---

fn std_type(name: &str) -> vitni_app::PlaceTypeStatement {
    vitni_app::std_statement(name).expect("a std key")
}

fn dated(name: &str, date: GenealogicalDate) -> vitni_app::PlaceTypeStatement {
    vitni_app::PlaceTypeStatement {
        date: Some(date),
        ..std_type(name)
    }
}

async fn retype(ws: &Workspace, session: &Session, human_id: &str, statement: vitni_app::PlaceTypeStatement) {
    vitni_app::set_place_type(ws, session, human_id, statement, MutationMeta::default())
        .await
        .expect("set type");
}

async fn event_types(ws: &Workspace, human_id: &str) -> Vec<String> {
    vitni_app::change_log_for_place(ws, human_id)
        .await
        .expect("log")
        .into_iter()
        .map(|entry| entry.event_type)
        .collect()
}

async fn superseded_count(ws: &Workspace, human_id: &str) -> usize {
    event_types(ws, human_id)
        .await
        .iter()
        .filter(|event_type| *event_type == "AssertionSuperseded")
        .count()
}

/// The live type entries of a place as `(assertion id, statement)`, in assertion order.
async fn live_types(ws: &Workspace, human_id: &str) -> Vec<(String, vitni_app::PlaceTypeStatement)> {
    let view = ws
        .store()
        .find_place(human_id)
        .await
        .expect("find")
        .expect("place exists");
    view.place_types_with_assertions()
        .iter()
        .map(|entry| (entry.assertion_id.to_string(), entry.value.value.clone()))
        .collect()
}

async fn created_assertion(ws: &Workspace, human_id: &str) -> String {
    ws.store()
        .find_place(human_id)
        .await
        .expect("find")
        .expect("place exists")
        .created_assertion()
        .expect("created")
        .to_string()
}

async fn current_type(ws: &Workspace, human_id: &str) -> Option<vitni_app::PlaceTypeStatement> {
    show_place(ws, human_id).await.expect("show").expect("found").place_type
}

fn imported() -> Provenance {
    Provenance {
        origin: Some(vitni_core::origin::RecordOrigin {
            dataset: vitni_core::origin::DatasetId::global("digitalarkivet"),
            record: "plac:Haugen".to_owned(),
            item: None,
            digest: None,
            run: vitni_core::ids::ImportRunId::from_uuid(Uuid::from_u128(9)),
        }),
        ..Provenance::default()
    }
}

#[tokio::test]
async fn a_retype_after_creation_is_plain_and_leaves_the_creation_entry_and_origin_live() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: std_type("farm"),
            name: Some("Haugen".to_owned()),
        },
        imported(),
        &[],
    )
    .await
    .expect("place");
    let place_id = ws
        .store()
        .find_place(&place)
        .await
        .expect("find")
        .expect("exists")
        .place_id()
        .expect("id");

    retype(&ws, &session, &place, std_type("parish")).await;

    assert_eq!(
        superseded_count(&ws, &place).await,
        0,
        "the creation is never superseded"
    );
    let created = created_assertion(&ws, &place).await;
    let live = live_types(&ws, &place).await;
    assert_eq!(live.len(), 2, "the created type and the retype are both live: {live:?}");
    assert_eq!(live[0], (created.clone(), std_type("farm")));
    assert_eq!(live[1].1, std_type("parish"));
    assert_eq!(current_type(&ws, &place).await, Some(std_type("parish")));
    assert!(
        ws.store()
            .created_origin("place", &place_id.to_string())
            .await
            .expect("origin")
            .is_some(),
        "the place.created origin row is still live"
    );
}

#[tokio::test]
async fn undoing_the_first_retype_restores_the_created_type() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Haugen", std_type("farm")).await;
    retype(&ws, &session, &place, std_type("parish")).await;
    let retype_id = live_types(&ws, &place).await[1].0.clone();

    vitni_app::undo_place_assertion(&ws, &session, &place, &retype_id, None)
        .await
        .expect("undo");

    assert_eq!(current_type(&ws, &place).await, Some(std_type("farm")));
    assert_eq!(live_types(&ws, &place).await.len(), 1);
}

#[tokio::test]
async fn a_second_undated_retype_supersedes_the_first_retype_only() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Haugen", std_type("farm")).await;
    let created = created_assertion(&ws, &place).await;
    retype(&ws, &session, &place, std_type("parish")).await;
    let first_retype = live_types(&ws, &place).await[1].0.clone();

    retype(&ws, &session, &place, std_type("county")).await;

    assert_eq!(superseded_count(&ws, &place).await, 1);
    let live = live_types(&ws, &place).await;
    let ids: Vec<&str> = live.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&created.as_str()), "the creation entry stays live");
    assert!(!ids.contains(&first_retype.as_str()), "the first retype is replaced");
    assert_eq!(live.len(), 2);
    assert_eq!(current_type(&ws, &place).await, Some(std_type("county")));
}

#[tokio::test]
async fn dated_types_accumulate_and_a_same_date_reset_supersedes_only_that_entry() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Haugen", vitni_app::PlaceTypeStatement::default()).await;
    retype(&ws, &session, &place, dated("farm", year(1838))).await;
    retype(&ws, &session, &place, dated("parish", year(1964))).await;
    assert_eq!(superseded_count(&ws, &place).await, 0);
    assert_eq!(live_types(&ws, &place).await.len(), 2, "1838 and 1964 are both live");

    retype(&ws, &session, &place, dated("hamlet", year(1838))).await;
    assert_eq!(superseded_count(&ws, &place).await, 1);
    let respelled = GenealogicalDate {
        original_text: Some("abt 1838".to_owned()),
        ..year(1838)
    };
    retype(&ws, &session, &place, dated("village", respelled)).await;
    assert_eq!(
        superseded_count(&ws, &place).await,
        2,
        "another spelling of 1838 is the same date"
    );

    let live = live_types(&ws, &place).await;
    let keys: Vec<String> = live
        .iter()
        .map(|(_, statement)| {
            statement
                .type_key
                .as_ref()
                .map(|key| key.as_str().to_owned())
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(live.len(), 2, "{keys:?}");
    assert!(keys.contains(&"std:village".to_owned()), "{keys:?}");
    assert!(
        keys.contains(&"std:parish".to_owned()),
        "the 1964 entry is untouched: {keys:?}"
    );

    let as_of_1900 = show_place_as_of(&ws, &place, year(1900))
        .await
        .expect("show")
        .expect("found");
    assert_eq!(
        as_of_1900.place_type.and_then(|statement| statement.type_key),
        std_type("village").type_key,
        "as of 1900 the 1838 type is in effect"
    );
}

#[tokio::test]
async fn the_first_type_set_on_an_untyped_place_is_plain() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Haugen", vitni_app::PlaceTypeStatement::default()).await;

    retype(&ws, &session, &place, std_type("farm")).await;

    assert_eq!(superseded_count(&ws, &place).await, 0);
    assert_eq!(current_type(&ws, &place).await, Some(std_type("farm")));
}

#[tokio::test]
async fn a_caller_supplied_supersedes_passes_through_unchanged() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Haugen", std_type("farm")).await;
    let created = created_assertion(&ws, &place).await;

    vitni_app::set_place_type(
        &ws,
        &session,
        &place,
        std_type("parish"),
        MutationMeta {
            supersedes: Some(&created),
            ..MutationMeta::default()
        },
    )
    .await
    .expect("explicit supersession");

    assert_eq!(superseded_count(&ws, &place).await, 1);
    let live = live_types(&ws, &place).await;
    assert_eq!(live.len(), 1, "the created entry was replaced as asked: {live:?}");
    assert_eq!(live[0].1, std_type("parish"));
}

#[tokio::test]
async fn a_caller_supplied_supersedes_wins_over_the_same_date_entry() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Haugen", std_type("farm")).await;
    let created = created_assertion(&ws, &place).await;
    vitni_app::set_place_type(&ws, &session, &place, std_type("parish"), MutationMeta::default())
        .await
        .expect("plain retype accumulates");

    vitni_app::set_place_type(
        &ws,
        &session,
        &place,
        std_type("city"),
        MutationMeta {
            supersedes: Some(&created),
            ..MutationMeta::default()
        },
    )
    .await
    .expect("explicit supersession");

    let live: Vec<_> = live_types(&ws, &place)
        .await
        .into_iter()
        .map(|(_, value)| value)
        .collect();
    assert_eq!(
        live,
        [std_type("parish"), std_type("city")],
        "only the named entry was replaced"
    );
}

#[tokio::test]
async fn an_empty_statement_is_refused() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Haugen", std_type("farm")).await;

    let result = vitni_app::set_place_type(
        &ws,
        &session,
        &place,
        vitni_app::PlaceTypeStatement::default(),
        MutationMeta::default(),
    )
    .await;

    assert!(
        matches!(result, Err(AppError::PlaceDomain(PlaceError::EmptyPlaceType))),
        "{result:?}"
    );
}
