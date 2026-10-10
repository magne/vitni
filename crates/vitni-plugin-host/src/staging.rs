//! The `staging` capability (ADR 0040 §1, §4): an importer submits one record graph per source record,
//! and the host plans and writes them through `vitni-app`.
//!
//! A bulk import's graphs are held until the guest returns, then planned as one; the frontend's
//! [`PlanReviewer`] is shown the plan and answers its possible matches, and the plan is committed with
//! the commit's progress reported to the frontend — whose cancel stops it between two writes. An
//! assisted import's graphs are planned as one and committed on each `submit`, and the guest learns
//! what each entity became.

use vitni_app::{
    CommitControl, CommitOutcome, EntityFields, EntityRef, ImportPlan, ImportReview, LinkKind, MatchReply, NewFact,
    PairAnswer, PlanError, PlanReply, PlanStep, RecordGraph, ReviewReply, RunToEnd, StagedCitation, StagedEntity,
    StagedEvent, StagedFamily, StagedLink, StagedMedia, StagedNote, StagedPerson, StagedPlace, StagedRepository,
    StagedSource, StagedTag, Timestamp,
};
use vitni_core::geo::GeoCoordinates;
use vitni_core::matching::MatchableKind;

use crate::bindings::imports::vitni::host_api::{staging, types};
use crate::capability::Capability;
use crate::error::PluginError;
use crate::present::PresentError;
use crate::review::{DeferMatches, PlanReviewer};
use crate::state::{
    HostState, Staging, media_ref_input, to_address, to_age, to_association_role, to_attribute, to_capability_error,
    to_child_relationship, to_confidence, to_event_type, to_external_id, to_fact_type, to_genealogical_date,
    to_note_type, to_person_name, to_place_type, to_restrictions, to_role, to_sex, to_source_media_type,
};
use crate::{ProgressControl, ProgressStep, ProgressUpdate};

/// How many writes pass between two progress reports of a bulk commit.
const REPORT_EVERY: u32 = 10;

/// Reads the export date a guest declared: the instant when it parses, else the date as declared.
///
/// A date that cannot be read degrades to no date — the conservative, additive-only default (ADR 0029
/// §3): a malformed date is the guest's format-parsing problem, not a capability violation. It is
/// logged and kept verbatim, so the run can say why nothing was replaced.
fn read_file_date(declared: Option<String>) -> (Option<Timestamp>, Option<String>) {
    let Some(declared) = declared else {
        return (None, None);
    };
    if let Some(asserted_at) = Timestamp::parse_rfc3339(&declared) {
        return (Some(asserted_at), None);
    }
    tracing::warn!(
        %declared,
        "the file's export date cannot be read, so the import replaces no single value (ADR 0029 §3)"
    );
    (None, Some(declared))
}

impl staging::Host for HostState {
    fn begin_run(
        &mut self,
        dataset_hint: Option<String>,
        source_label: Option<String>,
        file_asserted_at: Option<String>,
    ) -> impl Future<Output = Result<(), types::CapabilityError>> {
        std::future::ready(self.declare_run(dataset_hint.as_deref(), source_label.as_deref(), file_asserted_at))
    }

    async fn submit(
        &mut self,
        graphs: Vec<staging::RecordGraph>,
    ) -> Result<staging::SubmitOutcome, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let mut submitted = Vec::new();
        for graph in graphs {
            let graph = to_graph(graph);
            graph
                .validate()
                .map_err(|error| types::CapabilityError::InvalidInput(error.to_string()))?;
            submitted.push(graph);
        }
        match self.staging {
            Staging::Held => {
                self.staged.extend(submitted);
                Ok(staging::SubmitOutcome::Staged)
            }
            Staging::Immediate => self.commit_now(submitted).await,
        }
    }
}

impl HostState {
    /// Adds what a commit wrote to the import run, if this invocation writes one.
    fn absorb(&mut self, outcome: &CommitOutcome) {
        if let Some(run) = self.run.as_mut() {
            run.absorb(outcome);
        }
    }

