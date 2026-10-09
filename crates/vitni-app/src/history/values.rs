//! The value a change-log entry recorded, and on a correction the value it replaced (#545).
//!
//! Each aggregate's event payload is decoded into a frontend-neutral [`ChangeValue`]: a scalar the
//! frontend formats with its own labels, or a reference to another record, labelled here by one pass
//! over the store per change log.

use std::collections::HashMap;

use vitni_core::address::Address;
use vitni_core::citation::CitationEventBody;
use vitni_core::date::GenealogicalDate;
use vitni_core::dna::{Centimorgans, DnaGenomeBuild, DnaProvider, DnaTestType};
use vitni_core::dna_match::DnaMatchEventBody;
use vitni_core::dna_test::DnaTestEventBody;
use vitni_core::enums::{
    AssociationRole, ChildParentRelationship, EventType, NoteType, ParticipantRole, PlaceType, RepositoryType,
    Restriction, Sex,
};
use vitni_core::event::EventEventBody;
use vitni_core::family::FamilyEventBody;
use vitni_core::media::MediaEventBody;
use vitni_core::note::NoteEventBody;
use vitni_core::person::event::PersonEventBody;
use vitni_core::place::PlaceEventBody;
use vitni_core::provenance::{Confidence, EvidenceAnalysis};
use vitni_core::repository::RepositoryEventBody;
use vitni_core::research_note::{ResearchNoteEventBody, SubjectRef};
use vitni_core::source::SourceEventBody;
use vitni_core::tag::TagEventBody;
use vitni_core::text::{Attribute, ExternalId};
use vitni_db::StoredEvent;

use crate::error::AppError;
use crate::workspace::Workspace;

use super::ChangeLogEntry;

/// What one change set: the value it asserted, in a shape the frontend formats with its own labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeValue {
    /// Free text as written: a title, a name, a page, a colour, a checksum.
    Text(String),
    /// A genealogical date.
    Date(GenealogicalDate),
    /// A person's sex.
    Sex(Sex),
    /// An event's type.
    EventType(EventType),
    /// A place's type.
    PlaceType(PlaceType),
    /// A note's type.
    NoteType(NoteType),
    /// A repository's type.
    RepositoryType(RepositoryType),
    /// A DNA test's provider.
    DnaProvider(DnaProvider),
    /// A DNA test's type.
    DnaTestType(DnaTestType),
    /// A DNA test's genome build.
    DnaGenomeBuild(DnaGenomeBuild),
    /// A citation's surety.
    Confidence(Confidence),
    /// A record's privacy restrictions; empty when they were cleared.
    Restrictions(Vec<Restriction>),
    /// A whole number (a tag's priority).
    Number(i64),
    /// A citation's evidence analysis.
    EvidenceAxes(EvidenceAnalysis),
    /// A DNA match's observed totals.
    DnaObserved {
        /// The shared centimorgans.
        shared_cm: Centimorgans,
        /// The number of shared segments.
        segment_count: u32,
    },
    /// Another record the change linked, with the role it was linked in, if any.
    Record {
        /// The linked record.
        record: RecordValue,
        /// The role the record holds in the link (a participant's role, a child's relationship).
        role: Option<RecordRole>,
    },
}

/// A record a change linked, labelled for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordValue {
    /// The record's aggregate kind (`place`, `person`, `tag`, …).
    pub kind: String,
    /// The record's aggregate id. Never shown; the key [`label_records`] resolves the rest by.
    pub id: String,
    /// The record's user-facing id; `None` for a tag, which has none.
    pub human_id: Option<String>,
    /// The record's display label (a name, a title), when its kind has one.
    pub label: Option<String>,
}

/// The role a linked record holds in the link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordRole {
    /// A person's role in an event.
    Participant(ParticipantRole),
    /// The relationship an association asserts.
    Association(AssociationRole),
    /// A child's relationship to one parent.
    Child(ChildParentRelationship),
}

