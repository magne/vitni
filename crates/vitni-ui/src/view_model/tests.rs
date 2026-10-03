use super::{
    AttachedRefVm, CitationDetail, DEFAULT_TAG_COLOR, DEFAULT_TAG_PRIORITY, DashboardVm, DataQualityVm, MediaRefVm,
    PersonDetail, PersonDraft, ProvenanceDraft, RecordDraft, TagDetail, TagDraft, TimelineKind, citation_row,
    citation_tabs, evidence_axes, person_row, person_tabs,
};
use crate::i18n::Localizer;
use crate::presentation::ConfidenceLevel;
use crate::presentation::EvidenceAxis;
use crate::presentation::RestrictionKind;
use std::collections::BTreeSet;
use vitni_app::ImportRunId;
use vitni_app::{
    ActivityDetail, AssociationRole, AssociationSummary, Calendar, ChangeLogEntry, CitationSummary, Confidence,
    DateModifier, DatePoint, DateQuality, EvidenceAnalysis, EvidenceKind, EvidenceLevel, Fact, FactSummary, FactType,
    GenealogicalDate, GenealogicalDateBody, InformationKind, NameSummary, NameType, OperatorKind, PersonName,
    PersonRow, PersonSummary, Restriction, RunRef, Sex, SourceQuality, Surname, TagRef, WorkspaceCounts,
};

/// A change-log entry for the activity-feed tests.
fn log_entry(kind: &str, human_id: Option<&str>, operator: OperatorKind, who: &str) -> ChangeLogEntry {
    ChangeLogEntry {
        aggregate_kind: kind.to_owned(),
        aggregate_human_id: human_id.map(ToOwned::to_owned),
        assertion_id: "a".to_owned(),
        sequence: 1,
        event_type: "PersonCreated".to_owned(),
        occurred_at: "2026-06-22T14:35:00Z".to_owned(),
        operator_display: Some(who.to_owned()),
        operator_kind: operator,
        confidence: Some(Confidence::Normal),
        rationale: None,
        citations: Vec::new(),
        evidence_analysis: None,
        detail: None,
        can_undo: false,
        run: None,
    }
}

/// The import run the run-row tests fold entries under.
fn run_ref(records: Option<u32>) -> RunRef {
    RunRef {
        id: ImportRunId::from_uuid(uuid::Uuid::from_u128(9)),
        source_label: "tree.ged".to_owned(),
        plugin: "gedcom-import".to_owned(),
        records,
    }
}

/// An entry the run wrote into `human_id`'s record.
fn imported(human_id: &str, assertion_id: &str, can_undo: bool) -> ChangeLogEntry {
    let mut entry = log_entry("person", Some(human_id), OperatorKind::Software, "gedcom-import");
    entry.assertion_id = assertion_id.to_owned();
    entry.can_undo = can_undo;
    entry.run = Some(run_ref(Some(3)));
    entry
}

#[test]
fn dashboard_renders_an_import_run_row_and_labels_records_by_name() {
    let loc = Localizer::for_test("en");
    // `summary()` is the person I0001 / "Ada Lovelace".
    let person = summary();
    // The app folds an import run into one row; then a human edit on a person.
    let children = vec![imported("I0002", "c", false), imported("I0003", "b", false)];
    let mut import = log_entry("", None, OperatorKind::Software, "gedcom-import");
    import.event_type = "ImportRun".to_owned();
    import.detail = Some(ActivityDetail::ImportRun {
        run: run_ref(Some(3)),
        count: 2,
        children,
    });
    let activity = vec![import, log_entry("person", Some("I0001"), OperatorKind::Human, "magne")];
    let vm = DashboardVm::build(WorkspaceCounts::default(), &[person], &activity, &loc, 4);

    assert_eq!(vm.recent.len(), 2);
    assert_eq!(vm.recent[0].what, "Imported from tree.ged");
    assert_eq!(
        vm.recent[0].count.as_deref(),
        Some("3 records"),
        "the run's own record count, not the rows the window happened to read"
    );
    assert_eq!(vm.recent[0].children.len(), 2, "the row carries its children");
    assert!(vm.recent[0].record.is_none(), "an import run spans many records");
    // The human edit links to the person by display name, not the human id.
    let linked = vm.recent[1].record.as_ref().expect("person record");
    assert_eq!(linked.label, "Ada Lovelace");
    assert_eq!(linked.human_id, "I0001");
    assert_eq!(vm.recent[1].count, None);
    // Jump-back surfaces the same named record.
    assert_eq!(vm.jump_back.len(), 1);
    assert_eq!(vm.jump_back[0].record.label, "Ada Lovelace");
}

#[test]
fn dashboard_summary_names_the_fact_kind() {
    let loc = Localizer::for_test("en");
    let mut entry = log_entry("person", Some("I0001"), OperatorKind::Human, "magne");
    entry.event_type = "FactAsserted".to_owned();
    entry.detail = Some(ActivityDetail::Fact {
        fact_type: FactType::Occupation,
    });
    let vm = DashboardVm::build(WorkspaceCounts::default(), &[summary()], &[entry], &loc, 4);
    assert_eq!(
        vm.recent[0].what, "Occupation asserted",
        "a fact assertion names its kind"
    );
}

fn agg(human_id: &str, id: &str) -> vitni_app::AggRef {
    vitni_app::AggRef {
        human_id: human_id.to_owned(),
        id: id.to_owned(),
    }
}

/// The engine's evidence for a pair: `terms` as (feature, outcome, weight in basis points).
fn evidence(
    score_bp: u16,
    band: vitni_app::MatchBand,
    terms: &[(vitni_app::Feature, vitni_app::OutcomeEvidence, i32)],
) -> vitni_app::MatchEvidence {
    let mut features = Vec::new();
    for (feature, outcome, weight_bp) in terms {
        features.push(vitni_app::FeatureEvidence {
            feature: *feature,
            outcome: *outcome,
            weight_bp: *weight_bp,
        });
    }
    vitni_app::MatchEvidence {
        score_bp,
        band,
        engine: vitni_app::EngineVersion(4),
        cultures: Vec::new(),
        features,
    }
}

fn duplicate(
    kind: vitni_app::MatchableKind,
    a: vitni_app::AggRef,
    b: vitni_app::AggRef,
    assessment: vitni_app::MatchEvidence,
) -> vitni_app::CheckFinding {
    vitni_app::CheckFinding::PossibleDuplicate { kind, a, b, assessment }
}

#[test]
fn data_quality_maps_check_findings_to_navigable_rows() {
    use vitni_app::{CheckFinding, Feature, MatchBand, MatchableKind, OutcomeEvidence};
    let loc = Localizer::for_test("en");
    // `summary()` is person I0001 / "Ada Lovelace"; the findings flag her lifespan and one dup pair.
    let findings = vec![
        CheckFinding::DeathBeforeBirth(agg("I0001", "I0001")),
        duplicate(
            MatchableKind::Person,
            agg("I0001", "a-id"),
            agg("I0002", "b-id"),
            evidence(
                9_700,
                MatchBand::Probable,
                &[(Feature::GivenName, OutcomeEvidence::Agree, 30_000)],
            ),
        ),
    ];
    let vm = DataQualityVm::build(&[summary()], &findings, &loc);

    assert_eq!(vm.death_before_birth.len(), 1);
    // The flagged person is a navigable People record labelled by display name, not the id.
    assert_eq!(vm.death_before_birth[0].human_id, "I0001");
    assert_eq!(vm.death_before_birth[0].label, "Ada Lovelace");
    assert_eq!(vm.matches.len(), 1, "each duplicate finding is one pair");
    let pair = &vm.matches[0];
    assert_eq!(pair.a.category, crate::navigation::Category::People);
    assert_eq!(pair.a.label, "Ada Lovelace", "a person is named, not numbered");
    assert_eq!(pair.b.label, "I0002", "an unnamed person falls back to its id");
    assert_eq!(pair.percent, 97);
    assert_eq!(pair.band, "probable match");
    assert_eq!(pair.reasons, ["Same given name (+3.0)"]);
}

