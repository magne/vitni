//! The spatial conformance corpus for [`Store::places_in_bbox`] (ADR 0024 §3), through the public
//! `Store` API, on SQLite and (under `--features postgres`, with a Docker daemon) Postgres: every test
//! body is one `async fn` over a `&Store`, run once per engine, so both engines are held to the same
//! answers — boundary-touching points, shared polygon edges, and points one microdegree outside a
//! query edge, where a float32 R\*Tree or a fuzzy `box` comparison would otherwise let them disagree.

#![cfg(any(feature = "sqlite", feature = "postgres"))]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use time::macros::datetime;
use uuid::Uuid;
use vitni_core::enums::PlaceType;
use vitni_core::geo::{GeoCoordinates, Microdegrees, PlaceGeometry};
use vitni_core::ids::{AgentId, AssertionId, HumanId, PlaceId};
use vitni_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
use vitni_db::Store;

/// A point on the east edge of [`QUERY`].
const ON_EDGE: u128 = 1;
/// A point on the north-west corner of [`QUERY`].
const ON_CORNER: u128 = 2;
/// Points one microdegree outside each of [`QUERY`]'s four edges.
const NORTH_OUT: u128 = 3;
const SOUTH_OUT: u128 = 4;
const EAST_OUT: u128 = 5;
const WEST_OUT: u128 = 6;
/// A polygon whose southern edge lies on [`QUERY`]'s northern edge.
const TOUCHING_POLYGON: u128 = 7;
/// The same polygon moved one microdegree north, clear of [`QUERY`].
const NEAR_POLYGON: u128 = 8;
/// Two polygons sharing the edge at longitude 13°.
const WEST_OF_SHARED: u128 = 9;
const EAST_OF_SHARED: u128 = 10;
/// A point in the southern and western hemispheres.
const SOUTH_WEST: u128 = 11;

/// A query box in microdegrees: `(min_lat, min_lon, max_lat, max_lon)`.
type Bounds = (i32, i32, i32, i32);

/// The viewport the boundary points are placed around: latitude 60°–61°, longitude 10°–11°.
const QUERY: Bounds = (60_000_000, 10_000_000, 61_000_000, 11_000_000);

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-10-15 12:00:00 UTC)),
            rationale: None,
            confidence: None,
            citations: Vec::new(),
            evidence_analysis: None,
            origin: None,
        },
    }
}

fn place_id(n: u128) -> PlaceId {
    PlaceId::from_uuid(Uuid::from_u128(n))
}

fn coordinates(latitude: i32, longitude: i32) -> GeoCoordinates {
    GeoCoordinates {
        latitude: Microdegrees::from_microdegrees(latitude),
        longitude: Microdegrees::from_microdegrees(longitude),
    }
}

/// An axis-aligned rectangular polygon over the given microdegree bounds.
fn rectangle(min_lat: i32, min_lon: i32, max_lat: i32, max_lon: i32) -> PlaceGeometry {
    PlaceGeometry::Polygon {
        exterior: vec![
            coordinates(min_lat, min_lon),
            coordinates(max_lat, min_lon),
            coordinates(max_lat, max_lon),
            coordinates(min_lat, max_lon),
            coordinates(min_lat, min_lon),
        ],
        holes: Vec::new(),
    }
}

/// The assertion id the `n`th place's geometry is asserted under (its creation uses `n * 100`).
fn geometry_assertion(n: u128) -> u128 {
    n * 100 + 1
}

async fn place(store: &Store, n: u128, assertion: u128, command: PlaceCommand) {
    store
        .execute_place(
            &place_id(n).to_string(),
            PlaceCommandEnvelope {
                meta: meta(assertion),
                command,
            },
        )
        .await
        .unwrap();
}

/// Creates place `n` and asserts `geometry` on it — a point through the undated `AssertCoordinates`
/// path, a polygon through `AssertGeometry`, so both ways a geometry reaches the index are covered.
async fn located_place(store: &Store, n: u128, geometry: PlaceGeometry) {
    let command = PlaceCommand::CreatePlace {
        place_id: place_id(n),
        human_id: HumanId::new(format!("P{n:04}")),
        place_type: PlaceType::Parish,
    };
    place(store, n, n * 100, command).await;
    let command = match geometry {
        PlaceGeometry::Point(coordinates) => PlaceCommand::AssertCoordinates {
            place_id: place_id(n),
            coordinates,
        },
        PlaceGeometry::Polygon { .. } => PlaceCommand::AssertGeometry {
            place_id: place_id(n),
            geometry,
            date: None,
        },
    };
    place(store, n, geometry_assertion(n), command).await;
}

