# Event Sourcing in Rust — from scratch, the DCB model, and cqrs-es vs. disintegrate

> APIs below were read directly from the published crate sources: **cqrs-es 0.5.0** and
> **disintegrate 4.0.0**. They're accurate to those versions but not compile-checked here —
> run `cargo check` before trusting verbatim. Both 0.5 and 4.0 changed their public APIs from
> the previous major versions, so older tutorials will be wrong.

---

## 1. A minimal Postgres event store (no framework)

Both frameworks below are just opinionated layers over essentially this. Understanding it makes
the rest obvious.

### Schema

```sql
create table events (
    id           bigserial   primary key,            -- global order; projection cursor
    stream_id    uuid        not null,               -- the entity instance
    stream_type  text        not null,               -- e.g. 'account'
    version      bigint      not null,               -- per-stream sequence, 1-based
    event_type   text        not null,               -- e.g. 'Deposited'
    payload      jsonb       not null,
    metadata     jsonb       not null default '{}',
    occurred_at  timestamptz not null default now(),
    unique (stream_id, version)                       -- the entire concurrency story
);
create index on events (stream_id, version);
```

The `unique (stream_id, version)` constraint **is** optimistic concurrency control: a writer
computes the next version from the events it just read; if a concurrent writer got there first,
the insert raises a unique violation and the command retries from a fresh read.

### Append + load (sqlx)

```rust
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AccountEvent {
    Opened { owner: String },
    Deposited { amount: i64 },
    Withdrawn { amount: i64 },
}

pub struct Persisted { pub version: i64, pub event: AccountEvent }

#[derive(thiserror::Error, Debug)]
pub enum AppendError {
    #[error("optimistic concurrency conflict")] Conflict,
    #[error(transparent)] Db(#[from] sqlx::Error),
}

pub async fn load(pool: &PgPool, stream_id: Uuid) -> sqlx::Result<Vec<Persisted>> {
    let rows = sqlx::query!(
        "select version, payload from events where stream_id = $1 order by version",
        stream_id
    ).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|r| Persisted {
        version: r.version,
        event: serde_json::from_value(r.payload).expect("event decode"),
    }).collect())
}

/// `expected_version` is the highest version the caller saw when it built state.
/// New events are written at expected_version + 1, +2, ...
pub async fn append(
    pool: &PgPool,
    stream_id: Uuid,
    stream_type: &str,
    expected_version: i64,
    new_events: &[AccountEvent],
) -> Result<(), AppendError> {
    let mut tx = pool.begin().await?;
    let mut version = expected_version;
    for ev in new_events {
        version += 1;
        let event_type = match ev {
            AccountEvent::Opened { .. }    => "Opened",
            AccountEvent::Deposited { .. } => "Deposited",
            AccountEvent::Withdrawn { .. } => "Withdrawn",
        };
        let res = sqlx::query!(
            "insert into events (stream_id, stream_type, version, event_type, payload)
             values ($1, $2, $3, $4, $5)",
            stream_id, stream_type, version, event_type, serde_json::to_value(ev).unwrap(),
        ).execute(&mut *tx).await;

        if let Err(sqlx::Error::Database(db)) = &res {
            // Postgres names the unique index events_stream_id_version_key by default
            if db.constraint() == Some("events_stream_id_version_key") {
                return Err(AppendError::Conflict);
            }
        }
        res?;
    }
    tx.commit().await?;
    Ok(())
}
```

### The command loop: load → fold → decide → append

```rust
#[derive(Default)]
struct AccountState { open: bool, balance: i64 }

fn fold(events: &[Persisted]) -> AccountState {
    let mut s = AccountState::default();
    for Persisted { event, .. } in events {
        match event {
            AccountEvent::Opened { .. }       => s.open = true,
            AccountEvent::Deposited { amount } => s.balance += *amount,
            AccountEvent::Withdrawn { amount } => s.balance -= *amount,
        }
    }
    s
}

// decide() takes current state + a command and returns new events or a domain error.
// Then: append(pool, id, "account", events.last_version, &new_events) — retry on Conflict.
```