#[test]
fn a_place_duplicate_is_a_place_row_with_its_reasons() {
    use vitni_app::{Feature, MatchBand, MatchableKind, OutcomeEvidence};
    let loc = Localizer::for_test("en");
    let findings = vec![duplicate(
        MatchableKind::Place,
        agg("P0001", "p1-id"),
        agg("P0002", "p2-id"),
        evidence(
            8_000,
            MatchBand::Possible,
            &[
                (Feature::PlaceType, OutcomeEvidence::Agree, 10_000),
                (Feature::Enclosure, OutcomeEvidence::Missing, 0),
                (Feature::PlaceName, OutcomeEvidence::Agree, 50_000),
                (Feature::Coordinates, OutcomeEvidence::Disagree, -12_345),
            ],
        ),
    )];
    let vm = DataQualityVm::build(&[summary()], &findings, &loc);

    let pair = &vm.matches[0];
    assert_eq!(pair.a.category, crate::navigation::Category::Places);
    assert_eq!((pair.a.human_id.as_str(), pair.b.human_id.as_str()), ("P0001", "P0002"));
    assert_eq!(pair.percent, 80);
    assert_eq!(
        pair.reasons,
        [
            "Same place name (+5.0)",
            "Same place type (+1.0)",
            "Different coordinates (\u{2212}1.2)",
        ],
        "the strongest term first; a term with no value on one side explains nothing"
    );
}

#[test]
fn a_tag_pair_is_no_possible_match() {
    use vitni_app::{MatchBand, MatchableKind};
    let loc = Localizer::for_test("en");
    let findings = vec![duplicate(
        MatchableKind::Tag,
        agg("Emigrant", "0190-tag-a"),
        agg("Emigrants", "0190-tag-b"),
        evidence(7_000, MatchBand::Possible, &[]),
    )];
    let vm = DataQualityVm::build(&[], &findings, &loc);
    assert!(
        vm.matches.is_empty(),
        "a tag is never decided as a pair: {:?}",
        vm.matches
    );
    assert_eq!(vm.match_counts, [] as [String; 0]);
}

#[test]
fn the_possible_matches_are_counted_per_kind() {
    use vitni_app::{MatchBand, MatchableKind};
    let loc = Localizer::for_test("en");
    let pair =
        |kind, a: &str, b: &str| duplicate(kind, agg(a, a), agg(b, b), evidence(7_000, MatchBand::Possible, &[]));
    let findings = vec![
        pair(MatchableKind::Place, "P0001", "P0002"),
        pair(MatchableKind::Person, "I0001", "I0002"),
        pair(MatchableKind::Place, "P0003", "P0004"),
    ];
    let vm = DataQualityVm::build(&[], &findings, &loc);
    assert_eq!(vm.match_counts, ["Person: 1", "Place: 2"], "in kind order");
    assert_eq!(vm.matches[0].kind_label, "Place");
}

#[test]
fn match_reasons_localize_to_norwegian() {
    use vitni_app::{Feature, MatchBand, OutcomeEvidence};
    let loc = Localizer::for_test("no");
    let reasons = loc.match_reasons(&evidence(
        6_000,
        MatchBand::Possible,
        &[
            (Feature::Birth, OutcomeEvidence::Partial(7_000), 12_000),
            (Feature::Surname, OutcomeEvidence::Conflict, -50_000),
        ],
    ));
    assert_eq!(reasons, ["Fødsel ligner (+1,2)", "Etternavn er i strid (\u{2212}5,0)"]);
}

#[test]
fn every_feature_and_outcome_has_a_reason_in_each_language() {
    use vitni_app::{Feature, MatchBand, OutcomeEvidence};
    let features = [
        Feature::GivenName,
        Feature::Surname,
        Feature::Sex,
        Feature::Birth,
        Feature::Death,
        Feature::BirthPlace,
        Feature::DeathPlace,
        Feature::Lifespan,
        Feature::Father,
        Feature::Mother,
        Feature::Partners,
        Feature::Children,
        Feature::Patronymic,
        Feature::Occupation,
        Feature::Record,
        Feature::Partner,
        Feature::Marriage,
        Feature::MarriagePlace,
        Feature::EventType,
        Feature::Date,
        Feature::Place,
        Feature::Principal,
        Feature::Participants,
        Feature::PlaceName,
        Feature::PlaceType,
        Feature::Enclosure,
        Feature::Coordinates,
        Feature::Title,
        Feature::Author,
        Feature::Publication,
        Feature::Repository,
        Feature::Name,
        Feature::Address,
        Feature::Source,
        Feature::Page,
        Feature::Checksum,
        Feature::Path,
        Feature::Text,
    ];
    let outcomes = [
        OutcomeEvidence::Agree,
        OutcomeEvidence::Partial(5_000),
        OutcomeEvidence::Disagree,
        OutcomeEvidence::Conflict,
    ];
    for language in ["en", "no"] {
        let loc = Localizer::for_test(language);
        for feature in features {
            for outcome in outcomes {
                let reasons = loc.match_reasons(&evidence(5_000, MatchBand::Possible, &[(feature, outcome, 0)]));
                assert_eq!(reasons.len(), 1);
                assert!(
                    !reasons[0].contains("match-") && !reasons[0].contains('{'),
                    "{language} {feature:?} {outcome:?}: {}",
                    reasons[0]
                );
            }
        }
    }
}

#[test]
fn data_quality_reports_zero_counts_with_no_findings() {
    let vm = DataQualityVm::build(&[summary()], &[], &Localizer::for_test("en"));
    assert!(vm.death_before_birth.is_empty(), "{:?}", vm.death_before_birth);
    assert!(vm.matches.is_empty(), "{:?}", vm.matches);
}

#[test]
fn history_folds_a_records_import_run_into_one_undoable_row() {
    use super::{collapse_history, first_undoable};
    let loc = Localizer::for_test("en");
    // Newest-first order: "newest" is the more recent imported assertion, "older" the one behind it.
    let entries = vec![
        log_entry("person", Some("I0001"), OperatorKind::Human, "magne"),
        imported("I0001", "newest", true),
        imported("I0001", "older", true),
    ];
    let rows = collapse_history(&entries, &loc);
    assert_eq!(rows.len(), 2, "the run's two entries fold into one row");
    assert_eq!(rows[0].what, "Person created", "the human edit stays an individual row");
    assert_eq!(
        rows[1].what, "Imported from tree.ged",
        "the row names what the run imported"
    );
    assert_eq!(
        rows[1].count.as_deref(),
        Some("2 changes"),
        "on one record the count is of its own changes, not of records imported"
    );
    assert_eq!(rows[1].children.len(), 2, "the row lists its children");
    assert_eq!(rows[1].children[0].assertion_id, "newest");
    assert!(rows[1].can_undo, "the row carries the newest entry's can_undo");
    assert_eq!(
        rows[1].assertion_id, "newest",
        "the row's undo target is its newest assertion"
    );
    assert_eq!(
        first_undoable(&rows).map(|entry| entry.assertion_id.as_str()),
        Some("newest"),
        "⌘Z retracts the newest assertion of the run, not an older entry buried behind it"
    );
}

