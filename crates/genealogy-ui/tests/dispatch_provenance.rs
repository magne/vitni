//! Dispatch-layer provenance round trip: a `ProvenanceDraft` filled on an edit form reaches the
//! change log through `dispatch_*_edit`, proving the UI intent layer threads the operator's intent
//! (rationale · confidence · citations · evidence analysis) into every mutation, not just the two
//! it carried before (PR25).
//!
//! These run against a real on-disk workspace over `genealogy-app`'s public surface only — the
//! presentation layer never names a `genealogy-core` type, even in tests (ADR 0008).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    Age, AgeBound, Agent, AgentId, AgentKind, AppDefaults, Attribute, Calendar, ChangeLogEntry, DateInput,
    DateModifier, DatePoint, DateQuality, EventType, EvidenceLevel, FactType, GenealogicalDate, GenealogicalDateBody,
    NewCitation, NewEvent, NewMedia, NewNote, NewPerson, NewSource, OperatorConfig, ParticipantRole, PersonNameParts,
    Provenance, Rect, Session, Workspace, WorkspaceDefaults, build_genealogical_date, change_log_for_citation,
    change_log_for_event, change_log_for_media, change_log_for_person, change_log_for_source, create_citation,
    create_event, create_media, create_note, create_person, create_source, create_tag, show_citation, show_event,
    show_media, show_person, show_source,
};
use genealogy_ui::{
    CitationEdit, ConfidenceLevel, EventEdit, EvidenceKind, InformationKind, Localizer, MediaEdit, MergePersons,
    PersonEdit, ProvenanceDraft, SourceChangeSetRequest, SourceQuality, dispatch_citation_edit, dispatch_edit,
    dispatch_event_edit, dispatch_media_edit, dispatch_merge, dispatch_source_change_set,
};
use uuid::Uuid;

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

