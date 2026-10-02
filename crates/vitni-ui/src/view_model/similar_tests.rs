use vitni_app::{
    AggRef, DateParts, DraftRecord, MatchAssessment, MatchBand, MatchableKind, PersonNameParts, PlaceType,
    SimilarRecord,
};

use super::{
    NewNoteFields, NewPersonFields, NewPlaceFields, NewRecordDraft, PersonDraft, PickerHit, PickerRowVm, PlaceDraft,
    RecordDraft, RepositoryDraft, SimilarHitVm, SourceDraft, query_draft, rank_picker_rows, ranks_by_similarity,
};
use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{Category, NewRecordRequest};
use crate::picker::PICKER_MAX_ROWS;

fn year(year: i32) -> DateParts {
    DateParts {
        year,
        month: None,
        day: None,
    }
}

fn person_draft(given: &str, surname: &str, born: &str) -> PersonDraft {
    PersonDraft {
        given: given.to_owned(),
        surname: surname.to_owned(),
        born: born.to_owned(),
        ..PersonDraft::new()
    }
}

#[test]
fn a_person_being_created_is_matched_on_its_name_and_birth() {
    let draft = person_draft("Gulbrand", "Olsøn", "1852");
    let Some(DraftRecord::Person { name, birth }) = draft.similar_draft() else {
        panic!("a typed name is matchable");
    };
    assert_eq!(name.given.as_deref(), Some("Gulbrand"));
    assert_eq!(name.surname.as_deref(), Some("Olsøn"));
    assert_eq!(birth, Some(year(1852)));
}

#[test]
fn nothing_typed_or_an_existing_person_is_not_matched() {
    assert_eq!(PersonDraft::new().similar_draft(), None);
    let editing = PersonDraft {
        existing_human_id: Some("I0001".to_owned()),
        ..person_draft("Ole", "Olsen", "")
    };
    assert_eq!(editing.similar_draft(), None, "the hint is for records being created");
}

#[test]
fn an_unreadable_birth_blocks_the_save_and_is_not_matched_on() {
    let draft = person_draft("Ole", "Olsen", "spring");
    assert!(!draft.is_valid());
    let Some(DraftRecord::Person { birth, .. }) = draft.similar_draft() else {
        panic!("the name is still matchable");
    };
    assert_eq!(birth, None);
}

#[test]
fn a_typed_birth_rides_the_create_request() {
    let draft = person_draft("Ole", "Olsen", "3 Mar 1852");
    assert!(draft.is_valid());
    assert_eq!(
        draft.to_request().birth,
        Some(DateParts {
            year: 1852,
            month: Some(3),
            day: Some(3),
        })
    );
    assert_eq!(person_draft("Ole", "Olsen", " ").to_request().birth, None);
}

#[test]
fn a_new_person_from_a_picker_carries_its_birth_and_is_matched() {
    let fields = NewPersonFields {
        given: "Ole".to_owned(),
        surname: "Olsen".to_owned(),
        born: "1850".to_owned(),
    };
    let draft = NewRecordDraft::Person(fields.clone());
    let Some(NewRecordRequest::Person(request)) = draft.to_request() else {
        panic!("a named person is savable");
    };
    assert_eq!(request.birth, Some(year(1850)));
    assert_eq!(
        draft.similar_draft(),
        Some(DraftRecord::Person {
            name: PersonNameParts::simple(Some("Ole".to_owned()), Some("Olsen".to_owned())),
            birth: Some(year(1850)),
        })
    );
    let unreadable = NewRecordDraft::Person(NewPersonFields {
        born: "spring".to_owned(),
        ..fields
    });
    assert_eq!(unreadable.to_request(), None, "an unreadable birth is not savable");
}

#[test]
fn place_source_and_repository_drafts_are_matched_but_a_note_is_not() {
    let place = NewRecordDraft::Place(NewPlaceFields {
        place_type: PlaceType::Farm,
        name: "Nordås".to_owned(),
    });
    assert_eq!(
        place.similar_draft(),
        Some(DraftRecord::Place {
            name: "Nordås".to_owned(),
            place_type: Some(PlaceType::Farm),
        })
    );
    let note = NewRecordDraft::Note(NewNoteFields {
        text: "a note".to_owned(),
    });
    assert_eq!(note.similar_draft(), None);

    let place = PlaceDraft {
        name: "Nordås".to_owned(),
        ..PlaceDraft::new()
    };
    assert!(matches!(place.similar_draft(), Some(DraftRecord::Place { .. })));
    let source = SourceDraft {
        title: "Fana ministerialbok".to_owned(),
        author: "Fana prestegjeld".to_owned(),
        ..SourceDraft::new()
    };
    assert_eq!(
        source.similar_draft(),
        Some(DraftRecord::Source {
            title: "Fana ministerialbok".to_owned(),
            author: Some("Fana prestegjeld".to_owned()),
        })
    );
    let repository = RepositoryDraft {
        name: "Arkivverket".to_owned(),
        ..RepositoryDraft::new()
    };
    assert!(matches!(
        repository.similar_draft(),
        Some(DraftRecord::Repository { .. })
    ));
    assert_eq!(RepositoryDraft::new().similar_draft(), None);
}