fn year(year: i32) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(year),
            month: None,
            day: None,
        })),
        quality: DateQuality::Normal,
        time: None,
        new_year_begins: None,
        sort_value: 0,
        original_text: None,
    }
}

fn birth_name() -> PersonName {
    PersonName {
        name_type: NameType::BirthName,
        given: Some("Ada".to_owned()),
        surnames: vec![Surname {
            prefix: None,
            surname: "Lovelace".to_owned(),
            primary: true,
            connector: None,
        }],
        suffix: None,
        title: None,
        nickname: None,
        call_name: None,
        date: None,
        language: None,
        transliterations: Vec::new(),
    }
}

fn occupation_fact() -> FactSummary {
    FactSummary {
        fact: Fact {
            fact_type: FactType::Occupation,
            date: None,
            place_id: None,
            value: Some("Mathematician".to_owned()),
        },
        confidence: Some(Confidence::High),
        // The fact's backing citation, resolved from the assertion envelope (ADR 0020).
        citations: vec![vitni_app::CitationRef {
            human_id: "C0002".to_owned(),
            id: "55555555-5555-7555-8555-555555555555".to_owned(),
            assertion_id: Some("aaaaaaaa-0000-7000-8000-000000000002".to_owned()),
            source: None,
            source_title: None,
            page: None,
            backs_count: 0,
            confidence: None,
            analysis: None,
            asserted_by: None,
            asserted_by_kind: None,
            asserted_at: None,
        }],
        assertion_id: "aaaaaaaa-0000-7000-8000-000000000002".to_owned(),
    }
}

fn summary() -> PersonSummary {
    PersonSummary {
        human_id: "I0001".to_owned(),
        evidence_level: EvidenceLevel::Conclusion,
        display_name: Some("Ada Lovelace".to_owned()),
        given: Some("Ada".to_owned()),
        surname: Some("Lovelace".to_owned()),
        surname_prefix: None,
        nickname: None,
        name_prefix: None,
        name_suffix: None,
        name_type: None,
        primary_name_assertion: None,
        names: vec![NameSummary {
            name: birth_name(),
            confidence: Some(Confidence::High),
            source_count: 1,
            assertion_id: "aaaaaaaa-0000-7000-8000-000000000001".to_owned(),
        }],
        sex: Some(Sex::Female),
        birth_date: None,
        death_date: None,
        facts: vec![occupation_fact()],
        associations: vec![AssociationSummary {
            other: vitni_app::AggRef {
                human_id: "I0002".to_owned(),
                id: "11111111-1111-7111-8111-111111111111".to_owned(),
            },
            role: AssociationRole::Godparent,
            confidence: Some(Confidence::Normal),
            source_count: 0,
            assertion_id: "aaaaaaaa-0000-7000-8000-000000000004".to_owned(),
        }],
        participations: Vec::new(),
        citations: vec![vitni_app::CitationRef {
            human_id: "C0001".to_owned(),
            id: "22222222-2222-7222-8222-222222222222".to_owned(),
            assertion_id: Some("aaaaaaaa-0000-7000-8000-000000000005".to_owned()),
            source: None,
            source_title: None,
            page: None,
            backs_count: 0,
            confidence: None,
            analysis: None,
            asserted_by: None,
            asserted_by_kind: None,
            asserted_at: None,
        }],
        media: Vec::new(),
        notes: vec![
            vitni_app::AttachedRef {
                human_id: "N0001".to_owned(),
                id: "33333333-3333-7333-8333-333333333333".to_owned(),
                note_type: None,
                text: None,
                language: None,
                assertion_id: "aaaaaaaa-0000-7000-8000-000000000006".to_owned(),
            },
            vitni_app::AttachedRef {
                human_id: "N0002".to_owned(),
                id: "44444444-4444-7444-8444-444444444444".to_owned(),
                note_type: None,
                text: None,
                language: None,
                assertion_id: "aaaaaaaa-0000-7000-8000-000000000007".to_owned(),
            },
        ],
        tags: Vec::new(),
        tag_refs: Vec::new(),
        restrictions: BTreeSet::new(),
        merged: Vec::new(),
        claim_owners: std::collections::BTreeMap::new(),
    }
}

#[test]
fn row_localizes_name_sex_and_initials() {
    let loc = Localizer::for_test("en");
    let row = person_row(&summary(), &loc);
    assert_eq!(row.id, "I0001");
    assert_eq!(row.title, "Ada Lovelace");
    assert_eq!(row.subtitle.as_deref(), Some("female"));
    assert_eq!(row.avatar.as_deref(), Some("AL"));
}

#[test]
fn person_list_row_renders_identically_to_the_full_summary_row() {
    use super::person_list_row;
    let loc = Localizer::for_test("en");
    let summary = summary();
    // The lightweight list-row DTO carries the same name/sex fields the summary does, so it must build
    // the identical RowVm (same title, sex subtitle, and initials avatar).
    let light = PersonRow {
        human_id: summary.human_id.clone(),
        display_name: summary.display_name.clone(),
        given: summary.given.clone(),
        surname: summary.surname.clone(),
        sex: summary.sex.clone(),
    };
    assert_eq!(person_list_row(&light, &loc), person_row(&summary, &loc));
}

#[test]
fn empty_draft_builds_a_create_request_with_no_name() {
    use super::PersonDraft;
    let request = PersonDraft::new().to_request();
    assert_eq!(request.existing_human_id, None, "an empty draft creates a new person");
    assert!(request.name.is_none(), "a blank name asserts nothing");
    assert_eq!(request.sex, Some(Sex::Unknown));
    assert!(request.new_sources.is_empty() && request.new_citations.is_empty());
}

#[test]
fn edit_draft_seeds_from_the_summary_and_targets_the_existing_person() {
    use super::PersonDraft;
    let draft = PersonDraft::from_summary(&summary());
    assert_eq!(draft.existing_human_id.as_deref(), Some("I0001"));
    assert_eq!(draft.given, "Ada");
    assert_eq!(draft.surname, "Lovelace");
    assert_eq!(draft.sex, Sex::Female);
    let request = draft.to_request();
    assert_eq!(
        request.existing_human_id.as_deref(),
        Some("I0001"),
        "an edit targets the person"
    );
    let name = request.name.expect("a seeded name");
    assert_eq!(name.given.as_deref(), Some("Ada"));
    assert_eq!(name.surname.as_deref(), Some("Lovelace"));
}

#[test]
fn a_person_edit_draft_seeds_the_restrictions_and_offers_the_field() {
    let summary = PersonSummary {
        restrictions: BTreeSet::from([Restriction::Privacy]),
        ..summary()
    };
    let draft = PersonDraft::from_summary(&summary);
    assert_eq!(draft.restrictions, vec![RestrictionKind::Privacy]);
    assert_eq!(
        draft.editable_restrictions(),
        Some([RestrictionKind::Privacy].as_slice()),
        "a stored person offers the restriction field"
    );
    assert_eq!(
        PersonDraft::new().editable_restrictions(),
        None,
        "the create change-set carries no restrictions, so the create form hides the field"
    );
}

#[test]
fn a_person_draft_differing_only_in_restrictions_is_dirty_and_yields_one_edit() {
    let seed = PersonDraft::from_summary(&summary());
    let mut draft = seed.clone();
    draft.set_restrictions(vec![RestrictionKind::Locked]);
    assert!(
        draft.is_dirty_against(&seed),
        "a restriction change alone makes Save available"
    );
    let edit = draft.restriction_edit(&seed, "I0001").expect("a restriction edit");
    assert_eq!(
        edit,
        crate::navigation::PersonEdit::SetRestrictions {
            human_id: "I0001".to_owned(),
            restrictions: vec![RestrictionKind::Locked],
        }
    );
}

