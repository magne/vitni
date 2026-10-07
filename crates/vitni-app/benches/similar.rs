//! Record-matching benchmarks at scale (ADR 0038 §7, *Consequences*; ADR 0048): the `match_keys`
//! index build, one `find_similar` over a fresh index, one edit followed by the lookup that rekeys it,
//! the `match_pairs` build that scores every pair, the Dashboard's two loads — its stats and activity,
//! and its data-quality checks over built pairs — one edit followed by those checks, and the plan of
//! one record an assisted import submits (ADR 0040 §2).
//!
//! Each size is seeded, measured and dropped before the next, so only one workspace is in memory. The
//! pairs build is measured only up to [`PAIRS_SAMPLED_UP_TO`]: at 100k persons one build takes minutes,
//! and the checks after it are what a user waits for.
//!
//! The workspace is seeded with persons, each with a dated birth event in Norway, whose names and years
//! are drawn from Norwegian-style pools with spelling variants, so blocking buckets are realistically
//! crowded and every comparison applies the Norwegian and Danish packs the region table selects.
//! The log is built with the pure `decide`/`evolve` of the core and bulk-loaded through
//! [`vitni_db::Store::insert_raw_events`], then the projections are rebuilt — the same rows the
//! command path writes, without a round trip per command.

#![expect(clippy::expect_used, reason = "benchmark setup aborts on failure")]

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cqrs_es::DomainEvent;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use tempfile::TempDir;
use time::macros::datetime;
use uuid::Uuid;
use vitni_app::{
    AppDefaults, CheckFinding, DatasetId, DateParts, EntityFields, EntityRef, ImportReview, LinkKind, MatchBand,
    MatchableKind, MutationMeta, NewImportRun, OperatorConfig, PendingRun, PersonNameParts, RecordGraph, Session,
    StagedEntity, StagedEvent, StagedLink, StagedPerson, StagedPlace, Workspace, WorkspaceDefaults, assert_event_date,
    evidence_health, find_similar, gregorian_date, person_names, recent_activity, run_checks, workspace_counts,
};
use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole, PlaceType, Sex};
use vitni_core::event::command::EventCommand;
use vitni_core::event::{EventRefs, EventState};
use vitni_core::ids::{AgentId, AssertionId, EventId, HumanId, PersonId, PlaceId};
use vitni_core::name::{NameType, PersonName, Surname};
use vitni_core::person::PersonState;
use vitni_core::person::command::PersonCommand;
use vitni_core::place::command::PlaceCommand;
use vitni_core::place::{PlaceRefs, PlaceState};
use vitni_core::place_name::PlaceName;
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use vitni_db::{DbError, RawEvent};

/// The person counts benchmarked; the largest is the ADR's 100k.
const SIZES: [usize; 2] = [10_000, 100_000];

/// The largest size the pairs build is measured at.
const PAIRS_SAMPLED_UP_TO: usize = 10_000;

/// How many of the strongest possible matches the checks list, as the Dashboard does.
const SHOWN: usize = 5;

/// How many activity rows the Dashboard lists.
const ACTIVITY: u32 = 12;

/// Persons whose rows are built and inserted together while seeding, bounding the rows held at once.
const SEED_CHUNK: usize = 10_000;

const PERSON_BASE: u128 = 0x0000_0001_0000_0000;
const EVENT_BASE: u128 = 0x0000_0002_0000_0000;
const COUNTRY: u128 = 0x0000_0003_0000_0000;
const ASSERTION_BASE: u128 = 0x9000_0000_0000_0000;

const GIVEN: [&str; 40] = [
    "Ole",
    "Ola",
    "Olav",
    "Hans",
    "Johannes",
    "Johan",
    "Jon",
    "Jens",
    "Peder",
    "Per",
    "Anders",
    "Andreas",
    "Nils",
    "Niels",
    "Lars",
    "Lars",
    "Kristian",
    "Christian",
    "Gulbrand",
    "Guldbrand",
    "Anne",
    "Anna",
    "Kari",
    "Karen",
    "Kathrine",
    "Marte",
    "Martha",
    "Ingeborg",
    "Ingebor",
    "Marit",
    "Maren",
    "Berit",
    "Guri",
    "Gunhild",
    "Sigrid",
    "Siri",
    "Randi",
    "Ragnhild",
    "Elen",
    "Helene",
];