    /// Records the document's declarations (see [`staging::Host::begin_run`]).
    fn declare_run(
        &mut self,
        dataset_hint: Option<&str>,
        source_label: Option<&str>,
        file_asserted_at: Option<String>,
    ) -> Result<(), types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        tracing::debug!(?dataset_hint, ?source_label, "import declared");
        self.dataset_hint = dataset_hint.map(str::to_owned);
        (self.file_asserted_at, self.unreadable_file_date) = read_file_date(file_asserted_at);
        if let Some(run) = &self.run {
            run.pending()
                .set_file_date(self.file_asserted_at, self.unreadable_file_date.clone());
        }
        Ok(())
    }

    /// Plans `graphs` as one, asks the user about each possible match through the frontend (ADR 0040
    /// §4), shows what they add to stored records (ADR 0046), and commits them at once, returning what
    /// each of their entities became — or that the user skipped the record or cancelled the session
    /// there, writing nothing. With no frontend to ask, every possible match is left for later.
    async fn commit_now(&mut self, graphs: Vec<RecordGraph>) -> Result<staging::SubmitOutcome, types::CapabilityError> {
        let template = self.provenance();
        let mut review = ImportReview::plan(&self.workspace, &self.session, graphs, self.file_asserted_at)
            .await
            .map_err(|error| plan_capability_error(&error))?;
        while let Some(question) = review
            .next_question(&self.workspace)
            .await
            .map_err(|error| to_capability_error(&error))?
        {
            let reply = match self.io.presenter.as_mut() {
                Some(presenter) => presenter
                    .review_match(question)
                    .await
                    .map_err(|error| types::CapabilityError::Backend(error.to_string()))?,
                None => MatchReply::Pair(Box::new(PairAnswer::Later)),
            };
            let answer = match reply {
                MatchReply::Pair(answer) => *answer,
                MatchReply::Skip => return Ok(staging::SubmitOutcome::Skipped),
                MatchReply::Cancel => return Ok(staging::SubmitOutcome::Cancelled),
            };
            review
                .answer(&self.workspace, &self.session, answer)
                .await
                .map_err(|error| plan_capability_error(&error))?;
        }
        match self.confirm_plan(&review).await? {
            PlanReply::Import => {}
            PlanReply::Skip => return Ok(staging::SubmitOutcome::Skipped),
            PlanReply::Cancel => return Ok(staging::SubmitOutcome::Cancelled),
        }
        let operator = self
            .run
            .as_ref()
            .map_or_else(|| self.session.clone(), |run| run.pending().operator().clone());
        let plan = review.plan_so_far().clone();
        match review
            .commit(&self.workspace, &self.session, &operator, &template, &mut RunToEnd)
            .await
        {
            Ok(outcome) => {
                self.absorb(&outcome);
                Ok(staging::SubmitOutcome::Committed(committed(&plan, &outcome)))
            }
            Err(failure) => {
                self.absorb(&failure.outcome);
                Err(to_capability_error(&failure.error))
            }
        }
    }

    /// Shows the frontend the stored records `review` adds to, with the fields each gains, and returns
    /// whether to write them (ADR 0046). A record that adds to no stored record, or one with no
    /// frontend to ask, is imported without asking.
    async fn confirm_plan(&mut self, review: &ImportReview) -> Result<PlanReply, types::CapabilityError> {
        let summary = review.summary();
        let Some(presenter) = self.io.presenter.as_mut() else {
            return Ok(PlanReply::Import);
        };
        if summary.records.is_empty() {
            return Ok(PlanReply::Import);
        }
        presenter
            .confirm_plan(summary)
            .await
            .map_err(|error| types::CapabilityError::Backend(error.to_string()))
    }

    /// Plans every graph the guest submitted, shows the plan to the frontend's reviewer and reviews its
    /// possible matches as the reviewer answers (ADR 0040 §4), then commits it, reporting progress and
    /// stopping when the frontend cancels. Nothing is written when the guest was cancelled while
    /// parsing, or the reviewer discards the plan or cancels the review. With no reviewer, every
    /// possible match is left for later.
    ///
    /// # Errors
    ///
    /// [`PluginError::Guest`] when the graphs cannot be planned together (two stage one origin),
    /// [`PluginError::Runtime`] when the reviewer cannot be reached, or [`PluginError::Commit`] when the
    /// workspace cannot be read or a write fails.
    pub(crate) async fn commit_staged(&mut self) -> Result<(), PluginError> {
        let graphs = std::mem::take(&mut self.staged);
        if graphs.is_empty() || self.cancelled {
            return Ok(());
        }
        let template = self.provenance();
        let mut review = ImportReview::plan(&self.workspace, &self.session, graphs, self.file_asserted_at)
            .await
            .map_err(plan_plugin_error)?;
        let mut reviewer = self.reviewer.take().unwrap_or_else(|| Box::new(DeferMatches));
        if !self.settle(&mut review, reviewer.as_mut()).await? {
            return Ok(());
        }
        let operator = self
            .run
            .as_ref()
            .map_or_else(|| self.session.clone(), |run| run.pending().operator().clone());
        let mut control = Reporter {
            progress: &mut self.io.progress,
            cancelled: false,
        };
        let result = review
            .commit(&self.workspace, &self.session, &operator, &template, &mut control)
            .await;
        if control.cancelled
            && let Some(run) = self.run.as_mut()
        {
            run.cancel();
        }
        match result {
            Ok(outcome) => {
                self.absorb(&outcome);
                Ok(())
            }
            Err(failure) => {
                self.absorb(&failure.outcome);
                Err(PluginError::Commit(failure.error.to_string()))
            }
        }
    }

    /// Shows `review`'s plan to `reviewer` and answers its possible matches as the reviewer does, then,
    /// when anything was asked, shows the plan as the answers leave it. Returns `false` when the reviewer
    /// discarded either plan or cancelled the review, so nothing is written.
    async fn settle(&self, review: &mut ImportReview, reviewer: &mut dyn PlanReviewer) -> Result<bool, PluginError> {
        let unreachable = |error: PresentError| PluginError::Runtime(error.to_string());
        match reviewer.plan(review.summary()).await.map_err(unreachable)? {
            PlanStep::Discard => return Ok(false),
            PlanStep::DeferMatches => {
                review.defer_rest();
                return Ok(true);
            }
            PlanStep::Review => {}
        }
        let mut asked = false;
        while let Some(question) = review
            .next_question(&self.workspace)
            .await
            .map_err(|error| PluginError::Commit(error.to_string()))?
        {
            asked = true;
            let answered = match reviewer.review_match(question).await.map_err(unreachable)? {
                ReviewReply::Pair(answer) => review.answer(&self.workspace, &self.session, *answer).await,
                ReviewReply::SameForGroup(decision) => {
                    review.answer_group(&self.workspace, &self.session, *decision).await
                }
                ReviewReply::DeferRest => {
                    review.defer_rest();
                    Ok(())
                }
                ReviewReply::Cancel => return Ok(false),
            };
            answered.map_err(plan_plugin_error)?;
        }
        if !asked {
            return Ok(true);
        }
        reviewer.confirm(review.summary()).await.map_err(unreachable)
    }
}

