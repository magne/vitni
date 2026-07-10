//! The intermediate model a Gramps XML document parses into and emits from.
//!
//! Records are addressed by their Gramps `handle` (the `hlink` target); `gramps_id` is the
//! user-facing id (e.g. `I0001`). Cross-references between records are handle strings, kept as the
//! Gramps document holds them — the plugin glue resolves them to workspace human ids. Value types
//! (names, dates, event/association kinds) come from [`genealogy_interchange`].

use genealogy_interchange::{AssociationKind, Date, EventKind, Name};

/// A parsed Gramps XML database: the records we model, each keyed by its `handle`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Database {
    /// `<person>` records.
    pub people: Vec<Person>,
    /// `<family>` records.
    pub families: Vec<Family>,
    /// Top-level `<event>` records.
    pub events: Vec<Event>,
    /// `<placeobj>` records.
    pub places: Vec<Place>,
    /// `<source>` records.
    pub sources: Vec<Source>,
    /// `<citation>` records.
    pub citations: Vec<Citation>,
    /// `<repository>` records.
    pub repositories: Vec<Repository>,
    /// `<object>` (media) records.
    pub objects: Vec<MediaObject>,
    /// `<note>` records.
    pub notes: Vec<Note>,
    /// `<tag>` records.
    pub tags: Vec<Tag>,
}

/// Biological sex as Gramps records it (`<gender>` — `M`/`F`/`U`, plus `X` for GEDCOM 7 intersex).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    /// `M`.
    Male,
    /// `F`.
    Female,
    /// `U`.
    Unknown,
    /// `X` — intersex (no native Gramps numeric code; the lossy mapping is the plugin's concern).
    Intersex,
}

/// A `<person>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Person {
    /// The internal handle (the `hlink` target).
    pub handle: String,
    /// The user-facing id (e.g. `I0001`).
    pub gramps_id: Option<String>,
    /// The primary name.
    pub name: Option<Name>,
    /// The recorded gender.
    pub gender: Option<Gender>,
    /// Events the person took part in (`<eventref>`), each with its participation payload (role, the
    /// participant-scoped attributes including `"Age"`, and note/citation refs — ADR 0019).
    pub event_refs: Vec<EventRef>,
    /// Handles of citations backing the person's claims (`<citationref>`).
    pub citation_refs: Vec<String>,
    /// Handles of attached notes (`<noteref>`).
    pub note_refs: Vec<String>,
    /// Handles of attached media (`<objref>`).
    pub media_refs: Vec<String>,
    /// Person-to-person associations (`<personref>`).
    pub person_refs: Vec<PersonRef>,
    /// The Gramps privacy flag (the `priv` attribute). Gramps has no multi-value RESN, so this maps
    /// lossily to/from the restriction set (data-model §16).
    pub private: bool,
}

/// A `<personref>`: a handle to another person and the relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRef {
    /// The associated person's handle.
    pub hlink: String,
    /// The relationship (`rel`); `None` when unspecified.
    pub rel: Option<AssociationKind>,
}

/// An `<eventref>`: the handle of a referenced event plus this participant's payload — the Gramps
/// `role` (an `EventRoleType` string), the eventref `<attribute>`s (including the `"Age"` attribute),
/// and its note/citation refs (Gramps DTD `eventref = (attribute*, noteref*, citationref*)`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EventRef {
    /// The referenced event's handle.
    pub hlink: String,
    /// The participant's role (the `role` attribute), kept verbatim; `None` when unspecified.
    pub role: Option<String>,
    /// The eventref attributes (`<attribute type=… value=…>`), including the `"Age"` attribute.
    pub attributes: Vec<EventRefAttribute>,
    /// Handles of notes about this participation (`<noteref>`).
    pub note_refs: Vec<String>,
    /// Handles of citations backing this participation (`<citationref>`).
    pub citation_refs: Vec<String>,
}

impl EventRef {
    /// A bare reference to `hlink` with no participation payload — the common primary-participant case.
    #[must_use]
    pub fn bare(hlink: impl Into<String>) -> Self {
        Self {
            hlink: hlink.into(),
            ..Self::default()
        }
    }

