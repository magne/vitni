//! Succession write-path integration test (ADR 0026 §3): `PlaceEdit::AssertSuccession` dispatches to
//! `assert_place_succession` with the **anchor prepended** to the ceasing places, so the app never
//! sees the `SuccessionAnchorMismatch` the use-case rejects on. Runs against a real on-disk workspace
//! over `vitni-app`'s public surface only (mirrors `place_geography_edit.rs`).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, Calendar, DateInput, DateModifier, DatePoint, DateQuality,
    GenealogicalDateBody, NewPlace, OperatorConfig, PlaceType, Provenance, Session, SuccessionKind, Workspace,
    WorkspaceDefaults, create_place, show_place,
};
use vitni_ui::{PlaceEdit, ProvenanceDraft, dispatch_place_edit};

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

async fn setup() -> (Workspace, Session, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, session(), dir)
}

async fn place(ws: &Workspace, session: &Session, name: &str, place_type: PlaceType) -> String {
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

fn year(value: i32) -> DateInput {
    DateInput {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        body: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(value),
            month: None,
            day: None,
        })),
        new_year_begins: None,
        original_text: None,
        time: None,
    }
}

/// The anchor rides in `human_id`, the other ceasing places in `from_extra` — the dispatcher joins the
/// two into the app's `from_human_ids`, so a many→one merge lands without an anchor mismatch and every
/// ceasing place gains the survivor as a successor.
#[tokio::test]
async fn a_merge_dispatch_prepends_the_anchor_to_the_ceasing_places() {
    let (ws, session, _dir) = setup().await;
    let aker = place(&ws, &session, "Aker", PlaceType::Municipality).await;
    let kristiania = place(&ws, &session, "Kristiania", PlaceType::Municipality).await;
    let oslo = place(&ws, &session, "Oslo", PlaceType::Municipality).await;

    dispatch_place_edit(
        &ws,
        &session,
        &PlaceEdit::AssertSuccession {
            human_id: aker.clone(),
            from_extra: vec![kristiania.clone()],
            to: vec![oslo.clone()],
            kind: SuccessionKind::Merged,
            date: Some(year(1948)),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("dispatch AssertSuccession");

    let survivor = show_place(&ws, &oslo).await.expect("show").expect("place");
    let mut ceased: Vec<String> = survivor.predecessors.iter().map(|rel| rel.human_id.clone()).collect();
    ceased.sort();
    let mut expected = vec![aker.clone(), kristiania.clone()];
    expected.sort();
    assert_eq!(
        ceased, expected,
        "the anchor and the extra ceasing place are both recorded"
    );
    assert_eq!(survivor.predecessors[0].kind, SuccessionKind::Merged);

    let anchor = show_place(&ws, &aker).await.expect("show").expect("place");
    assert_eq!(anchor.successors[0].human_id, oslo);
    let extra = show_place(&ws, &kristiania).await.expect("show").expect("place");
    assert_eq!(extra.successors[0].human_id, oslo);
}

/// A split names many resulting places in `to`; `from_extra` stays empty (the anchor is the only place
/// that ceased). An omitted date leaves the succession undated rather than defaulting to "now".
#[tokio::test]
async fn a_split_dispatch_records_every_resulting_place_and_stays_undated() {
    let (ws, session, _dir) = setup().await;
    let county = place(&ws, &session, "Old County", PlaceType::County).await;
    let north = place(&ws, &session, "North County", PlaceType::County).await;
    let south = place(&ws, &session, "South County", PlaceType::County).await;

    dispatch_place_edit(
        &ws,
        &session,
        &PlaceEdit::AssertSuccession {
            human_id: county.clone(),
            from_extra: Vec::new(),
            to: vec![north.clone(), south.clone()],
            kind: SuccessionKind::Split,
            date: None,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("dispatch AssertSuccession");

    let split = show_place(&ws, &county).await.expect("show").expect("place");
    let mut resulting: Vec<String> = split.successors.iter().map(|rel| rel.human_id.clone()).collect();
    resulting.sort();
    let mut expected = vec![north.clone(), south.clone()];
    expected.sort();
    assert_eq!(resulting, expected, "both resulting places are recorded");
    assert!(
        split.successors.iter().all(|rel| rel.date.is_none()),
        "a `None` date stays undated"
    );
    assert!(
        split.successors.iter().all(|rel| rel.kind == SuccessionKind::Split),
        "the picked kind threads through"
    );
}

/// A dated succession threads its [`DateInput`] through `build_genealogical_date`, so the structured
/// date (not a free-text caption) reaches the projection.
#[tokio::test]
async fn a_dated_succession_dispatch_records_the_structured_year() {
    let (ws, session, _dir) = setup().await;
    let old = place(&ws, &session, "Christiania", PlaceType::City).await;
    let new = place(&ws, &session, "Oslo", PlaceType::City).await;

    dispatch_place_edit(
        &ws,
        &session,
        &PlaceEdit::AssertSuccession {
            human_id: old.clone(),
            from_extra: Vec::new(),
            to: vec![new.clone()],
            kind: SuccessionKind::Renamed,
            date: Some(year(1925)),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("dispatch AssertSuccession");

    let summary = show_place(&ws, &old).await.expect("show").expect("place");
    let date = summary.successors[0].date.as_ref().expect("a dated succession");
    assert_eq!(date.sort_value, 19_250_000, "the year drives the sort key");
    assert_eq!(date.calendar, Calendar::Gregorian);
}
