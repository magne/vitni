//! The person profile built from the workspace's views (ADR 0038 §2), and the relatives that separate
//! two same-named people born the same year (§4).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, ChildParentRelationship, DateParts, MutationMeta, NewEvent, NewFact, NewParticipation, NewPerson,
    NewPlace, OperatorConfig, PersonNameParts, Provenance, Session, Workspace, WorkspaceDefaults, add_child,
    add_partner, assert_event_date, assert_fact, assert_participation, assert_place_enclosed_by, assert_sex,
    create_event, create_family, create_person, create_place, link_place, person_profile,
};
use vitni_core::age::Age;
use vitni_core::date::DateQuality;
use vitni_core::enums::{EventType, EvidenceLevel, FactType, ParticipantRole, PlaceType, Sex};
use vitni_core::ids::{AgentId, ImportRunId, PlaceId};
use vitni_core::matching::profile::{PersonProfile, VitalKind};
use vitni_core::matching::{DateBasis, Feature, MatchBand, MatchData, MatchSettings, Outcome, assess_persons};
use vitni_core::origin::{DatasetId, RecordOrigin};
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

/// Builds one workspace's records through the use cases.
struct Records {
    workspace: Workspace,
    session: Session,
    _dir: tempfile::TempDir,
}

impl Records {
    async fn new() -> Self {
        let (workspace, dir) = workspace().await;
        Self {
            workspace,
            session: session(),
            _dir: dir,
        }
    }