const SURNAMES: [&str; 40] = [
    "Olsen",
    "Olsøn",
    "Hansen",
    "Hanssen",
    "Johansen",
    "Johannessen",
    "Jensen",
    "Pedersen",
    "Andersen",
    "Nilsen",
    "Larsen",
    "Kristiansen",
    "Christiansen",
    "Olsdatter",
    "Hansdatter",
    "Pedersdatter",
    "Haugen",
    "Haug",
    "Bakken",
    "Bakke",
    "Nordby",
    "Moen",
    "Berg",
    "Lien",
    "Dahl",
    "Strand",
    "Solberg",
    "Nygaard",
    "Nygård",
    "Aas",
    "Ås",
    "Holm",
    "Lund",
    "Sæther",
    "Sether",
    "Rud",
    "Eide",
    "Vik",
    "Lie",
    "Brekke",
];

/// A deterministic pseudo-random sequence (a 64-bit LCG), so every run seeds the same workspace.
struct Draw(u64);

impl Draw {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let bound = u64::try_from(bound).expect("bound fits u64");
        usize::try_from((self.0 >> 33) % bound).expect("draw fits usize")
    }
}

fn meta(counter: &mut u128) -> AssertionMeta {
    *counter += 1;
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(*counter)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-09-30 12:00:00 UTC)),
            rationale: None,
            confidence: Some(Confidence::Normal),
            citations: Vec::new(),
            evidence_analysis: None,
            origin: None,
        },
    }
}

fn name(given: &str, surname: &str) -> PersonName {
    PersonName {
        name_type: NameType::BirthName,
        given: Some(given.to_owned()),
        surnames: vec![Surname {
            prefix: None,
            surname: surname.to_owned(),
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

fn year(year: i32) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(year),
            month: None,
            day: None,
        })),
        time: None,
        new_year_begins: None,
        sort_value: 0,
        original_text: None,
    }
}

/// The stored row of one event, as the command path writes it.
fn row<E: DomainEvent>(aggregate_type: &str, aggregate_id: &str, sequence: i64, event: &E) -> RawEvent {
    RawEvent {
        aggregate_type: aggregate_type.to_owned(),
        aggregate_id: aggregate_id.to_owned(),
        sequence,
        event_type: event.event_type(),
        event_version: event.event_version(),
        payload: serde_json::to_value(event).expect("serialize event"),
        metadata: serde_json::json!({}),
    }
}

/// One person with a dated birth: the person's and the birth event's rows.
fn person_rows(i: usize, draw: &mut Draw, counter: &mut u128) -> Vec<RawEvent> {
    let offset = u128::try_from(i).expect("index fits u128");
    let (person_id, event_id) = (
        PersonId::from_uuid(Uuid::from_u128(PERSON_BASE + offset)),
        EventId::from_uuid(Uuid::from_u128(EVENT_BASE + offset)),
    );
    let born = 1750 + i32::try_from(draw.next(200)).expect("year fits i32");
    let sex = if draw.next(2) == 0 { Sex::Male } else { Sex::Female };
    let mut rows = Vec::new();

    let mut event = EventState::default();
    let refs = EventRefs { place_exists: true };
    let event_commands = [
        EventCommand::CreateEvent {
            event_id,
            human_id: HumanId::new(format!("E{:07}", i + 1)),
            event_type: EventType::Birth,
        },
        EventCommand::AssertDate {
            event_id,
            date: year(born),
        },
        EventCommand::LinkPlace {
            event_id,
            place_id: country_id(),
        },
    ];
    for command in event_commands {
        for emitted in vitni_core::event::decide(&event, command, &meta(counter), &refs).expect("decide event") {
            vitni_core::event::evolve(&mut event, &emitted);
            let sequence = i64::try_from(rows.len() + 1).expect("sequence fits i64");
            rows.push(row("event", &event_id.to_string(), sequence, &emitted));
        }
    }

    let mut person = PersonState::default();
    let person_commands = [
        PersonCommand::CreatePerson {
            person_id,
            human_id: HumanId::new(format!("I{:07}", i + 1)),
            evidence_level: EvidenceLevel::Conclusion,
            external_ids: Vec::new(),
        },
        PersonCommand::AssertName {
            person_id,
            name: name(GIVEN[draw.next(GIVEN.len())], SURNAMES[draw.next(SURNAMES.len())]),
        },
        PersonCommand::AssertSex { person_id, sex },
        PersonCommand::AssertParticipation {
            person_id,
            event_id,
            role: ParticipantRole::Primary,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
        },
    ];
    let mut sequence = 0;
    for command in person_commands {
        for emitted in vitni_core::person::decide(&person, command, &meta(counter)).expect("decide person") {
            vitni_core::person::evolve(&mut person, &emitted);
            sequence += 1;
            rows.push(row("person", &person_id.to_string(), sequence, &emitted));
        }
    }
    rows
}

