//! `find_similar`, `assess` and `similar_pairs` over the `match_keys` blocking index (ADR 0038 §7, §8):
//! the hard true matches are found and a stranger is not, the index follows edits and pack changes, and
//! blocking loses no pair a score over every pair would show.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, AppError, CheckFinding, DateParts, IdentityDecision, MatchBand, MatchableKind, MutationMeta, NewEvent,
    NewParticipation, NewPerson, NewPlace, NewSource, OperatorConfig, PersonNameParts, Provenance, Session, Workspace,
    WorkspaceDefaults, assert_event_date, assert_participation, assert_sex, assess, change_log_for_person,
    create_event, create_person, create_place, create_source, create_tag, distinguish_persons, find_similar,
    merge_persons, run_checks, similar_pairs, undo_assertion,
};
use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole, PlaceType, Sex};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

const TOY: &str = include_str!("../../vitni-core/tests/fixtures/matching/toy.toml");

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

/// One workspace and the records built in it through the use cases.
struct Records {
    workspace: Workspace,
    session: Session,
    dir: tempfile::TempDir,
}

impl Records {
    async fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        Workspace::init(&dir.path().join("ws"), &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Self::open(&dir).await;
        let session = Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Tester".to_owned()),
        });
        Self {
            workspace,
            session,
            dir,
        }
    }

    async fn open(dir: &tempfile::TempDir) -> Workspace {
        Workspace::open(&dir.path().join("ws"), &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open workspace")
    }

    async fn reopen(&mut self) {
        self.workspace = Self::open(&self.dir).await;
    }

    async fn person(&self, given: &str, surname: &str) -> String {
        let new = NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                (!given.is_empty()).then(|| given.to_owned()),
                (!surname.is_empty()).then(|| surname.to_owned()),
            )),
            evidence_level: EvidenceLevel::Conclusion,
            external_ids: Vec::new(),
        };
        let person = create_person(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create person");
        assert_sex(
            &self.workspace,
            &self.session,
            &person,
            Sex::Male,
            MutationMeta::default(),
        )
        .await
        .expect("assert sex");
        person
    }

    /// A person born in `year`; returns the person and the birth event.
    async fn born(&self, given: &str, surname: &str, year: i32) -> (String, String) {
        let person = self.person(given, surname).await;
        let new = NewEvent {
            human_id: None,
            event_type: EventType::Birth,
        };
        let event = create_event(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create event");
        self.date(&event, year).await;
        assert_participation(
            &self.workspace,
            &self.session,
            &person,
            &event,
            NewParticipation::with_role(ParticipantRole::Primary),
            MutationMeta::default(),
        )
        .await
        .expect("participate");
        (person, event)
    }

    async fn date(&self, event: &str, year: i32) {
        let date = DateParts {
            year,
            month: None,
            day: None,
        };
        assert_event_date(&self.workspace, &self.session, event, date, MutationMeta::default())
            .await
            .expect("date event");
    }

    /// Every person pair `similar_pairs` proposes, as sorted human-id pairs.
    async fn person_pairs(&self) -> BTreeSet<(String, String)> {
        let mut pairs = BTreeSet::new();
        for pair in similar_pairs(&self.workspace, MatchableKind::Person, MatchBand::Possible)
            .await
            .expect("similar pairs")
        {
            pairs.insert(sorted(pair.a.human_id, pair.b.human_id));
        }
        pairs
    }

    /// Every person pair the duplicate check reports, as sorted human-id pairs.
    async fn duplicate_findings(&self) -> BTreeSet<(String, String)> {
        let mut pairs = BTreeSet::new();
        for finding in run_checks(&self.workspace).await.expect("run checks") {
            if let CheckFinding::PossibleDuplicate {
                kind: MatchableKind::Person,
                a,
                b,
                assessment: _,
            } = finding
            {
                pairs.insert(sorted(a.human_id, b.human_id));
            }
        }
        pairs
    }

    async fn similar(&self, kind: MatchableKind, target: &str) -> Vec<String> {
        find_similar(&self.workspace, kind, target, MatchBand::Possible, 50)
            .await
            .expect("find similar")
            .into_iter()
            .map(|similar| similar.record.human_id)
            .collect()
    }
}