async fn seed_corpus(store: &Store) {
    let (min_lat, min_lon, max_lat, max_lon) = QUERY;
    let mid_lat = i32::midpoint(min_lat, max_lat);
    let mid_lon = i32::midpoint(min_lon, max_lon);
    let points = [
        (ON_EDGE, mid_lat, max_lon),
        (ON_CORNER, max_lat, min_lon),
        (NORTH_OUT, max_lat + 1, mid_lon),
        (SOUTH_OUT, min_lat - 1, mid_lon),
        (EAST_OUT, mid_lat, max_lon + 1),
        (WEST_OUT, mid_lat, min_lon - 1),
        (SOUTH_WEST, -34_000_000, -18_500_000),
    ];
    for (n, latitude, longitude) in points {
        located_place(store, n, PlaceGeometry::Point(coordinates(latitude, longitude))).await;
    }
    let polygons = [
        (TOUCHING_POLYGON, rectangle(max_lat, 10_200_000, 61_500_000, 10_800_000)),
        (NEAR_POLYGON, rectangle(max_lat + 1, 10_200_000, 61_500_000, 10_800_000)),
        (
            WEST_OF_SHARED,
            rectangle(60_000_000, 12_000_000, 61_000_000, 13_000_000),
        ),
        (
            EAST_OF_SHARED,
            rectangle(60_000_000, 13_000_000, 61_000_000, 14_000_000),
        ),
    ];
    for (n, geometry) in polygons {
        located_place(store, n, geometry).await;
    }
}

/// [`Store::places_in_bbox`] over microdegree bounds, sorted, so an answer compares as a set.
async fn in_bbox(store: &Store, min_lat: i32, min_lon: i32, max_lat: i32, max_lon: i32) -> Vec<String> {
    let degrees = |value: i32| Microdegrees::from_microdegrees(value).to_degrees();
    let mut hits = store
        .places_in_bbox(degrees(min_lat), degrees(min_lon), degrees(max_lat), degrees(max_lon))
        .await
        .unwrap();
    hits.sort();
    hits
}

fn ids(places: &[u128]) -> Vec<String> {
    let mut ids: Vec<String> = places.iter().map(|&n| place_id(n).to_string()).collect();
    ids.sort();
    ids
}

/// Every `(query, expected places)` pair of the corpus.
fn corpus() -> Vec<(Bounds, Vec<String>)> {
    let (min_lat, min_lon, max_lat, max_lon) = QUERY;
    vec![
        (QUERY, ids(&[ON_EDGE, ON_CORNER, TOUCHING_POLYGON])),
        // A zero-width query on the shared edge touches both polygons; one microdegree either side
        // of it touches only the polygon on that side.
        (
            (60_500_000, 13_000_000, 60_500_000, 13_000_000),
            ids(&[WEST_OF_SHARED, EAST_OF_SHARED]),
        ),
        ((60_500_000, 13_000_001, 60_500_000, 14_000_000), ids(&[EAST_OF_SHARED])),
        ((60_500_000, 12_000_000, 60_500_000, 12_999_999), ids(&[WEST_OF_SHARED])),
        // A point query on a polygon's vertex.
        ((60_000_000, 12_000_000, 60_000_000, 12_000_000), ids(&[WEST_OF_SHARED])),
        // The southern and western hemispheres: a corner hit, then one microdegree short of it.
        ((-34_000_000, -19_000_000, -33_000_000, -18_500_000), ids(&[SOUTH_WEST])),
        ((-33_999_999, -19_000_000, -33_000_000, -18_500_000), ids(&[])),
        // An inverted box: both engines apply the same literal comparison, so it matches only a geometry
        // spanning the whole inverted gap, and none here does.
        ((max_lat, min_lon, min_lat, max_lon), ids(&[])),
        // Far from everything.
        ((10_000_000, 10_000_000, 11_000_000, 11_000_000), ids(&[])),
    ]
}