/// The value `event` asserted, or `None` for a change that carries none (a creation, a merge, a
/// correction) or one too large for a line (a geometry, a segment).
pub(super) fn extract_value(event: &StoredEvent) -> Option<ChangeValue> {
    let payload = &event.payload;
    match event.aggregate_type.as_str() {
        "person" => person_value(serde_json::from_str(payload).ok()?),
        "family" => family_value(serde_json::from_str(payload).ok()?),
        "event" => event_value(serde_json::from_str(payload).ok()?),
        "place" => place_value(serde_json::from_str(payload).ok()?),
        "source" => source_value(serde_json::from_str(payload).ok()?),
        "citation" => citation_value(serde_json::from_str(payload).ok()?),
        "repository" => repository_value(serde_json::from_str(payload).ok()?),
        "media" => media_value(serde_json::from_str(payload).ok()?),
        "note" => note_value(serde_json::from_str(payload).ok()?),
        "tag" => tag_value(serde_json::from_str(payload).ok()?),
        "dna_test" => dna_test_value(serde_json::from_str(payload).ok()?),
        "dna_match" => dna_match_value(serde_json::from_str(payload).ok()?),
        "research_note" => research_note_value(serde_json::from_str(payload).ok()?),
        _ => None,
    }
}

fn person_value(body: PersonEventBody) -> Option<ChangeValue> {
    match body {
        PersonEventBody::NameAsserted { name, .. } => text(crate::person::render_name(&name)),
        PersonEventBody::SexAsserted { sex, .. } => Some(ChangeValue::Sex(sex)),
        PersonEventBody::FactAsserted { fact, .. } => match (fact.value, fact.date, fact.place_id) {
            (Some(value), _, _) => text(value),
            (None, Some(date), _) => Some(ChangeValue::Date(date)),
            (None, None, Some(place)) => Some(record("place", &place)),
            (None, None, None) => None,
        },
        PersonEventBody::ParticipationAsserted { event_id, role, .. } => {
            Some(record_as("event", &event_id, RecordRole::Participant(role)))
        }
        PersonEventBody::AssociationAsserted { other, role, .. } => {
            Some(record_as("person", &other, RecordRole::Association(role)))
        }
        PersonEventBody::MediaAttached { media, .. } => Some(record("media", &media.media_id)),
        PersonEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        PersonEventBody::CitationAdded { citation_id, .. } => Some(record("citation", &citation_id)),
        PersonEventBody::ExternalIdAdded { external_id, .. } => external_id_text(&external_id),
        PersonEventBody::Tagged { tag_id, .. } | PersonEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        PersonEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        PersonEventBody::PersonCreated { .. }
        | PersonEventBody::HumanIdChanged { .. }
        | PersonEventBody::AssertionRetracted { .. }
        | PersonEventBody::AssertionSuperseded { .. }
        | PersonEventBody::PersonsMerged { .. }
        | PersonEventBody::PersonsDistinguished { .. } => None,
    }
}

fn family_value(body: FamilyEventBody) -> Option<ChangeValue> {
    match body {
        FamilyEventBody::PartnerAdded { person_id, .. } | FamilyEventBody::PartnerRemoved { person_id, .. } => {
            Some(record("person", &person_id))
        }
        FamilyEventBody::ChildAdded { child_id, .. } | FamilyEventBody::ChildRemoved { child_id, .. } => {
            Some(record("person", &child_id))
        }
        FamilyEventBody::ChildRelationshipAsserted {
            child_id, relationship, ..
        } => Some(record_as("person", &child_id, RecordRole::Child(relationship))),
        FamilyEventBody::CitationAdded { citation_id, .. } => Some(record("citation", &citation_id)),
        FamilyEventBody::FamilyEventLinked { event_id, .. } => Some(record("event", &event_id)),
        FamilyEventBody::MediaAttached { media, .. } => Some(record("media", &media.media_id)),
        FamilyEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        FamilyEventBody::Tagged { tag_id, .. } | FamilyEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        FamilyEventBody::ExternalIdAdded { external_id, .. } => external_id_text(&external_id),
        FamilyEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        FamilyEventBody::FamilyCreated { .. }
        | FamilyEventBody::AssertionRetracted { .. }
        | FamilyEventBody::AssertionSuperseded { .. }
        | FamilyEventBody::HumanIdChanged { .. }
        | FamilyEventBody::FamiliesMerged { .. }
        | FamilyEventBody::FamiliesDistinguished { .. } => None,
    }
}