### Projections (the read side)

A separate worker tails `events` ordered by the global `id`, keeps a stored cursor
(`last_processed_id`), and upserts read-model tables. Drive it by polling, or use Postgres
`LISTEN/NOTIFY` to wake on new inserts. Read models are disposable — delete the cursor and
replay from `id = 0` to rebuild.

That's a complete event-sourced system. Everything below is ergonomics and concurrency
semantics layered on top.

---

## 2. The Dynamic Consistency Boundary (DCB) model

**Classic aggregates (DDD).** You decide *at design time* on a fixed cluster of data — the
aggregate — that forms one transactional consistency boundary. Every command loads the whole
aggregate, checks invariants within it, and the aggregate's own event stream is the unit of
optimistic concurrency (the `(stream_id, version)` guard from Part 1).

The trouble: real invariants don't always respect the boundary you drew. "Subscribe a student
to a course" touches the **course** (has it hit capacity?) and the **student** (are they over
their course limit?). With aggregates you're pushed toward one of:

- a giant aggregate that swallows both (kills concurrency, unnatural model), or
- two aggregates + a **saga/process manager** reaching eventual consistency with compensations, or
- checking the second invariant against a read model that may lag (a correctness hole).

**DCB inverts this.** There is no fixed aggregate. For each *decision* you declare a **query over
the event log** selecting exactly the events relevant to the invariant you're enforcing —
possibly spanning several entities. The set of events that query matches **is** the consistency
boundary, and it is computed *dynamically, per decision*. Concurrency control becomes: *append my
new events only if no event matching my query has appeared since I read.* The guard is on a
**query**, not on a single stream.

So the name is literal:
- **Dynamic** — derived per decision from a query, not fixed up front.
- **Consistency boundary** — the matched events are what must not change between read and write
  for the decision to stay valid.

Mechanically DCB needs two things the naive store in Part 1 lacks:
1. Events **tagged with identifiers and types** so queries can select across entities.
2. An event store supporting **conditional append against a query** (append iff no matching event
   exists after position *X*).

`disintegrate` implements exactly this. `#[id]` tags + `#[stream(...)]` groupings build the
queryable index; `state_query()` declares which events to fold into the decision state; and the
optional `validation_query()` narrows *which* of those events actually invalidate the decision.
That last knob is the subtle bit: in a withdrawal you must fold deposits to know the balance, but
a concurrent *deposit* should **not** invalidate the withdrawal — only a competing *withdrawal*
should. So the state query is broad and the validation query is narrow.

**Trade-offs.** DCB removes sagas for cross-entity invariants, yields small composable decisions,
and lets you add a use case by adding a `Decision` without touching existing code. The costs: the
store must support multi-identifier queries + conditional append (heavier than append-to-stream);
"where do my invariants live" is less localized without an aggregate to point at; and the
ecosystem is younger. DCB originates with Sara Pellegrini's talk **"Kill Aggregate!"** — the
canonical reference, and the acknowledged inspiration for disintegrate. (Search for it; I can't
verify links.)

---

## 3. cqrs-es vs. disintegrate on one concrete domain

**Domain: course enrollment.** Two invariants that deliberately straddle two entities:

- **(A)** A course holds at most `capacity` students. *(lives inside the course)*
- **(B)** A student is enrolled in at most 10 courses. *(lives inside the student)*

Subscribing one student to one course must enforce **both at once**. This is the textbook case
that's awkward for aggregates and natural for DCB.

### cqrs-es 0.5 — you must pick an aggregate

Make `Course` the aggregate. It enforces **(A)** perfectly (it owns its roster). But **(B)** —
the student's enrollments in *other* courses — lies outside the Course boundary, so you have to
consult a read model via `Services`, which is **not transactionally consistent** with the course
stream.