    async fn person_from(&self, given: &str, surname: &str, sex: Sex, provenance: Provenance) -> String {
        let new = NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                Some(given.to_owned()),
                Some(surname.to_owned()),
            )),
            evidence_level: EvidenceLevel::Conclusion,
            external_ids: Vec::new(),
        };
        let person = create_person(&self.workspace, &self.session, new, provenance, &[])
            .await
            .expect("create person");
        assert_sex(&self.workspace, &self.session, &person, sex, MutationMeta::default())
            .await
            .expect("assert sex");
        person
    }

    async fn person(&self, given: &str, surname: &str, sex: Sex) -> String {
        self.person_from(given, surname, sex, Provenance::default()).await
    }

    async fn place(&self, name: &str, place_type: PlaceType, enclosed_by: Option<&str>) -> String {
        let new = NewPlace {
            human_id: None,
            place_type,
            name: Some(name.to_owned()),
        };
        let place = create_place(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create place");
        if let Some(enclosing) = enclosed_by {
            assert_place_enclosed_by(
                &self.workspace,
                &self.session,
                &place,
                enclosing,
                MutationMeta::default(),
            )
            .await
            .expect("enclose place");
        }
        place
    }

    async fn place_id(&self, human_id: &str) -> PlaceId {
        self.workspace
            .store()
            .find_place(human_id)
            .await
            .expect("find place")
            .and_then(|view| view.place_id())
            .expect("place projected")
    }

    /// An event of `event_type` in `year`, at `place`, with `person` taking part in `participation`.
    async fn event(
        &self,
        person: &str,
        event_type: EventType,
        (year, place): (i32, Option<&str>),
        participation: NewParticipation,
    ) {
        let new = NewEvent {
            human_id: None,
            event_type,
        };
        let event = create_event(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create event");
        let date = DateParts {
            year,
            month: Some(6),
            day: Some(1),
        };
        assert_event_date(&self.workspace, &self.session, &event, date, MutationMeta::default())
            .await
            .expect("date event");
        if let Some(place) = place {
            link_place(&self.workspace, &self.session, &event, place, MutationMeta::default())
                .await
                .expect("link place");
        }
        assert_participation(
            &self.workspace,
            &self.session,
            person,
            &event,
            participation,
            MutationMeta::default(),
        )
        .await
        .expect("participate");
    }

    async fn born(&self, person: &str, year: i32, place: Option<&str>) {
        let primary = NewParticipation::with_role(ParticipantRole::Primary);
        self.event(person, EventType::Birth, (year, place), primary).await;
    }

    /// A family of `partners` with `children`, each child born to every partner.
    async fn family(&self, partners: &[&str], children: &[&str]) {
        let partners: Vec<(&str, ChildParentRelationship)> = partners
            .iter()
            .map(|partner| (*partner, ChildParentRelationship::Birth))
            .collect();
        self.family_of(&partners, children).await;
    }

    /// A family of `partners` with `children`, each child related to each partner as it states.
    async fn family_of(&self, partners: &[(&str, ChildParentRelationship)], children: &[&str]) {
        let family = create_family(&self.workspace, &self.session, Provenance::default(), &[])
            .await
            .expect("create family");
        for (partner, _) in partners {
            add_partner(
                &self.workspace,
                &self.session,
                &family,
                partner,
                MutationMeta::default(),
            )
            .await
            .expect("add partner");
        }
        for child in children {
            let relationships = partners
                .iter()
                .map(|(partner, relationship)| ((*partner).to_owned(), relationship.clone()))
                .collect();
            add_child(
                &self.workspace,
                &self.session,
                &family,
                child,
                relationships,
                MutationMeta::default(),
            )
            .await
            .expect("add child");
        }
    }

    async fn profile(&self, person: &str) -> PersonProfile {
        person_profile(&self.workspace, person).await.expect("person profile")
    }
}

fn assess(a: &PersonProfile, b: &PersonProfile) -> vitni_core::matching::MatchAssessment {
    let data = MatchData::embedded().expect("embedded match data");
    assess_persons(a, b, &data, &MatchSettings::default())
}

#[tokio::test]
async fn two_ole_olsens_born_the_same_year_separate_on_their_fathers() {
    let records = Records::new().await;
    let norge = records.place("Norge", PlaceType::Country, None).await;
    let parish = records.place("Ringsaker", PlaceType::Parish, Some(&norge)).await;
    let mut oles = Vec::new();
    for father_born in [1815, 1830] {
        let ole = records.person("Ole", "Olsen", Sex::Male).await;
        records.born(&ole, 1850, Some(&parish)).await;
        let father = records.person("Ole", "Hansen", Sex::Male).await;
        records.born(&father, father_born, Some(&parish)).await;
        records.family(&[&father], &[&ole]).await;
        oles.push(records.profile(&ole).await);
    }
    let apart = assess(&oles[0], &oles[1]);
    let father = apart
        .features
        .iter()
        .find(|f| f.feature == Feature::Father)
        .expect("fathers compared");
    assert_eq!(father.outcome, Outcome::Disagree, "{father:?}");
    assert!(apart.band < MatchBand::Probable, "{apart:#?}");

    let mut fatherless = oles[1].clone();
    fatherless.parents.clear();
    let unknown = assess(&oles[0], &fatherless);
    assert_eq!(unknown.band, MatchBand::Probable, "{unknown:#?}");
}

#[tokio::test]
async fn the_profile_carries_vitals_places_occupations_and_relatives() {
    let records = Records::new().await;
    let norge = records.place("Norge", PlaceType::Country, None).await;
    let parish = records.place("Ringsaker", PlaceType::Parish, Some(&norge)).await;
    let farm = records.place("Haugen", PlaceType::Farm, Some(&parish)).await;
    let ole = records.person("Ole", "Olsen", Sex::Male).await;
    let primary = || NewParticipation::with_role(ParticipantRole::Primary);
    records
        .event(&ole, EventType::Christening, (1850, Some(&farm)), primary())
        .await;
    records.event(&ole, EventType::Burial, (1910, None), primary()).await;
    let occupation = NewFact {
        fact_type: FactType::Occupation,
        value: Some("Husmann".to_owned()),
        date: None,
    };
    assert_fact(
        &records.workspace,
        &records.session,
        &ole,
        occupation,
        MutationMeta::default(),
    )
    .await
    .expect("assert occupation");
    let (father, mother) = (
        records.person("Ole", "Hansen", Sex::Male).await,
        records.person("Marte", "Pedersdatter", Sex::Female).await,
    );
    records.born(&mother, 1822, Some(&parish)).await;
    records.family(&[&father, &mother], &[&ole]).await;
    let (wife, child) = (
        records.person("Kari", "Nilsdatter", Sex::Female).await,
        records.person("Anne", "Olsdatter", Sex::Female).await,
    );
    records.born(&child, 1880, None).await;
    records.family(&[&ole, &wife], &[&child]).await;

    let profile = records.profile(&ole).await;
    assert_eq!(profile.names[0].given.as_deref(), Some("Ole"));
    assert_eq!(profile.sex, Some(Sex::Male));
    let kinds: Vec<VitalKind> = profile.vitals.iter().map(|v| v.kind).collect();
    assert_eq!(kinds, [VitalKind::Baptism, VitalKind::Burial]);
    let place = profile.vitals[0].place.as_ref().expect("baptism place");
    assert_eq!(place.id, Some(records.place_id(&farm).await));
    assert_eq!(place.names[0].text, "Haugen");
    assert_eq!(place.country.as_deref(), Some("Norge"));
    let mut enclosing = place.enclosing.clone();
    enclosing.sort();
    let mut expected = vec![records.place_id(&parish).await, records.place_id(&norge).await];
    expected.sort();
    assert_eq!(enclosing, expected);
    assert_eq!(profile.occupations, ["Husmann"]);

    let parents: Vec<(Option<&str>, Option<Sex>)> = profile
        .parents
        .iter()
        .map(|p| (p.names[0].given.as_deref(), p.sex.clone()))
        .collect();
    assert_eq!(
        parents,
        [(Some("Ole"), Some(Sex::Male)), (Some("Marte"), Some(Sex::Female))]
    );
    let mother_birth = profile.parents[1].birth.as_ref().expect("mother's birth");
    assert_eq!(mother_birth.kind, VitalKind::Birth);
    assert!(
        profile.lineage.iter().any(|mention| mention.country == "Norge"),
        "the parents' places select cultures: {:?}",
        profile.lineage
    );
    assert_eq!(profile.partners.len(), 1);
    assert_eq!(profile.partners[0].names[0].given.as_deref(), Some("Kari"));
    assert_eq!(profile.children.len(), 1);
    assert!(profile.children[0].birth.is_some());
}

#[tokio::test]
async fn a_step_father_is_not_a_parent() {
    let records = Records::new().await;
    let ole = records.person("Ole", "Olsen", Sex::Male).await;
    let (mother, step_father) = (
        records.person("Marte", "Pedersdatter", Sex::Female).await,
        records.person("Hans", "Nilsen", Sex::Male).await,
    );
    records
        .family_of(
            &[
                (&mother, ChildParentRelationship::Birth),
                (&step_father, ChildParentRelationship::Step),
            ],
            &[&ole],
        )
        .await;
    let profile = records.profile(&ole).await;
    let parents: Vec<Option<&str>> = profile.parents.iter().map(|p| p.names[0].given.as_deref()).collect();
    assert_eq!(parents, [Some("Marte")]);
    assert!(records.profile(&step_father).await.children.is_empty());
}

#[tokio::test]
async fn a_census_age_stands_in_for_an_unrecorded_birth() {
    let records = Records::new().await;
    let ole = records.person("Ole", "Olsen", Sex::Male).await;
    let enumerated = NewParticipation {
        age: Some(Age {
            years: Some(15),
            ..Age::default()
        }),
        ..NewParticipation::with_role(ParticipantRole::Primary)
    };
    records.event(&ole, EventType::Census, (1865, None), enumerated).await;
    let profile = records.profile(&ole).await;
    assert_eq!(profile.vitals.len(), 1, "one estimated birth: {:?}", profile.vitals);
    let birth = &profile.vitals[0];
    assert_eq!(birth.kind, VitalKind::Birth);
    assert_eq!(birth.basis, DateBasis::FromAge);
    let date = birth.date.as_ref().expect("estimated date");
    assert_eq!(date.quality, DateQuality::Calculated);
    assert_eq!(
        date.sort_value,
        vitni_app::gregorian_date(DateParts {
            year: 1850,
            month: None,
            day: None
        })
        .sort_value
    );

    records.born(&ole, 1849, None).await;
    let recorded = records.profile(&ole).await;
    assert!(
        recorded.vitals.iter().all(|v| v.basis == DateBasis::Recorded),
        "a recorded birth wins: {:?}",
        recorded.vitals
    );
}

#[tokio::test]
async fn the_profile_carries_the_creating_origin_only() {
    let records = Records::new().await;
    let origin = |item: &str| RecordOrigin {
        dataset: DatasetId::global("digitalarkivet"),
        record: "bf01036389000123".to_owned(),
        item: Some(item.to_owned()),
        digest: None,
        run: ImportRunId::from_uuid(Uuid::from_u128(9)),
    };
    let from = |item: &str| Provenance {
        origin: Some(origin(item)),
        ..Provenance::default()
    };
    let child = records.person_from("Ole", "Olsen", Sex::Male, from("person:1")).await;
    let father = records.person_from("Ole", "Hansen", Sex::Male, from("person:2")).await;
    let profile = records.profile(&child).await;
    let keys: Vec<(&str, &str, Option<&str>)> = profile
        .origins
        .iter()
        .map(|o| (o.dataset.as_str(), o.record.as_str(), o.item.as_deref()))
        .collect();
    assert_eq!(keys, [("digitalarkivet", "bf01036389000123", Some("person:1"))]);
    assert!(
        records
            .profile(&records.person("Per", "Olsen", Sex::Male).await)
            .await
            .origins
            .is_empty()
    );

    let father = records.profile(&father).await;
    let assessment = assess(&profile, &father);
    let record = assessment
        .features
        .iter()
        .find(|f| f.feature == Feature::Record)
        .expect("two items of one record");
    assert_eq!(record.outcome, Outcome::Conflict);
}

#[tokio::test]
async fn an_unknown_person_is_not_found() {
    let records = Records::new().await;
    let error = person_profile(&records.workspace, "I9999")
        .await
        .expect_err("no such person");
    assert!(
        matches!(error, vitni_app::AppError::PersonNotFound(ref id) if id == "I9999"),
        "{error:?}"
    );
}
