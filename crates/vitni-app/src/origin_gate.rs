//! The origin gate (ADR 0037 §4): decides, for each command an import issues, whether it is already
//! on record from the same source record, and so whether it is written at all.
//!
//! Every write that carries an origin passes through [`gate`] just before it is executed. The gate
//! previews the events the command would emit, derives the field they assert and the digest of what
//! they assert (`vitni_db::indexed_field`, the same derivation the `record_origins` index uses), and
//! compares with what earlier runs asserted into that field from the same item:
//!
//! - **Already asserted.** A list-valued row with the same digest that is still live: the write is a
//!   no-op.
//! - **Corrected by the user.** The latest single-valued row is no longer live: the user's correction
//!   stands.
//! - **Single-valued field with a value.** The field's current value, whoever set it — an earlier run,
//!   or the user at the keyboard with no row at all — is reconciled by ADR 0029 §1: the same value is a
//!   no-op; a different one is superseded when the file is at least as recent as its assertion, and left
//!   alone when the file is older or has no export date (§3).
//! - **Tombstoned.** A list-valued row with the same digest that is no longer live: the user retracted
//!   or superseded that value, and a later run does not re-assert it.
//! - **Otherwise** (a new item, a new value in a list-valued field): the write goes ahead.
//!
//! The digest the gate computes is stamped onto the command's origin, so the log records what was read.
//! A write that goes ahead also starts the session's [`PendingRun`], if it has not started yet, so an
//! import that turns out to be a no-op leaves no run behind.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;
use vitni_core::assertions::{Envelope, EventBody};
use vitni_core::ids::{AssertionId, ImportRunId};
use vitni_core::import_run::{ImportRunCommand, ImportRunCommandEnvelope, NewImportRun};
use vitni_core::origin::{ContentDigest, DatasetId, RecordOrigin};
use vitni_core::provenance::{AssertionMeta, Timestamp};
use vitni_db::{DbError, IndexedField, Store};

use crate::error::AppError;
use crate::history::LiveAssertion;
use crate::session::Session;
use crate::use_case::{self, Provenance};

/// An import run that is started on its first effective write (ADR 0037 §5): its id is minted up
/// front, so every write can carry it in its origin, but `ImportRunStarted` is written only once the
/// gate lets a write through.
#[derive(Debug)]
pub struct PendingRun {
    operator: Session,
    id: ImportRunId,
    run: NewImportRun,
    file_asserted_at: Mutex<Option<Timestamp>>,
    started: AtomicBool,
    commands: AtomicU32,
}

impl PendingRun {
    /// A run over `run`, operated by the invoking human's `operator` session. The run's own
    /// `file_asserted_at` is ignored in favour of the one the importer later declares
    /// ([`Self::set_file_asserted_at`]).
    #[must_use]
    pub fn new(operator: Session, run: NewImportRun) -> Self {
        let id = operator.new_import_run_id();
        Self {
            operator,
            id,
            run,
            file_asserted_at: Mutex::new(None),
            started: AtomicBool::new(false),
            commands: AtomicU32::new(0),
        }
    }

    /// The run's id, stamped on every origin it writes.
    #[must_use]
    pub fn id(&self) -> ImportRunId {
        self.id
    }

    /// The dataset the run's records belong to.
    #[must_use]
    pub fn dataset(&self) -> &DatasetId {
        &self.run.dataset
    }

    /// The invoking human's session.
    #[must_use]
    pub fn operator(&self) -> &Session {
        &self.operator
    }

    /// Records the document's own export date (ADR 0029 §2), before its first write.
    pub fn set_file_asserted_at(&self, file_asserted_at: Option<Timestamp>) {
        if let Ok(mut slot) = self.file_asserted_at.lock() {
            *slot = file_asserted_at;
        }
    }

    /// The document's export date, if it declared one.
    #[must_use]
    pub fn file_asserted_at(&self) -> Option<Timestamp> {
        self.file_asserted_at.lock().ok().and_then(|slot| *slot)
    }