fn country_id() -> PlaceId {
    PlaceId::from_uuid(Uuid::from_u128(COUNTRY))
}

/// The country every birth takes place in.
fn country_rows(counter: &mut u128) -> Vec<RawEvent> {
    let place_id = country_id();
    let commands = [
        PlaceCommand::CreatePlace {
            place_id,
            human_id: HumanId::new("P0000001"),
            place_type: PlaceType::Country,
        },
        PlaceCommand::AssertName {
            place_id,
            name: PlaceName {
                text: "Norge".to_owned(),
                language: None,
                date: None,
            },
        },
    ];
    let refs = PlaceRefs {
        enclosing_exists: true,
        missing_succession_place: None,
    };
    let (mut state, mut rows) = (PlaceState::default(), Vec::new());
    for command in commands {
        for emitted in vitni_core::place::decide(&state, command, &meta(counter), &refs).expect("decide place") {
            vitni_core::place::evolve(&mut state, &emitted);
            let sequence = i64::try_from(rows.len() + 1).expect("sequence fits i64");
            rows.push(row("place", &place_id.to_string(), sequence, &emitted));
        }
    }
    rows
}

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(0xA)),
        display: Some("Bench".to_owned()),
        email: None,
    }
}

/// A seeded workspace of `persons`, its projections rebuilt and its match keys not yet built.
struct Dataset {
    persons: usize,
    workspace: Workspace,
    session: Session,
    _dir: TempDir,
}

async fn seed(persons: usize) -> Dataset {
    // On disk under target/, not in a RAM-backed /tmp: a 100k-person workspace is gigabytes.
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("tempdir");
    let path = dir.path().join("ws");
    Workspace::init(&path, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&path, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open");
    let (mut draw, mut counter) = (Draw(0x5eed), ASSERTION_BASE);
    let store = workspace.store();
    let mut rows = country_rows(&mut counter);
    for start in (0..persons).step_by(SEED_CHUNK) {
        for i in start..persons.min(start + SEED_CHUNK) {
            rows.extend(person_rows(i, &mut draw, &mut counter));
        }
        store
            .insert_raw_events(std::mem::take(&mut rows).into_iter().map(Ok::<_, DbError>))
            .await
            .expect("insert events");
    }
    store.rebuild_projections().await.expect("rebuild projections");
    let session = Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(0xA)),
        display: None,
    });
    Dataset {
        persons,
        workspace,
        session,
        _dir: dir,
    }
}

async fn lookup(dataset: &Dataset) -> usize {
    find_similar(
        &dataset.workspace,
        MatchableKind::Person,
        "I0000001",
        MatchBand::Possible,
        20,
    )
    .await
    .expect("find similar")
    .len()
}