#[tokio::test]
async fn a_spelling_variant_is_found_and_a_stranger_is_not() {
    let records = Records::new().await;
    let (guldbrand, _) = records.born("Guldbrand", "Olsen", 1852).await;
    let (gulbrand, _) = records.born("Gulbrand", "Olsøn", 1852).await;
    let (stranger, _) = records.born("Kari", "Hansen", 1900).await;
    let found = records.similar(MatchableKind::Person, &guldbrand).await;
    assert_eq!(found, [gulbrand], "never the target itself, nor {stranger}");
}

#[tokio::test]
async fn a_birth_in_the_neighbouring_decade_is_found() {
    let records = Records::new().await;
    let (a, _) = records.born("Ole", "Olsen", 1849).await;
    let (b, _) = records.born("Ole", "Olsen", 1851).await;
    assert_eq!(records.similar(MatchableKind::Person, &a).await, [b]);
}

#[tokio::test]
async fn results_are_ranked_and_bounded_by_band_and_limit() {
    let records = Records::new().await;
    let (target, _) = records.born("Ole", "Olsen", 1850).await;
    let (same, _) = records.born("Ole", "Olsen", 1850).await;
    let (near, _) = records.born("Ola", "Olsen", 1853).await;
    let all = find_similar(
        &records.workspace,
        MatchableKind::Person,
        &target,
        MatchBand::Unlikely,
        50,
    )
    .await
    .expect("find similar");
    assert_eq!(all[0].record.human_id, same, "the closer match first: {all:?}");
    assert!(all.iter().any(|similar| similar.record.human_id == near));
    assert!(
        all.windows(2)
            .all(|pair| pair[0].assessment.band >= pair[1].assessment.band)
    );
    let one = find_similar(
        &records.workspace,
        MatchableKind::Person,
        &target,
        MatchBand::Unlikely,
        1,
    )
    .await
    .expect("find similar");
    assert_eq!(one.len(), 1);
    let probable = find_similar(
        &records.workspace,
        MatchableKind::Person,
        &target,
        MatchBand::Probable,
        50,
    )
    .await
    .expect("find similar");
    assert!(
        probable
            .iter()
            .all(|similar| similar.assessment.band >= MatchBand::Probable)
    );
}

#[tokio::test]
async fn a_result_names_the_record_by_human_id_and_aggregate_id() {
    let records = Records::new().await;
    let (a, _) = records.born("Ole", "Olsen", 1850).await;
    let (b, _) = records.born("Ole", "Olsen", 1850).await;
    let found = find_similar(&records.workspace, MatchableKind::Person, &a, MatchBand::Possible, 5)
        .await
        .expect("find similar");
    assert_eq!(found[0].record.human_id, b);
    let id = Uuid::parse_str(&found[0].record.id).expect("the aggregate id is a UUID");
    let view = records
        .workspace
        .store()
        .find_person(&b)
        .await
        .expect("find")
        .expect("person");
    assert_eq!(view.person_id().map(|p| p.as_uuid()), Some(id));
}

#[tokio::test]
async fn a_changed_birth_date_moves_the_person_to_its_new_decade() {
    let records = Records::new().await;
    let (a, _) = records.born("Ole", "Olsen", 1850).await;
    let (b, birth) = records.born("Ole", "Olsen", 1920).await;
    assert!(records.similar(MatchableKind::Person, &a).await.is_empty());
    records.date(&birth, 1850).await;
    assert_eq!(
        records.similar(MatchableKind::Person, &a).await,
        [b],
        "the event's new date rekeyed its principal"
    );
}

#[tokio::test]
async fn a_workspace_pack_rebuilds_the_index() {
    let mut records = Records::new().await;
    let zorbo = records.person("Zorbo", "").await;
    let quimble = records.person("Quimble", "").await;
    assert!(records.similar(MatchableKind::Person, &zorbo).await.is_empty());

    let ws = records.dir.path().join("ws");
    std::fs::create_dir_all(ws.join("matching/cultures")).expect("mkdir");
    std::fs::write(ws.join("matching/cultures/toy.toml"), TOY).expect("write pack");
    let manifest = std::fs::read_to_string(ws.join("workspace.toml")).expect("read manifest");
    let manifest = manifest.replace("[matching]", "[matching]\ndefault_cultures = [\"toy\"]");
    std::fs::write(ws.join("workspace.toml"), manifest).expect("write manifest");
    records.reopen().await;

    assert_eq!(records.similar(MatchableKind::Person, &zorbo).await, [quimble]);
}