#[test]
fn exactly_the_kinds_a_query_names_are_ranked_by_similarity() {
    for category in Category::all() {
        assert_eq!(
            ranks_by_similarity(category),
            query_draft(category, "Ole Olsen").is_some(),
            "{category:?}"
        );
    }
}

#[test]
fn a_picker_query_is_matched_as_the_record_it_would_create() {
    assert!(matches!(
        query_draft(Category::People, "Ole Olsen"),
        Some(DraftRecord::Person { .. })
    ));
    assert!(matches!(
        query_draft(Category::Sources, "Fana"),
        Some(DraftRecord::Source { .. })
    ));
    assert_eq!(query_draft(Category::Notes, "a note"), None);
    assert_eq!(query_draft(Category::People, " "), None);
}

fn similar(human_id: &str, band: MatchBand, score: f64) -> SimilarRecord {
    SimilarRecord {
        record: AggRef {
            human_id: human_id.to_owned(),
            id: format!("{human_id}-uuid"),
        },
        assessment: MatchAssessment {
            score,
            band,
            features: Vec::new(),
            cultures: Vec::new(),
            parts: Vec::new(),
            engine: vitni_app::ENGINE_VERSION,
        },
    }
}

#[test]
fn a_hit_names_the_record_its_score_and_band() {
    let loc = Localizer::for_test("en");
    let hit = SimilarHitVm::build(
        MatchableKind::Person,
        &similar("I0042", MatchBand::Probable, 0.87),
        Some("Guldbrand Olsen".to_owned()),
        &loc,
    );
    assert_eq!(hit.record.human_id, "I0042");
    assert_eq!(hit.record.label, "Guldbrand Olsen");
    assert_eq!(hit.record.category, Category::People);
    assert_eq!(hit.percent, 87);
    assert!(hit.line.contains("I0042") && hit.line.contains("87"), "{}", hit.line);
    assert!(hit.compare.is_some(), "a person pair can be compared");

    let tag = SimilarHitVm::build(
        MatchableKind::Tag,
        &similar("Emigrant", MatchBand::Possible, 0.6),
        None,
        &loc,
    );
    assert_eq!(tag.record.human_id, "Emigrant-uuid", "a tag opens by its id");
    assert_eq!(tag.record.label, "Emigrant");
    assert_eq!(tag.compare, None, "tags are never decided about");
}

#[test]
fn every_matchable_kind_has_its_category_and_back() {
    for kind in MatchableKind::ALL {
        assert_eq!(Category::from_matchable_kind(kind).matchable_kind(), Some(kind));
    }
    assert_eq!(Category::DnaTests.matchable_kind(), None);
    assert_eq!(Category::Dashboard.matchable_kind(), None);
}

fn row(id: &str, title: &str) -> RowVm {
    RowVm {
        id: id.to_owned(),
        title: title.to_owned(),
        ..RowVm::default()
    }
}

fn hit(id: &str, percent: u8) -> PickerHit {
    PickerHit {
        human_id: id.to_owned(),
        title: format!("title of {id}"),
        percent,
    }
}

fn ids(rows: &[PickerRowVm]) -> Vec<(&str, Option<u8>)> {
    rows.iter().map(|row| (row.row.id.as_str(), row.percent)).collect()
}

#[test]
fn engine_hits_rank_first_then_the_text_matches() {
    let matched = vec![row("I0001", "Ole Olsen"), row("I0003", "Ole Olsen")];
    let hits = [hit("I0003", 91), hit("I0007", 64)];
    let ranked = rank_picker_rows(matched, &hits, &[]);
    assert_eq!(
        ids(&ranked),
        [("I0003", Some(91)), ("I0007", Some(64)), ("I0001", None)],
        "a hit the text missed still shows, and a hit the text matched shows once"
    );
    assert_eq!(ranked[0].row.title, "Ole Olsen", "a text match keeps its own row");
    assert_eq!(ranked[1].row.title, "title of I0007");
}

#[test]
fn an_excluded_hit_is_dropped_and_the_list_stays_capped() {
    let ranked = rank_picker_rows(Vec::new(), &[hit("I0003", 91)], &["I0003".to_owned()]);
    assert_eq!(ranked, Vec::new());

    let matched: Vec<RowVm> = (0..PICKER_MAX_ROWS).map(|n| row(&format!("I{n:04}"), "Ole")).collect();
    let ranked = rank_picker_rows(matched.clone(), &[hit("I0099", 70)], &[]);
    assert_eq!(ranked.len(), PICKER_MAX_ROWS);
    assert_eq!(ranked[0].row.id, "I0099");

    let unranked = rank_picker_rows(matched, &[], &[]);
    assert!(unranked.iter().all(|row| row.percent.is_none()));
}
