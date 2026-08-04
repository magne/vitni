//! Integration tests for the Postgres backend of the engine-neutral [`Store`] (ADR 0002).
//!
//! These exercise the real Postgres wiring against a containerized server: `test-containers-util`
//! reuses one container per process (`genealogy-pg`) and gives each test a fresh, randomly-named
//! database (dropped when the `PostgresTestDb` guard falls out of scope), so tests stay isolated
//! while sharing one container. A running Docker daemon is required; the tests compile only under
//! `--features postgres`. Most tests use only the public `Store` surface — no `sqlx`/`postgres-es`/
//! `cqrs-es` types — proving the abstraction holds for Postgres exactly as for SQLite. The
//! `human_id` schema-shape tests at the bottom (ADR 0032) are the deliberate exception: proving a
//! generated column, its indexes, and the drop-and-replay migration exist on disk has no
//! `Store`-level surface to check, so those alone open a raw `sqlx::PgPool` against the same
//! database the `Store` under test uses.

#![cfg(feature = "postgres")]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use genealogy_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use genealogy_core::enums::{EvidenceLevel, PlaceType, SuccessionKind};
use genealogy_core::id_format::IdFormat;
use genealogy_core::ids::{AgentId, AssertionId, CitationId, HumanId, PersonId, PlaceId, SourceId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use genealogy_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use genealogy_db::{CommandError, PlaceSuccessionRecord, Store};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};
use test_containers_util::sqlx_pg::PostgresTestDb;
use time::macros::datetime;
use uuid::Uuid;

/// The shared container name; one container is started per test process and reused across tests.
const CONTAINER: &str = "genealogy-pg";

/// An empty migrator — our schema is created by `Store::open`, not by sqlx migrations, but the test
/// helper requires one.
static MIGRATIONS: Migrator = sqlx::migrate!();

/// Opens a `Store` over a fresh, isolated Postgres database. The returned guard must be kept alive
/// for the database's lifetime; it is dropped (and the database deleted) at the end of the test.
async fn store() -> (Store, PostgresTestDb) {
    let db = PostgresTestDb::create(CONTAINER, &MIGRATIONS, None, None).await;
    let store = Store::open(db.dsn()).await.unwrap();
    (store, db)
}

fn person_format() -> IdFormat {
    IdFormat::parse("I%04d").unwrap()
}

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
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

