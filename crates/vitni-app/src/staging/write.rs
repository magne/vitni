//! The writes one staged entity or link turns into: the existing use-cases, each stamped with the
//! entity's or link's origin (ADR 0040 §5).
//!
//! A plan's dry run and its commit issue the same writes, so what the dry run reports is what the
//! commit writes. A new entity's create carries its first value (a person's first name, a place's
//! name, a source's title); an existing entity re-asserts that value instead, which the origin gate
//! makes a no-op when it is already on record from the same item.

use vitni_core::enums::{EvidenceLevel, PlaceType};
use vitni_core::ids::ImportRunId;
use vitni_core::matching::MatchableKind;
use vitni_core::origin::{DatasetId, RecordOrigin};
use vitni_core::provenance::Timestamp;

use crate::error::AppError;
use crate::event::{ImportedMediaRef, NewEvent};
use crate::import::ImportedChild;
use crate::person::{NewParticipation, NewPerson};
use crate::session::Session;
use crate::staging::graph::{EntityFields, EntityRef, LinkKind, StagedEntity, StagedPerson};
use crate::use_case::{MediaRefInput, MutationMeta, Provenance};
use crate::workspace::Workspace;
use crate::{
    NewCitation, NewMedia, NewNote, NewPlace, NewRepository, NewSource, citation, event, family, import, media, note,
    person, place, repository, source, tag,
};

/// Issues the writes of a plan's entities and links, as one session.
pub(crate) struct Writer<'a> {
    pub workspace: &'a Workspace,
    pub session: &'a Session,
    /// The confidence and other provenance every write carries, without its origin.
    pub template: &'a Provenance,
    /// The dataset and run origins are stamped with; `None` outside an import run.
    pub run: Option<(&'a DatasetId, ImportRunId)>,
    /// The document's own export date (ADR 0029 §2).
    pub file_asserted_at: Option<Timestamp>,
}

/// The human id each reference of one write resolves to.
pub(crate) trait Resolve {
    /// The human id (a tag's id) `reference` names, or `None` when it names nothing written.
    fn human_id(&self, reference: &EntityRef) -> Option<String>;
    /// The kind of the record `reference` names, if it names one.
    fn kind(&self, reference: &EntityRef) -> Option<MatchableKind>;
}

/// The kinds of record a citation, media object, note or tag is attached to.
#[derive(Clone, Copy)]
enum Owner {
    Person,
    Family,
    Event,
    Citation,
}

/// The owner kind of the record `reference` names; a reference to anything else is dangling, as
/// validation never lets one through.
fn owner_of(reference: &EntityRef, resolve: &dyn Resolve) -> Result<Owner, LinkError> {
    match resolve.kind(reference) {
        Some(MatchableKind::Person) => Ok(Owner::Person),
        Some(MatchableKind::Family) => Ok(Owner::Family),
        Some(MatchableKind::Event) => Ok(Owner::Event),
        Some(MatchableKind::Citation) => Ok(Owner::Citation),
        Some(
            MatchableKind::Place
            | MatchableKind::Source
            | MatchableKind::Repository
            | MatchableKind::Media
            | MatchableKind::Note
            | MatchableKind::Tag,
        )
        | None => Err(LinkError::Dangling),
    }
}

/// Why a link was not written.
#[derive(Debug)]
pub(crate) enum LinkError {
    /// An end names nothing written.
    Dangling,
    /// The write failed.
    App(AppError),
}

impl From<AppError> for LinkError {
    fn from(error: AppError) -> Self {
        Self::App(error)
    }
}