fn event_value(body: EventEventBody) -> Option<ChangeValue> {
    match body {
        EventEventBody::EventCreated { event_type, .. } | EventEventBody::EventTypeSet { event_type, .. } => {
            Some(ChangeValue::EventType(event_type))
        }
        EventEventBody::DateAsserted { date, .. } => Some(ChangeValue::Date(date)),
        EventEventBody::DescriptionSet { description, .. } => text(description),
        EventEventBody::PlaceLinked { place_id, .. } => Some(record("place", &place_id)),
        EventEventBody::AddressAdded { address, .. } => address_text(&address),
        EventEventBody::CitationAdded { citation_id, .. } => Some(record("citation", &citation_id)),
        EventEventBody::MediaAttached { media, .. } => Some(record("media", &media.media_id)),
        EventEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        EventEventBody::Tagged { tag_id, .. } | EventEventBody::Untagged { tag_id, .. } => Some(record("tag", &tag_id)),
        EventEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        EventEventBody::AssertionRetracted { .. }
        | EventEventBody::AssertionSuperseded { .. }
        | EventEventBody::HumanIdChanged { .. }
        | EventEventBody::EventsMerged { .. }
        | EventEventBody::EventsDistinguished { .. } => None,
    }
}

fn place_value(body: PlaceEventBody) -> Option<ChangeValue> {
    match body {
        PlaceEventBody::PlaceCreated { place_type, .. } | PlaceEventBody::PlaceTypeSet { place_type, .. } => {
            Some(ChangeValue::PlaceType(place_type))
        }
        PlaceEventBody::NameAsserted { name, .. } => text(name.text),
        PlaceEventBody::EnclosedByAsserted { enclosed_by, .. } => Some(record("place", &enclosed_by.place_id)),
        PlaceEventBody::CoordinatesAsserted { coordinates, .. } => text(format!(
            "{:.4}, {:.4}",
            coordinates.latitude.to_degrees(),
            coordinates.longitude.to_degrees()
        )),
        PlaceEventBody::CodeSet { code, .. } => text(code),
        PlaceEventBody::CitationAdded { citation_id, .. } => Some(record("citation", &citation_id)),
        PlaceEventBody::MediaAttached { media, .. } => Some(record("media", &media.media_id)),
        PlaceEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        PlaceEventBody::Tagged { tag_id, .. } | PlaceEventBody::Untagged { tag_id, .. } => Some(record("tag", &tag_id)),
        PlaceEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        PlaceEventBody::GeometryAsserted { .. }
        | PlaceEventBody::SuccessionAsserted { .. }
        | PlaceEventBody::AssertionRetracted { .. }
        | PlaceEventBody::AssertionSuperseded { .. }
        | PlaceEventBody::HumanIdChanged { .. }
        | PlaceEventBody::PlacesMerged { .. }
        | PlaceEventBody::PlacesDistinguished { .. } => None,
    }
}

fn source_value(body: SourceEventBody) -> Option<ChangeValue> {
    match body {
        SourceEventBody::TitleSet { title, .. } => text(title),
        SourceEventBody::AuthorSet { author, .. } => text(author),
        SourceEventBody::PubInfoSet { pub_info, .. } => text(pub_info),
        SourceEventBody::AbbrevSet { abbrev, .. } => text(abbrev),
        SourceEventBody::RepositoryLinked { repo_ref, .. } => Some(record("repository", &repo_ref.repository_id)),
        SourceEventBody::AttributeAdded { attribute, .. } => attribute_text(&attribute),
        SourceEventBody::MediaAttached { media, .. } => Some(record("media", &media.media_id)),
        SourceEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        SourceEventBody::Tagged { tag_id, .. } | SourceEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        SourceEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        SourceEventBody::SourceCreated { .. }
        | SourceEventBody::AssertionRetracted { .. }
        | SourceEventBody::AssertionSuperseded { .. }
        | SourceEventBody::HumanIdChanged { .. }
        | SourceEventBody::SourcesMerged { .. }
        | SourceEventBody::SourcesDistinguished { .. } => None,
    }
}

fn citation_value(body: CitationEventBody) -> Option<ChangeValue> {
    match body {
        CitationEventBody::CitationCreated { source_id, .. } => Some(record("source", &source_id)),
        CitationEventBody::PageSet { page, .. } => text(page),
        CitationEventBody::DateAsserted { date, .. } => Some(ChangeValue::Date(date)),
        CitationEventBody::ConfidenceSet { confidence, .. } => Some(ChangeValue::Confidence(confidence)),
        CitationEventBody::EvidenceAnalysisSet { analysis, .. } => Some(ChangeValue::EvidenceAxes(analysis)),
        CitationEventBody::AttributeAdded { attribute, .. } => attribute_text(&attribute),
        CitationEventBody::MediaAttached { media, .. } => Some(record("media", &media.media_id)),
        CitationEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        CitationEventBody::Tagged { tag_id, .. } | CitationEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        CitationEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        CitationEventBody::AssertionRetracted { .. }
        | CitationEventBody::AssertionSuperseded { .. }
        | CitationEventBody::HumanIdChanged { .. }
        | CitationEventBody::CitationsMerged { .. }
        | CitationEventBody::CitationsDistinguished { .. } => None,
    }
}