```rust
use cqrs_es::{Aggregate, EventSink};
use std::collections::HashSet;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Course { capacity: u32, enrolled: HashSet<String> }

impl Aggregate for Course {
    const TYPE: &'static str = "course";
    type Command  = CourseCommand;
    type Event    = CourseEvent;
    type Error    = CourseError;
    type Services = EnrollmentServices;     // access to a student-enrollment read model

    // NOTE the 0.5 signature: &mut self, and you push into `sink` instead of
    // returning Vec<Event>. (0.4 returned Result<Vec<Self::Event>, _>.)
    async fn handle(
        &mut self,
        cmd: Self::Command,
        svc: &Self::Services,
        sink: &EventSink<Self>,
    ) -> Result<(), Self::Error> {
        match cmd {
            CourseCommand::Subscribe { student } => {
                // Invariant A — strongly consistent, inside the aggregate boundary:
                if self.enrolled.len() as u32 >= self.capacity {
                    return Err(CourseError::CourseFull);
                }
                if self.enrolled.contains(&student) {
                    return Err(CourseError::AlreadyEnrolled);
                }
                // Invariant B — crosses the boundary; we must ask a projection.
                // This read model can LAG: two concurrent subscriptions in different
                // courses can both pass `< 10` and overshoot. This is the aggregate tax.
                if svc.student_course_count(&student).await? >= 10 {
                    return Err(CourseError::StudentAtLimit);
                }
                sink.write(CourseEvent::StudentSubscribed { student }, self).await;
            }
        }
        Ok(())
    }

    fn apply(&mut self, ev: Self::Event) {
        match ev {
            CourseEvent::Created { capacity }       => self.capacity = capacity,
            CourseEvent::StudentSubscribed { student } => { self.enrolled.insert(student); }
        }
    }
}

// Wiring:
// let cqrs = CqrsFramework::new(store, vec![Box::new(my_query)], services);
// cqrs.execute(course_id, CourseCommand::Subscribe { student }).await?;
```

To make **(B)** truly consistent you'd model a `Student` aggregate, route enrollment through it,
and coordinate Course↔Student with a saga — more moving parts, eventual consistency, compensations.

### disintegrate 4.0 — one decision, a boundary spanning both

No aggregate. The decision's state is a **tuple of two state queries**: one filtered by
`course_id`, one by `student_id`. Both invariants are checked against the log and the result is
conditionally appended in one shot.

```rust
use disintegrate::{Decision, Event, StateMutate, StateQuery};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Event, Serialize, Deserialize)]
#[stream(EnrollmentEvent, [CourseCreated, StudentSubscribed])]
enum DomainEvent {
    CourseCreated   { #[id] course_id: String, capacity: u32 },
    StudentSubscribed { #[id] course_id: String, #[id] student_id: String },
}

// State A: this course's roster (events auto-filtered by course_id)
#[derive(Default, StateQuery, Clone, Serialize, Deserialize)]
#[state_query(EnrollmentEvent)]
struct CourseRoster { #[id] course_id: String, capacity: u32, enrolled: u32 }

impl StateMutate for CourseRoster {
    fn mutate(&mut self, e: Self::Event) {
        match e {
            EnrollmentEvent::CourseCreated { capacity, .. } => self.capacity = capacity,
            EnrollmentEvent::StudentSubscribed { .. }       => self.enrolled += 1,
        }
    }
}

// State B: this student's load (events auto-filtered by student_id)
#[derive(Default, StateQuery, Clone, Serialize, Deserialize)]
#[state_query(EnrollmentEvent)]
struct StudentLoad { #[id] student_id: String, count: u32 }

impl StateMutate for StudentLoad {
    fn mutate(&mut self, e: Self::Event) {
        if let EnrollmentEvent::StudentSubscribed { .. } = e { self.count += 1; }
    }
}

struct Subscribe { course_id: String, student_id: String }

impl Decision for Subscribe {
    type Event      = DomainEvent;
    type StateQuery = (CourseRoster, StudentLoad);   // <-- the dynamic boundary spans both
    type Error      = EnrollError;

    fn state_query(&self) -> Self::StateQuery {
        (
            CourseRoster { course_id: self.course_id.clone(), ..Default::default() },
            StudentLoad  { student_id: self.student_id.clone(), ..Default::default() },
        )
    }

    fn process(&self, (course, student): &Self::StateQuery)
        -> Result<Vec<Self::Event>, Self::Error>
    {
        if course.enrolled >= course.capacity { return Err(EnrollError::CourseFull); }
        if student.count >= 10                { return Err(EnrollError::StudentAtLimit); }
        Ok(vec![DomainEvent::StudentSubscribed {
            course_id:  self.course_id.clone(),
            student_id: self.student_id.clone(),
        }])
    }
}

// Wiring:
// let dm = disintegrate_postgres::decision_maker(event_store, NoSnapshot);
// dm.make(Subscribe { course_id, student_id }).await?;
```