#[tokio::test]
async fn a_bad_matching_setting_fails_the_lookup_not_the_workspace() {
    let mut records = Records::new().await;
    let person = records.person("Ole", "Olsen").await;
    let ws = records.dir.path().join("ws");
    let manifest = std::fs::read_to_string(ws.join("workspace.toml")).expect("read manifest");
    std::fs::write(
        ws.join("workspace.toml"),
        manifest.replace("[matching]", "[matching]\nprobable = 101"),
    )
    .expect("write manifest");
    records.reopen().await;
    let error = find_similar(
        &records.workspace,
        MatchableKind::Person,
        &person,
        MatchBand::Possible,
        5,
    )
    .await
    .expect_err("an invalid threshold");
    assert!(matches!(error, AppError::Config(_)), "{error:?}");
}

#[tokio::test]
async fn a_projection_rebuild_rebuilds_the_index_on_next_use() {
    let records = Records::new().await;
    let (a, _) = records.born("Ole", "Olsen", 1850).await;
    let (b, _) = records.born("Ole", "Olsen", 1850).await;
    assert_eq!(
        records.similar(MatchableKind::Person, &a).await,
        std::slice::from_ref(&b)
    );
    records.workspace.rebuild_projections().await.expect("rebuild");
    assert_eq!(
        records.workspace.store().match_keys_fingerprint().await.expect("state"),
        None
    );
    assert_eq!(records.similar(MatchableKind::Person, &a).await, [b]);
}

#[tokio::test]
async fn an_unknown_target_is_not_found() {
    let records = Records::new().await;
    let error = find_similar(
        &records.workspace,
        MatchableKind::Person,
        "I9999",
        MatchBand::Possible,
        5,
    )
    .await
    .expect_err("no such person");
    assert!(matches!(error, AppError::PersonNotFound(id) if id == "I9999"));
    let error = assess(&records.workspace, MatchableKind::Place, "P1", "P2")
        .await
        .expect_err("no such place");
    assert!(matches!(error, AppError::PlaceNotFound(_)));
}

#[tokio::test]
async fn places_sources_and_tags_are_found_by_their_own_keys() {
    let records = Records::new().await;
    let place = |name: &str| NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some(name.to_owned()),
    };
    let (ws, session) = (&records.workspace, &records.session);
    let nordaas = create_place(ws, session, place("Nordaas"), Provenance::default(), &[])
        .await
        .expect("place");
    let nordas = create_place(ws, session, place("Nordås"), Provenance::default(), &[])
        .await
        .expect("place");
    create_place(ws, session, place("Bergen"), Provenance::default(), &[])
        .await
        .expect("place");
    assert_eq!(records.similar(MatchableKind::Place, &nordaas).await, [nordas]);

    let source = |title: &str| NewSource {
        human_id: None,
        title: Some(title.to_owned()),
    };
    let fana = create_source(
        ws,
        session,
        source("Ministerialbok for Fana"),
        Provenance::default(),
        &[],
    )
    .await
    .expect("source");
    let fana_again = create_source(ws, session, source("Fana ministerialbok"), Provenance::default(), &[])
        .await
        .expect("source");
    create_source(ws, session, source("Folketelling 1865"), Provenance::default(), &[])
        .await
        .expect("source");
    assert_eq!(records.similar(MatchableKind::Source, &fana).await, [fana_again]);

    let emigrant = create_tag(ws, session, "Emigrant".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    create_tag(ws, session, "Soldier".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    assert!(records.similar(MatchableKind::Tag, &emigrant).await.is_empty());
}

/// The duplicate check covers every matchable kind, carrying the engine's evidence for each pair.
#[tokio::test]
async fn the_duplicate_check_reports_a_place_pair_with_the_engines_evidence() {
    let records = Records::new().await;
    let place = |name: &str| NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some(name.to_owned()),
    };
    let (ws, session) = (&records.workspace, &records.session);
    let nordaas = create_place(ws, session, place("Nordaas"), Provenance::default(), &[])
        .await
        .expect("place");
    let nordas = create_place(ws, session, place("Nordås"), Provenance::default(), &[])
        .await
        .expect("place");
    let expected = assess(ws, MatchableKind::Place, &nordaas, &nordas)
        .await
        .expect("assess")
        .evidence();

    let mut places = Vec::new();
    for finding in run_checks(ws).await.expect("run checks") {
        if let CheckFinding::PossibleDuplicate {
            kind: MatchableKind::Place,
            a,
            b,
            assessment,
        } = finding
        {
            places.push((sorted(a.human_id, b.human_id), assessment));
        }
    }
    assert_eq!(places, [(sorted(nordaas, nordas), expected.clone())]);
    assert!(
        !expected.features.is_empty(),
        "the finding carries the terms behind its score"
    );
}