/// A person with no initial name, so the only `NameAsserted` in the log is the one under test.
async fn person(ws: &Workspace, session: &Session) -> String {
    create_person(
        ws,
        session,
        NewPerson {
            human_id: None,
            name: None,
            evidence_level: EvidenceLevel::Conclusion,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("person")
}

/// Creates a source + citation and returns the citation `human_id` a draft can attach.
async fn citation(ws: &Workspace, session: &Session) -> String {
    let source = create_source(
        ws,
        session,
        NewSource {
            human_id: None,
            title: Some("Parish register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("source");
    create_citation(
        ws,
        session,
        NewCitation {
            human_id: None,
            source,
            page: Some("f. 3".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("citation")
}

/// A draft carrying every axis of operator intent, so a passing round trip proves all four fields
/// are threaded (not just confidence, as before PR25).
fn filled_draft(citation_id: String) -> ProvenanceDraft {
    ProvenanceDraft {
        rationale: "  Baptism register gives the date  ".to_owned(),
        confidence: Some(ConfidenceLevel::High),
        citations: vec![citation_id],
        dna_matches: Vec::new(),
        source: Some(SourceQuality::Original),
        information: Some(InformationKind::Primary),
        evidence: Some(EvidenceKind::Direct),
        supersedes: None,
    }
}

/// The entry that carries the draft's (unique) rationale.
fn entry_with_rationale<'a>(log: &'a [ChangeLogEntry], rationale: &str) -> &'a ChangeLogEntry {
    log.iter()
        .find(|entry| entry.rationale.as_deref() == Some(rationale))
        .expect("the dispatched mutation is logged with its rationale")
}

#[tokio::test]
async fn an_edit_carries_the_drafts_provenance_into_the_change_log() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
    let citation = citation(&ws, &session).await;
    let draft = filled_draft(citation);

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AssertName {
            human_id: person.clone(),
            name: PersonNameParts::simple(Some("Ada".to_owned()), Some("Byron".to_owned())),
        },
        &draft,
    )
    .await
    .expect("dispatch AssertName");

    let log = change_log_for_person(&ws, &person).await.expect("log");
    let entry = entry_with_rationale(&log, "Baptism register gives the date");
    assert_eq!(entry.event_type, "NameAsserted", "the name assertion carries the draft");
    assert_eq!(
        entry.confidence,
        Some(genealogy_app::Confidence::High),
        "confidence threads through"
    );
    assert_eq!(entry.citations.len(), 1, "the backing citation threads through");
    assert_eq!(
        entry.evidence_analysis,
        Some(genealogy_app::EvidenceAnalysis {
            source: SourceQuality::Original,
            information: InformationKind::Primary,
            evidence: EvidenceKind::Direct,
        }),
        "the evidence analysis threads through"
    );
}

#[tokio::test]
async fn a_source_create_carries_the_drafts_provenance_and_fields() {
    let (ws, session, _dir) = setup().await;
    let citation = citation(&ws, &session).await;
    let draft = filled_draft(citation);

    let human_id = dispatch_source_change_set(
        &ws,
        &session,
        &SourceChangeSetRequest {
            human_id: None,
            title: Some("Trinity Church baptisms".to_owned()),
            author: Some("Rev. Smith".to_owned()),
            publication: None,
            abbreviation: None,
        },
        &draft,
    )
    .await
    .expect("dispatch source create");

    let source = show_source(&ws, &human_id).await.expect("show").expect("source");
    assert_eq!(source.title.as_deref(), Some("Trinity Church baptisms"));
    assert_eq!(source.author.as_deref(), Some("Rev. Smith"));

    let log = change_log_for_source(&ws, &human_id).await.expect("log");
    let entry = entry_with_rationale(&log, "Baptism register gives the date");
    assert_eq!(
        entry.confidence,
        Some(genealogy_app::Confidence::High),
        "confidence threads through"
    );
    assert_eq!(
        entry.citations.len(),
        1,
        "the backing citation threads onto the non-create command"
    );
}

#[tokio::test]
async fn an_attach_flow_carries_provenance() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
    let citation = citation(&ws, &session).await;
    let note = create_note(
        &ws,
        &session,
        NewNote {
            human_id: None,
            text: Some("An estate inventory".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("note");
    let draft = filled_draft(citation);

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AttachNote {
            human_id: person.clone(),
            note_id: note,
        },
        &draft,
    )
    .await
    .expect("dispatch AttachNote");

    let log = change_log_for_person(&ws, &person).await.expect("log");
    let entry = entry_with_rationale(&log, "Baptism register gives the date");
    assert_eq!(entry.event_type, "NoteAttached", "the note attach carries the draft");
    assert_eq!(
        entry.confidence,
        Some(genealogy_app::Confidence::High),
        "confidence threads through"
    );
    assert_eq!(entry.citations.len(), 1, "the backing citation threads through");
}

// --- PR29 step 3: per-row corrections dispatch through the intent layer ---

/// A per-row Edit fills the draft's `supersedes` with the row's assertion id; Save supersedes the
/// prior claim (replaces, not appends) rather than adding a second row.
#[tokio::test]
async fn an_edit_with_supersedes_replaces_the_fact_rather_than_appending() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AssertFact {
            human_id: person.clone(),
            fact_type: FactType::Occupation,
            value: Some("Carpenter".to_owned()),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("dispatch AssertFact");

    let target = show_person(&ws, &person)
        .await
        .expect("show")
        .expect("person")
        .facts
        .first()
        .expect("one fact")
        .assertion_id
        .clone();

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AssertFact {
            human_id: person.clone(),
            fact_type: FactType::Occupation,
            value: Some("Joiner".to_owned()),
        },
        &ProvenanceDraft {
            supersedes: Some(target.clone()),
            ..ProvenanceDraft::default()
        },
    )
    .await
    .expect("dispatch superseding AssertFact");

    let summary = show_person(&ws, &person).await.expect("show").expect("person");
    assert_eq!(summary.facts.len(), 1, "the edit supersedes rather than appends");
    assert_eq!(summary.facts[0].fact.value.as_deref(), Some("Joiner"));
    assert_ne!(
        summary.facts[0].assertion_id, target,
        "the surviving row has a new assertion id"
    );
}

/// A Retract dispatch threads the draft's rationale into the change log's retraction entry.
#[tokio::test]
async fn a_retract_carries_the_drafts_rationale() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
    let note = create_note(
        &ws,
        &session,
        NewNote {
            human_id: None,
            text: Some("An estate inventory".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("note");
    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AttachNote {
            human_id: person.clone(),
            note_id: note,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("attach note");
    let attach = show_person(&ws, &person)
        .await
        .expect("show")
        .expect("person")
        .notes
        .first()
        .expect("one note")
        .assertion_id
        .clone();

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::UndoAssertion {
            human_id: person.clone(),
            assertion_id: attach,
        },
        &ProvenanceDraft {
            rationale: "  attached to the wrong person  ".to_owned(),
            ..ProvenanceDraft::default()
        },
    )
    .await
    .expect("retract");

    let log = change_log_for_person(&ws, &person).await.expect("log");
    let entry = entry_with_rationale(&log, "attached to the wrong person");
    assert_eq!(
        entry.event_type, "AssertionRetracted",
        "the retraction carries the rationale"
    );
    assert!(
        show_person(&ws, &person)
            .await
            .expect("show")
            .expect("person")
            .notes
            .is_empty(),
        "the retracted note is gone from the conclusion view"
    );
}

/// The new `PersonEdit::Tag` intent applies and removes a tag round trip (data-model §9).
#[tokio::test]
async fn tag_and_untag_round_trip_through_dispatch() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
    let tag = create_tag(&ws, &session, "Direct ancestor".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::Tag {
            human_id: person.clone(),
            tag_id: tag.clone(),
            remove: false,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("apply tag");
    assert_eq!(
        show_person(&ws, &person).await.expect("show").expect("person").tags,
        vec![tag.clone()],
        "the tag is applied"
    );

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::Tag {
            human_id: person.clone(),
            tag_id: tag,
            remove: true,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("remove tag");
    assert!(
        show_person(&ws, &person)
            .await
            .expect("show")
            .expect("person")
            .tags
            .is_empty(),
        "the tag is removed"
    );
}

/// A named person, so a merge has two resolvable records.
async fn named_person(ws: &Workspace, session: &Session, given: &str, surname: &str) -> String {
    create_person(
        ws,
        session,
        NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                Some(given.to_owned()),
                Some(surname.to_owned()),
            )),
            evidence_level: EvidenceLevel::Conclusion,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("person")
}

/// The `PersonsMerged` entry from the survivor's change log.
fn merge_entry(log: &[ChangeLogEntry]) -> &ChangeLogEntry {
    log.iter()
        .find(|entry| entry.event_type == "PersonsMerged")
        .expect("the merge is logged as a PersonsMerged event")
}

/// A merge dispatched with a rationale threads that (trimmed) rationale onto the `PersonsMerged`
/// event's provenance.
#[tokio::test]
async fn a_merge_carries_its_rationale_into_the_change_log() {
    let (ws, session, dir) = setup().await;
    let loc = Localizer::for_workspace(&dir.path().join("ws"), None);
    let survivor = named_person(&ws, &session, "John", "Smith").await;
    let merged = named_person(&ws, &session, "John", "Smyth").await;

    dispatch_merge(
        &ws,
        &session,
        &loc,
        &MergePersons {
            surviving_human_id: survivor.clone(),
            merged_human_id: merged,
            rationale: Some("  Same person: name variant  ".to_owned()),
        },
    )
    .await
    .expect("dispatch merge");

    let log = change_log_for_person(&ws, &survivor).await.expect("log");
    assert_eq!(
        merge_entry(&log).rationale.as_deref(),
        Some("Same person: name variant"),
        "the merge event carries the trimmed rationale"
    );
}

/// A merge dispatched with a blank rationale normalizes to `None`, so the app records its default
/// ("Merge") rather than an empty string.
#[tokio::test]
async fn a_blank_merge_rationale_falls_back_to_the_default() {
    let (ws, session, dir) = setup().await;
    let loc = Localizer::for_workspace(&dir.path().join("ws"), None);
    let survivor = named_person(&ws, &session, "Mary", "Doe").await;
    let merged = named_person(&ws, &session, "Mary", "Doe").await;

    dispatch_merge(
        &ws,
        &session,
        &loc,
        &MergePersons {
            surviving_human_id: survivor.clone(),
            merged_human_id: merged,
            rationale: Some("   ".to_owned()),
        },
    )
    .await
    .expect("dispatch merge");

    let log = change_log_for_person(&ws, &survivor).await.expect("log");
    assert_eq!(
        merge_entry(&log).rationale.as_deref(),
        Some("Merge"),
        "a blank rationale falls back to the app default"
    );
}

/// The `PersonEdit::AssertParticipation` intent records a participation with its age, attributes, and
/// notes (ADR 0019), all surfacing on the person projection.
#[tokio::test]
async fn assert_participation_dispatches() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
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
    let note = create_note(
        &ws,
        &session,
        NewNote {
            human_id: None,
            text: Some("a witness note".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("note");

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AssertParticipation {
            human_id: person.clone(),
            event_id: event,
            role: ParticipantRole::Groom,
            age: Some(Age {
                bound: Some(AgeBound::GreaterThan),
                years: Some(28),
                months: None,
                days: None,
                phrase: None,
            }),
            attributes: vec![Attribute {
                attribute_type: "occupation".to_owned(),
                value: "farmer".to_owned(),
            }],
            notes: vec![note.clone()],
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("assert participation");

    let summary = show_person(&ws, &person).await.expect("show").expect("person");
    assert_eq!(summary.participations.len(), 1, "the participation is recorded");
    let row = &summary.participations[0];
    assert_eq!(row.role, ParticipantRole::Groom);
    assert_eq!(row.age.as_ref().and_then(|age| age.years), Some(28));
    assert_eq!(row.attributes.len(), 1, "the attribute surfaces");
    assert_eq!(row.notes.len(), 1, "the note resolves");
    assert_eq!(row.notes[0].human_id, note);
}

/// A per-row Edit of a participation supersedes the prior row. Because the intent carries the full
/// prefilled extras, changing only the role must not drop the age, attributes, or notes (ADR 0019).
#[tokio::test]
async fn superseding_a_participation_preserves_age_attributes_and_notes() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
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
    let note = create_note(
        &ws,
        &session,
        NewNote {
            human_id: None,
            text: Some("a witness note".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("note");

    let age = Age {
        years: Some(30),
        ..Age::default()
    };
    let attributes = vec![Attribute {
        attribute_type: "residence".to_owned(),
        value: "Bergen".to_owned(),
    }];
    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AssertParticipation {
            human_id: person.clone(),
            event_id: event.clone(),
            role: ParticipantRole::Witness,
            age: Some(age.clone()),
            attributes: attributes.clone(),
            notes: vec![note.clone()],
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("assert participation");

    let target = show_person(&ws, &person)
        .await
        .expect("show")
        .expect("person")
        .participations
        .first()
        .expect("one participation")
        .assertion_id
        .clone();

    // A role-only edit carries the full prefilled extras and supersedes the prior row.
    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AssertParticipation {
            human_id: person.clone(),
            event_id: event,
            role: ParticipantRole::Groom,
            age: Some(age.clone()),
            attributes: attributes.clone(),
            notes: vec![note.clone()],
        },
        &ProvenanceDraft {
            supersedes: Some(target),
            ..ProvenanceDraft::default()
        },
    )
    .await
    .expect("dispatch superseding participation");

    let summary = show_person(&ws, &person).await.expect("show").expect("person");
    assert_eq!(
        summary.participations.len(),
        1,
        "the edit supersedes rather than appends"
    );
    let row = &summary.participations[0];
    assert_eq!(row.role, ParticipantRole::Groom, "the role changed");
    assert_eq!(row.age.as_ref().and_then(|a| a.years), Some(30), "the age survived");
    assert_eq!(row.attributes.len(), 1, "the attribute survived");
    assert_eq!(row.notes.len(), 1, "the note survived");
}

/// A structured date exercising the full grammar: an Estimated, Julian, modified date carrying its
/// verbatim original text.
fn full_grammar_date(modifier: DateModifier) -> GenealogicalDate {
    build_genealogical_date(DateInput {
        calendar: Calendar::Julian,
        quality: DateQuality::Estimated,
        body: GenealogicalDateBody::Structured(modifier),
        new_year_begins: None,
        original_text: Some("abt 14 Jun 1876".to_owned()),
        time: None,
    })
}

fn point(year: i32, month: u8, day: u8) -> DatePoint {
    DatePoint {
        year: Some(year),
        month: Some(month),
        day: Some(day),
    }
}

#[tokio::test]
async fn event_set_date_dispatches_the_full_grammar() {
    let (ws, session, _dir) = setup().await;
    let citation = citation(&ws, &session).await;
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
    let date = full_grammar_date(DateModifier::About(point(1876, 6, 14)));

    dispatch_event_edit(
        &ws,
        &session,
        &EventEdit::SetDate {
            human_id: event.clone(),
            date: date_input(&date),
        },
        &filled_draft(citation),
    )
    .await
    .expect("dispatch SetDate");

    let summary = show_event(&ws, &event).await.expect("show").expect("event");
    assert_eq!(summary.date.as_ref(), Some(&date), "the full date round-trips");
    let log = change_log_for_event(&ws, &event).await.expect("log");
    entry_with_rationale(&log, "Baptism register gives the date");
}

#[tokio::test]
async fn citation_set_date_dispatches_the_full_grammar() {
    let (ws, session, _dir) = setup().await;
    let backing = citation(&ws, &session).await;
    let target = citation(&ws, &session).await;
    let date = full_grammar_date(DateModifier::Range {
        start: point(1876, 6, 14),
        end: point(1880, 1, 1),
    });

    dispatch_citation_edit(
        &ws,
        &session,
        &CitationEdit::SetDate {
            human_id: target.clone(),
            date: date_input(&date),
        },
        &filled_draft(backing),
    )
    .await
    .expect("dispatch SetDate");

    let summary = show_citation(&ws, &target).await.expect("show").expect("citation");
    assert_eq!(summary.date.as_ref(), Some(&date), "the full date round-trips");
    let log = change_log_for_citation(&ws, &target).await.expect("log");
    entry_with_rationale(&log, "Baptism register gives the date");
}

#[tokio::test]
async fn media_set_date_dispatches_the_full_grammar() {
    let (ws, session, _dir) = setup().await;
    let citation = citation(&ws, &session).await;
    let media = create_media(
        &ws,
        &session,
        NewMedia {
            human_id: None,
            path: Some("portrait.jpg".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("media");
    let date = full_grammar_date(DateModifier::Span {
        start: point(1876, 6, 14),
        end: point(1880, 1, 1),
    });

    dispatch_media_edit(
        &ws,
        &session,
        &MediaEdit::SetDate {
            human_id: media.clone(),
            date: date_input(&date),
        },
        &filled_draft(citation),
    )
    .await
    .expect("dispatch SetDate");

    let summary = show_media(&ws, &media).await.expect("show").expect("media");
    assert_eq!(summary.date.as_ref(), Some(&date), "the full date round-trips");
    let log = change_log_for_media(&ws, &media).await.expect("log");
    entry_with_rationale(&log, "Baptism register gives the date");
}

/// Rebuilds the [`DateInput`] the UI carries in a `SetDate` intent from a built date (the inverse of
/// `build_genealogical_date`, for the tests' assertions).
fn date_input(date: &GenealogicalDate) -> DateInput {
    DateInput {
        calendar: date.calendar,
        quality: date.quality,
        body: date.modifier.clone(),
        new_year_begins: date.new_year_begins,
        original_text: date.original_text.clone(),
        time: date.time,
    }
}

/// The media viewer's Set region action dispatches `PersonEdit::SetMediaRegion`, which supersedes the
/// attach assertion with the new crop + caption while the audit trail keeps both (ADR 0017 §GUI).
#[tokio::test]
async fn set_media_region_supersedes_the_person_media_crop() {
    let (ws, session, _dir) = setup().await;
    let subject = person(&ws, &session).await;
    let media = create_media(
        &ws,
        &session,
        NewMedia {
            human_id: None,
            path: Some("photos/group.jpg".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("media");
    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AttachMedia {
            human_id: subject.clone(),
            media_id: media.clone(),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("attach media");

    let attach = show_person(&ws, &subject)
        .await
        .expect("show")
        .expect("person")
        .media
        .first()
        .expect("one media ref")
        .assertion_id
        .clone();

    let crop = Rect {
        left: 10,
        top: 20,
        width: 30,
        height: 40,
    };
    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::SetMediaRegion {
            human_id: subject.clone(),
            assertion_id: attach.clone(),
            crop: Some(crop),
            caption: Some("face".to_owned()),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("set region");

    let after = show_person(&ws, &subject).await.expect("show").expect("person");
    let row = after.media.first().expect("one media ref");
    assert_eq!(row.crop, Some(crop), "the superseding ref carries the new crop");
    assert_eq!(row.caption.as_deref(), Some("face"));
    assert_ne!(row.assertion_id, attach, "the surviving row has a new assertion id");

    let log = change_log_for_person(&ws, &subject).await.expect("log");
    assert!(
        log.iter().any(|entry| entry.event_type == "AssertionSuperseded"),
        "the region change is recorded as a supersession"
    );
}