impl Writer<'_> {
    /// The provenance of a write from `item` of `record`.
    fn provenance(&self, record: &str, item: Option<&str>) -> Provenance {
        let origin = self.run.map(|(dataset, run)| RecordOrigin {
            dataset: dataset.clone(),
            record: record.to_owned(),
            item: item.map(str::to_owned),
            digest: None,
            run,
        });
        Provenance {
            origin,
            ..self.template.clone()
        }
    }

    fn meta(&self, record: &str, item: Option<&str>) -> MutationMeta<'static> {
        MutationMeta {
            provenance: self.provenance(record, item),
            ..MutationMeta::default()
        }
    }

    /// Creates `entity` from `record`, then writes the rest of its fields; returns its human id (a
    /// tag's id), or `None` for a citation whose source names nothing written.
    pub(crate) async fn create(
        &self,
        record: &str,
        entity: &StagedEntity,
        resolve: &dyn Resolve,
    ) -> Result<Option<String>, AppError> {
        let (ws, session) = (self.workspace, self.session);
        let item = entity.item.as_deref();
        let provenance = self.provenance(record, item);
        let human_id = match &entity.fields {
            EntityFields::Person(fields) => {
                let new = NewPerson {
                    human_id: None,
                    name: fields.names.first().cloned(),
                    evidence_level: EvidenceLevel::Persona,
                    external_ids: fields.external_ids.clone(),
                };
                person::create_person(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Family(fields) if fields.external_ids.is_empty() => {
                family::create_family(ws, session, provenance, &[]).await?
            }
            EntityFields::Family(fields) => {
                let ids = fields.external_ids.clone();
                family::create_family_with_external_ids(ws, session, ids, provenance, &[]).await?
            }
            EntityFields::Event(fields) => {
                let new = NewEvent {
                    human_id: None,
                    event_type: fields.event_type.clone(),
                };
                event::create_event(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Place(fields) => {
                let new = NewPlace {
                    human_id: None,
                    // The type, when the record states one, is its own assertion below.
                    place_type: PlaceType::Custom("place".to_owned()),
                    name: Some(fields.name.clone()),
                };
                place::create_place(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Source(fields) => {
                let new = NewSource {
                    human_id: None,
                    title: fields.title.clone(),
                };
                source::create_source(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Citation(fields) => {
                let Some(source) = resolve.human_id(&fields.source) else {
                    return Ok(None);
                };
                let new = NewCitation {
                    human_id: None,
                    source,
                    page: fields.page.clone(),
                };
                citation::create_citation(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Media(fields) => {
                let new = NewMedia {
                    human_id: None,
                    path: fields.path.clone(),
                };
                media::create_media(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Note(fields) => {
                let new = NewNote {
                    human_id: None,
                    text: Some(fields.text.clone()),
                };
                note::create_note(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Repository(fields) => {
                let new = NewRepository {
                    human_id: None,
                    name: Some(fields.name.clone()),
                };
                repository::create_repository(ws, session, new, provenance, &[]).await?
            }
            EntityFields::Tag(fields) => tag::create_tag(ws, session, fields.name.clone(), provenance, &[]).await?,
        };
        self.fields(record, entity, &human_id).await?;
        Ok(Some(human_id))
    }

    /// Writes `entity`'s fields onto the existing record `human_id`: the value its create would carry,
    /// then the rest.
    pub(crate) async fn update(&self, record: &str, entity: &StagedEntity, human_id: &str) -> Result<(), AppError> {
        let (ws, session) = (self.workspace, self.session);
        let meta = || self.meta(record, entity.item.as_deref());
        match &entity.fields {
            EntityFields::Person(fields) => {
                if let Some(name) = fields.names.first() {
                    person::add_name(ws, session, human_id, name.clone(), meta()).await?;
                }
            }
            EntityFields::Place(fields) => {
                place::add_place_name(ws, session, human_id, fields.name.clone(), meta()).await?;
            }
            EntityFields::Source(fields) => {
                if let Some(title) = &fields.title {
                    source::set_title(ws, session, human_id, title.clone(), meta()).await?;
                }
            }
            EntityFields::Citation(fields) => {
                if let Some(page) = &fields.page {
                    citation::set_page(ws, session, human_id, page.clone(), meta()).await?;
                }
            }
            EntityFields::Note(fields) => {
                note::set_note_text(ws, session, human_id, fields.text.clone(), None, meta()).await?;
            }
            EntityFields::Repository(fields) => {
                repository::set_repository_name(ws, session, human_id, fields.name.clone(), meta()).await?;
            }
            EntityFields::Family(_) | EntityFields::Event(_) | EntityFields::Media(_) | EntityFields::Tag(_) => {}
        }
        self.fields(record, entity, human_id).await
    }

    /// The writes a person resolved onto another dataset's record still makes (ADR 0037 §4): its first
    /// name when that name is not already recorded, and its sex, reconciled by the file's date.
    pub(crate) async fn identity(&self, record: &str, entity: &StagedEntity, human_id: &str) -> Result<(), AppError> {
        let EntityFields::Person(fields) = &entity.fields else {
            return Ok(());
        };
        let provenance = self.provenance(record, entity.item.as_deref());
        if let Some(name) = fields.names.first() {
            import::ensure_name_of(self.workspace, self.session, human_id, name.clone(), provenance.clone()).await?;
        }
        self.sex(fields, human_id, provenance).await
    }

    async fn sex(&self, fields: &StagedPerson, human_id: &str, provenance: Provenance) -> Result<(), AppError> {
        let Some(sex) = fields.sex.clone() else {
            return Ok(());
        };
        let (ws, session) = (self.workspace, self.session);
        import::import_assert_sex(ws, session, human_id, sex, self.file_asserted_at, provenance).await
    }

    /// Every field of `entity` its create does not carry.
    async fn fields(&self, record: &str, entity: &StagedEntity, human_id: &str) -> Result<(), AppError> {
        let (ws, session) = (self.workspace, self.session);
        let item = entity.item.as_deref();
        let meta = || self.meta(record, item);
        match &entity.fields {
            EntityFields::Person(fields) => {
                for name in fields.names.iter().skip(1) {
                    person::add_name(ws, session, human_id, name.clone(), meta()).await?;
                }
                self.sex(fields, human_id, self.provenance(record, item)).await?;
                for fact in &fields.facts {
                    person::assert_fact(ws, session, human_id, fact.clone(), meta()).await?;
                }
                if !fields.restrictions.is_empty() {
                    person::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Family(fields) => {
                if !fields.restrictions.is_empty() {
                    family::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Event(fields) => {
                if let Some(date) = &fields.date {
                    event::assert_event_date_value(ws, session, human_id, date.clone(), meta()).await?;
                }
                for address in &fields.addresses {
                    event::assert_event_address(ws, session, human_id, address.clone(), meta()).await?;
                }
                if !fields.restrictions.is_empty() {
                    event::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Place(fields) => {
                if let Some(place_type) = &fields.place_type {
                    place::set_place_type(ws, session, human_id, place_type.clone(), meta()).await?;
                }
                if !fields.restrictions.is_empty() {
                    place::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Source(fields) => {
                if let Some(author) = &fields.author {
                    source::set_source_author(ws, session, human_id, author.clone(), meta()).await?;
                }
                if let Some(pub_info) = &fields.pub_info {
                    source::set_source_pub_info(ws, session, human_id, pub_info.clone(), meta()).await?;
                }
                if let Some(abbrev) = &fields.abbrev {
                    source::set_source_abbrev(ws, session, human_id, abbrev.clone(), meta()).await?;
                }
                if !fields.restrictions.is_empty() {
                    source::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Citation(fields) => {
                if let Some(confidence) = fields.confidence {
                    citation::set_citation_confidence(ws, session, human_id, confidence, meta()).await?;
                }
                if !fields.restrictions.is_empty() {
                    citation::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Media(fields) => {
                if let Some(mime) = &fields.mime {
                    media::set_media_mime(ws, session, human_id, mime.clone(), meta()).await?;
                }
                if !fields.restrictions.is_empty() {
                    media::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Note(fields) => {
                if let Some(note_type) = &fields.note_type {
                    note::set_note_type(ws, session, human_id, note_type.clone(), meta()).await?;
                }
                if !fields.restrictions.is_empty() {
                    note::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Repository(fields) => {
                if !fields.restrictions.is_empty() {
                    repository::set_restrictions(ws, session, human_id, fields.restrictions.clone(), meta()).await?;
                }
            }
            EntityFields::Tag(_) => {}
        }
        Ok(())
    }

    /// Writes one link from `item` of `record`.
    pub(crate) async fn link(
        &self,
        record: &str,
        item: Option<&str>,
        link: &LinkKind,
        resolve: &dyn Resolve,
    ) -> Result<(), LinkError> {
        let (ws, session) = (self.workspace, self.session);
        let end = |reference: &EntityRef| resolve.human_id(reference).ok_or(LinkError::Dangling);
        let owner = end(link.owner().reference)?;
        let meta = || self.meta(record, item);
        let provenance = || self.provenance(record, item);
        match link {
            LinkKind::Participation {
                event,
                role,
                age,
                attributes,
                notes,
                citations,
                ..
            } => {
                let event = end(event)?;
                let notes = notes.iter().map(end).collect::<Result<Vec<_>, _>>()?;
                let citations = citations.iter().map(end).collect::<Result<Vec<_>, _>>()?;
                let new = NewParticipation {
                    role: role.clone(),
                    age: age.clone(),
                    attributes: attributes.clone(),
                    notes,
                };
                let meta = MutationMeta {
                    citations: &citations,
                    ..meta()
                };
                person::assert_participation(ws, session, &owner, &event, new, meta).await?;
            }
            LinkKind::Partner { person, .. } => {
                import::import_add_partner(ws, session, &owner, &end(person)?, provenance()).await?;
            }
            LinkKind::Child {
                child, relationships, ..
            } => {
                let mut resolved = Vec::with_capacity(relationships.len());
                for (partner, relationship) in relationships {
                    resolved.push((end(partner)?, relationship.clone()));
                }
                let child = ImportedChild {
                    human_id: end(child)?,
                    relationships: resolved,
                };
                import::import_add_child(ws, session, &owner, child, provenance()).await?;
            }
            LinkKind::FamilyEvent { event, .. } => {
                family::link_family_event(ws, session, &owner, &end(event)?, meta()).await?;
            }
            LinkKind::EventPlace { place, .. } => event::link_place(ws, session, &owner, &end(place)?, meta()).await?,
            LinkKind::Enclosure { enclosing, .. } => {
                place::assert_place_enclosed_by(ws, session, &owner, &end(enclosing)?, meta()).await?;
            }
            LinkKind::CitationOf { .. }
            | LinkKind::MediaOf { .. }
            | LinkKind::NoteOf { .. }
            | LinkKind::TagOf { .. } => {
                self.attach(record, item, link, &owner, resolve).await?;
            }
            LinkKind::SourceRepository {
                repository,
                call_number,
                media_type,
                ..
            } => {
                let repository = end(repository)?;
                let (call_number, media_type) = (call_number.clone(), media_type.clone());
                source::link_source_repository(ws, session, &owner, &repository, call_number, media_type, meta())
                    .await?;
            }
            LinkKind::Association { other, role, .. } => {
                person::assert_association(ws, session, &owner, &end(other)?, role.clone(), meta()).await?;
            }
        }
        Ok(())
    }

    /// Writes one attachment link — a citation, media object, note or tag on `owner` (a human id).
    async fn attach(
        &self,
        record: &str,
        item: Option<&str>,
        link: &LinkKind,
        owner: &str,
        resolve: &dyn Resolve,
    ) -> Result<(), LinkError> {
        let (ws, session) = (self.workspace, self.session);
        let end = |reference: &EntityRef| resolve.human_id(reference).ok_or(LinkError::Dangling);
        let meta = || self.meta(record, item);
        let provenance = || self.provenance(record, item);
        match link {
            LinkKind::CitationOf { owner: of, citation } => {
                let citation = end(citation)?;
                match owner_of(of, resolve)? {
                    Owner::Family => family::add_family_citation(ws, session, owner, &citation, meta()).await?,
                    Owner::Event => event::add_event_citation(ws, session, owner, &citation, meta()).await?,
                    Owner::Person => person::add_person_citation(ws, session, owner, &citation, meta()).await?,
                    Owner::Citation => return Err(LinkError::Dangling),
                }
            }
            LinkKind::MediaOf {
                owner: of,
                media,
                crop,
                caption,
            } => {
                let media = end(media)?;
                let input = MediaRefInput {
                    crop: *crop,
                    caption: caption.clone(),
                };
                match owner_of(of, resolve)? {
                    Owner::Family => family::attach_family_media(ws, session, owner, &media, input, meta()).await?,
                    Owner::Event => {
                        let media = ImportedMediaRef {
                            media_human_id: media,
                            input,
                        };
                        event::import_attach_event_media(ws, session, owner, media, provenance()).await?;
                    }
                    Owner::Person => person::attach_person_media(ws, session, owner, &media, input, meta()).await?,
                    Owner::Citation => return Err(LinkError::Dangling),
                }
            }
            LinkKind::NoteOf { owner: of, note } => {
                let note = end(note)?;
                match owner_of(of, resolve)? {
                    Owner::Family => family::attach_family_note(ws, session, owner, &note, meta()).await?,
                    Owner::Event => event::import_attach_event_note(ws, session, owner, &note, provenance()).await?,
                    Owner::Citation => citation::attach_citation_note(ws, session, owner, &note, meta()).await?,
                    Owner::Person => person::attach_person_note(ws, session, owner, &note, meta()).await?,
                }
            }
            LinkKind::TagOf { owner: of, tag } => {
                let tag = end(tag)?;
                match owner_of(of, resolve)? {
                    Owner::Family => family::tag_family(ws, session, owner, &tag, false, meta()).await?,
                    Owner::Event => event::tag_event(ws, session, owner, &tag, false, meta()).await?,
                    Owner::Person => person::tag_person(ws, session, owner, &tag, false, meta()).await?,
                    Owner::Citation => return Err(LinkError::Dangling),
                }
            }
            LinkKind::Participation { .. }
            | LinkKind::Partner { .. }
            | LinkKind::Child { .. }
            | LinkKind::FamilyEvent { .. }
            | LinkKind::EventPlace { .. }
            | LinkKind::Enclosure { .. }
            | LinkKind::SourceRepository { .. }
            | LinkKind::Association { .. } => {}
        }
        Ok(())
    }
}
