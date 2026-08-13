//! Projection-rebuild and hot-query benchmarks (Phase 11 workstream B).
//!
//! Measures the cost centres ADR 0004 named: event-log **replay / projection rebuild**
//! ([`Store::rebuild_projections`]) at growing log sizes, plus the hot read paths (person
//! list/detail, the `places_in_bbox` R\*Tree query, and the `ResearchNote` reverse-by-subject
//! `json_each` index). A synthetic workspace is built through the *public* [`Store`] command surface
//! — the same pure-`decide` → event-store path the integration tests use — so every measured
//! datum is real replayed log, never hand-written projection rows. Numbers back the snapshotting
//! verdict in `docs/research/performance-profiling.md`.

#![cfg(feature = "sqlite")]
#![expect(clippy::expect_used, reason = "benchmark setup aborts on failure")]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use tempfile::TempDir;
use time::macros::datetime;
use uuid::Uuid;
use vitni_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use vitni_core::enums::{EvidenceLevel, FactType, PlaceType, Sex};
use vitni_core::fact::Fact;
use vitni_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use vitni_core::geo::{GeoCoordinates, Microdegrees};
use vitni_core::id_format::IdFormat;
use vitni_core::ids::{
    AgentId, AssertionId, CitationId, FamilyId, HumanId, PersonId, PlaceId, ResearchNoteId, SourceId,
};
use vitni_core::name::{NameType, PersonName, Surname};
use vitni_core::person::command::{PersonCommand, PersonCommandEnvelope};
use vitni_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use vitni_core::research_note::command::{ResearchNoteCommand, ResearchNoteCommandEnvelope};
use vitni_core::research_note::subject::SubjectRef;
use vitni_core::source::command::{SourceCommand, SourceCommandEnvelope};
use vitni_db::Store;

/// Disjoint UUID `u128` bases per aggregate so ids never collide across kinds or with assertion ids.
const PERSON_BASE: u128 = 0x0000_0001_0000_0000;
const PLACE_BASE: u128 = 0x0000_0002_0000_0000;
const FAMILY_BASE: u128 = 0x0000_0003_0000_0000;
const SOURCE_BASE: u128 = 0x0000_0004_0000_0000;
const CITATION_BASE: u128 = 0x0000_0005_0000_0000;
const RESEARCH_NOTE_BASE: u128 = 0x0000_0006_0000_0000;
const ASSERTION_BASE: u128 = 0x9000_0000_0000_0000;

/// Person counts driving each dataset size; the emitted event totals land near 1k / 10k / 50k and
/// are reported verbatim as the benchmark parameter. The top size is capped so a full `cargo bench`
/// finishes in a few minutes (see the findings doc).
const PERSON_SIZES: [usize; 3] = [180, 1_800, 9_000];

/// A fresh assertion `AssertionMeta` with a unique id, bumping the shared counter. Confidence/operator
/// are fixed — provenance shape is realistic, its content is irrelevant to replay cost.
fn next_meta(counter: &mut u128) -> AssertionMeta {
    *counter += 1;
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(*counter)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-06-18 12:00:00 UTC)),
            rationale: None,
            confidence: Some(Confidence::Normal),
            citations: Vec::new(),
            evidence_analysis: None,
        },
    }
}