fn repository_value(body: RepositoryEventBody) -> Option<ChangeValue> {
    match body {
        RepositoryEventBody::RepositoryTypeSet { repository_type, .. } => {
            Some(ChangeValue::RepositoryType(repository_type))
        }
        RepositoryEventBody::NameSet { name, .. } => text(name),
        RepositoryEventBody::AddressAdded { address, .. } => address_text(&address),
        RepositoryEventBody::UrlAdded { url, .. } => text(url.href),
        RepositoryEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        RepositoryEventBody::Tagged { tag_id, .. } | RepositoryEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        RepositoryEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        RepositoryEventBody::RepositoryCreated { .. }
        | RepositoryEventBody::AssertionRetracted { .. }
        | RepositoryEventBody::AssertionSuperseded { .. }
        | RepositoryEventBody::HumanIdChanged { .. }
        | RepositoryEventBody::RepositoriesMerged { .. }
        | RepositoryEventBody::RepositoriesDistinguished { .. } => None,
    }
}

fn media_value(body: MediaEventBody) -> Option<ChangeValue> {
    match body {
        MediaEventBody::PathSet { path, .. } => crate::tag_usage::media_label(&path).map(ChangeValue::Text),
        MediaEventBody::ChecksumSet { checksum, .. } => text(checksum),
        MediaEventBody::MimeSet { mime, .. } => text(mime),
        MediaEventBody::DateAsserted { date, .. } => Some(ChangeValue::Date(date)),
        MediaEventBody::AttributeAdded { attribute, .. } => attribute_text(&attribute),
        MediaEventBody::CitationAdded { citation_id, .. } => Some(record("citation", &citation_id)),
        MediaEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        MediaEventBody::Tagged { tag_id, .. } | MediaEventBody::Untagged { tag_id, .. } => Some(record("tag", &tag_id)),
        MediaEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        MediaEventBody::MediaCreated { .. }
        | MediaEventBody::AssertionRetracted { .. }
        | MediaEventBody::AssertionSuperseded { .. }
        | MediaEventBody::HumanIdChanged { .. }
        | MediaEventBody::MediaMerged { .. }
        | MediaEventBody::MediaDistinguished { .. } => None,
    }
}

fn note_value(body: NoteEventBody) -> Option<ChangeValue> {
    match body {
        NoteEventBody::NoteTypeSet { note_type, .. } => Some(ChangeValue::NoteType(note_type)),
        NoteEventBody::RichTextSet { text: body, .. } => {
            crate::tag_usage::note_snippet(&body.text).map(ChangeValue::Text)
        }
        NoteEventBody::Tagged { tag_id, .. } | NoteEventBody::Untagged { tag_id, .. } => Some(record("tag", &tag_id)),
        NoteEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        NoteEventBody::NoteCreated { .. }
        | NoteEventBody::AssertionRetracted { .. }
        | NoteEventBody::AssertionSuperseded { .. }
        | NoteEventBody::HumanIdChanged { .. }
        | NoteEventBody::NotesMerged { .. }
        | NoteEventBody::NotesDistinguished { .. } => None,
    }
}

fn tag_value(body: TagEventBody) -> Option<ChangeValue> {
    match body {
        TagEventBody::TagCreated { name, .. } | TagEventBody::TagRenamed { name, .. } => text(name),
        TagEventBody::TagColorSet { color, .. } => text(color),
        TagEventBody::TagPrioritySet { priority, .. } => Some(ChangeValue::Number(i64::from(priority))),
        TagEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
    }
}

fn dna_test_value(body: DnaTestEventBody) -> Option<ChangeValue> {
    match body {
        DnaTestEventBody::DnaTestCreated { person_id, .. } => Some(record("person", &person_id)),
        DnaTestEventBody::ProviderSet { provider, .. } => Some(ChangeValue::DnaProvider(provider)),
        DnaTestEventBody::KitIdSet { kit_id, .. } => text(kit_id),
        DnaTestEventBody::TestTypeSet { test_type, .. } => Some(ChangeValue::DnaTestType(test_type)),
        DnaTestEventBody::GenomeBuildSet { genome_build, .. } => Some(ChangeValue::DnaGenomeBuild(genome_build)),
        DnaTestEventBody::HaplogroupAsserted { haplogroup, .. } => text(haplogroup),
        DnaTestEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        DnaTestEventBody::Tagged { tag_id, .. } | DnaTestEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        DnaTestEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        DnaTestEventBody::AssertionRetracted { .. }
        | DnaTestEventBody::AssertionSuperseded { .. }
        | DnaTestEventBody::HumanIdChanged { .. } => None,
    }
}

