//! Dispatch-layer coverage for the `ResearchNote` slice (ADR 0028, issue #194): the create form's
//! change-set, every `ResearchNoteEdit`, the detail/list loaders, and the reverse-lookup rows the four
//! conclusion-bearing detail screens show.
//!
//! Runs against a real on-disk workspace over `vitni-app`'s public surface only — the presentation
//! layer never names a `vitni-core` type, even in tests (ADR 0008).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]
#![expect(clippy::panic, reason = "an unexpected intent outcome aborts the test")]

use uuid::Uuid;
use vitni_app::EventType;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, EvidenceLevel, NewEvent, NewPerson, NewPlace, OperatorConfig,
    PersonNameParts, PlaceType, Provenance, Session, Workspace, WorkspaceDefaults, change_log_for_research_note,
    create_event, create_person, create_place, create_tag, show_research_note,
};
use vitni_ui::{
    Category, Intent, IntentOutcome, Localizer, ProvenanceDraft, ResearchNoteChangeSetRequest, ResearchNoteEdit,
    RestrictionKind, SubjectRequest, dispatch, dispatch_research_note_change_set, dispatch_research_note_edit,
    event_tabs, family_tabs, person_tabs, place_tabs,
};

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

async fn setup() -> (Workspace, Session, Localizer, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let loc = Localizer::for_workspace(&ws, None);
    (workspace, session(), loc, dir)
}

async fn person(ws: &Workspace, session: &Session, given: &str) -> String {
    create_person(
        ws,
        session,
        NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                Some(given.to_owned()),
                Some("Smith".to_owned()),
            )),
            evidence_level: EvidenceLevel::Conclusion,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("person")
}

fn subject(category: Category, human_id: &str) -> SubjectRequest {
    SubjectRequest {
        category,
        human_id: human_id.to_owned(),
    }
}

/// The create request every test starts from: one person subject, a title, and a body.
fn create_request(person: &str) -> ResearchNoteChangeSetRequest {
    ResearchNoteChangeSetRequest {
        human_id: None,
        subjects: vec![subject(Category::People, person)],
        title: Some("Same person as the 1865 census entry?".to_owned()),
        body: Some("The parish register agrees on the birth year.".to_owned()),
        language: Some("en".to_owned()),
    }
}

/// Loads the research note's detail through `dispatch`, panicking on any other outcome.
async fn load_detail(ws: &Workspace, loc: &Localizer, human_id: &str) -> Box<vitni_ui::ResearchNoteDetail> {
    let outcome = dispatch(
        ws,
        loc,
        &Intent::ShowResearchNote {
            human_id: human_id.to_owned(),
        },
    )
    .await
    .expect("dispatch ShowResearchNote");
    let IntentOutcome::ResearchNoteDetail(detail) = outcome else {
        panic!("expected a research-note detail, got {outcome:?}");
    };
    detail
}

#[tokio::test]
async fn a_create_commits_the_subjects_title_and_body_then_loads_as_a_detail() {
    let (ws, session, loc, _dir) = setup().await;
    let ada = person(&ws, &session, "Ada").await;

    let human_id = dispatch_research_note_change_set(
        &ws,
        &session,
        &create_request(&ada),
        &ProvenanceDraft {
            rationale: "  the census and the register agree  ".to_owned(),
            ..ProvenanceDraft::default()
        },
    )
    .await
    .expect("dispatch create");
    assert_eq!(human_id, "A0001");

    let detail = load_detail(&ws, &loc, &human_id).await;
    assert_eq!(detail.title, "Same person as the 1865 census entry?");
    assert_eq!(
        detail.body.as_deref(),
        Some("The parish register agrees on the birth year.")
    );
    assert_eq!(detail.language.as_deref(), Some("en"));
    assert_eq!(detail.subjects.len(), 1);
    assert_eq!(detail.subjects[0].category, Category::People);
    assert_eq!(
        detail.subjects[0].human_id, ada,
        "the subject carries the id the UI links by, not the aggregate UUID"
    );
    assert!(
        !detail.history.is_empty(),
        "the dispatcher fills the History tab from the change log"
    );
    let log = change_log_for_research_note(&ws, &human_id).await.expect("log");
    assert!(
        log.iter()
            .any(|entry| entry.rationale.as_deref() == Some("the census and the register agree")),
        "the create's provenance reaches the change log:\n{log:?}"
    );
}

