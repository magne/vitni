//! The `staging` capability (ADR 0040 §1, §4): an importer submits one record graph per source record,
//! and the host plans and writes them through `vitni-app`.
//!
//! A bulk import's graphs are held until the guest returns, then planned as one and committed, with
//! the commit's progress reported to the frontend — whose cancel stops it between two writes. An
//! assisted import's graph is planned and committed on each `submit`, and the guest learns what each
//! entity became.

use vitni_app::{
    CommitControl, CommitOutcome, EntityFields, EntityRef, LinkKind, NewFact, PlanError, RecordGraph, RunToEnd,
    StagedCitation, StagedEntity, StagedEvent, StagedFamily, StagedLink, StagedMedia, StagedNote, StagedPerson,
    StagedPlace, StagedRepository, StagedSource, StagedTag, Timestamp, commit_import, plan_import,
};
use vitni_core::matching::MatchableKind;

use crate::bindings::imports::vitni::host_api::{staging, types};
use crate::capability::Capability;
use crate::error::PluginError;
use crate::run::ActiveRun;
use crate::state::{
    HostState, Staging, media_ref_input, to_address, to_age, to_association_role, to_attribute, to_capability_error,
    to_child_relationship, to_confidence, to_event_type, to_external_id, to_fact_type, to_genealogical_date,
    to_note_type, to_person_name, to_place_type, to_restrictions, to_role, to_sex, to_source_media_type,
};
use crate::{ProgressControl, ProgressUpdate};

/// How many writes pass between two progress reports of a bulk commit.
const REPORT_EVERY: u32 = 10;

impl staging::Host for HostState {
    fn begin_run(
        &mut self,
        dataset_hint: Option<String>,
        source_label: Option<String>,
        file_asserted_at: Option<String>,
    ) -> impl Future<Output = Result<(), types::CapabilityError>> {
        std::future::ready(self.declare_run(dataset_hint.as_deref(), source_label.as_deref(), file_asserted_at))
    }

    async fn submit(&mut self, graph: staging::RecordGraph) -> Result<staging::SubmitOutcome, types::CapabilityError> {
        if !self.grants.allows(Capability::Commands) {
            return Err(types::CapabilityError::Denied);
        }
        let graph = to_graph(graph);
        graph
            .validate()
            .map_err(|error| types::CapabilityError::InvalidInput(error.to_string()))?;
        match self.staging {
            Staging::Held => {
                self.staged.push(graph);
                Ok(staging::SubmitOutcome::Staged)
            }
            Staging::Immediate => self.commit_one(graph).await,
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
        // A missing or unparseable date degrades to `None` — the conservative, additive-only default
        // (ADR 0029 §3): a malformed date is the guest's format-parsing problem, not a capability
        // violation.
        self.file_asserted_at = file_asserted_at.and_then(|value| Timestamp::parse_rfc3339(&value));
        if let Some(run) = &self.run {
            run.pending().set_file_asserted_at(self.file_asserted_at);
        }
        Ok(())
    }

    /// Plans and commits one graph at once, returning what each of its entities became.
    async fn commit_one(&mut self, graph: RecordGraph) -> Result<staging::SubmitOutcome, types::CapabilityError> {
        let template = self.provenance();
        let (workspace, session) = (&self.workspace, &self.session);
        let plan = plan_import(workspace, session, vec![graph], self.file_asserted_at)
            .await
            .map_err(|error| plan_capability_error(&error))?;
        match commit_import(workspace, session, &plan, &template, &mut RunToEnd).await {
            Ok(outcome) => {
                self.absorb(&outcome);
                Ok(staging::SubmitOutcome::Committed(committed(&outcome)))
            }
            Err(failure) => {
                self.absorb(&failure.outcome);
                Err(to_capability_error(&failure.error))
            }
        }
    }

    /// Plans every graph the guest submitted and commits the plan, reporting progress and stopping
    /// when the frontend cancels. Nothing is written when the guest was cancelled while parsing.
    ///
    /// # Errors
    ///
    /// [`PluginError::Guest`] when the graphs cannot be planned together (two stage one origin), or
    /// [`PluginError::Commit`] when the workspace cannot be read or a write fails.
    pub(crate) async fn commit_staged(&mut self) -> Result<(), PluginError> {
        let graphs = std::mem::take(&mut self.staged);
        if graphs.is_empty() || self.run.as_ref().is_some_and(ActiveRun::cancelled) {
            return Ok(());
        }
        let template = self.provenance();
        let (workspace, session, progress) = (&self.workspace, &self.session, &mut self.io.progress);
        let file_asserted_at = self.file_asserted_at;
        let plan = plan_import(workspace, session, graphs, file_asserted_at)
            .await
            .map_err(|error| match error {
                PlanError::Graph(error) => PluginError::Guest(error.to_string()),
                PlanError::App(error) => PluginError::Commit(error.to_string()),
            })?;
        let mut control = Reporter {
            progress,
            cancelled: false,
        };
        let result = commit_import(workspace, session, &plan, &template, &mut control).await;
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
            step: "writing".to_owned(),
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

/// The capability error a guest sees when its graph cannot be planned.
fn plan_capability_error(error: &PlanError) -> types::CapabilityError {
    match error {
        PlanError::Graph(error) => types::CapabilityError::InvalidInput(error.to_string()),
        PlanError::App(error) => to_capability_error(error),
    }
}

/// What a committed graph's entities became.
fn committed(outcome: &CommitOutcome) -> Vec<staging::CommittedEntity> {
    outcome
        .entities
        .iter()
        .map(|entity| staging::CommittedEntity {
            local_id: entity.local_id,
            human_id: entity.human_id.clone(),
            created: entity.created,
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

fn to_fact(fact: types::Fact) -> NewFact {
    NewFact {
        fact_type: to_fact_type(fact.fact_type),
        value: fact.value,
        date: fact.date.map(to_genealogical_date),
    }
}

fn to_kind(kind: staging::EntityKind) -> MatchableKind {
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
        staging::EntityRef::Existing(existing) => EntityRef::Existing {
            kind: to_kind(existing.kind),
            human_id: existing.human_id,
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