fn bench_index_build(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("match_keys_build");
    group.sample_size(10);
    {
        group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
            b.iter_custom(|iterations| {
                let mut total = Duration::ZERO;
                for _ in 0..iterations {
                    rt.block_on(dataset.workspace.rebuild_projections()).expect("rebuild");
                    let start = Instant::now();
                    rt.block_on(lookup(dataset));
                    total += start.elapsed();
                }
                total
            });
        });
    }
    group.finish();
}

fn bench_find_similar(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("find_similar");
    group.sample_size(10);
    {
        rt.block_on(lookup(dataset));
        group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
            b.iter(|| rt.block_on(lookup(dataset)));
        });
    }
    group.finish();
}

fn bench_edit_then_lookup(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("edit_then_find_similar");
    group.sample_size(10);
    {
        let mut next_year = 1800;
        group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
            b.iter(|| {
                next_year = if next_year == 1800 { 1801 } else { 1800 };
                let date = DateParts {
                    year: next_year,
                    month: None,
                    day: None,
                };
                rt.block_on(assert_event_date(
                    &dataset.workspace,
                    &dataset.session,
                    "E0000002",
                    date,
                    MutationMeta::default(),
                ))
                .expect("edit a birth");
                rt.block_on(lookup(dataset))
            });
        });
    }
    group.finish();
}

/// The Dashboard's first load: the counts, evidence health and recent activity with its persons' names.
async fn dashboard(dataset: &Dataset) -> usize {
    let workspace = &dataset.workspace;
    workspace_counts(workspace).await.expect("counts");
    evidence_health(workspace).await.expect("evidence health");
    let mut persons = Vec::new();
    for entry in recent_activity(workspace, ACTIVITY).await.expect("activity") {
        if entry.aggregate_kind == "person" {
            persons.extend(entry.aggregate_human_id);
        }
    }
    person_names(workspace, &persons).await.expect("names").len()
}

/// The Dashboard's second load: the data-quality checks with the flagged persons' names.
async fn checks(dataset: &Dataset) -> usize {
    let quality = run_checks(&dataset.workspace, SHOWN).await.expect("run checks");
    let mut persons = Vec::new();
    for finding in &quality.findings {
        match finding {
            CheckFinding::DeathBeforeBirth(record) => persons.push(record.human_id.clone()),
            CheckFinding::PossibleDuplicate { a, b, .. } => {
                persons.extend([a.human_id.clone(), b.human_id.clone()]);
            }
        }
    }
    person_names(&dataset.workspace, &persons).await.expect("names");
    quality.duplicates.len()
}

fn bench_dashboard(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("dashboard_load");
    group.sample_size(10);
    group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
        b.iter(|| rt.block_on(dashboard(dataset)));
    });
    group.finish();
}

fn bench_pairs_build(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("match_pairs_build");
    group.sample_size(10);
    {
        group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
            b.iter_custom(|iterations| {
                let mut total = Duration::ZERO;
                for _ in 0..iterations {
                    rt.block_on(dataset.workspace.rebuild_projections()).expect("rebuild");
                    rt.block_on(lookup(dataset));
                    let start = Instant::now();
                    rt.block_on(checks(dataset));
                    total += start.elapsed();
                }
                total
            });
        });
    }
    group.finish();
}

fn bench_checks(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("data_quality_load");
    group.sample_size(10);
    {
        rt.block_on(checks(dataset));
        group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
            b.iter(|| rt.block_on(checks(dataset)));
        });
    }
    group.finish();
}

fn bench_edit_then_checks(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("edit_then_data_quality_load");
    group.sample_size(10);
    {
        let mut next_year = 1800;
        group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
            b.iter(|| {
                next_year = if next_year == 1800 { 1801 } else { 1800 };
                let date = DateParts {
                    year: next_year,
                    month: None,
                    day: None,
                };
                rt.block_on(assert_event_date(
                    &dataset.workspace,
                    &dataset.session,
                    "E0000002",
                    date,
                    MutationMeta::default(),
                ))
                .expect("edit a birth");
                rt.block_on(checks(dataset))
            });
        });
    }
    group.finish();
}