/// Reports a bulk commit's progress to the frontend's sink, and stops the commit when it cancels.
struct Reporter<'a> {
    progress: &'a mut crate::ProgressFn,
    cancelled: bool,
}

impl CommitControl for Reporter<'_> {
    fn proceed(&mut self, done: u32, total: u32) -> bool {
        if !done.is_multiple_of(REPORT_EVERY) && done + 1 != total {
            return true;
        }
        let update = ProgressUpdate {
            step: ProgressStep::Writing,
            processed: done,
            total: Some(total),
        };
        if (self.progress)(update) == ProgressControl::Cancel {
            self.cancelled = true;
            return false;
        }
        true
    }
}

/// The plugin error of an import whose graphs cannot be planned.
fn plan_plugin_error(error: PlanError) -> PluginError {
    match error {
        PlanError::Graph(error) => PluginError::Guest(error.to_string()),
        PlanError::App(error) => PluginError::Commit(error.to_string()),
    }
}

/// The capability error a guest sees when its graph cannot be planned.
fn plan_capability_error(error: &PlanError) -> types::CapabilityError {
    match error {
        PlanError::Graph(error) => types::CapabilityError::InvalidInput(error.to_string()),
        PlanError::App(error) => to_capability_error(error),
    }
}

/// What the entities of a committed plan's graphs became.
fn committed(plan: &ImportPlan, outcome: &CommitOutcome) -> Vec<staging::CommittedEntity> {
    outcome
        .entities
        .iter()
        .filter_map(|entity| Some((plan.graphs.get(entity.graph)?, entity)))
        .map(|(graph, entity)| staging::CommittedEntity {
            record: graph.record.clone(),
            local_id: entity.local_id,
            human_id: entity.human_id.clone(),
        })
        .collect()
}