`make()` appends `StudentSubscribed` **only if** no event matching the combined query (this
course's events *or* this student's events) appeared since the state was read. Both invariants
are enforced atomically — no saga, no lagging projection, no read-your-writes gap. That is the
DCB payoff, and it's the same code shape whether the boundary is one entity or five.

Disintegrate also ships a given/when/then `TestHarness` for unit-testing decisions without a DB:

```rust
disintegrate::TestHarness::given([
        DomainEvent::CourseCreated { course_id: "c1".into(), capacity: 1 },
        DomainEvent::StudentSubscribed { course_id: "c1".into(), student_id: "s1".into() },
    ])
    .when(Subscribe { course_id: "c1".into(), student_id: "s2".into() })
    .then_err(EnrollError::CourseFull);
```

### Choosing

| Pick **cqrs-es** when…                                                  | Pick **disintegrate** when…                                            |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Invariants sit cleanly inside one entity                                | Invariants span entities and you want to avoid sagas                   |
| Team already thinks in DDD aggregates                                   | You're greenfield and open to the DCB mental model                     |
| You want the more popular, battle-tested option (~24k recent downloads) | You're fine on Postgres (SQLite is DIY — see §5) and a younger 4.0 lib |
| You want more backends (postgres / mysql / dynamo) and examples         | You value small, composable, independently-added decisions             |

Both persist to the same kind of Postgres `events` table from Part 1. They differ in **what you
can query and how concurrency is guarded** — append-to-stream (cqrs-es) vs. conditional-append-
against-a-query (disintegrate) — not in the storage primitive.

---

## 4. Deep dive: disintegrate's `validation_query`

Every disintegrate decision uses **two** queries that answer different questions:

- **`state_query`** — *what to fold* to compute the decision. Must be broad enough to be correct.
- **`validation_query`** — *what counts as a conflict*: the minimal set of events whose
  arrival since you read could flip the decision from valid to invalid. Defaults to the full
  state query.

### The mechanism (from the 4.0 source)

`DecisionMaker::make` runs:

1. `load(state_query())` — folds every matching event and records `version` = the global
   `event_id` of the **last event folded** (your read position / origin).
2. `process(&state)` — your business logic, producing events.
3. `persist(loaded_state, events, validation_query())`.

Inside `persist` the decisive lines are:

```rust
let query = validation_query.unwrap_or_else(|| state.query_all());
event_store.append(events, query, loaded_state.version).await
```

`append(events, query, origin)` commits the new events **only if no event matching `query` has
`event_id > origin`**. That conditional-append-against-a-query *is* the concurrency guard — there
is no per-stream version; the guard is the query itself.

### Why two queries: the withdrawal case

To authorize a withdrawal you need the balance, so `state_query` must fold **both** deposits and
withdrawals. If `validation_query` is left at its default, *any* concurrent deposit or withdrawal
aborts your append and forces a retry. But a concurrent deposit only **increases** the balance —
it can never invalidate an already-valid withdrawal. Conflicting on it is a pure false positive
(wasted retries, worse throughput, potential livelock under deposit-heavy load).

