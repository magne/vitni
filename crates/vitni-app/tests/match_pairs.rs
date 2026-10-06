//! The `match_pairs` projection (ADR 0048) follows every record a pair's score reads: after each kind of
//! edit, the pairs refreshed from the records it touched equal the pairs scored again from scratch.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeMap;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, DateParts, IdentityDecision, MatchBand, MatchableKind, MutationMeta, NewCitation, NewEvent,
    NewParticipation, NewPerson, NewPlace, NewRepository, NewSource, OperatorConfig, PersonNameParts, Provenance,
    Session, Workspace, WorkspaceDefaults, add_child, add_partner, assert_event_date, assert_participation,
    assert_place_enclosed_by, assert_sex, change_log_for_event, change_log_for_person, change_log_for_place,
    create_citation, create_event, create_family, create_person, create_place, create_repository, create_source,
    distinguish_persons, link_family_event, link_place, link_source_repository, merge_persons, set_repository_name,
    set_title, similar_pairs, undo_assertion, undo_event_assertion, undo_place_assertion,
};
use vitni_core::enums::{
    ChildParentRelationship, EventType, EvidenceLevel, ParticipantRole, PlaceType, Sex, SourceMediaType,
};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

struct Records {
    workspace: Workspace,
    session: Session,
    _dir: tempfile::TempDir,
}

/// Every stored pair of every kind: `(a, b, band, score)`.
type Pairs = BTreeMap<MatchableKind, Vec<(String, String, MatchBand, u64)>>;