#[tokio::test]
async fn a_create_with_no_subject_is_rejected_by_the_core() {
    let (ws, session, _loc, _dir) = setup().await;

    let result = dispatch_research_note_change_set(
        &ws,
        &session,
        &ResearchNoteChangeSetRequest {
            human_id: None,
            subjects: Vec::new(),
            title: Some("Nothing to argue about".to_owned()),
            body: None,
            language: None,
        },
        &ProvenanceDraft::default(),
    )
    .await;

    assert!(
        result.is_err(),
        "a research note must name at least one subject (ADR 0028 §2): {result:?}"
    );
}

#[tokio::test]
async fn a_create_with_a_non_subject_category_is_rejected_before_any_write() {
    let (ws, session, _loc, _dir) = setup().await;

    let result = dispatch_research_note_change_set(
        &ws,
        &session,
        &ResearchNoteChangeSetRequest {
            human_id: None,
            subjects: vec![subject(Category::Sources, "S0001")],
            title: None,
            body: None,
            language: None,
        },
        &ProvenanceDraft::default(),
    )
    .await;

    assert!(
        result.is_err(),
        "only Person/Family/Event/Place may be a subject: {result:?}"
    );
    assert!(
        show_research_note(&ws, "A0001").await.expect("show").is_none(),
        "nothing was written"
    );
}