Overriding `validation_query` to exclude deposits keeps correctness (only a competing *withdrawal*
can drop the balance below the requested amount) while removing the spurious conflicts:

```rust
use disintegrate::{Decision, Event, EventId, StateMutate, StateQuery, StreamQuery};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Event, Serialize, Deserialize)]
#[stream(AccountEvent, [AccountOpened, AmountDeposited, AmountWithdrawn])]
enum DomainEvent {
    AccountOpened   { #[id] account_id: String },
    AmountDeposited { #[id] account_id: String, amount: u32 },
    AmountWithdrawn { #[id] account_id: String, amount: u32 },
}

#[derive(Default, StateQuery, Clone, Serialize, Deserialize)]
#[state_query(AccountEvent)]
struct Balance { #[id] account_id: String, amount: i64 }

impl StateMutate for Balance {
    fn mutate(&mut self, e: Self::Event) {
        match e {
            AccountEvent::AmountDeposited { amount, .. } => self.amount += amount as i64,
            AccountEvent::AmountWithdrawn { amount, .. } => self.amount -= amount as i64,
            AccountEvent::AccountOpened { .. } => {}
        }
    }
}

struct Withdraw { account_id: String, amount: u32 }

impl Decision for Withdraw {
    type Event = AccountEvent;
    type StateQuery = Balance;
    type Error = BankError;

    fn state_query(&self) -> Self::StateQuery {
        Balance { account_id: self.account_id.clone(), ..Default::default() }
    }

    // Fold deposits + withdrawals (above) to get the balance, but only a concurrent
    // WITHDRAWAL can invalidate this decision, so that is all we conflict on.
    fn validation_query<ID: EventId>(&self) -> Option<StreamQuery<ID, Self::Event>> {
        Some(
            Balance { account_id: self.account_id.clone(), ..Default::default() }
                .exclude_events(&["AmountDeposited"])   // derived helper on the StateQuery
        )
    }

    fn process(&self, b: &Self::StateQuery) -> Result<Vec<Self::Event>, Self::Error> {
        if b.amount < self.amount as i64 { return Err(BankError::InsufficientFunds); }
        Ok(vec![AccountEvent::AmountWithdrawn {
            account_id: self.account_id.clone(), amount: self.amount,
        }])
    }
}
```

`exclude_events` is generated by the `StateQuery` derive and matches on the event **name** string,
so confirm the names line up with your variants (check against `Event::SCHEMA.events` if a filter
silently no-ops).

**The rule:** `validation_query` = every event type that could move the state in the direction
that invalidates the decision. Add a `FeeCharged` event that also reduces the balance, and it must
go back into the conflict set.

---

## 5. Can these crates use SQLite — and how much work?

The two libraries are in completely different positions.

### cqrs-es — already supported

There is an official **`sqlite-es`** (v0.5.0, updated Apr 2026, tracking cqrs-es 0.5). It
implements the same `PersistedEventRepository` trait as `postgres-es`, so it's a one-line
dependency swap — **zero implementation work**. Caveats: low adoption (~172 recent downloads — pin
it and skim the source), and SQLite's single-writer model means no concurrent write throughput.

### disintegrate — do it yourself

No `disintegrate-sqlite` exists; the only backend is `disintegrate-postgres`. You'd implement the
core `EventStore` trait (`append` + `stream`) over SQLite via sqlx. Inspecting what the Postgres
backend actually does, the work splits sharply:

**Ports cleanly:**
- the `event` table with one **indexed column per domain identifier** (added via
  `ALTER TABLE ADD COLUMN`, which SQLite supports), JSON/BLOB payloads, an autoincrement
  `event_id` for global order;
- `RETURNING event_id` (SQLite ≥ 3.35);
- the conditional append itself (`... WHERE NOT EXISTS (matching events with event_id > origin)`
  inside a transaction).