    /// Whether any write went ahead, so the run was started and must be closed.
    #[must_use]
    pub fn started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }

    /// How many commands the run wrote.
    #[must_use]
    pub fn commands(&self) -> u32 {
        self.commands.load(Ordering::SeqCst)
    }

    /// Starts the run, unless it already has: its first effective write does, and so does closing a
    /// run that wrote nothing but has resolutions to record.
    ///
    /// # Errors
    ///
    /// A store error writing `ImportRunStarted`.
    pub async fn ensure_started(&self, store: &Store) -> Result<(), AppError> {
        if self.started.load(Ordering::SeqCst) {
            return Ok(());
        }
        let mut run = self.run.clone();
        run.file_asserted_at = self.file_asserted_at();
        let envelope = ImportRunCommandEnvelope {
            meta: self.operator.new_meta(Provenance::default(), Vec::new()),
            command: ImportRunCommand::StartImportRun { run_id: self.id, run },
        };
        store
            .execute_import_run(&self.id.to_string(), envelope)
            .await
            .map_err(use_case::map_command_error)?;
        self.started.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Admits one effective write: starts the run on the first, and counts it.
    async fn admit(&self, store: &Store) -> Result<(), AppError> {
        self.ensure_started(store).await?;
        self.commands.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// A dry run of an import's writes (ADR 0040 §2): the gate decides each write as it would, records the
/// ones it would let through, and executes none. A plan runs an existing record's writes under one to
/// tell an unchanged record from an updated one, through the very use-cases its commit runs.
#[derive(Debug, Default)]
pub struct DryRun {
    writes: Mutex<Vec<DryWrite>>,
}

/// A write a dry run would have made: the aggregate it lands on and the field it asserts, when it
/// asserts one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryWrite {
    /// The aggregate's kind (`Aggregate::TYPE`).
    pub kind: &'static str,
    /// The aggregate's id.
    pub aggregate_id: String,
    /// The field it asserts (`person.FactAsserted.Occupation`), if any.
    pub field: Option<String>,
}

impl DryRun {
    /// The writes recorded so far, emptying the record.
    pub fn take(&self) -> Vec<DryWrite> {
        self.writes
            .lock()
            .map(|mut writes| std::mem::take(&mut *writes))
            .unwrap_or_default()
    }

    fn record(&self, write: DryWrite) {
        if let Ok(mut writes) = self.writes.lock() {
            writes.push(write);
        }
    }
}

/// A command envelope the gate can preview and redirect: implemented for every aggregate that
/// imports write to.
pub(crate) trait GatedEnvelope: Sized + Clone {
    /// The aggregate's event body.
    type Body: EventBody + Serialize + DeserializeOwned;
    /// The aggregate's `Aggregate::TYPE`.
    const KIND: &'static str;

    /// The envelope's assertion inputs, whose origin the gate reads and stamps.
    fn meta(&mut self) -> &mut AssertionMeta;

    /// The events the envelope would emit against `aggregate_id`.
    async fn preview(store: &Store, aggregate_id: &str, envelope: Self) -> Result<Vec<Envelope<Self::Body>>, AppError>;

    /// The envelope rewritten to supersede `target` with its command (ADR 0004 §2).
    fn supersede(self, aggregate_id: Uuid, target: AssertionId) -> Self;
}

/// What the gate decided for one write.
enum Decision {
    Skip,
    Write,
    Supersede(AssertionId),
}

/// Passes `envelope` through the origin gate: returns the envelope to execute (possibly rewritten to
/// supersede an earlier import's value), or `None` when the write is already on record. An envelope
/// without an origin passes through untouched. Under a [`DryRun`] nothing passes: a write that would
/// go ahead is recorded instead.
///
/// # Errors
///
/// A domain rejection from the preview, or a store error.
pub(crate) async fn gate<E: GatedEnvelope>(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    mut envelope: E,
) -> Result<Option<E>, AppError> {
    if envelope.meta().context.origin.is_none() {
        if let Some(dry_run) = session.dry_run() {
            dry_run.record(DryWrite {
                kind: E::KIND,
                aggregate_id: aggregate_id.to_owned(),
                field: None,
            });
            return Ok(None);
        }
        return Ok(Some(envelope));
    }
    let previewed = E::preview(store, aggregate_id, envelope.clone()).await?;
    let target = Target {
        kind: E::KIND,
        aggregate_id,
    };
    let (decision, field) = decide(store, session, envelope.meta(), &previewed, target).await?;
    if let Some(dry_run) = session.dry_run() {
        match decision {
            Decision::Skip => {}
            Decision::Write | Decision::Supersede(_) => dry_run.record(DryWrite {
                kind: E::KIND,
                aggregate_id: aggregate_id.to_owned(),
                field,
            }),
        }
        return Ok(None);
    }
    let envelope = match decision {
        Decision::Skip => return Ok(None),
        Decision::Write => envelope,
        Decision::Supersede(target) => match Uuid::parse_str(aggregate_id) {
            Ok(id) => envelope.supersede(id, target),
            Err(_) => envelope,
        },
    };
    if let Some(run) = session.import_run() {
        run.admit(store).await?;
    }
    Ok(Some(envelope))
}

/// The aggregate instance a gated write lands on.
#[derive(Clone, Copy)]
struct Target<'a> {
    kind: &'static str,
    aggregate_id: &'a str,
}