impl Records {
    async fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ws");
        Workspace::init(&path, &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Workspace::open(&path, &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open workspace");
        let session = Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Tester".to_owned()),
        });
        Self {
            workspace,
            session,
            _dir: dir,
        }
    }

    async fn person(&self, given: &str, surname: &str, sex: Sex) -> String {
        let new = NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                Some(given.to_owned()),
                Some(surname.to_owned()),
            )),
            evidence_level: EvidenceLevel::Conclusion,
            external_ids: Vec::new(),
        };
        let person = create_person(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create person");
        assert_sex(&self.workspace, &self.session, &person, sex, MutationMeta::default())
            .await
            .expect("assert sex");
        person
    }

    async fn event(&self, event_type: EventType, year: i32) -> String {
        let new = NewEvent {
            human_id: None,
            event_type,
        };
        let event = create_event(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create event");
        self.date(&event, year).await;
        event
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

    async fn take_part(&self, person: &str, event: &str) {
        let role = NewParticipation::with_role(ParticipantRole::Primary);
        assert_participation(
            &self.workspace,
            &self.session,
            person,
            event,
            role,
            MutationMeta::default(),
        )
        .await
        .expect("participate");
    }

    /// A family of `father` and `mother` with `child` born to both.
    async fn family(&self, father: &str, mother: &str, child: &str) -> String {
        let (ws, session) = (&self.workspace, &self.session);
        let family = create_family(ws, session, Provenance::default(), &[])
            .await
            .expect("create family");
        for partner in [father, mother] {
            add_partner(ws, session, &family, partner, MutationMeta::default())
                .await
                .expect("add partner");
        }
        let relationships = vec![
            (father.to_owned(), ChildParentRelationship::Birth),
            (mother.to_owned(), ChildParentRelationship::Birth),
        ];
        add_child(ws, session, &family, child, relationships, MutationMeta::default())
            .await
            .expect("add child");
        family
    }

    async fn place(&self, name: &str, place_type: PlaceType) -> String {
        let new = NewPlace {
            human_id: None,
            place_type,
            name: Some(name.to_owned()),
        };
        create_place(&self.workspace, &self.session, new, Provenance::default(), &[])
            .await
            .expect("create place")
    }

    async fn pairs(&self) -> Pairs {
        let mut every = Pairs::new();
        for kind in MatchableKind::ALL {
            let pairs = similar_pairs(&self.workspace, kind, MatchBand::Possible)
                .await
                .expect("similar pairs");
            let mut rows = Vec::new();
            for pair in pairs {
                rows.push((pair.a.id, pair.b.id, pair.band, pair.score.to_bits()));
            }
            every.insert(kind, rows);
        }
        every
    }

    /// The pairs refreshed from the records touched equal the pairs scored from scratch.
    async fn assert_refreshed_as_rebuilt(&self, after: &str) {
        let refreshed = self.pairs().await;
        self.workspace
            .store()
            .rebuild_projections()
            .await
            .expect("rebuild projections");
        let rebuilt = self.pairs().await;
        assert_eq!(refreshed, rebuilt, "after {after}");
    }
}

/// Two of everything a score reads, alike, so that every edit below changes some pair's score: farms in
/// parishes in a county, a family of Ole Olsen and Kari Hansdatter marrying in 1875 with a son Hans born
/// 1880 at the farm, Hans's own family with Anne Larsdatter, and a church book held by an archive with
/// a citation into it.
struct Scenario {
    county: String,
    farms: [String; 2],
    fathers: [String; 2],
    fathers_births: [String; 2],
    sons: [String; 2],
    births: [String; 2],
    marriages: [String; 2],
    archives: [String; 2],
    sources: [String; 2],
}

impl Scenario {
    async fn new(records: &Records) -> Self {
        let (ws, session) = (&records.workspace, &records.session);
        let county = records.place("Akershus", PlaceType::County).await;
        let parish = records.place("Asker", PlaceType::Parish).await;
        let farms = [
            records.place("Nordaas", PlaceType::Farm).await,
            records.place("Nordås", PlaceType::Farm).await,
        ];
        for farm in &farms {
            assert_place_enclosed_by(ws, session, farm, &parish, MutationMeta::default())
                .await
                .expect("enclose a farm");
        }
        assert_place_enclosed_by(ws, session, &parish, &county, MutationMeta::default())
            .await
            .expect("enclose the parish");
        let mut fathers = Vec::new();
        let mut fathers_births = Vec::new();
        let mut sons = Vec::new();
        let mut births = Vec::new();
        let mut marriages = Vec::new();
        for farm in &farms {
            let father = records.person("Ole", "Olsen", Sex::Male).await;
            let father_born = records.event(EventType::Birth, 1850).await;
            records.take_part(&father, &father_born).await;
            let mother = records.person("Kari", "Hansdatter", Sex::Female).await;
            let son = records.person("Hans", "Olsen", Sex::Male).await;
            let birth = records.event(EventType::Birth, 1880).await;
            link_place(ws, session, &birth, farm, MutationMeta::default())
                .await
                .expect("link place");
            records.take_part(&son, &birth).await;
            let family = records.family(&father, &mother, &son).await;
            let marriage = records.event(EventType::Marriage, 1875).await;
            link_family_event(ws, session, &family, &marriage, MutationMeta::default())
                .await
                .expect("link marriage");
            let wife = records.person("Anne", "Larsdatter", Sex::Female).await;
            let own = create_family(ws, session, Provenance::default(), &[])
                .await
                .expect("create family");
            for partner in [&son, &wife] {
                add_partner(ws, session, &own, partner, MutationMeta::default())
                    .await
                    .expect("add partner");
            }
            fathers.push(father);
            fathers_births.push(father_born);
            sons.push(son);
            births.push(birth);
            marriages.push(marriage);
        }
        let (archives, sources) = archives(records).await;
        let pair = |items: Vec<String>| -> [String; 2] { items.try_into().expect("two") };
        Self {
            county,
            farms,
            fathers: pair(fathers),
            fathers_births: pair(fathers_births),
            sons: pair(sons),
            births: pair(births),
            marriages: pair(marriages),
            archives,
            sources,
        }
    }
}

/// Two church books, each held by its own archive, each with a citation into page 12.
async fn archives(records: &Records) -> ([String; 2], [String; 2]) {
    let (ws, session) = (&records.workspace, &records.session);
    let mut archives = Vec::new();
    let mut sources = Vec::new();
    for name in ["Statsarkivet", "Statsarkivet i Oslo"] {
        let new = NewRepository {
            human_id: None,
            name: Some(name.to_owned()),
        };
        let archive = create_repository(ws, session, new, Provenance::default(), &[])
            .await
            .expect("repository");
        let new = NewSource {
            human_id: None,
            title: Some("Kirkebok for Asker".to_owned()),
        };
        let source = create_source(ws, session, new, Provenance::default(), &[])
            .await
            .expect("source");
        link_source_repository(
            ws,
            session,
            &source,
            &archive,
            None,
            SourceMediaType::Book,
            MutationMeta::default(),
        )
        .await
        .expect("hold");
        let new = NewCitation {
            human_id: None,
            source: source.clone(),
            page: Some("12".to_owned()),
        };
        create_citation(ws, session, new, Provenance::default(), &[])
            .await
            .expect("citation");
        archives.push(archive);
        sources.push(source);
    }
    let pair = |items: Vec<String>| -> [String; 2] { items.try_into().expect("two") };
    (pair(archives), pair(sources))
}

/// The assertion of the latest `event_type` in `log`: what an undo of that edit retracts.
fn latest(log: Vec<vitni_app::ChangeLogEntry>, event_type: &str) -> String {
    log.into_iter()
        .filter(|entry| entry.event_type == event_type)
        .max_by_key(|entry| entry.sequence)
        .expect("logged")
        .assertion_id
}

#[tokio::test]
async fn the_pairs_follow_every_record_a_score_reads() {
    let records = Records::new().await;
    let (ws, session) = (&records.workspace, &records.session);
    let scenario = Scenario::new(&records).await;
    records.assert_refreshed_as_rebuilt("the records were made").await;

    let son = &scenario.sons[0];
    let sexed = latest(change_log_for_person(ws, son).await.expect("log"), "SexAsserted");
    undo_assertion(ws, session, son, &sexed, None)
        .await
        .expect("unsex a son");
    records.assert_refreshed_as_rebuilt("a son lost his sex").await;

    let denmark = records.place("Danmark", PlaceType::Country).await;
    link_place(
        ws,
        session,
        &scenario.fathers_births[0],
        &denmark,
        MutationMeta::default(),
    )
    .await
    .expect("a father born in Denmark");
    records
        .assert_refreshed_as_rebuilt("a father was born in Denmark")
        .await;

    let country = records.place("Norge", PlaceType::Country).await;
    assert_place_enclosed_by(ws, session, &scenario.county, &country, MutationMeta::default())
        .await
        .expect("put the county in a country");
    records
        .assert_refreshed_as_rebuilt("the county was put in a country")
        .await;

    let farm = &scenario.farms[0];
    let named = latest(change_log_for_place(ws, farm).await.expect("log"), "NameAsserted");
    undo_place_assertion(ws, session, farm, &named, None)
        .await
        .expect("unname a farm");
    records.assert_refreshed_as_rebuilt("a farm lost its name").await;

    let father = &scenario.fathers[0];
    let named = latest(change_log_for_person(ws, father).await.expect("log"), "NameAsserted");
    undo_assertion(ws, session, father, &named, None)
        .await
        .expect("unname a father");
    records.assert_refreshed_as_rebuilt("a father lost his name").await;

    let birth = &scenario.births[0];
    let dated = latest(change_log_for_event(ws, birth).await.expect("log"), "DateAsserted");
    undo_event_assertion(ws, session, birth, &dated, None)
        .await
        .expect("undate a birth");
    records.assert_refreshed_as_rebuilt("a birth lost its date").await;

    let son = &scenario.sons[0];
    let named = latest(change_log_for_person(ws, son).await.expect("log"), "NameAsserted");
    undo_assertion(ws, session, son, &named, None)
        .await
        .expect("unname a son");
    records.assert_refreshed_as_rebuilt("a son lost his name").await;

    let marriage = &scenario.marriages[0];
    let dated = latest(change_log_for_event(ws, marriage).await.expect("log"), "DateAsserted");
    undo_event_assertion(ws, session, marriage, &dated, None)
        .await
        .expect("undate a marriage");
    records.assert_refreshed_as_rebuilt("a marriage lost its date").await;

    set_repository_name(
        ws,
        session,
        &scenario.archives[1],
        "Riksarkivet".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("rename an archive");
    records.assert_refreshed_as_rebuilt("an archive was renamed").await;

    set_title(
        ws,
        session,
        &scenario.sources[1],
        "Kirkebok for Asker prestegjeld".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("retitle a source");
    records.assert_refreshed_as_rebuilt("a source was retitled").await;

    distinguish_persons(
        ws,
        session,
        &scenario.sons[0],
        &scenario.sons[1],
        IdentityDecision::default(),
    )
    .await
    .expect("distinguish");
    records.assert_refreshed_as_rebuilt("two sons were held distinct").await;

    merge_persons(
        ws,
        session,
        &scenario.fathers[0],
        &scenario.fathers[1],
        IdentityDecision::default(),
    )
    .await
    .expect("merge the fathers");
    records.assert_refreshed_as_rebuilt("the fathers were merged").await;
}