/// Duplicates of every kind are listed together, the most similar first, whatever their kind.
#[tokio::test]
async fn the_duplicate_check_ranks_pairs_across_kinds() {
    let records = Records::new().await;
    records.born("Ole", "Olsen", 1850).await;
    records.born("Ole", "Olsen", 1856).await;
    let place = |name: &str| NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some(name.to_owned()),
    };
    let (ws, session) = (&records.workspace, &records.session);
    for name in ["Nordaas", "Nordås"] {
        create_place(ws, session, place(name), Provenance::default(), &[])
            .await
            .expect("place");
    }
    let mut ranks = Vec::new();
    let mut kinds = Vec::new();
    for finding in run_checks(ws).await.expect("run checks") {
        if let CheckFinding::PossibleDuplicate {
            kind,
            a: _,
            b: _,
            assessment,
        } = finding
        {
            kinds.push(kind);
            ranks.push((assessment.band, assessment.score_bp));
        }
    }
    assert_eq!(kinds, [MatchableKind::Place, MatchableKind::Person]);
    assert_eq!(ranks.len(), 2, "{ranks:?}");
    assert!(
        ranks[0] > ranks[1],
        "the place, the more similar pair, first: {ranks:?}"
    );
}

/// Every pair the engine shows over a brute-force score of all pairs is a pair the index yields.
#[tokio::test]
async fn blocking_loses_no_pair_a_score_of_every_pair_would_show() {
    let records = Records::new().await;
    let mut persons = Vec::new();
    for (given, surname, year) in [
        ("Guldbrand", "Olsen", Some(1852)),
        ("Gulbrand", "Olsøn", Some(1852)),
        ("Jon", "Olsen", Some(1849)),
        ("Johannes", "Olsen", Some(1853)),
        ("Ole", "Haugen", Some(1850)),
        ("Ole", "Nordby", Some(1851)),
        ("Ole", "Olsen", None),
        ("Katherine", "Brown", Some(1874)),
        ("Catherine", "Brown", Some(1874)),
        ("Kari", "Olsen", Some(1880)),
        ("Kathrine", "Olsen", Some(1876)),
        ("Anne", "Hansen", Some(1900)),
        ("", "Hansen", None),
        ("Per", "Berg", Some(1800)),
        ("Kristian", "Hansen", Some(1860)),
        ("Christian", "Hansen", Some(1861)),
    ] {
        let person = match year {
            Some(year) => records.born(given, surname, year).await.0,
            None => records.person(given, surname).await,
        };
        persons.push(person);
    }
    let mut expected = BTreeSet::new();
    for (i, a) in persons.iter().enumerate() {
        for b in &persons[i + 1..] {
            let assessment = assess(&records.workspace, MatchableKind::Person, a, b)
                .await
                .expect("assess");
            if assessment.band >= MatchBand::Possible {
                expected.insert(BTreeSet::from([a.clone(), b.clone()]));
            }
        }
    }
    assert!(
        expected.len() >= 4,
        "the fixture must exercise several pairs: {expected:?}"
    );
    let pairs = similar_pairs(&records.workspace, MatchableKind::Person, MatchBand::Possible)
        .await
        .expect("similar pairs");
    let found: BTreeSet<BTreeSet<String>> = pairs
        .iter()
        .map(|pair| BTreeSet::from([pair.a.human_id.clone(), pair.b.human_id.clone()]))
        .collect();
    assert_eq!(found.len(), pairs.len(), "each pair once");
    assert_eq!(found, expected);
    for pair in &pairs {
        assert!(pair.a.id < pair.b.id);
    }
}

