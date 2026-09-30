//! The profiles built from the workspace's views (ADR 0038 §2): the relatives that separate two
//! same-named people born the same year (§4), one marriage from a church book and from a GEDCOM file, a
//! farm matched to its parish, and one citation of one church-book page.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    Address, AppDefaults, ChildParentRelationship, DateParts, MutationMeta, NewCitation, NewEvent, NewFact, NewMedia,
    NewNote, NewParticipation, NewPerson, NewPlace, NewRepository, NewSource, OperatorConfig, PersonNameParts,
    Provenance, Session, SourceMediaType, Workspace, WorkspaceDefaults, add_child, add_partner, add_repository_address,
    assert_citation_date, assert_event_date, assert_fact, assert_participation, assert_place_enclosed_by, assert_sex,
    citation_profile, create_citation, create_event, create_family, create_media, create_note, create_person,
    create_place, create_repository, create_source, create_tag, event_profile, family_profile, link_family_event,
    link_place, link_source_repository, media_profile, note_profile, person_profile, place_profile, repository_profile,
    set_media_checksum, set_note_text, set_source_author, set_source_pub_info, source_profile, tag_profile,
};
use vitni_core::age::Age;
use vitni_core::date::DateQuality;
use vitni_core::enums::{EventType, EvidenceLevel, FactType, ParticipantRole, PlaceType, Sex};
use vitni_core::ids::{AgentId, ImportRunId, PlaceId};
use vitni_core::matching::profile::{FamilyProfile, PersonProfile, VitalKind};
use vitni_core::matching::{
    DateBasis, Feature, MatchBand, MatchData, MatchSettings, Outcome, assess_citations, assess_events, assess_families,
    assess_persons, assess_places, assess_tags,
};
use vitni_core::media_path::MediaPath;
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
    ) -> String {
        let date = DateParts {
            year,
            month: Some(6),
            day: Some(1),
        };
        self.event_on(person, event_type, (date, place), participation).await
    }

    /// An event of `event_type` on `date`, at `place`, with `person` taking part in `participation`.
    async fn event_on(
        &self,
        person: &str,
        event_type: EventType,
        (date, place): (DateParts, Option<&str>),
        participation: NewParticipation,
    ) -> String {
        let new = NewEvent {
            human_id: None,
            event_type,
        };
        let event = create_event(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create event");
        assert_event_date(&self.workspace, &self.session, &event, date, MutationMeta::default())
            .await
            .expect("date event");
        if let Some(place) = place {
            link_place(&self.workspace, &self.session, &event, place, MutationMeta::default())
                .await
                .expect("link place");
        }
        self.participate(person, &event, participation).await;
        event
    }

    async fn participate(&self, person: &str, event: &str, participation: NewParticipation) {
        assert_participation(
            &self.workspace,
            &self.session,
            person,
            event,
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

    async fn born_on(&self, person: &str, date: DateParts, place: Option<&str>) {
        let primary = NewParticipation::with_role(ParticipantRole::Primary);
        self.event_on(person, EventType::Birth, (date, place), primary).await;
    }

    /// A family of `partners` with `children`, each child born to every partner.
    async fn family(&self, partners: &[&str], children: &[&str]) -> String {
        let partners: Vec<(&str, ChildParentRelationship)> = partners
            .iter()
            .map(|partner| (*partner, ChildParentRelationship::Birth))
            .collect();
        self.family_of(&partners, children, Provenance::default()).await
    }

    /// A family of `partners` with `children`, each child related to each partner as it states.
    async fn family_of(
        &self,
        partners: &[(&str, ChildParentRelationship)],
        children: &[&str],
        provenance: Provenance,
    ) -> String {
        let family = create_family(&self.workspace, &self.session, provenance, &[])
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
        family
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
            Provenance::default(),
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

fn on(year: i32, month: u8, day: u8) -> DateParts {
    DateParts {
        year,
        month: Some(month),
        day: Some(day),
    }
}

fn from(dataset: &str, record: &str, item: &str) -> Provenance {
    Provenance {
        origin: Some(RecordOrigin {
            dataset: DatasetId::global(dataset),
            record: record.to_owned(),
            item: Some(item.to_owned()),
            digest: None,
            run: ImportRunId::from_uuid(Uuid::from_u128(9)),
        }),
        ..Provenance::default()
    }
}

fn aged(role: ParticipantRole, years: u16) -> NewParticipation {
    NewParticipation {
        age: Some(Age {
            years: Some(years),
            ..Age::default()
        }),
        ..NewParticipation::with_role(role)
    }
}

impl Records {
    /// The Ringsaker church book's entry for the 1877 marriage: the groom and bride with their ages and
    /// fathers, all items of one record.
    async fn church_book(&self, parish: &str) -> String {
        let entry = |item: &str| from("digitalarkivet", "vi01036389000412", item);
        let groom = self
            .person_from("Guldbrand", "Olsen", Sex::Male, entry("person:1"))
            .await;
        let bride = self
            .person_from("Marte", "Pedersdtr.", Sex::Female, entry("person:2"))
            .await;
        let groom_father = self.person_from("Ole", "", Sex::Male, entry("person:3")).await;
        let bride_father = self.person_from("Peder", "", Sex::Male, entry("person:4")).await;
        self.family(&[&groom_father], &[&groom]).await;
        self.family(&[&bride_father], &[&bride]).await;
        let wedding = self
            .event_on(
                &groom,
                EventType::Marriage,
                (on(1877, 10, 14), Some(parish)),
                aged(ParticipantRole::Groom, 27),
            )
            .await;
        self.participate(&bride, &wedding, aged(ParticipantRole::Bride, 24))
            .await;
        let couple = [
            (groom.as_str(), ChildParentRelationship::Birth),
            (bride.as_str(), ChildParentRelationship::Birth),
        ];
        let family = self.family_of(&couple, &[], entry("family:1")).await;
        self.link_marriage(&family, &wedding).await;
        family
    }

    /// The same marriage in a GEDCOM file: exact births in the parish, the partners as the marriage's
    /// primary participants.
    async fn gedcom(&self, parish: &str) -> String {
        let record = |xref: &str| from("gedcom:3f2a", xref, "");
        let husband = self.person_from("Gulbrand", "Olsøn", Sex::Male, record("@I1@")).await;
        self.born_on(&husband, on(1850, 3, 4), Some(parish)).await;
        let wife = self
            .person_from("Marthe", "Pedersdatter", Sex::Female, record("@I2@"))
            .await;
        self.born_on(&wife, on(1853, 5, 2), Some(parish)).await;
        let primary = || NewParticipation::with_role(ParticipantRole::Primary);
        let wedding = self
            .event_on(
                &husband,
                EventType::Marriage,
                (on(1877, 10, 14), Some(parish)),
                primary(),
            )
            .await;
        self.participate(&wife, &wedding, primary()).await;
        let family = self.family(&[&husband, &wife], &[]).await;
        self.link_marriage(&family, &wedding).await;
        family
    }

    async fn link_marriage(&self, family: &str, event: &str) {
        link_family_event(&self.workspace, &self.session, family, event, MutationMeta::default())
            .await
            .expect("link marriage");
    }

    async fn family_profile(&self, family: &str) -> FamilyProfile {
        family_profile(&self.workspace, family).await.expect("family profile")
    }
}

#[tokio::test]
async fn one_marriage_from_a_church_book_and_a_gedcom_file_is_probable() {
    let records = Records::new().await;
    let norge = records.place("Norge", PlaceType::Country, None).await;
    let parish = records.place("Ringsaker", PlaceType::Parish, Some(&norge)).await;
    let church = records.family_profile(&records.church_book(&parish).await).await;
    let gedcom = records.family_profile(&records.gedcom(&parish).await).await;
    let data = MatchData::embedded().expect("embedded match data");
    let assessment = assess_families(&church, &gedcom, &data, &MatchSettings::default());
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    assert_eq!(assessment.parts.len(), 2, "{assessment:#?}");
    let marriage = assessment
        .features
        .iter()
        .find(|f| f.feature == Feature::Marriage)
        .expect("marriage compared");
    assert_eq!(marriage.outcome, Outcome::Agree);

    let (church_event, gedcom_event) = (
        church.marriage.expect("church-book marriage"),
        gedcom.marriage.expect("GEDCOM marriage"),
    );
    let events = assess_events(&church_event, &gedcom_event, &data, &MatchSettings::default());
    assert_eq!(events.band, MatchBand::Probable, "{events:#?}");
}

#[tokio::test]
async fn the_family_profile_carries_partners_children_and_the_marriage() {
    let records = Records::new().await;
    let norge = records.place("Norge", PlaceType::Country, None).await;
    let parish = records.place("Ringsaker", PlaceType::Parish, Some(&norge)).await;
    let family = records.church_book(&parish).await;
    let child = records.person("Anne", "Guldbrandsdatter", Sex::Female).await;
    records.born(&child, 1878, Some(&parish)).await;
    add_child(
        &records.workspace,
        &records.session,
        &family,
        &child,
        Vec::new(),
        MutationMeta::default(),
    )
    .await
    .expect("add child");
    let profile = records.family_profile(&family).await;

    let given: Vec<Option<&str>> = profile.partners.iter().map(|p| p.names[0].given.as_deref()).collect();
    assert_eq!(given, [Some("Guldbrand"), Some("Marte")]);
    for partner in &profile.partners {
        assert!(partner.partners.is_empty(), "the family compares partners itself");
        assert!(partner.children.is_empty(), "the family compares children itself");
        assert_eq!(partner.parents.len(), 1, "a partner keeps their father");
        assert_eq!(partner.vitals[0].basis, DateBasis::FromAge);
    }
    let groom_origins: Vec<Option<&str>> = profile.partners[0].origins.iter().map(|o| o.item.as_deref()).collect();
    assert_eq!(groom_origins, [Some("person:1")]);
    let children: Vec<Option<&str>> = profile.children.iter().map(|c| c.names[0].given.as_deref()).collect();
    assert_eq!(children, [Some("Anne")]);
    assert!(profile.children[0].birth.is_some());
    let origins: Vec<Option<&str>> = profile.origins.iter().map(|o| o.item.as_deref()).collect();
    assert_eq!(origins, [Some("family:1")]);

    let marriage = profile.marriage.expect("the linked marriage");
    assert_eq!(marriage.event_type, Some(EventType::Marriage));
    assert_eq!(marriage.place.and_then(|p| p.country).as_deref(), Some("Norge"));
    let roles: Vec<(ParticipantRole, Option<&str>)> = marriage
        .participants
        .iter()
        .map(|p| (p.role.clone(), p.person.names[0].given.as_deref()))
        .collect();
    assert_eq!(
        roles,
        [
            (ParticipantRole::Groom, Some("Guldbrand")),
            (ParticipantRole::Bride, Some("Marte"))
        ]
    );
}

#[tokio::test]
async fn the_event_profile_carries_type_date_place_and_participants() {
    let records = Records::new().await;
    let norge = records.place("Norge", PlaceType::Country, None).await;
    let parish = records.place("Ringsaker", PlaceType::Parish, Some(&norge)).await;
    let ole = records.person("Ole", "Olsen", Sex::Male).await;
    records.born(&ole, 1850, None).await;
    let census = records
        .event(
            &ole,
            EventType::Census,
            (1865, Some(&parish)),
            aged(ParticipantRole::Primary, 15),
        )
        .await;
    let neighbour = records.person("Anders", "Haugen", Sex::Male).await;
    records
        .participate(
            &neighbour,
            &census,
            NewParticipation::with_role(ParticipantRole::Neighbour),
        )
        .await;
    let origin = from("digitalarkivet", "bf01036389000123", "event:1");
    let recorded = create_event(
        &records.workspace,
        &records.session,
        NewEvent {
            human_id: None,
            event_type: EventType::Census,
        },
        origin,
        &[],
    )
    .await
    .expect("create event");

    let profile = event_profile(&records.workspace, &census).await.expect("event profile");
    assert_eq!(profile.event_type, Some(EventType::Census));
    let year = profile.date.as_ref().and_then(vitni_core::matching::date::year);
    assert_eq!(year, Some(1865));
    let place = profile.place.expect("census place");
    assert_eq!(place.id, Some(records.place_id(&parish).await));
    assert_eq!(place.country.as_deref(), Some("Norge"));
    let participants: Vec<(ParticipantRole, Option<&str>, bool)> = profile
        .participants
        .iter()
        .map(|p| {
            (
                p.role.clone(),
                p.person.names[0].given.as_deref(),
                p.person.birth.is_some(),
            )
        })
        .collect();
    assert_eq!(
        participants,
        [
            (ParticipantRole::Primary, Some("Ole"), true),
            (ParticipantRole::Neighbour, Some("Anders"), false)
        ]
    );
    assert!(profile.origins.is_empty());
    let recorded = event_profile(&records.workspace, &recorded)
        .await
        .expect("event profile");
    let items: Vec<Option<&str>> = recorded.origins.iter().map(|o| o.item.as_deref()).collect();
    assert_eq!(items, [Some("event:1")]);
}

#[tokio::test]
async fn an_unknown_family_or_event_is_not_found() {
    let records = Records::new().await;
    let family = family_profile(&records.workspace, "F9999")
        .await
        .expect_err("no such family");
    assert!(
        matches!(family, vitni_app::AppError::FamilyNotFound(ref id) if id == "F9999"),
        "{family:?}"
    );
    let event = event_profile(&records.workspace, "E9999")
        .await
        .expect_err("no such event");
    assert!(
        matches!(event, vitni_app::AppError::EventNotFound(ref id) if id == "E9999"),
        "{event:?}"
    );
}

#[tokio::test]
async fn a_farm_matched_to_its_parish_is_partial() {
    let records = Records::new().await;
    let norge = records.place("Norge", PlaceType::Country, None).await;
    let parish = records.place("Ringsaker", PlaceType::Parish, Some(&norge)).await;
    let farm = records.place("Haugen", PlaceType::Farm, Some(&parish)).await;
    let (farm, parish) = (
        place_profile(&records.workspace, &farm).await.expect("farm profile"),
        place_profile(&records.workspace, &parish)
            .await
            .expect("parish profile"),
    );
    assert_eq!(farm.place_type, Some(PlaceType::Farm));
    assert_eq!(farm.names[0].text, "Haugen");
    assert_eq!(
        farm.enclosing,
        [parish.id.expect("parish id"), records.place_id(&norge).await]
    );
    assert_eq!(farm.country.as_deref(), Some("Norge"));
    let data = MatchData::embedded().expect("embedded match data");
    let assessment = assess_places(&farm, &parish, &data, &MatchSettings::default());
    let enclosure = assessment
        .features
        .iter()
        .find(|f| f.feature == Feature::Enclosure)
        .expect("enclosure compared");
    assert!(matches!(enclosure.outcome, Outcome::Partial(_)), "{assessment:#?}");
}

#[tokio::test]
async fn the_place_profile_carries_the_creating_origin() {
    let records = Records::new().await;
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some("Haugen".to_owned()),
    };
    let place = create_place(
        &records.workspace,
        &records.session,
        new,
        from("digitalarkivet", "bf01036389000123", "place:1"),
        &[],
    )
    .await
    .expect("create place");
    let profile = place_profile(&records.workspace, &place).await.expect("place profile");
    let items: Vec<Option<&str>> = profile.origins.iter().map(|o| o.item.as_deref()).collect();
    assert_eq!(items, [Some("place:1")]);
}

impl Records {
    /// The regional archive at Hamar.
    async fn archive(&self) -> String {
        let new = NewRepository {
            human_id: None,
            name: Some("Statsarkivet på Hamar".to_owned()),
        };
        let repository = create_repository(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create repository");
        let address = Address {
            locality: Some("Hamar".to_owned()),
            country: Some("Norge".to_owned()),
            ..Address::default()
        };
        add_repository_address(
            &self.workspace,
            &self.session,
            &repository,
            address,
            MutationMeta::default(),
        )
        .await
        .expect("add address");
        repository
    }

    /// The Ringsaker church book, held at `archive`.
    async fn book(&self, archive: &str) -> String {
        let new = NewSource {
            human_id: None,
            title: Some("Ministerialbok for Ringsaker 1870-1880".to_owned()),
        };
        let source = create_source(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create source");
        let meta = MutationMeta::default;
        set_source_author(
            &self.workspace,
            &self.session,
            &source,
            "Ringsaker prestegjeld".to_owned(),
            meta(),
        )
        .await
        .expect("set author");
        set_source_pub_info(&self.workspace, &self.session, &source, "Kirkebok".to_owned(), meta())
            .await
            .expect("set publication");
        link_source_repository(
            &self.workspace,
            &self.session,
            &source,
            archive,
            None,
            SourceMediaType::Book,
            meta(),
        )
        .await
        .expect("link repository");
        source
    }

    /// A citation of `page` in `source`, dated 14 October 1877.
    async fn cite(&self, source: &str, page: &str, provenance: Provenance) -> String {
        let new = NewCitation {
            human_id: None,
            source: source.to_owned(),
            page: Some(page.to_owned()),
        };
        let citation = create_citation(&self.workspace, &self.session, new, provenance, &[])
            .await
            .expect("create citation");
        assert_citation_date(
            &self.workspace,
            &self.session,
            &citation,
            on(1877, 10, 14),
            MutationMeta::default(),
        )
        .await
        .expect("date citation");
        citation
    }
}

#[tokio::test]
async fn one_citation_of_one_church_book_page_is_probable() {
    let records = Records::new().await;
    let archive = records.archive().await;
    let book = records.book(&archive).await;
    let a = records.cite(&book, "s. 45, nr. 12", Provenance::default()).await;
    let b = records
        .cite(
            &book,
            "side 45 nr 12",
            from("digitalarkivet", "vi01036389000412", "citation:1"),
        )
        .await;
    let (a, b) = (
        citation_profile(&records.workspace, &a)
            .await
            .expect("citation profile"),
        citation_profile(&records.workspace, &b)
            .await
            .expect("citation profile"),
    );
    assert_eq!(a.page.as_deref(), Some("s. 45, nr. 12"));
    assert_eq!(a.date.as_ref().and_then(vitni_core::matching::date::year), Some(1877));
    assert!(a.origins.is_empty());
    let items: Vec<Option<&str>> = b.origins.iter().map(|o| o.item.as_deref()).collect();
    assert_eq!(items, [Some("citation:1")]);
    let source = a.source.as_ref().expect("the cited source");
    assert_eq!(source.title.as_deref(), Some("Ministerialbok for Ringsaker 1870-1880"));
    assert_eq!(source.author.as_deref(), Some("Ringsaker prestegjeld"));
    assert_eq!(source.publication.as_deref(), Some("Kirkebok"));
    let holder = &source.repositories[0];
    assert_eq!(holder.name.as_deref(), Some("Statsarkivet på Hamar"));
    assert_eq!(holder.addresses[0].locality.as_deref(), Some("Hamar"));
    assert!(holder.id.is_some());
    assert_eq!(
        source_profile(&records.workspace, &book).await.expect("source profile"),
        *source
    );
    assert_eq!(
        repository_profile(&records.workspace, &archive)
            .await
            .expect("repository profile"),
        *holder
    );
    let data = MatchData::embedded().expect("embedded match data");
    let assessment = assess_citations(&a, &b, &data, &MatchSettings::default());
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    assert_eq!(assessment.parts.len(), 1);
}

#[tokio::test]
async fn the_media_note_and_tag_profiles_carry_their_evidence() {
    let records = Records::new().await;
    let (workspace, session) = (&records.workspace, &records.session);
    let new = NewMedia {
        human_id: None,
        path: Some("media/portretter/ole.jpg".to_owned()),
    };
    let media = create_media(workspace, session, new, Provenance::default(), &[])
        .await
        .expect("create media");
    set_media_checksum(
        workspace,
        session,
        &media,
        "9f86d081".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("set checksum");
    let profile = media_profile(workspace, &media).await.expect("media profile");
    assert_eq!(profile.checksum.as_deref(), Some("9f86d081"));
    assert_eq!(
        profile.path,
        Some(MediaPath::File("media/portretter/ole.jpg".to_owned()))
    );

    let new = NewNote {
        human_id: None,
        text: Some("Flyttet til Amerika.".to_owned()),
    };
    let note = create_note(workspace, session, new, Provenance::default(), &[])
        .await
        .expect("create note");
    set_note_text(
        workspace,
        session,
        &note,
        "Flyttet til Amerika i 1882.".to_owned(),
        Some("nb".to_owned()),
        MutationMeta::default(),
    )
    .await
    .expect("set note text");
    let profile = note_profile(workspace, &note).await.expect("note profile");
    assert_eq!(profile.text.as_deref(), Some("Flyttet til Amerika i 1882."));
    assert_eq!(
        profile.language.as_ref().map(vitni_core::name::LanguageTag::as_str),
        Some("nb")
    );

    let brick = create_tag(workspace, session, "Brick Wall".to_owned(), Provenance::default(), &[])
        .await
        .expect("create tag");
    let wall = create_tag(workspace, session, "brick wall".to_owned(), Provenance::default(), &[])
        .await
        .expect("create tag");
    let (brick, wall) = (
        tag_profile(workspace, &brick).await.expect("tag profile"),
        tag_profile(workspace, &wall).await.expect("tag profile"),
    );
    assert_eq!(brick.name, "Brick Wall");
    assert_eq!(
        assess_tags(&brick, &wall, &MatchSettings::default()).band,
        MatchBand::Deterministic
    );
}

#[tokio::test]
async fn an_unknown_record_of_any_other_kind_is_not_found() {
    use vitni_app::AppError;
    let records = Records::new().await;
    let workspace = &records.workspace;
    let place = place_profile(workspace, "P9999").await.expect_err("no such place");
    assert!(
        matches!(place, AppError::PlaceNotFound(ref id) if id == "P9999"),
        "{place:?}"
    );
    let source = source_profile(workspace, "S9999").await.expect_err("no such source");
    assert!(
        matches!(source, AppError::SourceNotFound(ref id) if id == "S9999"),
        "{source:?}"
    );
    let repository = repository_profile(workspace, "R9999")
        .await
        .expect_err("no such repository");
    assert!(
        matches!(repository, AppError::RepositoryNotFound(ref id) if id == "R9999"),
        "{repository:?}"
    );
    let citation = citation_profile(workspace, "C9999")
        .await
        .expect_err("no such citation");
    assert!(
        matches!(citation, AppError::CitationNotFound(ref id) if id == "C9999"),
        "{citation:?}"
    );
    let media = media_profile(workspace, "O9999").await.expect_err("no such media");
    assert!(
        matches!(media, AppError::MediaNotFound(ref id) if id == "O9999"),
        "{media:?}"
    );
    let note = note_profile(workspace, "N9999").await.expect_err("no such note");
    assert!(
        matches!(note, AppError::NoteNotFound(ref id) if id == "N9999"),
        "{note:?}"
    );
    let missing = Uuid::from_u128(7).to_string();
    let tag = tag_profile(workspace, &missing).await.expect_err("no such tag");
    assert!(
        matches!(tag, AppError::TagNotFound(ref id) if *id == missing),
        "{tag:?}"
    );
}