#[test]
fn an_unchanged_person_restriction_set_yields_no_edit() {
    let seed = PersonDraft::from_summary(&summary());
    let draft = PersonDraft {
        given: "Augusta".to_owned(),
        ..seed.clone()
    };
    assert!(
        draft.restriction_edit(&seed, "I0001").is_none(),
        "only a changed restriction set dispatches the follow-up edit"
    );
}

#[test]
fn a_pending_citation_with_a_new_source_emits_both_referenced_by_the_name() {
    use super::{NewCitationFields, NewSourceFields, PersonDraft, RecordLink};
    let mut draft = PersonDraft::new();
    draft.given = "John".to_owned();
    draft.surname = "Smith".to_owned();
    draft.name_citation = RecordLink::New(NewCitationFields {
        source: RecordLink::New(NewSourceFields {
            title: "Baptism register".to_owned(),
        }),
        page: "p. 14".to_owned(),
    });
    let request = draft.to_request();
    assert_eq!(request.new_sources.len(), 1, "a pending source is created once");
    assert_eq!(request.new_citations.len(), 1, "a pending citation is created once");
    // The name cites the pending citation by its placeholder; the citation cites the pending source.
    let name_ref = request.name_citation.expect("the name cites the pending citation");
    assert_eq!(
        name_ref,
        crate::navigation::DraftCitationRef::Pending(PersonDraft::PENDING_KEY.to_owned())
    );
    let source_placeholder = format!("{}-source", PersonDraft::PENDING_KEY);
    assert_eq!(request.new_sources[0].placeholder, source_placeholder);
    assert_eq!(
        request.new_citations[0].source,
        crate::navigation::DraftSourceRef::Pending(source_placeholder)
    );
}

#[test]
fn a_pending_citation_against_an_existing_source_emits_no_new_source() {
    use super::{NewCitationFields, PersonDraft, RecordLink};
    use crate::picker::PickerSelection;
    let mut draft = PersonDraft::new();
    draft.given = "Mary".to_owned();
    draft.name_citation = RecordLink::New(NewCitationFields {
        source: RecordLink::Existing(PickerSelection {
            human_id: "S0001".to_owned(),
            title: "S0001".to_owned(),
        }),
        page: String::new(),
    });
    let request = draft.to_request();
    assert!(request.new_sources.is_empty(), "an existing source is not re-created");
    assert_eq!(request.new_citations.len(), 1);
    assert_eq!(
        request.new_citations[0].source,
        crate::navigation::DraftSourceRef::Existing("S0001".to_owned())
    );
}

#[test]
fn detail_keeps_structured_parts_and_localizes_in_norwegian() {
    let loc = Localizer::for_test("no");
    let detail = PersonDetail::from_summary(&summary(), &loc);
    assert_eq!(detail.given.as_deref(), Some("Ada"));
    assert_eq!(detail.surname.as_deref(), Some("Lovelace"));
    assert_eq!(detail.sex, "kvinne");
}

#[test]
fn detail_builds_name_fact_and_association_view_models() {
    let loc = Localizer::for_test("en");
    let detail = PersonDetail::from_summary(&summary(), &loc);

    // The personas badge surfaces the evidence level.
    assert!(!detail.is_persona);
    assert_eq!(detail.evidence_level_label, "Conclusion");

    assert_eq!(detail.names.len(), 1);
    assert_eq!(detail.names[0].type_label, "Birth name");
    assert_eq!(detail.names[0].display, "Ada Lovelace");
    // The name carries its surety + source count (the evidence-first cue).
    assert_eq!(detail.names[0].confidence, Some(ConfidenceLevel::High));
    assert_eq!(detail.names[0].source_count, 1);
    assert!(detail.names[0].has_source());

    assert_eq!(detail.facts.len(), 1);
    let fact = &detail.facts[0];
    assert_eq!(fact.type_label, "Occupation");
    assert_eq!(fact.value.as_deref(), Some("Mathematician"));
    assert_eq!(fact.confidence, Some(ConfidenceLevel::High));
    assert_eq!(fact.confidence_label, "High");
    // The source count comes from the fact's resolved envelope citations (ADR 0020).
    assert_eq!(fact.source_count, 1);
    assert!(fact.has_source(), "the resolved envelope citation is the fact's source");

    assert_eq!(detail.associations.len(), 1);
    assert_eq!(detail.associations[0].other_id, "I0002");
    assert_eq!(detail.associations[0].role_label, "Godparent");
    // The association carries its surety; the default fixture has no backing source.
    assert_eq!(detail.associations[0].confidence, Some(ConfidenceLevel::Normal));
    assert!(!detail.associations[0].has_source());
}

#[test]
fn detail_view_models_thread_assertion_ids_and_structured_prefill_fields() {
    let loc = Localizer::for_test("en");
    let detail = PersonDetail::from_summary(&summary(), &loc);

    // Each row carries the introducing assertion id (the per-row Edit/Retract target).
    assert_eq!(detail.names[0].assertion_id, "aaaaaaaa-0000-7000-8000-000000000001");
    assert_eq!(detail.facts[0].assertion_id, "aaaaaaaa-0000-7000-8000-000000000002");
    assert_eq!(
        detail.associations[0].assertion_id,
        "aaaaaaaa-0000-7000-8000-000000000004"
    );

    // Structured fields ride alongside the display labels, for faithful edit prefill.
    assert_eq!(detail.names[0].name_type, vitni_app::NameType::BirthName);
    assert_eq!(detail.facts[0].fact_type, FactType::Occupation);
    assert_eq!(detail.associations[0].role, AssociationRole::Godparent);

    // The person's attached citation carries its attach assertion id (the Detach target).
    assert_eq!(
        detail.citations[0].assertion_id.as_deref(),
        Some("aaaaaaaa-0000-7000-8000-000000000005")
    );
}

/// A merged cluster's rows name the member they came from, so the detail can attribute them; the
/// person's own rows name none (ADR 0039 §5).
#[test]
fn rows_from_a_merged_member_name_their_record() {
    let loc = Localizer::for_test("en");
    let mut summary = summary();
    summary
        .claim_owners
        .insert("aaaaaaaa-0000-7000-8000-000000000002".to_owned(), "I0007".to_owned());
    let detail = PersonDetail::from_summary(&summary, &loc);
    assert_eq!(detail.facts[0].merged_from.as_deref(), Some("I0007"));
    assert_eq!(detail.names[0].merged_from, None);
    assert_eq!(detail.associations[0].merged_from, None);
}

/// A two-record cluster: the conclusion `I0001` and the persona `I0007` imported from Digitalarkivet,
/// whose record also backs the fact `…0002`.
fn linked() -> vitni_app::LinkedRecords {
    let origin = vitni_app::OriginRef {
        origin: vitni_app::RecordOrigin {
            dataset: vitni_app::DatasetId::global("digitalarkivet"),
            record: "pf01".to_owned(),
            item: None,
            digest: None,
            run: ImportRunId::from_uuid(uuid::Uuid::from_u128(9)),
        },
        dataset_label: Some("Digitalarkivet".to_owned()),
        source_label: Some("1910 census".to_owned()),
        url: Some("https://www.digitalarkivet.no/pf01".to_owned()),
    };
    let record = |human_id: &str, evidence_level, origin, via: Option<&str>, root| vitni_app::LinkedRecord {
        record: vitni_app::AggRef {
            human_id: human_id.to_owned(),
            id: String::new(),
        },
        display_name: Some("Ada Lovelace".to_owned()),
        evidence_level,
        origin,
        via: via.map(str::to_owned),
        root,
    };
    vitni_app::LinkedRecords {
        records: vec![
            record("I0001", EvidenceLevel::Conclusion, None, None, true),
            record("I0007", EvidenceLevel::Persona, Some(origin.clone()), None, false),
            record("I0008", EvidenceLevel::Persona, None, Some("I0007"), false),
        ],
        claim_origins: [("aaaaaaaa-0000-7000-8000-000000000002".to_owned(), origin)].into(),
    }
}