/// Decides one write, returning the decision and the field the write asserts, if any.
async fn decide<B: EventBody + Serialize + DeserializeOwned>(
    store: &Store,
    session: &Session,
    meta: &mut AssertionMeta,
    previewed: &[Envelope<B>],
    target: Target<'_>,
) -> Result<(Decision, Option<String>), AppError> {
    let mut bodies = Vec::with_capacity(previewed.len());
    let mut primary = None;
    for event in previewed {
        let Some(field) = vitni_db::indexed_field(target.kind, &event.body)? else {
            continue;
        };
        bodies.push(&event.body);
        primary.get_or_insert(field);
    }
    let Some(origin) = meta.context.origin.as_deref_mut() else {
        return Ok((Decision::Write, primary.map(|field| field.field_key)));
    };
    origin.digest = Some(vitni_db::digest(&bodies)?);
    let Some(field) = primary else {
        return Ok((Decision::Write, None));
    };
    let decision = decide_field::<B>(store, session, origin, &field, target).await?;
    Ok((decision, Some(field.field_key)))
}

/// Decides a write asserting `field` from `origin`, against what earlier runs asserted there and,
/// for a single-valued field, against the field's current value, whoever set it.
async fn decide_field<B: Serialize + DeserializeOwned>(
    store: &Store,
    session: &Session,
    origin: &RecordOrigin,
    field: &IndexedField,
    target: Target<'_>,
) -> Result<Decision, AppError> {
    let rows = store
        .origin_rows(
            origin.dataset.as_str(),
            &origin.record,
            origin.item.as_deref(),
            &field.field_key,
        )
        .await?;
    if !vitni_db::single_valued(&field.field_key) {
        // A list value an import asserted is only ever retracted or superseded by the user (imports
        // supersede single values only), so any row with the same value, live or dead, is on record.
        return Ok(if rows.iter().any(|row| row.digest == field.digest) {
            Decision::Skip
        } else {
            Decision::Write
        });
    }
    // An earlier row can stay live under a later value (an import superseding what the user typed
    // over it), so only the latest row says whether the user corrected this item's claim, and only
    // the field's current value says whether the claim is already on record.
    if rows.last().is_some_and(|row| !row.live) {
        return Ok(Decision::Skip);
    }
    let Some(live) =
        crate::history::live_field_assertion(store, target.kind, target.aggregate_id, &field.field_key).await?
    else {
        return Ok(Decision::Write);
    };
    if live_digest::<B>(&live)? == field.digest {
        return Ok(Decision::Skip);
    }
    let Some(file_asserted_at) = session.import_run().and_then(|run| run.file_asserted_at()) else {
        return Ok(Decision::Skip);
    };
    Ok(if live.occurred_at <= file_asserted_at {
        Decision::Supersede(live.assertion_id)
    } else {
        Decision::Skip
    })
}

/// The digest of the value `live` asserts, comparable with an [`IndexedField`]'s.
fn live_digest<B: Serialize + DeserializeOwned>(live: &LiveAssertion) -> Result<ContentDigest, AppError> {
    let event: Envelope<B> = serde_json::from_str(&live.payload)
        .map_err(|e| AppError::Db(DbError::Backend(format!("decoding a field's live assertion: {e}"))))?;
    Ok(vitni_db::digest(&[&event.body])?)
}

