//! The intermediate model a GEDCOM document parses into and emits from.
//!
//! This is the plugin-neutral shape both the import and export glue map between the host's
//! `commands`/`query` DTOs and GEDCOM text. References between records use the GEDCOM cross-reference
//! id (`xref`, e.g. `I0001`); the plugin glue maps those to workspace human ids. The format-neutral
//! value types (names, dates, addresses, event/fact/association kinds) come from
//! [`genealogy_interchange`]; only the GEDCOM document/reference shape lives here.

pub use genealogy_interchange::{
    Address, Age, AgeBound, AssociationKind, Calendar, Date, DateModifier, DatePoint, DateQuality, EventKind, FactKind,
    Name, NameKind, Restriction, Sex,
};

/// A parsed GEDCOM document: the records we model.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tree {
    /// `INDI` records.
    pub individuals: Vec<Individual>,
    /// `FAM` records.
    pub families: Vec<Family>,
    /// Top-level `SOUR` records.
    pub sources: Vec<Source>,
}

/// A top-level `SOUR` record (a work / document).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Source {
    /// The cross-reference id (e.g. `S0001`).
    pub xref: String,
    /// The source title (`TITL`).
    pub title: Option<String>,
    /// The source author (`AUTH`).
    pub author: Option<String>,
    /// The publication info (`PUBL`).
    pub pub_info: Option<String>,
}

/// A citation: a `SOUR` pointer into a top-level source, with an optional page locator.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The cited source's xref.
    pub source_xref: String,
    /// The page locator (`PAGE`).
    pub page: Option<String>,
}

/// An inline `OBJE` media object.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaObject {
    /// The file path / URL (`FILE`).
    pub file: Option<String>,
    /// The title (`TITL`).
    pub title: Option<String>,
    /// The MIME / media type (`OBJE.FILE.FORM`).
    pub mime: Option<String>,
}

/// An event under an `INDI` or `FAM` (`BIRT`/`DEAT`/`MARR`/…) with its date, place, address, the
/// participant ages a source records, and event-level `ASSO` witnesses (data-model §17, ADR 0019).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The kind of event.
    pub kind: EventKind,
    /// When it occurred (`DATE`).
    pub date: Option<Date>,
    /// Where it occurred (`PLAC`).
    pub place: Option<String>,
    /// A postal address (`ADDR` + contact subtags).
    pub address: Option<Address>,
    /// The (single) participant's age at an `INDI` event (GEDCOM `2 AGE`).
    pub age: Option<Age>,
    /// The husband's age at a `FAM` event (GEDCOM `2 HUSB` / `3 AGE`).
    pub husband_age: Option<Age>,
    /// The wife's age at a `FAM` event (GEDCOM `2 WIFE` / `3 AGE`).
    pub wife_age: Option<Age>,
    /// Event-level `ASSO` witnesses (GEDCOM 7 `EVENT_DETAIL` `2 ASSO`), each with a role, backing
    /// citations, and notes.
    pub associations: Vec<EventAssociation>,
}

/// An event-level `ASSO` witness (GEDCOM 7 `EVENT_DETAIL`): another person's involvement in the
/// event, with a role, backing citations, and notes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EventAssociation {
    /// The associated person's xref.
    pub other_xref: String,
    /// The role (`ROLE`); `None` when unspecified.
    pub role: Option<AssociationKind>,
    /// Citations backing the witness's involvement (`SOUR` under the `ASSO`).
    pub citations: Vec<Citation>,
    /// Notes about the witness's involvement (`NOTE` under the `ASSO`).
    pub notes: Vec<String>,
}

/// A single-person fact parsed from a GEDCOM INDI attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    /// The kind of fact.
    pub kind: FactKind,
    /// The free-text value (the attribute payload).
    pub value: Option<String>,
    /// When the fact applied (`DATE`).
    pub date: Option<Date>,
}

/// A person-to-person association (`ASSO` on an `INDI`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Association {
    /// The associated person's xref.
    pub other_xref: String,
    /// The role (`ROLE`); `None` when unspecified.
    pub role: Option<AssociationKind>,
}

/// An `INDI` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Individual {
    /// The cross-reference id (e.g. `I0001`).
    pub xref: String,
    /// The stable id (`_UID`).
    pub uid: Option<String>,
    /// The person's name.
    pub name: Option<Name>,
    /// The recorded sex.
    pub sex: Option<Sex>,
    /// The person's events.
    pub events: Vec<Event>,
    /// The person's single-person facts (INDI attributes).
    pub facts: Vec<Fact>,
    /// The person's associations.
    pub associations: Vec<Association>,
    /// The person's citations.
    pub citations: Vec<Citation>,
    /// The person's inline media.
    pub media: Vec<MediaObject>,
    /// The person's notes.
    pub notes: Vec<String>,
    /// The person's privacy restrictions (GEDCOM v7 `RESN`).
    pub restrictions: Vec<Restriction>,
}

/// A `FAM` record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Family {
    /// The cross-reference id (e.g. `F0001`).
    pub xref: String,
    /// The stable id (`_UID`).
    pub uid: Option<String>,
    /// The partner xrefs (`HUSB`/`WIFE`).
    pub partners: Vec<String>,
    /// The children (`CHIL`) with their per-parent relationships (`_FREL`/`_MREL`).
    pub children: Vec<ChildRef>,
    /// The family's events.
    pub events: Vec<Event>,
    /// The family's privacy restrictions (GEDCOM v7 `RESN`).
    pub restrictions: Vec<Restriction>,
}

/// A child in a `FAM` (`CHIL`) with its relationship to the father (`_FREL`) and mother (`_MREL`),
/// where present. The relationship values are the raw GEDCOM strings (e.g. `Birth`, `Adopted`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChildRef {
    /// The child's cross-reference id (`CHIL @…@`).
    pub xref: String,
    /// The child's relationship to the father (`_FREL`), if recorded.
    pub father_relationship: Option<String>,
    /// The child's relationship to the mother (`_MREL`), if recorded.
    pub mother_relationship: Option<String>,
}