/// The *Linked records* rows: the root first, then each member with its origin, source and the
/// member it was linked through (ADR 0039 §5).
#[test]
fn linked_records_list_each_record_with_its_origin() {
    let loc = Localizer::for_test("en");
    let mut detail = PersonDetail::from_summary(&summary(), &loc);
    detail.link(&linked(), &loc);

    let ids: Vec<&str> = detail.linked.iter().map(|row| row.human_id.as_str()).collect();
    assert_eq!(ids, ["I0001", "I0007", "I0008"]);
    assert!(detail.linked[0].root);
    assert_eq!(detail.linked[0].origin, None);
    let persona = &detail.linked[1];
    assert!(!persona.root);
    assert_eq!(persona.name, "Ada Lovelace");
    assert_eq!(persona.evidence_level_label, "Persona");
    let origin = persona.origin.as_ref().expect("imported");
    assert_eq!(origin.label, "record pf01 in Digitalarkivet");
    assert_eq!(origin.url.as_deref(), Some("https://www.digitalarkivet.no/pf01"));
    assert_eq!(origin.source.as_deref(), Some("1910 census"));
    assert_eq!(persona.via, None);
    assert_eq!(detail.linked[2].via.as_deref(), Some("via I0007"));
}

/// A claim read from an import record names it in *Why we believe* (ADR 0037 §2).
#[test]
fn an_imported_fact_names_its_origin_record() {
    let loc = Localizer::for_test("en");
    let mut detail = PersonDetail::from_summary(&summary(), &loc);
    assert_eq!(detail.facts[0].origin, None, "until the cluster is read");
    detail.link(&linked(), &loc);
    let origin = detail.facts[0].origin.as_ref().expect("the fact's record");
    assert_eq!(origin.label, "record pf01 in Digitalarkivet");
}

/// A dataset whose run is not in the log is named by its scheme.
#[test]
fn an_origin_without_its_run_is_named_by_the_dataset_scheme() {
    let loc = Localizer::for_test("en");
    let mut linked = linked();
    for origin in linked.claim_origins.values_mut() {
        origin.dataset_label = None;
    }
    let mut detail = PersonDetail::from_summary(&summary(), &loc);
    detail.link(&linked, &loc);
    let origin = detail.facts[0].origin.as_ref().expect("the fact's record");
    assert_eq!(origin.label, "record pf01 in digitalarkivet");
}

/// The tab appears only for a person with records linked to it.
#[test]
fn the_linked_records_tab_shows_only_for_a_cluster() {
    let loc = Localizer::for_test("en");
    let mut detail = PersonDetail::from_summary(&summary(), &loc);
    assert!(!person_tabs(&detail, &loc).iter().any(|tab| tab.id == "linked"));
    detail.link(&linked(), &loc);
    let tabs = person_tabs(&detail, &loc);
    let tab = tabs.iter().find(|tab| tab.id == "linked").expect("linked tab");
    assert_eq!(tab.label, "Linked records");
    assert_eq!(tab.count, Some(2), "the members, not the person itself");
}

#[test]
fn persona_evidence_level_surfaces_on_the_badge() {
    let loc = Localizer::for_test("en");
    let mut summary = summary();
    summary.evidence_level = EvidenceLevel::Persona;
    let detail = PersonDetail::from_summary(&summary, &loc);
    assert!(detail.is_persona);
    assert_eq!(detail.evidence_level_label, "Persona");
}

#[test]
fn tabs_carry_localized_labels_and_related_counts() {
    let loc = Localizer::for_test("en");
    let detail = PersonDetail::from_summary(&summary(), &loc);
    let tabs = person_tabs(&detail, &loc);
    assert_eq!(tabs[0].id, "overview");
    assert_eq!(tabs[0].label, "Overview");
    assert_eq!(tabs[0].count, None);
    assert_eq!(tabs[1].id, "names");
    assert_eq!(tabs[1].count, Some(1));
    let facts = tabs.iter().find(|tab| tab.id == "facts").expect("facts tab");
    assert_eq!(facts.count, Some(1));
    let notes = tabs.iter().find(|tab| tab.id == "notes").expect("notes tab");
    assert_eq!(notes.count, Some(2));
    let history = tabs.iter().find(|tab| tab.id == "history").expect("history tab");
    assert_eq!(history.count, None, "history count is unknown until PR5");
}

#[test]
fn vitals_summarize_dated_birth_and_death() {
    let loc = Localizer::for_test("en");
    let mut summary = summary();
    summary.birth_date = Some(year(1850));
    summary.death_date = Some(year(1920));
    let detail = PersonDetail::from_summary(&summary, &loc);
    assert_eq!(detail.vitals.as_deref(), Some("b. 1850 · d. 1920"));
}

#[test]
fn vitals_absent_without_dated_vital_facts() {
    let loc = Localizer::for_test("en");
    // The default summary's only fact is an undated occupation.
    let detail = PersonDetail::from_summary(&summary(), &loc);
    assert_eq!(detail.vitals, None);
}

#[test]
fn missing_name_and_sex_use_placeholders() {
    let loc = Localizer::for_test("en");
    let summary = PersonSummary {
        human_id: "I0002".to_owned(),
        evidence_level: EvidenceLevel::Conclusion,
        display_name: None,
        given: None,
        surname: None,
        surname_prefix: None,
        nickname: None,
        name_prefix: None,
        name_suffix: None,
        name_type: None,
        primary_name_assertion: None,
        names: Vec::new(),
        sex: None,
        birth_date: None,
        death_date: None,
        facts: Vec::new(),
        associations: Vec::new(),
        participations: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        tag_refs: Vec::new(),
        restrictions: BTreeSet::from([Restriction::Privacy]),
        merged: Vec::new(),
        claim_owners: std::collections::BTreeMap::new(),
    };
    let row = person_row(&summary, &loc);
    assert_eq!(row.title, "(no name)");
    assert_eq!(row.subtitle.as_deref(), Some("-"));
    assert_eq!(row.avatar.as_deref(), Some("?"));
}

fn citation_summary() -> CitationSummary {
    CitationSummary {
        human_id: "C0001".to_owned(),
        source: Some(vitni_app::AggRef {
            human_id: "S0001".to_owned(),
            id: "55555555-5555-7555-8555-555555555555".to_owned(),
        }),
        page: Some("p. 42".to_owned()),
        date: Some(year(1880)),
        confidence: Some(Confidence::High),
        evidence_analysis: Some(EvidenceAnalysis {
            source: SourceQuality::Original,
            information: InformationKind::Primary,
            evidence: EvidenceKind::Direct,
        }),
        attributes: vec![vitni_app::CitationAttributeRef {
            attribute_type: "quality".to_owned(),
            value: "good".to_owned(),
            assertion_id: "aaaaaaaa-0000-7000-8000-00000000000a".to_owned(),
        }],
        media: vec![vitni_app::MediaRefSummary {
            human_id: "O0001".to_owned(),
            id: "66666666-6666-7666-8666-666666666666".to_owned(),
            caption: None,
            crop: None,
            path: None,
            mime: None,
            assertion_id: "aaaaaaaa-0000-7000-8000-00000000000b".to_owned(),
        }],
        notes: vec![vitni_app::AttachedRef {
            human_id: "N0001".to_owned(),
            id: "77777777-7777-7777-8777-777777777777".to_owned(),
            note_type: Some(vitni_app::NoteType::Transcript),
            text: Some("1850 was the first U.S. census to name every household member.".to_owned()),
            language: Some("en".to_owned()),
            assertion_id: "aaaaaaaa-0000-7000-8000-00000000000c".to_owned(),
        }],
        tags: vec![TagRef {
            id: "0190-tag".to_owned(),
            name: "Direct ancestor".to_owned(),
            color: Some("#e5534b".to_owned()),
            priority: Some(1),
        }],
        restrictions: BTreeSet::new(),
        merged: Vec::new(),
        claim_owners: std::collections::BTreeMap::new(),
    }
}