fn sorted(a: String, b: String) -> (String, String) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Three near-identical Ole Olsens: every pair of them is proposed until decided.
async fn three_oles(records: &Records) -> (String, String, String) {
    let (a, _) = records.born("Ole", "Olsen", 1850).await;
    let (b, _) = records.born("Ole", "Olsen", 1850).await;
    let (c, _) = records.born("Ole", "Olsen", 1850).await;
    (a, b, c)
}

/// A pair the user said are different people is never proposed again, by any consumer (ADR 0039 §3).
#[tokio::test]
async fn a_distinguished_pair_is_never_proposed_again() {
    let records = Records::new().await;
    let (a, b, c) = three_oles(&records).await;
    assert!(records.person_pairs().await.contains(&sorted(a.clone(), b.clone())));

    distinguish_persons(
        &records.workspace,
        &records.session,
        &b,
        &a,
        IdentityDecision::default(),
    )
    .await
    .expect("distinguish");

    let expected = BTreeSet::from([sorted(a.clone(), c.clone()), sorted(b.clone(), c.clone())]);
    assert_eq!(records.person_pairs().await, expected);
    assert_eq!(records.duplicate_findings().await, expected);
    assert_eq!(
        records.similar(MatchableKind::Person, &a).await,
        std::slice::from_ref(&c)
    );
    assert_eq!(records.similar(MatchableKind::Person, &b).await, [c]);
}

#[tokio::test]
async fn a_merged_pair_is_not_proposed_as_a_duplicate() {
    let records = Records::new().await;
    let (a, b, c) = three_oles(&records).await;
    merge_persons(
        &records.workspace,
        &records.session,
        &a,
        &b,
        IdentityDecision::default(),
    )
    .await
    .expect("merge");

    assert!(!records.person_pairs().await.contains(&sorted(a.clone(), b.clone())));
    assert!(!records.similar(MatchableKind::Person, &b).await.contains(&a));
    assert!(records.similar(MatchableKind::Person, &b).await.contains(&c));
}

/// A merged record is hidden behind its root, and a distinction against any record of a cluster holds
/// for the whole cluster (ADR 0039 §4).
#[tokio::test]
async fn a_cluster_is_proposed_once_and_judged_distinct_as_a_whole() {
    let records = Records::new().await;
    let (a, b, c) = three_oles(&records).await;
    let decision = IdentityDecision::default;
    distinguish_persons(&records.workspace, &records.session, &c, &b, decision())
        .await
        .expect("distinguish");
    assert_eq!(
        records.person_pairs().await,
        BTreeSet::from([sorted(a.clone(), b.clone()), sorted(a.clone(), c.clone())])
    );

    merge_persons(&records.workspace, &records.session, &a, &b, decision())
        .await
        .expect("merge");
    assert!(
        records.person_pairs().await.is_empty(),
        "C is distinct from B, so from A's cluster"
    );
    assert!(records.duplicate_findings().await.is_empty());
    assert!(records.similar(MatchableKind::Person, &a).await.is_empty());
    assert!(records.similar(MatchableKind::Person, &c).await.is_empty());
}

#[tokio::test]
async fn undoing_a_distinction_proposes_the_pair_again() {
    let records = Records::new().await;
    let (a, b, _) = three_oles(&records).await;
    distinguish_persons(
        &records.workspace,
        &records.session,
        &a,
        &b,
        IdentityDecision::default(),
    )
    .await
    .expect("distinguish");
    let decision = change_log_for_person(&records.workspace, &a)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "PersonsDistinguished")
        .expect("logged");
    undo_assertion(&records.workspace, &records.session, &a, &decision.assertion_id, None)
        .await
        .expect("undo");

    assert!(records.person_pairs().await.contains(&sorted(a.clone(), b.clone())));
    assert!(records.similar(MatchableKind::Person, &a).await.contains(&b));
}