/// Maps a WIT record graph onto the app's.
fn to_graph(graph: staging::RecordGraph) -> RecordGraph {
    RecordGraph {
        record: graph.record,
        entities: graph.entities.into_iter().map(to_entity).collect(),
        links: graph.links.into_iter().map(to_link).collect(),
    }
}

fn to_entity(entity: staging::StagedEntity) -> StagedEntity {
    let fields = match entity.fields {
        staging::EntityFields::Person(person) => EntityFields::Person(StagedPerson {
            names: person.names.into_iter().map(to_person_name).collect(),
            sex: person.sex.map(to_sex),
            facts: person.facts.into_iter().map(to_fact).collect(),
            external_ids: person.external_ids.into_iter().map(to_external_id).collect(),
            restrictions: to_restrictions(person.restrictions),
        }),
        staging::EntityFields::Family(family) => EntityFields::Family(StagedFamily {
            external_ids: family.external_ids.into_iter().map(to_external_id).collect(),
            restrictions: to_restrictions(family.restrictions),
        }),
        staging::EntityFields::Event(event) => EntityFields::Event(StagedEvent {
            event_type: to_event_type(event.event_type),
            date: event.date.map(to_genealogical_date),
            addresses: event.addresses.into_iter().map(to_address).collect(),
            restrictions: to_restrictions(event.restrictions),
        }),
        staging::EntityFields::Place(place) => EntityFields::Place(StagedPlace {
            coordinates: place.coordinates.and_then(|point| to_coordinates(&place.name, point)),
            name: place.name,
            place_type: place.place_type.map(to_place_type),
            restrictions: to_restrictions(place.restrictions),
        }),
        staging::EntityFields::Source(source) => EntityFields::Source(StagedSource {
            title: source.title,
            author: source.author,
            pub_info: source.pub_info,
            abbrev: source.abbrev,
            restrictions: to_restrictions(source.restrictions),
        }),
        staging::EntityFields::Citation(citation) => EntityFields::Citation(StagedCitation {
            source: to_ref(citation.source),
            page: citation.page,
            confidence: citation.confidence.map(to_confidence),
            restrictions: to_restrictions(citation.restrictions),
        }),
        staging::EntityFields::Media(media) => EntityFields::Media(StagedMedia {
            path: media.path,
            mime: media.mime,
            restrictions: to_restrictions(media.restrictions),
        }),
        staging::EntityFields::Note(note) => EntityFields::Note(StagedNote {
            text: note.text,
            note_type: note.note_type.map(to_note_type),
            restrictions: to_restrictions(note.restrictions),
        }),
        staging::EntityFields::Repository(repository) => EntityFields::Repository(StagedRepository {
            name: repository.name,
            restrictions: to_restrictions(repository.restrictions),
        }),
        staging::EntityFields::Tag(tag) => EntityFields::Tag(StagedTag { name: tag.name }),
    };
    StagedEntity {
        local_id: entity.local_id,
        item: entity.item,
        fields,
    }
}

/// The point a guest staged for the place `name`, or `None`, with a warning, when it is not one: the
/// importers parse only valid points, so this guards against a guest that does not.
fn to_coordinates(name: &str, point: types::Coordinates) -> Option<GeoCoordinates> {
    let coordinates = GeoCoordinates::from_degrees(point.latitude, point.longitude);
    if coordinates.is_none() {
        tracing::warn!(
            place = name,
            latitude = point.latitude,
            longitude = point.longitude,
            "dropping a staged place's point that is not finite degrees in range"
        );
    }
    coordinates
}