fn dna_match_value(body: DnaMatchEventBody) -> Option<ChangeValue> {
    match body {
        DnaMatchEventBody::DnaMatchObserved {
            shared_cm,
            segment_count,
            ..
        } => Some(ChangeValue::DnaObserved {
            shared_cm,
            segment_count,
        }),
        DnaMatchEventBody::SharedAncestorAsserted { ancestor, .. } => {
            match (ancestor.ancestor_person_id, ancestor.note) {
                (Some(person), _) => Some(record("person", &person)),
                (None, Some(note)) => text(note),
                (None, None) => None,
            }
        }
        DnaMatchEventBody::NoteAttached { note_id, .. } => Some(record("note", &note_id)),
        DnaMatchEventBody::Tagged { tag_id, .. } | DnaMatchEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        DnaMatchEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        DnaMatchEventBody::SegmentAdded { .. }
        | DnaMatchEventBody::MatchConfirmed { .. }
        | DnaMatchEventBody::MatchRejected { .. }
        | DnaMatchEventBody::AssertionRetracted { .. }
        | DnaMatchEventBody::AssertionSuperseded { .. }
        | DnaMatchEventBody::HumanIdChanged { .. } => None,
    }
}

fn research_note_value(body: ResearchNoteEventBody) -> Option<ChangeValue> {
    match body {
        ResearchNoteEventBody::ResearchNoteCreated { title, .. } => title.and_then(text),
        ResearchNoteEventBody::SubjectAdded { subject, .. } | ResearchNoteEventBody::SubjectRemoved { subject, .. } => {
            match subject {
                SubjectRef::Person(id) => Some(record("person", &id)),
                SubjectRef::Family(id) => Some(record("family", &id)),
                SubjectRef::Event(id) => Some(record("event", &id)),
                SubjectRef::Place(id) => Some(record("place", &id)),
            }
        }
        ResearchNoteEventBody::RichTextSet { body, .. } => {
            crate::tag_usage::note_snippet(&body.text).map(ChangeValue::Text)
        }
        ResearchNoteEventBody::Tagged { tag_id, .. } | ResearchNoteEventBody::Untagged { tag_id, .. } => {
            Some(record("tag", &tag_id))
        }
        ResearchNoteEventBody::RestrictionsChanged { restrictions, .. } => Some(restrictions_value(restrictions)),
        ResearchNoteEventBody::AssertionRetracted { .. } | ResearchNoteEventBody::AssertionSuperseded { .. } => None,
    }
}

/// Text as written, unless it is blank.
fn text(value: String) -> Option<ChangeValue> {
    (!value.trim().is_empty()).then_some(ChangeValue::Text(value))
}

/// An unlabelled link to `id`, a record of `kind`; [`label_records`] fills in the rest.
fn record(kind: &str, id: &impl ToString) -> ChangeValue {
    ChangeValue::Record {
        record: unlabelled(kind, &id.to_string()),
        role: None,
    }
}

/// [`record`], held in `role`.
fn record_as(kind: &str, id: &impl ToString, role: RecordRole) -> ChangeValue {
    ChangeValue::Record {
        record: unlabelled(kind, &id.to_string()),
        role: Some(role),
    }
}

fn unlabelled(kind: &str, id: &str) -> RecordValue {
    RecordValue {
        kind: kind.to_owned(),
        id: id.to_owned(),
        human_id: None,
        label: None,
    }
}

fn restrictions_value(restrictions: impl IntoIterator<Item = Restriction>) -> ChangeValue {
    ChangeValue::Restrictions(restrictions.into_iter().collect())
}

/// An address on one line: its street lines, then locality, region, postal code and country.
fn address_text(address: &Address) -> Option<ChangeValue> {
    let mut parts: Vec<&str> = address.lines.iter().map(String::as_str).collect();
    for part in [
        &address.locality,
        &address.region,
        &address.postal_code,
        &address.country,
    ]
    .into_iter()
    .flatten()
    {
        parts.push(part);
    }
    text(parts.join(", "))
}