#[tokio::test]
async fn set_body_supersedes_the_argument_and_its_language() {
    let (ws, session, loc, _dir) = setup().await;
    let ada = person(&ws, &session, "Ada").await;
    let human_id = dispatch_research_note_change_set(&ws, &session, &create_request(&ada), &ProvenanceDraft::default())
        .await
        .expect("create");

    dispatch_research_note_edit(
        &ws,
        &session,
        &ResearchNoteEdit::SetBody {
            human_id: human_id.clone(),
            text: "The passenger list settles it.".to_owned(),
            language: Some("nb-NO".to_owned()),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("set body");

    let detail = load_detail(&ws, &loc, &human_id).await;
    assert_eq!(detail.body.as_deref(), Some("The passenger list settles it."));
    assert_eq!(detail.language.as_deref(), Some("nb-NO"));
}

#[tokio::test]
async fn subjects_are_added_and_removed_across_all_four_kinds() {
    let (ws, session, loc, _dir) = setup().await;
    let ada = person(&ws, &session, "Ada").await;
    let human_id = dispatch_research_note_change_set(&ws, &session, &create_request(&ada), &ProvenanceDraft::default())
        .await
        .expect("create");

    let place = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::City,
            name: Some("London".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    let event = create_event(
        &ws,
        &session,
        NewEvent {
            human_id: None,
            event_type: EventType::Marriage,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");

    for added in [subject(Category::Places, &place), subject(Category::Events, &event)] {
        dispatch_research_note_edit(
            &ws,
            &session,
            &ResearchNoteEdit::AddSubject {
                human_id: human_id.clone(),
                subject: added,
            },
            &ProvenanceDraft::default(),
        )
        .await
        .expect("add subject");
    }

    let detail = load_detail(&ws, &loc, &human_id).await;
    let mut kinds: Vec<Category> = detail.subjects.iter().map(|subject| subject.category).collect();
    kinds.sort_by_key(|category| category.id());
    assert_eq!(kinds, vec![Category::Events, Category::People, Category::Places]);

    dispatch_research_note_edit(
        &ws,
        &session,
        &ResearchNoteEdit::RemoveSubject {
            human_id: human_id.clone(),
            subject: subject(Category::Places, &place),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("remove subject");

    let detail = load_detail(&ws, &loc, &human_id).await;
    assert_eq!(detail.subjects.len(), 2, "the place subject is gone");
    assert!(
        detail
            .subjects
            .iter()
            .all(|subject| subject.category != Category::Places)
    );
    assert!(
        detail.subjects.iter().all(|subject| !subject.kind_label.is_empty()),
        "each row carries a localized kind label"
    );
}

#[tokio::test]
async fn tag_restrictions_and_undo_round_trip_through_dispatch() {
    let (ws, session, loc, _dir) = setup().await;
    let ada = person(&ws, &session, "Ada").await;
    let human_id = dispatch_research_note_change_set(&ws, &session, &create_request(&ada), &ProvenanceDraft::default())
        .await
        .expect("create");
    let tag = create_tag(&ws, &session, "Needs sources".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");

    dispatch_research_note_edit(
        &ws,
        &session,
        &ResearchNoteEdit::Tag {
            human_id: human_id.clone(),
            tag_id: tag.clone(),
            remove: false,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("tag");
    dispatch_research_note_edit(
        &ws,
        &session,
        &ResearchNoteEdit::SetRestrictions {
            human_id: human_id.clone(),
            restrictions: vec![RestrictionKind::Confidential],
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("restrict");

    let detail = load_detail(&ws, &loc, &human_id).await;
    assert_eq!(detail.tags.len(), 1, "the tag is applied");
    assert_eq!(detail.tags[0].name, "Needs sources");
    assert_eq!(detail.restrictions, vec![RestrictionKind::Confidential]);

    dispatch_research_note_edit(
        &ws,
        &session,
        &ResearchNoteEdit::Tag {
            human_id: human_id.clone(),
            tag_id: tag,
            remove: true,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("untag");

    // ⌘Z / the History tab's Undo retracts the newest undoable assertion.
    let target = load_detail(&ws, &loc, &human_id)
        .await
        .history
        .iter()
        .find(|entry| entry.can_undo)
        .expect("an undoable assertion")
        .assertion_id
        .clone();
    dispatch_research_note_edit(
        &ws,
        &session,
        &ResearchNoteEdit::UndoAssertion {
            human_id: human_id.clone(),
            assertion_id: target,
        },
        &ProvenanceDraft {
            rationale: "  recorded against the wrong note  ".to_owned(),
            ..ProvenanceDraft::default()
        },
    )
    .await
    .expect("undo");

    let detail = load_detail(&ws, &loc, &human_id).await;
    assert!(
        detail.tags.is_empty(),
        "the tag stayed removed and the undo left the note readable"
    );
    let log = change_log_for_research_note(&ws, &human_id).await.expect("log");
    assert!(
        log.iter().any(|entry| entry.event_type == "AssertionRetracted"
            && entry.rationale.as_deref() == Some("recorded against the wrong note")),
        "the retraction carries the operator's rationale:\n{log:?}"
    );
}

#[tokio::test]
async fn the_list_intent_returns_one_row_per_research_note() {
    let (ws, session, loc, _dir) = setup().await;
    let ada = person(&ws, &session, "Ada").await;
    let human_id = dispatch_research_note_change_set(&ws, &session, &create_request(&ada), &ProvenanceDraft::default())
        .await
        .expect("create");

    let outcome = dispatch(&ws, &loc, &Intent::ShowResearchNoteList)
        .await
        .expect("dispatch list");
    let IntentOutcome::List(rows) = outcome else {
        panic!("expected a list, got {outcome:?}");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, human_id);
    assert_eq!(rows[0].title, "Same person as the 1865 census entry?");
    assert!(
        rows[0].subtitle.as_deref().is_some_and(|sub| sub.contains(&ada)),
        "the row names its subject: {:?}",
        rows[0].subtitle
    );
}

#[tokio::test]
async fn a_missing_research_note_is_reported_as_not_found() {
    let (ws, _session, loc, _dir) = setup().await;

    let outcome = dispatch(
        &ws,
        &loc,
        &Intent::ShowResearchNote {
            human_id: "A9999".to_owned(),
        },
    )
    .await
    .expect("dispatch");
    let IntentOutcome::NotFound { human_id } = outcome else {
        panic!("expected NotFound, got {outcome:?}");
    };
    assert_eq!(human_id, "A9999");
}

#[tokio::test]
async fn the_reverse_lookup_rows_reach_all_four_subject_detail_screens() {
    let (ws, session, loc, _dir) = setup().await;
    let ada = person(&ws, &session, "Ada").await;
    let place = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::City,
            name: Some("London".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    let event = create_event(
        &ws,
        &session,
        NewEvent {
            human_id: None,
            event_type: EventType::Marriage,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");
    let family = vitni_app::create_family(&ws, &session, Provenance::default(), &[])
        .await
        .expect("family");

    let human_id = dispatch_research_note_change_set(
        &ws,
        &session,
        &ResearchNoteChangeSetRequest {
            human_id: None,
            subjects: vec![
                subject(Category::People, &ada),
                subject(Category::Families, &family),
                subject(Category::Events, &event),
                subject(Category::Places, &place),
            ],
            title: Some("One argument, four conclusions".to_owned()),
            body: None,
            language: None,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("create");

    let outcome = dispatch(&ws, &loc, &Intent::ShowPerson { human_id: ada })
        .await
        .expect("person");
    let IntentOutcome::Detail(person_detail) = outcome else {
        panic!("expected a person detail, got {outcome:?}");
    };
    assert_eq!(person_detail.research_notes.len(), 1);
    assert_eq!(person_detail.research_notes[0].id, human_id);
    assert_eq!(
        person_detail.research_notes[0].title, "One argument, four conclusions",
        "the reverse row shows the argument's title"
    );

    let outcome = dispatch(&ws, &loc, &Intent::ShowFamily { human_id: family })
        .await
        .expect("family");
    let IntentOutcome::FamilyDetail(family_detail) = outcome else {
        panic!("expected a family detail, got {outcome:?}");
    };
    assert_eq!(family_detail.research_notes.len(), 1);

    let outcome = dispatch(&ws, &loc, &Intent::ShowEvent { human_id: event })
        .await
        .expect("event");
    let IntentOutcome::EventDetail(event_detail) = outcome else {
        panic!("expected an event detail, got {outcome:?}");
    };
    assert_eq!(event_detail.research_notes.len(), 1);

    let outcome = dispatch(&ws, &loc, &Intent::ShowPlace { human_id: place })
        .await
        .expect("place");
    let IntentOutcome::PlaceDetail(place_detail) = outcome else {
        panic!("expected a place detail, got {outcome:?}");
    };
    assert_eq!(place_detail.research_notes.len(), 1);

    // Every one of the four tab strips offers the reverse tab, counted and localized, right after Notes.
    assert_reverse_tab(&person_tabs(&person_detail, &loc));
    assert_reverse_tab(&family_tabs(&family_detail, &loc));
    assert_reverse_tab(&event_tabs(&event_detail, &loc));
    assert_reverse_tab(&place_tabs(&place_detail, &loc));
}

/// Asserts one detail screen's tab strip offers the counted, localized reverse tab right after Notes.
fn assert_reverse_tab(tabs: &[vitni_ui::DetailTab]) {
    let position = tabs
        .iter()
        .position(|tab| tab.id == "research-notes")
        .expect("a Research notes tab");
    let notes = tabs.iter().position(|tab| tab.id == "notes").expect("a Notes tab");
    assert_eq!(position, notes + 1, "the reverse tab follows Notes");
    assert_eq!(
        tabs[position].count,
        Some(1),
        "it counts the arguments about the record"
    );
    assert_eq!(tabs[position].label, "Research notes", "the label is localized");
}

#[tokio::test]
async fn a_record_with_no_argument_about_it_has_an_empty_reverse_tab() {
    let (ws, session, loc, _dir) = setup().await;
    let ada = person(&ws, &session, "Ada").await;

    let outcome = dispatch(&ws, &loc, &Intent::ShowPerson { human_id: ada })
        .await
        .expect("person");
    let IntentOutcome::Detail(person_detail) = outcome else {
        panic!("expected a person detail, got {outcome:?}");
    };
    assert!(person_detail.research_notes.is_empty());
}