fn to_fact(fact: types::Fact) -> NewFact {
    NewFact {
        fact_type: to_fact_type(fact.fact_type),
        value: fact.value,
        date: fact.date.map(to_genealogical_date),
    }
}

pub(crate) fn to_kind(kind: staging::EntityKind) -> MatchableKind {
    match kind {
        staging::EntityKind::Person => MatchableKind::Person,
        staging::EntityKind::Family => MatchableKind::Family,
        staging::EntityKind::Event => MatchableKind::Event,
        staging::EntityKind::Place => MatchableKind::Place,
        staging::EntityKind::Source => MatchableKind::Source,
        staging::EntityKind::Citation => MatchableKind::Citation,
        staging::EntityKind::Media => MatchableKind::Media,
        staging::EntityKind::Note => MatchableKind::Note,
        staging::EntityKind::Repository => MatchableKind::Repository,
        staging::EntityKind::Tag => MatchableKind::Tag,
    }
}

fn to_ref(reference: staging::EntityRef) -> EntityRef {
    match reference {
        staging::EntityRef::Local(local_id) => EntityRef::Local(local_id),
        staging::EntityRef::Origin(origin) => EntityRef::Origin {
            kind: to_kind(origin.kind),
            record: origin.key.record,
            item: origin.key.item,
        },
    }
}

fn to_refs(references: Vec<staging::EntityRef>) -> Vec<EntityRef> {
    references.into_iter().map(to_ref).collect()
}

fn to_link(link: staging::StagedLink) -> StagedLink {
    let kind = match link.link {
        staging::LinkKind::Participation(participation) => LinkKind::Participation {
            person: to_ref(participation.person),
            event: to_ref(participation.event),
            role: to_role(participation.role),
            age: participation.age.map(to_age),
            attributes: participation.attributes.into_iter().map(to_attribute).collect(),
            notes: to_refs(participation.notes),
            citations: to_refs(participation.citations),
        },
        staging::LinkKind::Partner(member) => LinkKind::Partner {
            family: to_ref(member.family),
            person: to_ref(member.person),
        },
        staging::LinkKind::Child(child) => LinkKind::Child {
            family: to_ref(child.family),
            child: to_ref(child.child),
            relationships: child
                .relationships
                .into_iter()
                .map(|rel| (to_ref(rel.partner), to_child_relationship(&rel.relationship)))
                .collect(),
        },
        staging::LinkKind::FamilyEvent(pair) => LinkKind::FamilyEvent {
            family: to_ref(pair.owner),
            event: to_ref(pair.target),
        },
        staging::LinkKind::EventPlace(pair) => LinkKind::EventPlace {
            event: to_ref(pair.owner),
            place: to_ref(pair.target),
        },
        staging::LinkKind::Enclosure(pair) => LinkKind::Enclosure {
            place: to_ref(pair.owner),
            enclosing: to_ref(pair.target),
        },
        staging::LinkKind::CitationOf(pair) => LinkKind::CitationOf {
            owner: to_ref(pair.owner),
            citation: to_ref(pair.target),
        },
        staging::LinkKind::MediaOf(media) => {
            let input = media_ref_input(media.crop, media.caption);
            LinkKind::MediaOf {
                owner: to_ref(media.owner),
                media: to_ref(media.media),
                crop: input.crop,
                caption: input.caption,
            }
        }
        staging::LinkKind::NoteOf(pair) => LinkKind::NoteOf {
            owner: to_ref(pair.owner),
            note: to_ref(pair.target),
        },
        staging::LinkKind::TagOf(pair) => LinkKind::TagOf {
            owner: to_ref(pair.owner),
            tag: to_ref(pair.target),
        },
        staging::LinkKind::SourceRepository(link) => LinkKind::SourceRepository {
            source: to_ref(link.source),
            repository: to_ref(link.repository),
            call_number: link.call_number,
            media_type: to_source_media_type(link.media_type),
        },
        staging::LinkKind::Association(association) => LinkKind::Association {
            person: to_ref(association.person),
            other: to_ref(association.other),
            role: to_association_role(association.role),
        },
    };
    StagedLink {
        item: link.item,
        link: kind,
    }
}