/// Implements [`GatedEnvelope`] for one aggregate's command envelope. The `supersede` arm names the
/// command's `SupersedeAssertion` id field; an aggregate without one (Tag) writes the new value over
/// the old instead.
macro_rules! gated_envelope {
    ($module:ident, $Envelope:ident, $Body:ident, $Command:ident, $preview:ident, $kind:literal, $Id:ident, $id_field:ident) => {
        impl GatedEnvelope for vitni_core::$module::$Envelope {
            type Body = vitni_core::$module::$Body;
            const KIND: &'static str = $kind;

            fn meta(&mut self) -> &mut AssertionMeta {
                &mut self.meta
            }

            async fn preview(
                store: &Store,
                aggregate_id: &str,
                envelope: Self,
            ) -> Result<Vec<Envelope<Self::Body>>, AppError> {
                store
                    .$preview(aggregate_id, envelope)
                    .await
                    .map_err(use_case::map_command_error)
            }

            fn supersede(mut self, aggregate_id: Uuid, target: AssertionId) -> Self {
                self.command = vitni_core::$module::$Command::SupersedeAssertion {
                    $id_field: vitni_core::ids::$Id::from_uuid(aggregate_id),
                    target,
                    replacement: Box::new(self.command),
                };
                self
            }
        }
    };
    ($module:ident, $Envelope:ident, $Body:ident, $preview:ident, $kind:literal) => {
        impl GatedEnvelope for vitni_core::$module::$Envelope {
            type Body = vitni_core::$module::$Body;
            const KIND: &'static str = $kind;

            fn meta(&mut self) -> &mut AssertionMeta {
                &mut self.meta
            }

            async fn preview(
                store: &Store,
                aggregate_id: &str,
                envelope: Self,
            ) -> Result<Vec<Envelope<Self::Body>>, AppError> {
                store
                    .$preview(aggregate_id, envelope)
                    .await
                    .map_err(use_case::map_command_error)
            }

            fn supersede(self, _aggregate_id: Uuid, _target: AssertionId) -> Self {
                self
            }
        }
    };
}