**The interesting part — the hard piece disappears:** disintegrate-postgres carries an advanced
**epoch mechanism** built on `pg_try_advisory_xact_lock_shared` to stop a tailing reader from
skipping an event when two writers commit sequence-assigned ids out of order. SQLite has no
advisory locks — **but it serializes writes**, so event_ids are assigned and committed in strict
order and that hazard cannot occur. You *delete* this machinery rather than port it. The price is
exactly that serialization: one writer at a time (usually fine for embedded / single-node apps).

**The remaining real work:**
- **Listener:** Postgres uses a `LISTEN/NOTIFY` trigger for push-based subscriptions; SQLite has
  no server-side pub/sub, so `PgEventListener`'s push model becomes a polling loop (tail by
  `event_id` from a stored cursor, sleep, repeat).
- **Migrator rewrite** for the SQLite dialect (no sequences; `INTEGER PRIMARY KEY AUTOINCREMENT`).
- **Identifier type mapping** (String/i64/Uuid → TEXT/INTEGER/TEXT).

**Effort:** a focused core `EventStore` (append + stream, no listener/snapshots) is roughly a
weekend — a few hundred lines — and you get to *remove* the most complex piece. Add the polling
listener and snapshotter for parity and it's more like a week with proper concurrency tests. The
conceptual core (conditional append) is easy; you're reimplementing infrastructure, not inventing
semantics.

**Bottom line:** cqrs-es on SQLite is free today; disintegrate on SQLite is a small but real
project, made *easier* (not harder) by SQLite's single-writer model.

---

## 6. Migrating from cqrs-es to disintegrate later

This is **two separate migrations**. Moving the *code* is moderate and mostly mechanical; moving
the *data* is the real project, and almost all of its difficulty is decided by choices you make
*now*, before any migration.

### The code side — a remodel, not a rewrite

- **Events largely survive.** Keep the same variants and fields; re-annotate the enum with
  `#[derive(Event)]`, `#[stream(...)]`, and `#[id]` on the identifier fields.
- **Aggregates → Decisions.** Each `Aggregate::handle` arm becomes a `Decision`; `apply` logic
  becomes `StateMutate::mutate` on one or more `StateQuery` structs. The business rules transfer
  almost line-for-line.
- **Cross-aggregate checks get *simpler*.** Invariants you previously faked through `Services`
  (e.g. the student-enrollment-count problem in §3) collapse into multi-state decisions.
- **Projections are the bigger change.** cqrs-es dispatches `Query`/`View` synchronously on
  commit; disintegrate tails the global log via `EventListener`. You rewrite projections as
  log-tailing listeners and take on explicit at-least-once / idempotency handling.

Rough cost: a few hours per aggregate, plus the projection rework.

### The data side — schema-incompatible ETL

The stored shapes don't line up:

|             | cqrs-es (`postgres-es` / `sqlite-es`)           | disintegrate (`disintegrate-postgres`)       |
| ----------- | ----------------------------------------------- | -------------------------------------------- |
| Identity    | `(aggregate_type, aggregate_id, sequence)`      | global `event_id`                            |
| Ordering    | per-aggregate `sequence`                        | single global total order                    |
| Payload     | `jsonb`                                         | bytes (JSON / Avro / Protobuf / MessagePack) |
| Identifiers | aggregate id is the stream key (often implicit) | **one indexed column per domain id**         |

You can't migrate in place. You write a one-time job: read every old event, assign a global
order, run disintegrate's migrator to create the id columns, then bulk-insert in the new layout.

Two parts are genuinely tricky:

1. **Ordering.** cqrs-es only orders events *within* an aggregate (`stream_all_events` is per
   aggregate type); there's no guaranteed cross-aggregate total order. disintegrate's whole
   concurrency/replay model rests on a global `event_id`, so you must manufacture that order from
   the physical insertion order of the old table and ensure it's monotonic and stable.