#[test]
fn citation_detail_maps_axes_confidence_and_attachments() {
    let loc = Localizer::for_test("en");
    let detail = CitationDetail::from_summary(&citation_summary(), &loc);
    assert_eq!(detail.source.as_deref(), Some("S0001"));
    assert_eq!(detail.page.as_deref(), Some("p. 42"));
    assert_eq!(detail.confidence, Some(ConfidenceLevel::High));
    assert_eq!(detail.confidence_label.as_deref(), Some("High"));
    assert_eq!(detail.evidence_axes.len(), 3);
    assert_eq!(detail.evidence_axes[0].axis, EvidenceAxis::Source);
    assert_eq!(detail.evidence_axes[0].label, "Original");
    assert_eq!(detail.evidence_axes[1].label, "Primary");
    assert_eq!(detail.evidence_axes[2].label, "Direct");
    assert_eq!(detail.attributes.len(), 1);
    assert_eq!(
        detail.media,
        vec![MediaRefVm {
            human_id: "O0001".to_owned(),
            assertion_id: "aaaaaaaa-0000-7000-8000-00000000000b".to_owned(),
            caption: None,
            crop: None,
            path: None,
            mime: None,
        }]
    );
    assert_eq!(
        detail.notes,
        vec![AttachedRefVm {
            human_id: "N0001".to_owned(),
            note_type: Some(vitni_app::NoteType::Transcript),
            type_label: Some("Transcript".to_owned()),
            text: Some("1850 was the first U.S. census to name every household member.".to_owned()),
            language: Some("en".to_owned()),
            assertion_id: "aaaaaaaa-0000-7000-8000-00000000000c".to_owned(),
        }]
    );
    // Tags surface name/colour/priority — never the id.
    assert_eq!(detail.tags[0].name, "Direct ancestor");
    assert_eq!(detail.tags[0].color.as_deref(), Some("#e5534b"));
    assert_eq!(detail.tags[0].priority, Some(1));
}

#[test]
fn evidence_axes_are_empty_without_analysis() {
    let loc = Localizer::for_test("en");
    let checked = evidence_axes(None, &loc);
    assert!(checked.is_empty(), "{checked:?}");
}

#[test]
fn citation_row_titles_by_source_and_subtitles_by_page() {
    let loc = Localizer::for_test("en");
    let row = citation_row(&citation_summary(), &loc);
    assert_eq!(row.id, "C0001");
    assert_eq!(row.title, "S0001");
    assert_eq!(row.subtitle.as_deref(), Some("p. 42"));
}

#[test]
fn citation_tabs_carry_attachment_counts() {
    let loc = Localizer::for_test("en");
    let detail = CitationDetail::from_summary(&citation_summary(), &loc);
    let tabs = citation_tabs(&detail, &loc);
    assert_eq!(tabs[0].id, "overview");
    let attributes = tabs.iter().find(|tab| tab.id == "attributes").expect("attributes tab");
    assert_eq!(attributes.count, Some(1));
    let tags = tabs.iter().find(|tab| tab.id == "tags").expect("tags tab");
    assert_eq!(tags.count, Some(1));
}

#[test]
fn a_fresh_tag_draft_seeds_the_create_defaults() {
    let draft = TagDraft::new();
    assert!(draft.existing_id.is_none());
    assert!(draft.name.is_empty(), "{:?}", draft.name);
    assert_eq!(draft.priority, DEFAULT_TAG_PRIORITY.to_string());
    assert_eq!(draft.color, DEFAULT_TAG_COLOR);
    // A blank name means the draft is not committable yet (Save disabled).
    assert!(!draft.is_valid());
    assert!(draft.to_request().is_none());
}

#[test]
fn a_tag_draft_is_valid_only_when_all_three_fields_are_present() {
    let mut draft = TagDraft::new();
    draft.name = "Direct ancestor".to_owned();
    assert!(draft.is_valid(), "name + default priority + default colour is valid");

    draft.priority = String::new();
    assert!(!draft.is_valid(), "an empty priority is invalid");
    draft.priority = "notanumber".to_owned();
    assert!(!draft.is_valid(), "a non-numeric priority is invalid");
    draft.priority = "2".to_owned();

    draft.color = "   ".to_owned();
    assert!(!draft.is_valid(), "an empty colour is invalid");
}

#[test]
fn a_valid_tag_draft_builds_a_trimmed_request() {
    let mut draft = TagDraft::new();
    draft.existing_id = Some("tag-uuid".to_owned());
    draft.name = "  Needs sources  ".to_owned();
    draft.priority = " 3 ".to_owned();
    draft.color = " #e0884a ".to_owned();
    let request = draft.to_request().expect("valid draft commits");
    assert_eq!(request.existing_id.as_deref(), Some("tag-uuid"));
    assert_eq!(request.name, "Needs sources");
    assert_eq!(request.priority, 3);
    assert_eq!(request.color, "#e0884a");
    assert!(request.restrictions.is_empty(), "a fresh draft carries no restrictions");
}