async fn create(store: &Store, n: u128, human_id: &str) {
    let person_id = PersonId::from_uuid(Uuid::from_u128(n));
    store
        .execute_person(
            &person_id.to_string(),
            PersonCommandEnvelope {
                meta: meta(n * 10),
                command: PersonCommand::CreatePerson {
                    person_id,
                    human_id: HumanId::new(human_id),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            },
        )
        .await
        .unwrap();
}

async fn name(store: &Store, n: u128, given: &str, surname: &str) {
    let person_id = PersonId::from_uuid(Uuid::from_u128(n));
    let name = PersonName {
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
    };
    store
        .execute_person(
            &person_id.to_string(),
            PersonCommandEnvelope {
                meta: meta(n * 10 + 1),
                command: PersonCommand::AssertName { person_id, name },
            },
        )
        .await
        .unwrap();
}

/// Creates place `n` with `human_id`, so a succession assertion has real endpoints to name.
async fn create_place(store: &Store, n: u128, human_id: &str) {
    let place_id = PlaceId::from_uuid(Uuid::from_u128(n));
    store
        .execute_place(
            &place_id.to_string(),
            PlaceCommandEnvelope {
                meta: meta(n * 10 + 2),
                command: PlaceCommand::CreatePlace {
                    place_id,
                    human_id: HumanId::new(human_id),
                    place_type: PlaceType::Municipality,
                },
            },
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn allocates_sequential_ids_and_survives_width_growth() {
    let (store, _db) = store().await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0001");

    create(&store, 1, "I0001").await;
    create(&store, 2, "I0002").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0003");

    // Past the zero-pad width, numbering must keep counting numerically, not lexicographically.
    create(&store, 9999, "I9999").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I10000");
}

#[tokio::test(flavor = "multi_thread")]
async fn next_human_id_takes_the_numeric_max_across_mixed_widths_and_ignores_junk() {
    let (store, _db) = store().await;
    // `I00000003` is both longer and numerically smaller than `I10001`; a naive length-descending
    // then lexical-descending scan would stop at the first (longest) group and hand back a number
    // that is not the true max. `ZZZZ` matches the format at no position at all, so it must be
    // skipped entirely, not just fail to parse a number from it.
    create(&store, 1, "I0001").await;
    create(&store, 2, "I10001").await;
    create(&store, 3, "I00000003").await;
    create(&store, 4, "ZZZZ").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I10002");
}

#[tokio::test(flavor = "multi_thread")]
async fn next_human_id_pages_past_a_run_of_unparseable_ids_longer_than_one_page() {
    let (store, _db) = store().await;
    // Forty ids of the same length as the real one, lexically greater (so a descending scan
    // visits them first) but not matching the `I%04d` format — more than the allocator's 32-row
    // page, so finding the real max requires paging past an exhausted first page.
    for n in 1..=40u128 {
        create(&store, n, &format!("Z{n:04}")).await;
    }
    create(&store, 41, "I0005").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0006");
}

#[tokio::test(flavor = "multi_thread")]
async fn next_human_id_is_the_first_id_when_every_stored_id_is_junk_or_absent() {
    let (store, _db) = store().await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0001");

    create(&store, 1, "not-an-id").await;
    create(&store, 2, "also-not-one").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0001");
}

#[tokio::test(flavor = "multi_thread")]
async fn find_and_list_reflect_the_projection() {
    let (store, _db) = store().await;
    create(&store, 2, "I0002").await;
    name(&store, 2, "Alan", "Turing").await;
    create(&store, 1, "I0001").await;
    name(&store, 1, "Ada", "Lovelace").await;

    let found = store.find_person("I0001").await.unwrap().expect("exists");
    assert_eq!(found.human_id().map(HumanId::as_str), Some("I0001"));
    assert_eq!(found.names()[0].given.as_deref(), Some("Ada"));

    assert!(store.find_person("I0404").await.unwrap().is_none());

    let ids = person_ids(&store).await;
    assert_eq!(ids, ["I0001", "I0002"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_domain_rejection_is_distinct_from_an_infrastructure_error() {
    let (store, _db) = store().await;
    create(&store, 1, "I0001").await;

    let person_id = PersonId::from_uuid(Uuid::from_u128(1));
    let err = store
        .execute_person(
            &person_id.to_string(),
            PersonCommandEnvelope {
                meta: meta(99),
                command: PersonCommand::CreatePerson {
                    person_id,
                    human_id: HumanId::new("I0001"),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            },
        )
        .await;
    assert!(
        matches!(err, Err(CommandError::Rejected(_))),
        "re-create is a domain rejection"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_citation_against_a_missing_source_is_rejected() {
    // Exercises the Postgres cross-aggregate resolver (the §9 aggregate tax): the cited source does
    // not exist in the read model, so the pure decide rejects the citation.
    let (store, _db) = store().await;
    let citation_id = CitationId::from_uuid(Uuid::from_u128(1));
    let err = store
        .execute_citation(
            &citation_id.to_string(),
            CitationCommandEnvelope {
                meta: meta(1000),
                command: CitationCommand::CreateCitation {
                    citation_id,
                    human_id: HumanId::new("C0001"),
                    source_id: SourceId::from_uuid(Uuid::from_u128(0xDEAD)),
                },
            },
        )
        .await;
    assert!(
        matches!(err, Err(CommandError::Rejected(_))),
        "a citation against a missing source is a domain rejection"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rebuild_reproduces_the_projection_from_the_log() {
    let (store, _db) = store().await;
    create(&store, 1, "I0001").await;
    name(&store, 1, "Ada", "Lovelace").await;
    create(&store, 2, "I0002").await;
    name(&store, 2, "Alan", "Turing").await;

    let before = person_ids(&store).await;
    assert_eq!(before, ["I0001", "I0002"]);

    // Drop and replay every projection from the (untouched) event log.
    store.rebuild_projections().await.unwrap();

    let after = person_ids(&store).await;
    assert_eq!(after, before, "rebuild reproduces the projection identically");
    let found = store.find_person("I0001").await.unwrap().expect("exists after rebuild");
    assert_eq!(found.names()[0].given.as_deref(), Some("Ada"));
}

/// The three places every succession test names: Aker, Kristiania and Oslo, already created.
async fn three_places(store: &Store) -> (PlaceId, PlaceId, PlaceId) {
    for (n, human_id) in [(1, "P0001"), (2, "P0002"), (3, "P0003")] {
        create_place(store, n, human_id).await;
    }
    (
        PlaceId::from_uuid(Uuid::from_u128(1)),
        PlaceId::from_uuid(Uuid::from_u128(2)),
        PlaceId::from_uuid(Uuid::from_u128(3)),
    )
}

/// Dispatches one Place command against `place_id`'s own stream under assertion id `assertion`.
async fn place_command(store: &Store, assertion: u128, place_id: PlaceId, command: PlaceCommand) {
    store
        .execute_place(
            &place_id.to_string(),
            PlaceCommandEnvelope {
                meta: meta(assertion),
                command,
            },
        )
        .await
        .unwrap();
}

/// An `AssertSuccession` anchored on `from[0]` — the place whose stream records it (ADR 0026 §3).
fn succession(from: &[PlaceId], to: &[PlaceId], kind: SuccessionKind, date: Option<GenealogicalDate>) -> PlaceCommand {
    PlaceCommand::AssertSuccession {
        place_id: from[0],
        from: from.to_vec(),
        to: to.to_vec(),
        kind,
        date,
    }
}

/// A minimal exact-year `GenealogicalDate`, with `sort_value` set directly (data-model §7.1).
fn year(value: i32) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(value),
            month: None,
            day: None,
        })),
        time: None,
        new_year_begins: None,
        sort_value: i64::from(value),
        original_text: None,
    }
}

/// The `assertion_id` string [`meta`] mints for `assertion` — what a succession row must carry.
fn assertion_id(assertion: u128) -> String {
    AssertionId::from_uuid(Uuid::from_u128(assertion)).to_string()
}

/// The counterpart place ids of a succession read, sorted so the assertion is order-independent.
fn counterparts(records: &[PlaceSuccessionRecord]) -> Vec<String> {
    let mut ids: Vec<String> = records.iter().map(|r| r.place_id.clone()).collect();
    ids.sort();
    ids
}

#[tokio::test(flavor = "multi_thread")]
async fn place_detail_reads_are_supported_on_postgres() {
    // #231 at the db layer: on Postgres both succession reads used to return `Unsupported`, so every
    // place-detail read failed outright. With no succession asserted they must answer "none", not
    // "unavailable".
    let (store, _db) = store().await;
    create_place(&store, 1, "P0001").await;

    let place_id = PlaceId::from_uuid(Uuid::from_u128(1)).to_string();
    assert_eq!(store.place_successors(&place_id).await.unwrap(), vec![]);
    assert_eq!(store.place_predecessors(&place_id).await.unwrap(), vec![]);
}

#[tokio::test(flavor = "multi_thread")]
async fn place_succession_index_is_symmetric_on_postgres() {
    let (store, _db) = store().await;
    let (aker, kristiania, oslo) = three_places(&store).await;

    // Aker + Kristiania merged into Oslo (1948) — recorded once, on Aker's stream, naming both
    // endpoint lists (ADR 0026 §3).
    place_command(
        &store,
        500,
        aker,
        succession(&[aker, kristiania], &[oslo], SuccessionKind::Merged, Some(year(1948))),
    )
    .await;

    let predecessors = store.place_predecessors(&oslo.to_string()).await.unwrap();
    let mut expected = vec![aker.to_string(), kristiania.to_string()];
    expected.sort();
    assert_eq!(counterparts(&predecessors), expected);
    assert_eq!(predecessors[0].kind, "\"Merged\"", "kind is JSON-serialized");
    assert_eq!(predecessors[0].assertion_id, assertion_id(500));
    let date_json = predecessors[0].date_json.as_ref().expect("the dated succession");
    let date: GenealogicalDate = serde_json::from_str(date_json).unwrap();
    assert_eq!(date.sort_value, 1948, "date_json round-trips");

    assert_eq!(
        counterparts(&store.place_successors(&aker.to_string()).await.unwrap()),
        vec![oslo.to_string()]
    );
    // The navigation the index exists for: Kristiania is a `from` endpoint but not the anchor, so its
    // own projection cannot answer this.
    assert_eq!(
        counterparts(&store.place_successors(&kristiania.to_string()).await.unwrap()),
        vec![oslo.to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn each_anchors_succession_rows_are_reindexed_independently() {
    // Two anchors each assert into Oslo; a later assertion on Aker's stream reindexes *Aker* only, so
    // Kristiania's row must survive untouched and Aker's must not double.
    let (store, _db) = store().await;
    let (aker, kristiania, oslo) = three_places(&store).await;

    place_command(
        &store,
        500,
        aker,
        succession(&[aker], &[oslo], SuccessionKind::Absorbed, None),
    )
    .await;
    place_command(
        &store,
        501,
        kristiania,
        succession(&[kristiania], &[oslo], SuccessionKind::Absorbed, None),
    )
    .await;
    place_command(
        &store,
        502,
        aker,
        succession(&[aker], &[oslo], SuccessionKind::Merged, Some(year(1948))),
    )
    .await;

    let predecessors = store.place_predecessors(&oslo.to_string()).await.unwrap();
    let mut expected = vec![aker.to_string(), aker.to_string(), kristiania.to_string()];
    expected.sort();
    assert_eq!(
        counterparts(&predecessors),
        expected,
        "Aker's two assertions plus Kristiania's one — no duplicates from the reindex"
    );
    assert_eq!(
        counterparts(&store.place_successors(&kristiania.to_string()).await.unwrap()),
        vec![oslo.to_string()],
        "Kristiania's row survives Aker's reindex"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn retracting_a_succession_clears_the_postgres_index() {
    let (store, _db) = store().await;
    let (aker, kristiania, oslo) = three_places(&store).await;
    place_command(
        &store,
        500,
        aker,
        succession(&[aker, kristiania], &[oslo], SuccessionKind::Merged, Some(year(1948))),
    )
    .await;

    place_command(
        &store,
        501,
        aker,
        PlaceCommand::RetractAssertion {
            place_id: aker,
            target: AssertionId::from_uuid(Uuid::from_u128(500)),
        },
    )
    .await;

    // The assertion left the anchor's projection, so the reindex must drop every row it produced —
    // including the links reachable from the two non-anchor endpoints.
    for place_id in [aker, kristiania, oslo] {
        let id = place_id.to_string();
        assert_eq!(store.place_successors(&id).await.unwrap(), vec![]);
        assert_eq!(store.place_predecessors(&id).await.unwrap(), vec![]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn superseding_a_succession_replaces_the_postgres_index_rows() {
    // `SupersedeAssertion` emits the retraction and the replacement in one dispatch batch, so the
    // reindex sees both at once — the delete-and-reinsert case.
    let (store, _db) = store().await;
    let (aker, kristiania, oslo) = three_places(&store).await;
    place_command(
        &store,
        500,
        aker,
        succession(&[aker, kristiania], &[oslo], SuccessionKind::Merged, Some(year(1948))),
    )
    .await;

    place_command(
        &store,
        501,
        aker,
        PlaceCommand::SupersedeAssertion {
            place_id: aker,
            target: AssertionId::from_uuid(Uuid::from_u128(500)),
            replacement: Box::new(succession(&[aker], &[oslo], SuccessionKind::Absorbed, Some(year(1950)))),
        },
    )
    .await;

    let successors = store.place_successors(&aker.to_string()).await.unwrap();
    assert_eq!(successors.len(), 1, "the superseded row is gone, the replacement is in");
    assert_eq!(successors[0].place_id, oslo.to_string());
    assert_eq!(successors[0].kind, "\"Absorbed\"");
    assert_eq!(successors[0].assertion_id, assertion_id(501));
    assert_eq!(
        store.place_successors(&kristiania.to_string()).await.unwrap(),
        vec![],
        "Kristiania was only an endpoint of the superseded assertion"
    );
}

/// Both succession reads for each of `places`, each list sorted, for a content-based rebuild
/// comparison. Not id-based on purpose: the metadata table's identity sequence keeps climbing across
/// a rebuild, and the rebuild inserts in `human_id` order rather than command order, so row ids and
/// cross-anchor ordering legitimately differ.
async fn succession_dump(
    store: &Store,
    places: &[PlaceId],
) -> Vec<(String, Vec<PlaceSuccessionRecord>, Vec<PlaceSuccessionRecord>)> {
    let mut dump = Vec::new();
    for place_id in places {
        let id = place_id.to_string();
        let mut successors = store.place_successors(&id).await.unwrap();
        let mut predecessors = store.place_predecessors(&id).await.unwrap();
        successors.sort_by(|a, b| (&a.place_id, &a.assertion_id).cmp(&(&b.place_id, &b.assertion_id)));
        predecessors.sort_by(|a, b| (&a.place_id, &a.assertion_id).cmp(&(&b.place_id, &b.assertion_id)));
        dump.push((id, successors, predecessors));
    }
    dump
}

#[tokio::test(flavor = "multi_thread")]
async fn rebuild_reproduces_the_postgres_succession_index() {
    let (store, _db) = store().await;
    let (aker, kristiania, oslo) = three_places(&store).await;
    place_command(
        &store,
        500,
        aker,
        succession(&[aker, kristiania], &[oslo], SuccessionKind::Merged, Some(year(1948))),
    )
    .await;

    let before = succession_dump(&store, &[aker, kristiania, oslo]).await;
    assert!(
        before.iter().any(|(_, s, p)| !s.is_empty() || !p.is_empty()),
        "the comparison is only meaningful over a non-empty index"
    );

    store.rebuild_projections().await.unwrap();

    let after = succession_dump(&store, &[aker, kristiania, oslo]).await;
    assert_eq!(before, after, "rebuild must reproduce the succession index");
}

/// The ordered `human_id`s of every person projection.
async fn person_ids(store: &Store) -> Vec<String> {
    store
        .list_persons()
        .await
        .unwrap()
        .iter()
        .filter_map(|v| v.human_id().map(|h| h.as_str().to_owned()))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_human_id_column_is_generated_and_indexed() {
    let (store, db) = store().await;
    create(&store, 1, "I0001").await;

    let pool = PgPool::connect(db.dsn()).await.unwrap();
    let is_generated: String = sqlx::query(
        "SELECT is_generated FROM information_schema.columns \
         WHERE table_name = 'person_view' AND column_name = 'human_id'",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .get("is_generated");
    assert_eq!(is_generated, "ALWAYS", "human_id must be a generated column (ADR 0032)");

    let indexdefs: Vec<String> = sqlx::query("SELECT indexdef FROM pg_indexes WHERE tablename = 'person_view'")
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|row| row.get("indexdef"))
        .collect();
    assert!(
        indexdefs.iter().any(|def| def.contains("person_view_human_id_idx")),
        "expected the equality index, got {indexdefs:?}"
    );
    assert!(
        indexdefs.iter().any(|def| def.contains("person_view_human_id_len_idx")),
        "expected the length index, got {indexdefs:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_legacy_view_table_is_dropped_and_rebuilt_from_the_log_on_reopen() {
    let (store, db) = store().await;
    create(&store, 1, "I0001").await;
    drop(store);

    // Downgrade `person_view` to the pre-ADR-0032, three-column shape — what a workspace created
    // before the `human_id` column existed would still have on disk. The event log (untouched
    // here) is what the reopen below must replay to repopulate it.
    let pool = PgPool::connect(db.dsn()).await.unwrap();
    sqlx::query("DROP TABLE person_view").execute(&pool).await.unwrap();
    sqlx::query(
        "CREATE TABLE person_view (\
         view_id TEXT NOT NULL PRIMARY KEY, version BIGINT NOT NULL, payload JSON NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    // Reopening detects the stale shape, drops and recreates the table, and replays the (intact)
    // event log to repopulate it.
    let store = Store::open(db.dsn()).await.unwrap();
    let found = store.find_person("I0001").await.unwrap().expect("rebuilt from the log");
    assert_eq!(found.human_id().map(HumanId::as_str), Some("I0001"));

    let pool = PgPool::connect(db.dsn()).await.unwrap();
    let is_generated: String = sqlx::query(
        "SELECT is_generated FROM information_schema.columns \
         WHERE table_name = 'person_view' AND column_name = 'human_id'",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .get("is_generated");
    assert_eq!(is_generated, "ALWAYS");
    pool.close().await;

    // Reopening again is idempotent: the table is already current, so no further drop/rebuild
    // happens, and the row is neither lost nor duplicated.
    drop(store);
    let store = Store::open(db.dsn()).await.unwrap();
    assert_eq!(
        person_ids(&store).await,
        ["I0001"],
        "a second reopen must not duplicate or drop the row"
    );
}