2. **Missing identifiers (the sharp one).** disintegrate's cross-entity queries need every
   relevant identifier *physically present on the event*, but cqrs-es history may never have
   recorded them — the aggregate boundary made the aggregate's own id implicit (it was the stream
   key, not necessarily a payload field). You can recover a `Course` event's `course_id` from
   `aggregate_id`, but if you later want those historical events queryable by `student_id` and it
   was never written into the payload, **you cannot retrofit it.** This semantic gap is the one
   thing you can't fix after the fact.

### What makes a future switch cheap — do this now

- **Framework-agnostic decision logic.** Put the core as pure functions
  (`state + command -> events | error`); let both `Aggregate::handle` and `Decision::process` be
  thin adapters over them. Makes the code migration nearly free.
- **Self-contained events.** Write every identifier you might ever query by into the payload —
  don't lean on the implicit aggregate id. Highest-leverage habit, because it's the part you can't
  reconstruct later.
- **Plain JSON payloads** (not Avro/Protobuf) so events stay portable, plus explicit event
  versioning.

### Verdict

Do the three things above and a later switch is a contained project: a focused ETL plus adapter
rewiring, best run as a **freeze-and-cutover** (the two storage models can't easily share data, so
live dual-write is awkward). Skip them and the migration is dominated by trying to recover
identifiers that were never stored — the difference between "a week of work" and "archaeology."

---

## 7. Worked example: framework-agnostic core + self-contained events

This makes §6's first two habits concrete on the enrollment domain. `decide` is written **once**,
framework-free; both adapters call it. The event carries **both** ids in its payload, so the same
stored rows work under either framework.

### The portable core — no framework imports

```rust
// Rule 2: every identifier this event could ever be queried by lives IN the payload.
// course_id is NOT left implicit as a "stream key"; student_id is here from day one.
#[derive(Clone, Serialize, Deserialize)]
pub enum EnrollmentEvent {
    CourseCreated     { course_id: String, capacity: u32 },
    StudentSubscribed { course_id: String, student_id: String },
}

#[derive(Default)] pub struct CourseState  { pub capacity: u32, pub enrolled: u32 }
#[derive(Default)] pub struct StudentState { pub course_count: u32 }

pub fn apply_course(s: &mut CourseState, e: &EnrollmentEvent) {
    match e {
        EnrollmentEvent::CourseCreated { capacity, .. } => s.capacity = *capacity,
        EnrollmentEvent::StudentSubscribed { .. }       => s.enrolled += 1,
    }
}
pub fn apply_student(s: &mut StudentState, e: &EnrollmentEvent) {
    if let EnrollmentEvent::StudentSubscribed { .. } = e { s.course_count += 1; }
}

pub struct Enroll { pub course_id: String, pub student_id: String }
#[derive(Debug)] pub enum EnrollError { CourseFull, StudentAtLimit }

// Rule 1: the WHOLE decision lives here. No cqrs-es, no disintegrate, no DB.
pub fn decide(course: &CourseState, student: &StudentState, cmd: &Enroll)
    -> Result<Vec<EnrollmentEvent>, EnrollError>
{
    if course.enrolled      >= course.capacity { return Err(EnrollError::CourseFull); }
    if student.course_count >= 10              { return Err(EnrollError::StudentAtLimit); }
    Ok(vec![EnrollmentEvent::StudentSubscribed {
        course_id:  cmd.course_id.clone(),
        student_id: cmd.student_id.clone(),
    }])
}
```

### Adapter A: cqrs-es (today)