fn tag_detail_sample() -> TagDetail {
    TagDetail {
        id: "tag-uuid".to_owned(),
        title: "Direct ancestor".to_owned(),
        name: Some("Direct ancestor".to_owned()),
        color: Some("#e5534b".to_owned()),
        priority: Some(2),
        total: 3,
        usage: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

#[test]
fn record_draft_from_detail_is_not_dirty_against_itself() {
    let seed = <TagDraft as RecordDraft>::from_detail(&tag_detail_sample());
    let draft = seed.clone();
    assert!(
        !draft.is_dirty_against(&seed),
        "a draft freshly seeded from a record has no unsaved change"
    );
}

#[test]
fn flipping_a_tag_scalar_makes_the_draft_dirty() {
    let seed = <TagDraft as RecordDraft>::from_detail(&tag_detail_sample());

    let mut renamed = seed.clone();
    renamed.name = "Renamed".to_owned();
    assert!(renamed.is_dirty_against(&seed), "a changed name is dirty");

    let mut recoloured = seed.clone();
    recoloured.color = "#000000".to_owned();
    assert!(recoloured.is_dirty_against(&seed), "a changed colour is dirty");

    let mut reprioritised = seed.clone();
    reprioritised.priority = "9".to_owned();
    assert!(reprioritised.is_dirty_against(&seed), "a changed priority is dirty");
}

#[test]
fn record_draft_is_valid_matches_the_tag_field_rules() {
    let mut draft = <TagDraft as RecordDraft>::from_detail(&tag_detail_sample());
    assert!(RecordDraft::is_valid(&draft), "a seeded tag draft is valid");

    draft.name = "   ".to_owned();
    assert!(!RecordDraft::is_valid(&draft), "a blank name is invalid");
    draft.name = "Direct ancestor".to_owned();

    draft.priority = "x".to_owned();
    assert!(!RecordDraft::is_valid(&draft), "a non-numeric priority is invalid");
}

#[test]
fn a_person_draft_from_detail_matches_the_edit_seed_and_is_valid() {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let detail = PersonDetail::from_summary(&summary(), &loc);
    let seed = <PersonDraft as RecordDraft>::from_detail(&detail);
    assert_eq!(seed, detail.edit_seed, "a person edit draft is its detail's edit seed");
    assert!(
        !seed.clone().is_dirty_against(&seed),
        "the seed is not dirty against itself"
    );
    assert!(
        RecordDraft::is_valid(&seed),
        "a person has no required scalar, so it is always valid"
    );

    let mut renamed = seed.clone();
    renamed.given = "Augusta".to_owned();
    assert!(renamed.is_dirty_against(&seed), "a changed given name is dirty");
}

#[test]
fn default_provenance_draft_maps_to_default_meta() {
    let draft = ProvenanceDraft::default();
    let provenance = draft.provenance();
    assert_eq!(
        provenance.confidence, None,
        "a default (mechanical) draft records no surety judgment (ADR 0021 §5)"
    );
    assert!(provenance.rationale.is_none(), "no rationale by default");
    assert!(
        provenance.evidence_analysis.is_none(),
        "no evidence analysis by default"
    );
    let meta = draft.meta();
    assert!(meta.citations.is_empty(), "no citations by default");
    assert!(meta.supersedes.is_none(), "a draft never supersedes (PR27)");
}

#[test]
fn filled_draft_maps_rationale_confidence_and_citations() {
    let mut draft = ProvenanceDraft {
        rationale: "   ".to_owned(),
        ..ProvenanceDraft::default()
    };
    assert!(
        draft.provenance().rationale.is_none(),
        "a blank/whitespace rationale drops to None"
    );
    draft.rationale = "  Baptism register gives the date  ".to_owned();
    assert_eq!(
        draft.provenance().rationale.as_deref(),
        Some("Baptism register gives the date"),
        "a real rationale is trimmed"
    );
    draft.confidence = Some(ConfidenceLevel::High);
    assert_eq!(
        draft.provenance().confidence,
        Some(Confidence::High),
        "confidence maps through"
    );
    draft.citations = vec!["C0001".to_owned(), "C0002".to_owned()];
    let meta = draft.meta();
    assert_eq!(
        meta.citations,
        &["C0001".to_owned(), "C0002".to_owned()],
        "citations pass through by borrow"
    );
}

#[test]
fn evidence_analysis_requires_all_three_axes() {
    let mut draft = ProvenanceDraft {
        source: Some(SourceQuality::Original),
        information: Some(InformationKind::Primary),
        ..ProvenanceDraft::default()
    };
    assert!(
        draft.provenance().evidence_analysis.is_none(),
        "a missing evidence axis yields no analysis"
    );
    draft.evidence = Some(EvidenceKind::Direct);
    let analysis = draft
        .provenance()
        .evidence_analysis
        .expect("all three axes chosen yields an analysis");
    assert_eq!(
        analysis,
        EvidenceAnalysis {
            source: SourceQuality::Original,
            information: InformationKind::Primary,
            evidence: EvidenceKind::Direct,
        }
    );
}

#[test]
fn a_person_events_tab_carries_each_participations_payload_and_provenance() {
    use vitni_app::{Age, AgeBound, AggRef, Attribute, Confidence, ParticipantRole, ParticipationRef};

    let loc = Localizer::for_test("en");
    let mut summary = summary();
    summary.participations = vec![
        ParticipationRef {
            event: AggRef {
                human_id: "E0001".to_owned(),
                id: "55555555-5555-7555-8555-555555555555".to_owned(),
            },
            role: ParticipantRole::Bride,
            date: None,
            place: None,
            age: Some(Age {
                bound: Some(AgeBound::GreaterThan),
                years: Some(42),
                months: None,
                days: None,
                phrase: None,
            }),
            attributes: vec![Attribute {
                attribute_type: "occupation".to_owned(),
                value: "farmer".to_owned(),
            }],
            notes: vec![AggRef {
                human_id: "N0001".to_owned(),
                id: "77777777-7777-7777-8777-777777777777".to_owned(),
            }],
            confidence: Some(Confidence::High),
            source_count: 1,
            assertion_id: "aaaaaaaa-0000-7000-8000-000000000008".to_owned(),
        },
        ParticipationRef {
            event: AggRef {
                human_id: "E0002".to_owned(),
                id: "66666666-6666-7666-8666-666666666666".to_owned(),
            },
            role: ParticipantRole::Witness,
            date: None,
            place: None,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
            confidence: Some(Confidence::Normal),
            source_count: 0,
            assertion_id: "aaaaaaaa-0000-7000-8000-000000000009".to_owned(),
        },
    ];
    let detail = PersonDetail::from_summary(&summary, &loc);
    assert_eq!(detail.events.len(), 2);
    assert_eq!(
        detail.events[0].age_label.as_deref(),
        Some("over 42y"),
        "the row localizes its age"
    );
    assert_eq!(detail.events[0].attributes.len(), 1);
    assert_eq!(detail.events[0].notes, vec!["N0001".to_owned()]);
    assert_eq!(detail.events[0].source_count, 1);
    assert_eq!(
        detail.events[1].age_label, None,
        "a row without an age has no age label"
    );
    assert_eq!(detail.events[1].source_count, 0);
}

#[test]
fn a_person_events_tab_sorts_participations_chronologically() {
    use vitni_app::{
        AggRef, Calendar, DateInput, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody,
        ParticipantRole, ParticipationRef, build_genealogical_date,
    };

    fn dated(year: i32) -> GenealogicalDate {
        build_genealogical_date(DateInput {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            body: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(year),
                month: None,
                day: None,
            })),
            new_year_begins: None,
            original_text: None,
            time: None,
        })
    }

    fn participation(human_id: &str, date: Option<GenealogicalDate>, assertion: &str) -> ParticipationRef {
        ParticipationRef {
            event: AggRef {
                human_id: human_id.to_owned(),
                id: format!("{human_id}-id"),
            },
            role: ParticipantRole::Primary,
            date,
            place: None,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
            confidence: None,
            source_count: 0,
            assertion_id: assertion.to_owned(),
        }
    }

    let loc = Localizer::for_test("en");
    let mut summary = summary();
    // Supplied newest-first + an undated row: the Events tab must reorder to oldest-first, undated last.
    summary.participations = vec![
        participation("E1902", Some(dated(1902)), "aaaaaaaa-0000-7000-8000-000000000101"),
        participation("E-UND", None, "aaaaaaaa-0000-7000-8000-000000000102"),
        participation("E1876", Some(dated(1876)), "aaaaaaaa-0000-7000-8000-000000000103"),
    ];
    let detail = PersonDetail::from_summary(&summary, &loc);
    let order: Vec<&str> = detail.events.iter().map(|event| event.event_id.as_str()).collect();
    assert_eq!(
        order,
        vec!["E1876", "E1902", "E-UND"],
        "participations render oldest-first with undated rows last"
    );
}