    /// Whether this reference carries no payload (role / attributes / note / citation refs), so it
    /// emits as a self-closing `<eventref hlink=…/>`.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.role.is_none() && self.attributes.is_empty() && self.note_refs.is_empty() && self.citation_refs.is_empty()
    }
}

/// A typed key/value attribute on an `<eventref>` (`<attribute type=… value=…>`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EventRefAttribute {
    /// The attribute type (the `type` attribute, e.g. `"Age"`).
    pub attribute_type: String,
    /// The attribute value (the `value` attribute).
    pub value: String,
}

/// A `<family>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Family {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `F0001`).
    pub gramps_id: Option<String>,
    /// The father's handle (`<father>`).
    pub father: Option<String>,
    /// The mother's handle (`<mother>`).
    pub mother: Option<String>,
    /// The children (`<childref>`) with their per-parent relationships (`frel`/`mrel`).
    pub child_refs: Vec<ChildRef>,
    /// The family's events (`<eventref>`), each with its participation payload (ADR 0019).
    pub event_refs: Vec<EventRef>,
    /// The Gramps privacy flag (the `priv` attribute; lossy to/from the restriction set — §16).
    pub private: bool,
}

/// A `<childref>` in a `<family>`: the child's handle and its relationship to the mother (`mrel`)
/// and father (`frel`), where present. The relationship values are the raw Gramps strings.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChildRef {
    /// The child's handle (`<childref hlink>`).
    pub hlink: String,
    /// The child's relationship to the mother (`mrel`), if recorded.
    pub mother_relationship: Option<String>,
    /// The child's relationship to the father (`frel`), if recorded.
    pub father_relationship: Option<String>,
}

/// A top-level `<event>` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `E0001`).
    pub gramps_id: Option<String>,
    /// The kind of event (`<type>`).
    pub kind: EventKind,
    /// When it occurred (`<dateval>`/`<daterange>`/`<datespan>`/`<datestr>`).
    pub date: Option<Date>,
    /// The handle of the place it occurred (`<place hlink>`).
    pub place_ref: Option<String>,
    /// A free-text description (`<description>`).
    pub description: Option<String>,
}

/// A `<placeobj>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Place {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `P0001`).
    pub gramps_id: Option<String>,
    /// The primary place name (`<pname value>`).
    pub name: Option<String>,
    /// The place type (the `type` attribute), kept verbatim.
    pub place_type: Option<String>,
    /// Handles of enclosing places (`<placeref>`), the hierarchy chain.
    pub enclosed_by: Vec<String>,
}

/// A `<source>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Source {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `S0001`).
    pub gramps_id: Option<String>,
    /// The title (`<stitle>`).
    pub title: Option<String>,
    /// The author (`<sauthor>`).
    pub author: Option<String>,
    /// Publication info (`<spubinfo>`).
    pub pub_info: Option<String>,
    /// Handles of linked repositories (`<reporef>`).
    pub repository_refs: Vec<String>,
}

/// A `<citation>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `C0001`).
    pub gramps_id: Option<String>,
    /// The handle of the cited source (`<sourceref>`).
    pub source_ref: Option<String>,
    /// The page locator (`<page>`).
    pub page: Option<String>,
    /// The confidence (`<confidence>`, 0–4); `None` when unspecified.
    pub confidence: Option<u8>,
}

/// A `<repository>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Repository {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `R0001`).
    pub gramps_id: Option<String>,
    /// The repository name (`<rname>`).
    pub name: Option<String>,
}

/// An `<object>` (media) record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaObject {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `O0001`).
    pub gramps_id: Option<String>,
    /// The file path / URL (`<file src>`).
    pub file: Option<String>,
    /// The MIME type (`<file mime>`).
    pub mime: Option<String>,
}

/// A `<note>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Note {
    /// The internal handle.
    pub handle: String,
    /// The user-facing id (e.g. `N0001`).
    pub gramps_id: Option<String>,
    /// The note text (`<text>`).
    pub text: Option<String>,
}

/// A `<tag>` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The internal handle.
    pub handle: String,
    /// The tag name (the `name` attribute).
    pub name: Option<String>,
}