/// A birth-name for person `i`, one primary surname drawn from a small rotating pool.
fn person_name(i: usize) -> PersonName {
    const GIVEN: [&str; 8] = ["Ada", "Alan", "Grace", "Edsger", "Barbara", "Donald", "Ole", "Kari"];
    const SURNAME: [&str; 6] = ["Lovelace", "Turing", "Hopper", "Dijkstra", "Nordmann", "Hansen"];
    PersonName {
        name_type: NameType::BirthName,
        given: Some(GIVEN[i % GIVEN.len()].to_owned()),
        surnames: vec![Surname {
            prefix: None,
            surname: SURNAME[i % SURNAME.len()].to_owned(),
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

/// The `HumanId` format [`add_person`] renders ids in — `I%07d`, matching `format!("I{:07}", …)`.
fn person_id_format() -> IdFormat {
    IdFormat::parse("I%07d").expect("valid format")
}

/// The last `human_id` [`add_person`] allocates for a `persons`-sized dataset (`i + 1` maxes at
/// `persons`) — a real point lookup, unlike the hard-coded `I0000001` first-row hit this replaces
/// (`docs/research/performance-profiling.md` §9).
fn last_person_human_id(persons: usize) -> String {
    person_id_format().render(persons as u64)
}

/// Point coordinates for place `i`, spread over a small Norwegian lat/lon grid so a wide viewport
/// bbox overlaps every place (a full-index worst case for [`Store::places_in_bbox`]).
fn place_coordinates(i: usize) -> GeoCoordinates {
    let i = i32::try_from(i).expect("place index fits i32");
    GeoCoordinates {
        latitude: Microdegrees::from_microdegrees(60_000_000 + i * 1_000),
        longitude: Microdegrees::from_microdegrees(9_000_000 + i * 1_000),
    }
}

/// Appends one person (create + name + sex + occupation fact = 4 events). Returns the event count.
async fn add_person(store: &Store, i: usize, counter: &mut u128) -> u64 {
    let person_id = PersonId::from_uuid(Uuid::from_u128(PERSON_BASE + i as u128));
    let commands = [
        PersonCommand::CreatePerson {
            person_id,
            human_id: HumanId::new(format!("I{:07}", i + 1)),
            evidence_level: EvidenceLevel::Conclusion,
        },
        PersonCommand::AssertName {
            person_id,
            name: person_name(i),
        },
        PersonCommand::AssertSex {
            person_id,
            sex: if i.is_multiple_of(2) { Sex::Female } else { Sex::Male },
        },
        PersonCommand::AssertFact {
            person_id,
            fact: Fact {
                fact_type: FactType::Occupation,
                date: None,
                place_id: None,
                value: Some("mathematician".to_owned()),
            },
        },
    ];
    for command in commands {
        store
            .execute_person(
                &person_id.to_string(),
                PersonCommandEnvelope {
                    meta: next_meta(counter),
                    command,
                },
            )
            .await
            .expect("execute person command");
    }
    4
}

/// Appends one place (create + coordinates = 2 events). Returns the event count.
async fn add_place(store: &Store, i: usize, counter: &mut u128) -> u64 {
    let place_id = PlaceId::from_uuid(Uuid::from_u128(PLACE_BASE + i as u128));
    let commands = [
        PlaceCommand::CreatePlace {
            place_id,
            human_id: HumanId::new(format!("P{:07}", i + 1)),
            place_type: PlaceType::Farm,
        },
        PlaceCommand::AssertCoordinates {
            place_id,
            coordinates: place_coordinates(i),
        },
    ];
    for command in commands {
        store
            .execute_place(
                &place_id.to_string(),
                PlaceCommandEnvelope {
                    meta: next_meta(counter),
                    command,
                },
            )
            .await
            .expect("execute place command");
    }
    2
}

/// Appends one source (create = 1 event). Returns the event count.
async fn add_source(store: &Store, i: usize, counter: &mut u128) -> u64 {
    let source_id = SourceId::from_uuid(Uuid::from_u128(SOURCE_BASE + i as u128));
    store
        .execute_source(
            &source_id.to_string(),
            SourceCommandEnvelope {
                meta: next_meta(counter),
                command: SourceCommand::CreateSource {
                    source_id,
                    human_id: HumanId::new(format!("S{:07}", i + 1)),
                },
            },
        )
        .await
        .expect("execute source command");
    1
}

/// Appends one citation into an existing source (create = 1 event). Returns the event count.
async fn add_citation(store: &Store, i: usize, sources: usize, counter: &mut u128) -> u64 {
    let citation_id = CitationId::from_uuid(Uuid::from_u128(CITATION_BASE + i as u128));
    let source_id = SourceId::from_uuid(Uuid::from_u128(SOURCE_BASE + (i % sources) as u128));
    store
        .execute_citation(
            &citation_id.to_string(),
            CitationCommandEnvelope {
                meta: next_meta(counter),
                command: CitationCommand::CreateCitation {
                    citation_id,
                    human_id: HumanId::new(format!("C{:07}", i + 1)),
                    source_id,
                },
            },
        )
        .await
        .expect("execute citation command");
    1
}

/// Appends one family with two partners and two children drawn from existing persons
/// (create + 2 partners + 2 children = 5 events). Returns the event count.
async fn add_family(store: &Store, i: usize, persons: usize, counter: &mut u128) -> u64 {
    let family_id = FamilyId::from_uuid(Uuid::from_u128(FAMILY_BASE + i as u128));
    let member =
        |offset: usize| PersonId::from_uuid(Uuid::from_u128(PERSON_BASE + ((i * 4 + offset) % persons) as u128));
    let commands = [
        FamilyCommand::CreateFamily {
            family_id,
            human_id: HumanId::new(format!("F{:07}", i + 1)),
        },
        FamilyCommand::AddPartner {
            family_id,
            person_id: member(0),
        },
        FamilyCommand::AddPartner {
            family_id,
            person_id: member(1),
        },
        FamilyCommand::AddChild {
            family_id,
            child_id: member(2),
        },
        FamilyCommand::AddChild {
            family_id,
            child_id: member(3),
        },
    ];
    for command in commands {
        store
            .execute_family(
                &family_id.to_string(),
                FamilyCommandEnvelope {
                    meta: next_meta(counter),
                    command,
                },
            )
            .await
            .expect("execute family command");
    }
    5
}

/// Appends one research note naming two existing persons as subjects (create = 1 event). Returns the
/// event count. Note `i` names persons `i` and `i+1`, so person 0 is a subject of exactly note 0 —
/// the fixed target of the reverse-index query bench.
async fn add_research_note(store: &Store, i: usize, persons: usize, counter: &mut u128) -> u64 {
    let research_note_id = ResearchNoteId::from_uuid(Uuid::from_u128(RESEARCH_NOTE_BASE + i as u128));
    let subject = |offset: usize| {
        SubjectRef::Person(PersonId::from_uuid(Uuid::from_u128(
            PERSON_BASE + ((i + offset) % persons) as u128,
        )))
    };
    store
        .execute_research_note(
            &research_note_id.to_string(),
            ResearchNoteCommandEnvelope {
                meta: next_meta(counter),
                command: ResearchNoteCommand::CreateResearchNote {
                    research_note_id,
                    human_id: HumanId::new(format!("A{:07}", i + 1)),
                    subjects: [subject(0), subject(1)].into_iter().collect(),
                    title: Some("Same person as the census entry?".to_owned()),
                },
            },
        )
        .await
        .expect("execute research note command");
    1
}

/// Builds a proportioned synthetic workspace from `persons`, appending across six aggregates through
/// the public command surface. Persons and sources are created before the families, citations, and
/// research notes that reference them, so no cross-aggregate resolver check rejects. Returns the
/// total number of events appended to the log.
async fn build_dataset(store: &Store, persons: usize) -> u64 {
    let families = persons / 4;
    let places = (persons / 8).max(1);
    let sources = (persons / 20).max(1);
    let citations = sources;
    let research_notes = persons / 4;

    let mut counter = ASSERTION_BASE;
    let mut events: u64 = 0;
    for i in 0..persons {
        events += add_person(store, i, &mut counter).await;
    }
    for i in 0..places {
        events += add_place(store, i, &mut counter).await;
    }
    for i in 0..sources {
        events += add_source(store, i, &mut counter).await;
    }
    for i in 0..citations {
        events += add_citation(store, i, sources, &mut counter).await;
    }
    for i in 0..families {
        events += add_family(store, i, persons, &mut counter).await;
    }
    for i in 0..research_notes {
        events += add_research_note(store, i, persons, &mut counter).await;
    }
    events
}

/// Opens a fresh on-disk SQLite store in a temp dir (the log must be persisted for a rebuild to
/// replay it). The [`TempDir`] is returned so the caller keeps the database alive.
async fn open_store() -> (Store, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = format!("sqlite://{}", dir.path().join("bench.sqlite3").display());
    (Store::open(&url).await.expect("open store"), dir)
}

/// One benchmark dataset: the store and the sizes needed to derive bench inputs (a real point
/// lookup key, throughput). `_dir` just keeps the backing temp file alive.
struct Dataset {
    persons: usize,
    events: u64,
    store: Store,
    _dir: TempDir,
}

/// The headline replay benchmark: `rebuild_projections` clears and re-derives every projection from
/// the whole event log. Throughput is set in events so criterion reports per-event replay cost.
fn bench_rebuild(c: &mut Criterion, rt: &tokio::runtime::Runtime, datasets: &[Dataset]) {
    let mut group = c.benchmark_group("rebuild");
    group.sample_size(10);
    for dataset in datasets {
        group.throughput(Throughput::Elements(dataset.events));
        group.bench_with_input(BenchmarkId::from_parameter(dataset.events), &dataset.events, |b, _| {
            b.iter(|| {
                rt.block_on(dataset.store.rebuild_projections())
                    .expect("rebuild projections");
            });
        });
    }
    group.finish();
}

/// The hot read paths, each at every dataset size: full person list, single person detail, the
/// next-`human_id` allocation, the spatial bbox query, and the research-note reverse-by-subject
/// lookup.
fn bench_queries(c: &mut Criterion, rt: &tokio::runtime::Runtime, datasets: &[Dataset]) {
    let subject = SubjectRef::Person(PersonId::from_uuid(Uuid::from_u128(PERSON_BASE)));
    let format = person_id_format();
    let mut group = c.benchmark_group("query");
    for dataset in datasets {
        let events = &dataset.events;
        let store = &dataset.store;
        group.bench_with_input(BenchmarkId::new("list_persons", events), events, |b, _| {
            b.iter(|| rt.block_on(store.list_persons()).expect("list persons"));
        });
        // The last allocated id, not the first — `fetch_optional` stops at the first row hit, so
        // looking up `I0000001` measured a first-row scan, not an indexed point lookup.
        let last_id = last_person_human_id(dataset.persons);
        group.bench_with_input(BenchmarkId::new("find_person", events), events, |b, _| {
            b.iter(|| rt.block_on(store.find_person(&last_id)).expect("find person"));
        });
        group.bench_with_input(BenchmarkId::new("next_person_human_id", events), events, |b, _| {
            b.iter(|| {
                rt.block_on(store.next_person_human_id(&format))
                    .expect("next person human id")
            });
        });
        group.bench_with_input(BenchmarkId::new("places_in_bbox", events), events, |b, _| {
            b.iter(|| {
                rt.block_on(store.places_in_bbox(59.0, 8.0, 62.0, 11.0))
                    .expect("bbox query")
            });
        });
        group.bench_with_input(
            BenchmarkId::new("research_notes_for_subject", events),
            events,
            |b, _| {
                b.iter(|| {
                    rt.block_on(store.list_research_notes_for_subject(subject))
                        .expect("research notes for subject");
                });
            },
        );
    }
    group.finish();
}

/// Builds each dataset once (kept alive for both benchmark groups), then runs the rebuild and query
/// benchmarks against the shared stores.
fn benches(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut datasets: Vec<Dataset> = Vec::with_capacity(PERSON_SIZES.len());
    for persons in PERSON_SIZES {
        let (store, dir) = rt.block_on(open_store());
        let events = rt.block_on(build_dataset(&store, persons));
        datasets.push(Dataset {
            persons,
            events,
            store,
            _dir: dir,
        });
    }
    bench_rebuild(c, &rt, &datasets);
    bench_queries(c, &rt, &datasets);
}

criterion_group!(store_benches, benches);
criterion_main!(store_benches);