/// An assisted importer's session for `operator`, writing a run of one dataset.
fn importer(operator: &Session) -> Session {
    let run = Arc::new(PendingRun::new(
        operator.clone(),
        NewImportRun {
            plugin: "bench-import".to_owned(),
            plugin_version: "0.1.0".to_owned(),
            dataset: DatasetId::lineage("bench", Uuid::from_u128(0xB)),
            dataset_label: "bench".to_owned(),
            source_label: "bench".to_owned(),
            source_path: None,
            file_asserted_at: None,
            dataset_hint: None,
        },
    ));
    Session::software("bench-import", "0.1.0").with_import_run(run)
}

/// One record as an assisted import submits it: a person born in 1850 in Norway, with the place its
/// own entity, so the plan matches both a person and a place.
fn record() -> RecordGraph {
    let entity = |local_id: u32, item: &str, fields: EntityFields| StagedEntity {
        local_id,
        item: Some(item.to_owned()),
        fields,
    };
    let person = StagedPerson {
        names: vec![PersonNameParts::simple(
            Some("Ole".to_owned()),
            Some("Hansen".to_owned()),
        )],
        sex: Some(Sex::Male),
        ..StagedPerson::default()
    };
    let birth = StagedEvent {
        event_type: EventType::Birth,
        date: Some(gregorian_date(DateParts {
            year: 1850,
            month: None,
            day: None,
        })),
        addresses: Vec::new(),
        restrictions: BTreeSet::default(),
    };
    let place = StagedPlace {
        name: "Norge".to_owned(),
        place_type: None,
        coordinates: None,
        restrictions: BTreeSet::default(),
    };
    RecordGraph {
        record: "bench:1".to_owned(),
        entities: vec![
            entity(0, "person", EntityFields::Person(person)),
            entity(1, "birth", EntityFields::Event(birth)),
            entity(2, "place", EntityFields::Place(place)),
        ],
        links: vec![
            StagedLink {
                item: Some("birth".to_owned()),
                link: LinkKind::Participation {
                    person: EntityRef::Local(0),
                    event: EntityRef::Local(1),
                    role: ParticipantRole::Primary,
                    age: None,
                    attributes: Vec::new(),
                    notes: Vec::new(),
                    citations: Vec::new(),
                },
            },
            StagedLink {
                item: Some("birth-place".to_owned()),
                link: LinkKind::EventPlace {
                    event: EntityRef::Local(1),
                    place: EntityRef::Local(2),
                },
            },
        ],
    }
}

/// The plan of one assisted record, as each submit makes it: its candidates found, nothing written.
async fn plan_record(dataset: &Dataset, session: &Session) -> usize {
    let review = ImportReview::plan(&dataset.workspace, session, vec![record()], None)
        .await
        .expect("plan");
    review.plan_so_far().entities.len()
}

fn bench_plan_record(c: &mut Criterion, rt: &tokio::runtime::Runtime, dataset: &Dataset) {
    let mut group = c.benchmark_group("plan_assisted_record");
    group.sample_size(10);
    {
        let session = importer(&dataset.session);
        rt.block_on(plan_record(dataset, &session));
        group.bench_with_input(BenchmarkId::from_parameter(dataset.persons), dataset, |b, dataset| {
            b.iter(|| rt.block_on(plan_record(dataset, &session)));
        });
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    for persons in SIZES {
        let dataset = rt.block_on(seed(persons));
        bench_index_build(c, &rt, &dataset);
        bench_find_similar(c, &rt, &dataset);
        bench_edit_then_lookup(c, &rt, &dataset);
        bench_plan_record(c, &rt, &dataset);
        bench_dashboard(c, &rt, &dataset);
        if persons <= PAIRS_SAMPLED_UP_TO {
            bench_pairs_build(c, &rt, &dataset);
        }
        bench_checks(c, &rt, &dataset);
        bench_edit_then_checks(c, &rt, &dataset);
    }
}

criterion_group!(similar, benches);
criterion_main!(similar);