#[test]
fn the_timeline_merges_facts_and_participations_oldest_first_undated_last() {
    use vitni_app::{
        AggRef, Calendar, DateInput, DateModifier, DatePoint, DateQuality, Fact, FactType, GenealogicalDate,
        GenealogicalDateBody, ParticipantRole, ParticipationRef, build_genealogical_date,
    };

    fn dated(year: i32) -> GenealogicalDate {
        build_genealogical_date(DateInput {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            body: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(year),
                month: None,
                day: None,
            })),
            new_year_begins: None,
            original_text: None,
            time: None,
        })
    }

    fn fact(fact_type: FactType, date: Option<GenealogicalDate>, value: &str, assertion: &str) -> FactSummary {
        FactSummary {
            fact: Fact {
                fact_type,
                date,
                place_id: None,
                value: Some(value.to_owned()),
            },
            confidence: Some(Confidence::High),
            citations: Vec::new(),
            assertion_id: assertion.to_owned(),
        }
    }

    fn participation(human_id: &str, date: Option<GenealogicalDate>, assertion: &str) -> ParticipationRef {
        ParticipationRef {
            event: AggRef {
                human_id: human_id.to_owned(),
                id: format!("{human_id}-id"),
            },
            role: ParticipantRole::Primary,
            date,
            place: None,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
            confidence: None,
            source_count: 0,
            assertion_id: assertion.to_owned(),
        }
    }

    let loc = Localizer::for_test("en");
    let mut summary = summary();
    // A dated + an undated fact and dated + undated participations, supplied out of order: the
    // timeline must reorder to oldest-first, undated last, with facts before events on a tie (MAX).
    summary.facts = vec![
        fact(
            FactType::Occupation,
            None,
            "Carpenter",
            "aaaaaaaa-0000-7000-8000-000000000201",
        ),
        fact(
            FactType::Residence,
            Some(dated(1888)),
            "New York",
            "aaaaaaaa-0000-7000-8000-000000000202",
        ),
    ];
    summary.participations = vec![
        participation("E1902", Some(dated(1902)), "aaaaaaaa-0000-7000-8000-000000000203"),
        participation("E-UND", None, "aaaaaaaa-0000-7000-8000-000000000204"),
        participation("E1876", Some(dated(1876)), "aaaaaaaa-0000-7000-8000-000000000205"),
    ];
    let detail = PersonDetail::from_summary(&summary, &loc);

    assert_eq!(
        detail.timeline.len(),
        5,
        "every fact and participation contributes a row"
    );
    // Oldest dated event first.
    assert_eq!(detail.timeline[0].kind, TimelineKind::Event);
    assert_eq!(detail.timeline[0].event_id.as_deref(), Some("E1876"));
    // Then the dated fact.
    assert_eq!(detail.timeline[1].kind, TimelineKind::Fact);
    assert!(detail.timeline[1].date.is_some(), "the 1888 fact is dated");
    // Then the later dated event.
    assert_eq!(detail.timeline[2].kind, TimelineKind::Event);
    assert_eq!(detail.timeline[2].event_id.as_deref(), Some("E1902"));
    // Undated rows last; the stable tie-break keeps the fact (pushed first) before the event.
    assert_eq!(detail.timeline[3].kind, TimelineKind::Fact);
    assert!(detail.timeline[3].date.is_none(), "the undated fact carries no date");
    assert_eq!(detail.timeline[4].kind, TimelineKind::Event);
    assert_eq!(detail.timeline[4].event_id.as_deref(), Some("E-UND"));
    assert!(
        detail.timeline[4].date.is_none(),
        "the undated participation carries no date"
    );
}

#[test]
fn the_timeline_tab_count_matches_the_merged_row_count() {
    let loc = Localizer::for_test("en");
    let detail = PersonDetail::from_summary(&summary(), &loc);
    let tabs = person_tabs(&detail, &loc);
    let timeline = tabs
        .iter()
        .find(|tab| tab.id == "timeline")
        .expect("the person tab strip carries a Timeline tab");
    assert_eq!(timeline.label, "Timeline");
    assert_eq!(
        timeline.count,
        Some(detail.timeline.len()),
        "the tab count is the merged fact + participation row count"
    );
}

#[test]
fn citation_ref_from_ref_annotates_a_software_asserter() {
    use super::citation_ref_from_ref;
    let loc = Localizer::for_test("en");
    let reference = vitni_app::CitationRef {
        human_id: "C0001".to_owned(),
        id: "55555555-5555-7555-8555-555555555555".to_owned(),
        assertion_id: None,
        source: None,
        source_title: None,
        page: None,
        backs_count: 0,
        confidence: None,
        analysis: None,
        asserted_by: Some("vitni-import".to_owned()),
        asserted_by_kind: Some(OperatorKind::Software),
        asserted_at: None,
    };
    let vm = citation_ref_from_ref(&reference, &loc);
    let asserted_by = vm.asserted_by.expect("asserted-by line is rendered");
    assert!(
        asserted_by.contains("software agent"),
        "the asserted-by line annotates the software agent kind: {asserted_by}"
    );
}

#[test]
fn attached_note_carries_its_localized_type_and_body() {
    // Issue #316: the transcribed evidence text of a source is an attached `NoteType::Transcript`
    // note, so an attach ref has to reach the screen as readable words — a localized type label and
    // the body, each its own column of the shared attached-records table (issue #304).
    let loc = Localizer::for_test("en");
    let reference = vitni_app::AttachedRef {
        human_id: "N0004".to_owned(),
        id: "88888888-8888-7888-8888-888888888888".to_owned(),
        note_type: Some(vitni_app::NoteType::Transcript),
        text: Some("Smith, John — age 0, b. New York".to_owned()),
        language: Some("en".to_owned()),
        assertion_id: "aaaaaaaa-0000-7000-8000-00000000000d".to_owned(),
    };
    let vm = AttachedRefVm::from_ref(&reference, &loc);
    assert_eq!(
        vm.type_label.as_deref(),
        Some("Transcript"),
        "the type label is localized"
    );
    assert_eq!(vm.text.as_deref(), Some("Smith, John — age 0, b. New York"));
    assert_eq!(vm.language.as_deref(), Some("en"), "the body's language rides along");
    assert_eq!(vm.human_id, "N0004", "the row's link target and label");
}

#[test]
fn an_untyped_textless_note_still_renders_its_id() {
    // Every column but the id can be absent, so a note created with neither a type nor a body is still
    // identifiable (and detachable) on the owner's Notes tab through its `human_id` link.
    let loc = Localizer::for_test("en");
    let reference = vitni_app::AttachedRef {
        human_id: "N0009".to_owned(),
        id: "99999999-9999-7999-8999-999999999999".to_owned(),
        note_type: None,
        text: None,
        language: None,
        assertion_id: "aaaaaaaa-0000-7000-8000-00000000000e".to_owned(),
    };
    let vm = AttachedRefVm::from_ref(&reference, &loc);
    assert_eq!(vm.human_id, "N0009");
    assert_eq!(vm.type_label, None);
    assert_eq!(vm.text, None);
    assert_eq!(vm.language, None);
}

#[test]
fn a_history_entry_shows_the_assessment_behind_an_identity_decision() {
    let loc = Localizer::for_test("en");
    let mut entry = log_entry("person", Some("I0001"), OperatorKind::Human, "magne");
    entry.event_type = "PersonsMerged".to_owned();
    entry.detail = Some(ActivityDetail::IdentityDecision {
        assessment: vitni_app::MatchEvidence {
            score_bp: 8712,
            band: vitni_app::MatchBand::Possible,
            engine: vitni_app::EngineVersion(4),
            cultures: Vec::new(),
            features: Vec::new(),
        },
    });
    let vm = super::HistoryEntryVm::from_entry(&entry, &loc);
    assert_eq!(vm.what, "Persona merged");
    assert_eq!(
        vm.evidence.as_deref(),
        Some("Matched at 87% · possible match · engine 4")
    );

    entry.detail = None;
    assert_eq!(super::HistoryEntryVm::from_entry(&entry, &loc).evidence, None);
}