async fn the_corpus_is_answered(store: &Store) {
    for ((min_lat, min_lon, max_lat, max_lon), expected) in corpus() {
        let hits = in_bbox(store, min_lat, min_lon, max_lat, max_lon).await;
        assert_eq!(hits, expected, "query ({min_lat}, {min_lon}) – ({max_lat}, {max_lon})");
    }
}

async fn boundaries_are_inclusive_and_exact(store: &Store) {
    seed_corpus(store).await;
    the_corpus_is_answered(store).await;
}

async fn a_retracted_geometry_leaves_the_index(store: &Store) {
    seed_corpus(store).await;
    let command = PlaceCommand::RetractAssertion {
        place_id: place_id(TOUCHING_POLYGON),
        target: AssertionId::from_uuid(Uuid::from_u128(geometry_assertion(TOUCHING_POLYGON))),
    };
    place(store, TOUCHING_POLYGON, 9_999, command).await;
    let (min_lat, min_lon, max_lat, max_lon) = QUERY;
    assert_eq!(
        in_bbox(store, min_lat, min_lon, max_lat, max_lon).await,
        ids(&[ON_EDGE, ON_CORNER])
    );
}

async fn a_rebuild_reproduces_the_answers(store: &Store) {
    seed_corpus(store).await;
    store.rebuild_projections().await.unwrap();
    the_corpus_is_answered(store).await;
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use vitni_db::Store;

    async fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        (Store::open(&url).await.unwrap(), dir)
    }

    macro_rules! sqlite_tests {
        ($($name:ident),+ $(,)?) => {
            $(
                #[tokio::test]
                async fn $name() {
                    let (store, _dir) = store().await;
                    super::$name(&store).await;
                }
            )+
        };
    }

    sqlite_tests!(
        boundaries_are_inclusive_and_exact,
        a_retracted_geometry_leaves_the_index,
        a_rebuild_reproduces_the_answers,
    );

    /// A workspace whose geometry index predates the exact bounding-box columns is reindexed on open.
    #[tokio::test]
    async fn an_index_without_exact_bounds_is_rebuilt_on_open() {
        let (store, dir) = store().await;
        super::seed_corpus(&store).await;
        drop(store);
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        for statement in [
            "DROP TABLE place_geometry",
            "DELETE FROM place_geometry_rtree",
            "CREATE TABLE place_geometry (id INTEGER PRIMARY KEY, place_id TEXT NOT NULL, wkb BLOB NOT NULL)",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool.close().await;

        let store = Store::open(&url).await.unwrap();
        super::the_corpus_is_answered(&store).await;
    }
}

#[cfg(feature = "postgres")]
mod postgres {
    use sqlx::migrate::Migrator;
    use test_containers_util::sqlx_pg::PostgresTestDb;
    use vitni_db::Store;

    static MIGRATIONS: Migrator = sqlx::migrate!();

    async fn store() -> (Store, PostgresTestDb) {
        let db = PostgresTestDb::create("vitni-pg", &MIGRATIONS, None, None).await;
        let store = Store::open(db.dsn()).await.unwrap();
        (store, db)
    }

    macro_rules! postgres_tests {
        ($($name:ident),+ $(,)?) => {
            $(
                #[tokio::test]
                async fn $name() {
                    let (store, _db) = store().await;
                    super::$name(&store).await;
                }
            )+
        };
    }

    postgres_tests!(
        boundaries_are_inclusive_and_exact,
        a_retracted_geometry_leaves_the_index,
        a_rebuild_reproduces_the_answers,
    );

    /// The viewport query's `box` expression matches the `GiST` index's, so the planner can use it.
    #[tokio::test]
    async fn the_viewport_query_can_use_the_gist_index() {
        let (store, db) = store().await;
        super::seed_corpus(&store).await;
        let pool = sqlx::PgPool::connect(db.dsn()).await.unwrap();
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query("SET enable_seqscan = off")
            .execute(&mut *connection)
            .await
            .unwrap();
        let plan: Vec<String> = sqlx::query_scalar(
            "EXPLAIN SELECT place_id FROM place_geometry \
             WHERE box(point(min_lon, min_lat), point(max_lon, max_lat)) && box(point(10.0, 60.0), point(11.0, 61.0))",
        )
        .fetch_all(&mut *connection)
        .await
        .unwrap();
        assert!(
            plan.iter().any(|line| line.contains("place_geometry_bbox_idx")),
            "{plan:#?}"
        );
    }
}