/// An attribute as `type: value`, the two halves both data.
fn attribute_text(attribute: &Attribute) -> Option<ChangeValue> {
    text(format!("{}: {}", attribute.attribute_type, attribute.value))
}

/// An external id as `authority value`.
fn external_id_text(external_id: &ExternalId) -> Option<ChangeValue> {
    text(format!("{} {}", external_id.authority, external_id.value))
}

/// Pairs each superseding entry with the value it replaced. `entries` are in stream order and line up
/// with `events`: a correction's `AssertionSuperseded { target }` shares its assertion id with the
/// replacement that follows it, and `target` names the assertion whose value was replaced.
pub(super) fn attach_replaced(events: &[StoredEvent], entries: &mut [ChangeLogEntry]) {
    let mut values: HashMap<String, ChangeValue> = HashMap::new();
    let mut targets: HashMap<String, String> = HashMap::new();
    for (event, entry) in events.iter().zip(entries.iter()) {
        if event.event_type == "AssertionSuperseded" {
            if let Some(target) = target_of(event) {
                targets.insert(entry.assertion_id.clone(), target);
            }
        } else if let Some(value) = &entry.value {
            values
                .entry(entry.assertion_id.clone())
                .or_insert_with(|| value.clone());
        }
    }
    for (event, entry) in events.iter().zip(entries.iter_mut()) {
        if event.event_type == "AssertionSuperseded" {
            continue;
        }
        if let Some(target) = targets.get(&entry.assertion_id) {
            entry.replaced = values.get(target).cloned();
        }
    }
}

/// The `target` a correction names.
fn target_of(event: &StoredEvent) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(&event.payload).ok()?;
    value.get("target")?.as_str().map(str::to_owned)
}

/// Labels every record `entries` link to, reading each record once. A record that no longer resolves
/// drops the value rather than show a bare id.
///
/// # Errors
///
/// [`AppError`] on a store read failure.
pub(super) async fn label_records(
    workspace: &Workspace,
    mut entries: Vec<ChangeLogEntry>,
) -> Result<Vec<ChangeLogEntry>, AppError> {
    let mut resolved: HashMap<(String, String), Option<RecordValue>> = HashMap::new();
    for entry in &mut entries {
        for slot in [&mut entry.value, &mut entry.replaced] {
            let Some(ChangeValue::Record { record, .. }) = slot.as_mut() else {
                continue;
            };
            let key = (record.kind.clone(), record.id.clone());
            let labelled = if let Some(labelled) = resolved.get(&key) {
                labelled.clone()
            } else {
                let labelled = resolve(workspace, record).await?;
                resolved.insert(key, labelled.clone());
                labelled
            };
            match labelled {
                Some(labelled) => *record = labelled,
                None => *slot = None,
            }
        }
    }
    Ok(entries)
}

/// `record` with its user-facing id and label, or `None` when it no longer resolves.
async fn resolve(workspace: &Workspace, record: &RecordValue) -> Result<Option<RecordValue>, AppError> {
    let store = workspace.store();
    if record.kind == "tag" {
        let name = store
            .find_tag(&record.id)
            .await?
            .and_then(|tag| tag.name().map(str::to_owned));
        return Ok(name.map(|name| RecordValue {
            label: Some(name),
            ..record.clone()
        }));
    }
    let Some(human_id) = store.human_id_of(&record.kind, &record.id).await? else {
        return Ok(None);
    };
    let label = record_label(workspace, &record.kind, &human_id).await?;
    Ok(Some(RecordValue {
        human_id: Some(human_id),
        label,
        ..record.clone()
    }))
}

/// A stored record's display label — a person's name, a place's title, a source's title, a
/// repository's name — or `None` for a kind shown by its id alone, or a record with no label.
///
/// # Errors
///
/// [`AppError`] on a store read failure.
pub(crate) async fn record_label(
    workspace: &Workspace,
    kind: &str,
    human_id: &str,
) -> Result<Option<String>, AppError> {
    let label = match kind {
        "person" => crate::show_person(workspace, human_id)
            .await?
            .and_then(|person| person.display_name),
        "place" => crate::show_place(workspace, human_id)
            .await?
            .map(|place| place.generated_title),
        "source" => crate::show_source(workspace, human_id)
            .await?
            .and_then(|source| source.title),
        "repository" => crate::show_repository(workspace, human_id)
            .await?
            .and_then(|repository| repository.name),
        _ => None,
    };
    Ok(label.filter(|label| !label.is_empty()))
}