gated_envelope!(
    person,
    PersonCommandEnvelope,
    PersonEventBody,
    PersonCommand,
    preview_person,
    "person",
    PersonId,
    person_id
);
gated_envelope!(
    family,
    FamilyCommandEnvelope,
    FamilyEventBody,
    FamilyCommand,
    preview_family,
    "family",
    FamilyId,
    family_id
);
gated_envelope!(
    place,
    PlaceCommandEnvelope,
    PlaceEventBody,
    PlaceCommand,
    preview_place,
    "place",
    PlaceId,
    place_id
);
gated_envelope!(
    source,
    SourceCommandEnvelope,
    SourceEventBody,
    SourceCommand,
    preview_source,
    "source",
    SourceId,
    source_id
);
gated_envelope!(
    citation,
    CitationCommandEnvelope,
    CitationEventBody,
    CitationCommand,
    preview_citation,
    "citation",
    CitationId,
    citation_id
);
gated_envelope!(
    event,
    EventCommandEnvelope,
    EventEventBody,
    EventCommand,
    preview_event,
    "event",
    EventId,
    event_id
);
gated_envelope!(
    dna_test,
    DnaTestCommandEnvelope,
    DnaTestEventBody,
    DnaTestCommand,
    preview_dna_test,
    "dna_test",
    DnaTestId,
    dna_test_id
);
gated_envelope!(
    dna_match,
    DnaMatchCommandEnvelope,
    DnaMatchEventBody,
    DnaMatchCommand,
    preview_dna_match,
    "dna_match",
    DnaMatchId,
    dna_match_id
);
gated_envelope!(
    repository,
    RepositoryCommandEnvelope,
    RepositoryEventBody,
    RepositoryCommand,
    preview_repository,
    "repository",
    RepositoryId,
    repository_id
);
gated_envelope!(
    note,
    NoteCommandEnvelope,
    NoteEventBody,
    NoteCommand,
    preview_note,
    "note",
    NoteId,
    note_id
);
gated_envelope!(
    media,
    MediaCommandEnvelope,
    MediaEventBody,
    MediaCommand,
    preview_media,
    "media",
    MediaId,
    media_id
);
gated_envelope!(
    research_note,
    ResearchNoteCommandEnvelope,
    ResearchNoteEventBody,
    ResearchNoteCommand,
    preview_research_note,
    "research_note",
    ResearchNoteId,
    research_note_id
);
gated_envelope!(tag, TagCommandEnvelope, TagEventBody, preview_tag, "tag");

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::macros::datetime;
    use uuid::Uuid;
    use vitni_core::enums::{EventType, EvidenceLevel, FactType};
    use vitni_core::ids::AgentId;
    use vitni_core::import_run::NewImportRun;
    use vitni_core::origin::{DatasetId, RecordOrigin};
    use vitni_core::provenance::{Agent, AgentKind, Timestamp};

    use super::PendingRun;
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::event::{NewEvent, create_event, set_event_description};
    use crate::history::{undo_assertion, undo_event_assertion};
    use crate::import_run::list_import_runs;
    use crate::person::{NewFact, NewPerson, assert_fact, create_person};
    use crate::session::Session;
    use crate::use_case::{MutationMeta, Provenance};
    use crate::workspace::Workspace;

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Tester".to_owned()),
            email: None,
        }
    }

    async fn workspace(dir: &std::path::Path) -> Workspace {
        let root = dir.join("ws");
        Workspace::init(&root, &operator(), &AppDefaults::default(), None).expect("init");
        Workspace::open(&root, &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open")
    }

    fn dataset() -> DatasetId {
        DatasetId::lineage("gedcom", Uuid::from_u128(5))
    }

    /// An importer's session writing as part of a fresh pending run, and that run.
    fn importer(file_asserted_at: Option<Timestamp>) -> (Session, Arc<PendingRun>) {
        let human = Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Tester".to_owned()),
        });
        let run = Arc::new(PendingRun::new(
            human,
            NewImportRun {
                plugin: "gedcom-import".to_owned(),
                plugin_version: "0.1.0".to_owned(),
                dataset: dataset(),
                dataset_label: "tree.ged".to_owned(),
                source_label: "tree.ged".to_owned(),
                source_path: None,
                file_asserted_at: None,
                dataset_hint: None,
            },
        ));
        run.set_file_asserted_at(file_asserted_at);
        let session = Session::software("gedcom-import", "0.1.0").with_import_run(Arc::clone(&run));
        (session, run)
    }

    fn from(run: &PendingRun, record: &str, item: Option<&str>) -> Provenance {
        Provenance {
            origin: Some(RecordOrigin {
                dataset: dataset(),
                record: record.to_owned(),
                item: item.map(str::to_owned),
                digest: None,
                run: run.id(),
            }),
            ..Provenance::default()
        }
    }

    fn meta(provenance: Provenance) -> MutationMeta<'static> {
        MutationMeta {
            provenance,
            ..MutationMeta::default()
        }
    }

    async fn person(workspace: &Workspace, session: &Session, run: &PendingRun) -> String {
        let new = NewPerson {
            human_id: None,
            name: None,
            evidence_level: EvidenceLevel::Persona,
            external_ids: Vec::new(),
        };
        create_person(workspace, session, new, from(run, "I1", None), &[])
            .await
            .expect("person")
    }

    async fn occupation(workspace: &Workspace, session: &Session, run: &PendingRun, person: &str, value: &str) {
        let new = NewFact {
            fact_type: FactType::Occupation,
            value: Some(value.to_owned()),
            date: None,
        };
        assert_fact(workspace, session, person, new, meta(from(run, "I1", None)))
            .await
            .expect("fact");
    }

    async fn event(workspace: &Workspace, session: &Session, run: &PendingRun) -> String {
        let new = NewEvent {
            human_id: None,
            event_type: EventType::Birth,
        };
        create_event(workspace, session, new, from(run, "I1", Some("event:BIRT:0")), &[])
            .await
            .expect("event")
    }

    async fn describe(workspace: &Workspace, session: &Session, run: &PendingRun, event: &str, text: &str) {
        let meta = meta(from(run, "I1", Some("event:BIRT:0")));
        set_event_description(workspace, session, event, text.to_owned(), meta)
            .await
            .expect("description");
    }

    async fn description(workspace: &Workspace, event: &str) -> Option<String> {
        let view = workspace.store().find_event(event).await.expect("find").expect("event");
        view.description().map(str::to_owned)
    }

    async fn events(workspace: &Workspace) -> u64 {
        workspace.store().event_count().await.expect("count")
    }

    #[tokio::test]
    async fn a_value_already_asserted_from_the_same_item_is_not_written_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let person = person(&workspace, &session, &run).await;
        occupation(&workspace, &session, &run, &person, "Farmer").await;
        let before = events(&workspace).await;

        occupation(&workspace, &session, &run, &person, "Farmer").await;
        assert_eq!(events(&workspace).await, before, "the second assertion is a no-op");
    }

    #[tokio::test]
    async fn a_new_value_in_a_list_field_is_added() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let person = person(&workspace, &session, &run).await;
        occupation(&workspace, &session, &run, &person, "Farmer").await;
        occupation(&workspace, &session, &run, &person, "Smith").await;

        let view = workspace
            .store()
            .find_person(&person)
            .await
            .expect("find")
            .expect("person");
        assert_eq!(view.facts().len(), 2);
    }

    /// Retracts, at the keyboard, the person's only live fact.
    async fn retract_the_fact(workspace: &Workspace, person: &str) {
        let view = workspace
            .store()
            .find_person(person)
            .await
            .expect("find")
            .expect("person");
        let [fact] = view.facts_with_assertions() else {
            panic!("expected one fact: {:?}", view.facts_with_assertions());
        };
        let keyboard = Session::software("keyboard", "0");
        undo_assertion(workspace, &keyboard, person, &fact.assertion_id.to_string(), None)
            .await
            .expect("retract");
    }

    async fn facts(workspace: &Workspace, person: &str) -> Vec<Option<String>> {
        let view = workspace
            .store()
            .find_person(person)
            .await
            .expect("find")
            .expect("person");
        view.facts().iter().map(|fact| fact.value.value.clone()).collect()
    }

    #[tokio::test]
    async fn a_list_value_the_user_retracted_is_not_reasserted_by_a_later_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let person = person(&workspace, &session, &run).await;
        occupation(&workspace, &session, &run, &person, "Farmer").await;
        retract_the_fact(&workspace, &person).await;
        let before = events(&workspace).await;

        let (session, rerun) = importer(None);
        occupation(&workspace, &session, &rerun, &person, "Farmer").await;
        assert!(facts(&workspace, &person).await.is_empty(), "the retraction stands");
        assert_eq!(events(&workspace).await, before);
        assert!(!rerun.started(), "nothing was written, so no run");
    }

    #[tokio::test]
    async fn a_different_list_value_is_still_added_after_a_retraction() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let person = person(&workspace, &session, &run).await;
        occupation(&workspace, &session, &run, &person, "Farmer").await;
        retract_the_fact(&workspace, &person).await;

        let (session, rerun) = importer(None);
        occupation(&workspace, &session, &rerun, &person, "Smith").await;
        assert_eq!(facts(&workspace, &person).await, [Some("Smith".to_owned())]);
    }

    #[tokio::test]
    async fn a_write_without_an_origin_is_never_gated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let person = person(&workspace, &session, &run).await;
        let keyboard = Session::software("keyboard", "0");
        for _ in 0..2 {
            let new = NewFact {
                fact_type: FactType::Occupation,
                value: Some("Farmer".to_owned()),
                date: None,
            };
            assert_fact(&workspace, &keyboard, &person, new, MutationMeta::default())
                .await
                .expect("fact");
        }
        let view = workspace
            .store()
            .find_person(&person)
            .await
            .expect("find")
            .expect("person");
        assert_eq!(view.facts().len(), 2, "two keyboard assertions are two claims");
    }

    #[tokio::test]
    async fn a_changed_single_value_supersedes_when_the_file_is_newer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let event = event(&workspace, &session, &run).await;
        describe(&workspace, &session, &run, &event, "at home").await;

        let (session, run) = importer(Some(Timestamp::new(datetime!(2100-01-01 00:00 UTC))));
        describe(&workspace, &session, &run, &event, "at church").await;
        assert_eq!(description(&workspace, &event).await.as_deref(), Some("at church"));
        let rows = workspace
            .store()
            .origin_rows(dataset().as_str(), "I1", Some("event:BIRT:0"), "event.DescriptionSet")
            .await
            .expect("rows");
        let live: Vec<bool> = rows.iter().map(|row| row.live).collect();
        assert_eq!(live, [false, true], "the earlier value was superseded");
    }

    #[tokio::test]
    async fn a_changed_single_value_supersedes_when_the_file_is_exactly_as_recent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let event = event(&workspace, &session, &run).await;
        describe(&workspace, &session, &run, &event, "at home").await;
        let rows = workspace
            .store()
            .origin_rows(dataset().as_str(), "I1", Some("event:BIRT:0"), "event.DescriptionSet")
            .await
            .expect("rows");
        let live_at = rows.first().expect("the live description's row").occurred_at;

        // ADR 0029 §1: a file exported at the very instant of the live value is at least as recent.
        let (session, run) = importer(Some(live_at));
        describe(&workspace, &session, &run, &event, "at church").await;
        assert_eq!(description(&workspace, &event).await.as_deref(), Some("at church"));
        let rows = workspace
            .store()
            .origin_rows(dataset().as_str(), "I1", Some("event:BIRT:0"), "event.DescriptionSet")
            .await
            .expect("rows");
        let live: Vec<bool> = rows.iter().map(|row| row.live).collect();
        assert_eq!(live, [false, true], "the earlier value was superseded");
    }

    #[tokio::test]
    async fn a_changed_single_value_is_left_alone_when_the_file_is_older_or_undated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let event = event(&workspace, &session, &run).await;
        describe(&workspace, &session, &run, &event, "at home").await;
        let before = events(&workspace).await;

        for file_asserted_at in [Some(Timestamp::new(datetime!(2000-01-01 00:00 UTC))), None] {
            let (session, run) = importer(file_asserted_at);
            describe(&workspace, &session, &run, &event, "at church").await;
            assert_eq!(description(&workspace, &event).await.as_deref(), Some("at home"));
            assert!(!run.started(), "nothing was written, so no run");
        }
        assert_eq!(events(&workspace).await, before);
    }

    #[tokio::test]
    async fn a_single_value_the_user_retracted_is_not_reasserted_over() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let event = event(&workspace, &session, &run).await;
        describe(&workspace, &session, &run, &event, "at home").await;
        let rows = workspace
            .store()
            .origin_rows(dataset().as_str(), "I1", Some("event:BIRT:0"), "event.DescriptionSet")
            .await
            .expect("rows");
        let human = Session::software("keyboard", "0");
        undo_event_assertion(&workspace, &human, &event, &rows[0].assertion_id.to_string(), None)
            .await
            .expect("undo");
        let before = events(&workspace).await;

        let (session, run) = importer(Some(Timestamp::new(datetime!(2100-01-01 00:00 UTC))));
        describe(&workspace, &session, &run, &event, "at church").await;
        assert_eq!(events(&workspace).await, before, "the user's correction stands");
    }

    #[tokio::test]
    async fn a_value_the_user_set_after_the_file_was_exported_is_not_superseded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let event = event(&workspace, &session, &run).await;
        describe(&workspace, &session, &run, &event, "at home").await;
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let exported = Timestamp::new(time::OffsetDateTime::now_utc());
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let keyboard = Session::software("keyboard", "0");
        set_event_description(
            &workspace,
            &keyboard,
            &event,
            "in the barn".to_owned(),
            MutationMeta::default(),
        )
        .await
        .expect("user edit");
        let before = events(&workspace).await;

        let (session, rerun) = importer(Some(exported));
        describe(&workspace, &session, &rerun, &event, "at church").await;
        assert_eq!(description(&workspace, &event).await.as_deref(), Some("in the barn"));
        assert_eq!(events(&workspace).await, before, "the user's later value stands");
    }

    /// An imported event whose description the user then types at the keyboard, as `text`.
    async fn event_with_typed_description(workspace: &Workspace, text: &str) -> String {
        let (session, run) = importer(None);
        let event = event(workspace, &session, &run).await;
        let keyboard = Session::software("keyboard", "0");
        set_event_description(workspace, &keyboard, &event, text.to_owned(), MutationMeta::default())
            .await
            .expect("user edit");
        event
    }

    #[tokio::test]
    async fn a_value_the_user_typed_is_superseded_when_the_file_is_newer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let event = event_with_typed_description(&workspace, "in the barn").await;

        let (session, run) = importer(Some(Timestamp::new(datetime!(2100-01-01 00:00 UTC))));
        describe(&workspace, &session, &run, &event, "at church").await;
        assert_eq!(description(&workspace, &event).await.as_deref(), Some("at church"));
        let view = workspace
            .store()
            .find_event(&event)
            .await
            .expect("find")
            .expect("event");
        let id = view.event_id().expect("event id").to_string();
        let stream = workspace
            .store()
            .read_aggregate_events("event", &id)
            .await
            .expect("stream");
        let types: Vec<&str> = stream.iter().map(|event| event.event_type.as_str()).collect();
        assert_eq!(
            types.get(types.len().saturating_sub(2)..),
            Some(&["AssertionSuperseded", "DescriptionSet"][..]),
            "the typed description was superseded, not overwritten: {types:?}"
        );
    }

    #[tokio::test]
    async fn a_value_the_user_typed_is_left_alone_when_the_file_is_older_or_undated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let event = event_with_typed_description(&workspace, "in the barn").await;
        let before = events(&workspace).await;

        for file_asserted_at in [Some(Timestamp::new(datetime!(2000-01-01 00:00 UTC))), None] {
            let (session, run) = importer(file_asserted_at);
            describe(&workspace, &session, &run, &event, "at church").await;
            assert_eq!(description(&workspace, &event).await.as_deref(), Some("in the barn"));
            assert!(!run.started(), "nothing was written, so no run");
        }
        assert_eq!(events(&workspace).await, before);
    }

    #[tokio::test]
    async fn a_value_the_user_typed_that_the_file_also_carries_is_not_written_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let event = event_with_typed_description(&workspace, "at church").await;
        let before = events(&workspace).await;

        let (session, run) = importer(Some(Timestamp::new(datetime!(2100-01-01 00:00 UTC))));
        describe(&workspace, &session, &run, &event, "at church").await;
        assert_eq!(events(&workspace).await, before);
        assert!(!run.started(), "nothing was written, so no run");
    }

    /// An event the file described as "at home", then the user as "in the barn", then a newer file
    /// as "at church" — superseding the user's value and leaving the first run's row live.
    async fn event_redescribed_over_a_typed_value(workspace: &Workspace) -> String {
        let (session, run) = importer(None);
        let event = event(workspace, &session, &run).await;
        describe(workspace, &session, &run, &event, "at home").await;
        let keyboard = Session::software("keyboard", "0");
        set_event_description(
            workspace,
            &keyboard,
            &event,
            "in the barn".to_owned(),
            MutationMeta::default(),
        )
        .await
        .expect("user edit");
        let (session, run) = importer(Some(Timestamp::new(datetime!(2100-01-01 00:00 UTC))));
        describe(workspace, &session, &run, &event, "at church").await;
        assert_eq!(description(workspace, &event).await.as_deref(), Some("at church"));
        event
    }

    #[tokio::test]
    async fn a_newer_file_reverting_to_an_earlier_runs_value_supersedes_the_current_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let event = event_redescribed_over_a_typed_value(&workspace).await;

        let (session, run) = importer(Some(Timestamp::new(datetime!(2100-06-01 00:00 UTC))));
        describe(&workspace, &session, &run, &event, "at home").await;
        assert_eq!(description(&workspace, &event).await.as_deref(), Some("at home"));
    }

    #[tokio::test]
    async fn a_single_value_the_user_retracted_over_an_older_live_row_is_not_reasserted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let event = event_redescribed_over_a_typed_value(&workspace).await;
        let rows = workspace
            .store()
            .origin_rows(dataset().as_str(), "I1", Some("event:BIRT:0"), "event.DescriptionSet")
            .await
            .expect("rows");
        let latest = rows.last().expect("row").assertion_id.to_string();
        let human = Session::software("keyboard", "0");
        undo_event_assertion(&workspace, &human, &event, &latest, None)
            .await
            .expect("undo");
        let before = events(&workspace).await;

        let (session, run) = importer(Some(Timestamp::new(datetime!(2100-06-01 00:00 UTC))));
        describe(&workspace, &session, &run, &event, "at church").await;
        assert_eq!(events(&workspace).await, before, "the user's correction stands");
    }

    #[tokio::test]
    async fn a_run_starts_on_its_first_effective_write_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let (session, run) = importer(None);
        let person = person(&workspace, &session, &run).await;
        occupation(&workspace, &session, &run, &person, "Farmer").await;
        assert!(run.started());
        assert_eq!(run.commands(), 2);

        let (session, rerun) = importer(None);
        occupation(&workspace, &session, &rerun, &person, "Farmer").await;
        assert!(!rerun.started(), "a re-run that writes nothing starts no run");
        let runs = list_import_runs(&workspace).await.expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run.id());
    }
}