```rust
impl DomainEvent for EnrollmentEvent {           // cqrs-es's trait
    fn event_type(&self) -> String {
        match self { Self::CourseCreated { .. } => "CourseCreated",
                     Self::StudentSubscribed { .. } => "StudentSubscribed" }.into()
    }
    fn event_version(&self) -> String { "1.0".into() }
}

impl Aggregate for CourseState {
    const TYPE: &'static str = "course";
    type Command = Enroll; type Event = EnrollmentEvent;
    type Error = EnrollError; type Services = EnrollServices;

    async fn handle(&mut self, cmd: Self::Command, svc: &Self::Services, sink: &EventSink<Self>)
        -> Result<(), Self::Error>
    {
        // Course slice is local & consistent; student slice comes from a projection
        // (NOT txn-consistent — the §3 aggregate tax). Same shape either way.
        let student = StudentState { course_count: svc.student_course_count(&cmd.student_id).await? };
        for ev in decide(self, &student, &cmd)? {     // <-- shared pure fn
            sink.write(ev, self).await;
        }
        Ok(())
    }
    fn apply(&mut self, e: Self::Event) { apply_course(self, &e); }   // <-- shared fold
}
```

### Adapter B: disintegrate (after you switch)

You **add annotations to the same core enum**. Because `course_id` and `student_id` are already in
the payload, existing JSON rows need **zero rewriting** — the `#[id]` tags just declare which
existing fields to index on.

```rust
#[derive(Clone, PartialEq, Eq, Event, Serialize, Deserialize)]
#[stream(EnrollmentStream, [CourseCreated, StudentSubscribed])]
pub enum EnrollmentEvent {
    CourseCreated     { #[id] course_id: String, capacity: u32 },
    StudentSubscribed { #[id] course_id: String, #[id] student_id: String },
}

#[derive(Default, StateQuery, Clone, Serialize, Deserialize)]
#[state_query(EnrollmentStream)]
struct Course { #[id] course_id: String, capacity: u32, enrolled: u32 }
impl StateMutate for Course {           // mirrors apply_course (sub-stream type)
    fn mutate(&mut self, e: Self::Event) { match e {
        EnrollmentStream::CourseCreated { capacity, .. } => self.capacity = capacity,
        EnrollmentStream::StudentSubscribed { .. }       => self.enrolled += 1,
    }}
}

#[derive(Default, StateQuery, Clone, Serialize, Deserialize)]
#[state_query(EnrollmentStream)]
struct Student { #[id] student_id: String, count: u32 }   // <-- only works because
impl StateMutate for Student {                            //     student_id is in the payload
    fn mutate(&mut self, e: Self::Event) {
        if let EnrollmentStream::StudentSubscribed { .. } = e { self.count += 1; }
    }
}

impl Decision for Enroll {
    type Event = EnrollmentEvent; type StateQuery = (Course, Student); type Error = EnrollError;
    fn state_query(&self) -> Self::StateQuery {
        (Course  { course_id:  self.course_id.clone(),  ..Default::default() },
         Student { student_id: self.student_id.clone(), ..Default::default() })
    }
    fn process(&self, (course, student): &Self::StateQuery) -> Result<Vec<Self::Event>, Self::Error> {
        let c = CourseState  { capacity: course.capacity, enrolled: course.enrolled };
        let s = StudentState { course_count: student.count };
        decide(&c, &s, self)              // <-- the SAME pure fn, verbatim
    }
}
```

The decision is shared verbatim; each adapter only differs in how it **assembles** the two state
slices — cqrs-es uses a local course plus a lagging projection for the student; disintegrate folds
both atomically from the log.

### The id rule, specifically

The trap is writing the event the way the `Course` aggregate "naturally" wants it:

```rust
StudentSubscribed { student_id }   // course_id implicit (it's the aggregate_id / stream key)
```

`course_id` you could still recover from `aggregate_id` during the ETL, so that one is only
friction. The **unrecoverable** case is any id the aggregate never needed: if later you want a
disintegrate decision indexed by `term_id` or `teacher_id`, and those were never written into the
payload because no aggregate invariant required them, they are simply gone — the "archaeology" from
§6. Putting every plausibly-queryable id in the payload now is the one thing you can't retrofit.

> Accurate to the 0.5 / 4.0 APIs read from source, but the cross-trait interplay here is fiddly —
> `cargo check` before trusting it verbatim.
