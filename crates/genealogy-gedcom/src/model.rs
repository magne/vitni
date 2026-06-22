//! The intermediate model a GEDCOM document parses into and emits from.
//!
//! This is the plugin-neutral shape both the import and export glue map between the host's
//! `commands`/`query` DTOs and GEDCOM text. References between records use the GEDCOM cross-reference
//! id (`xref`, e.g. `I0001`); the plugin glue maps those to workspace human ids.

/// A parsed GEDCOM document: the individuals and families it contains.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tree {
    /// `INDI` records, in document order.
    pub individuals: Vec<Individual>,
    /// `FAM` records, in document order.
    pub families: Vec<Family>,
    /// Top-level `SOUR` records, in document order.
    pub sources: Vec<Source>,
}

/// A top-level `SOUR` record: an id and a title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    /// The GEDCOM cross-reference id (without the surrounding `@`).
    pub xref: String,
    /// The `TITL`, if present.
    pub title: Option<String>,
}

/// A citation: a reference (`SOUR @S..@`) to a top-level source, with an optional `PAGE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The referenced source's xref (without the surrounding `@`).
    pub source_xref: String,
    /// The `PAGE` locator, if present.
    pub page: Option<String>,
}

/// An inline media object (`OBJE`): its `FILE` reference (a path or URL) and optional `TITL`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaObject {
    /// The `FILE` value (a path or URL), if present.
    pub file: Option<String>,
    /// The `TITL`, if present.
    pub title: Option<String>,
}

/// Biological sex as recorded by the GEDCOM `SEX` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    /// `M`.
    Male,
    /// `F`.
    Female,
    /// `U` or any other value.
    Unknown,
}

/// The kind of a GEDCOM event, mapped from its tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// `BIRT`.
    Birth,
    /// `DEAT`.
    Death,
    /// `MARR`.
    Marriage,
    /// `CHR` (christening) / `BAPM`.
    Baptism,
    /// `BURI`.
    Burial,
    /// `CENS`.
    Census,
    /// `RESI`.
    Residence,
    /// `IMMI`.
    Immigration,
    /// `EMIG`.
    Emigration,
}

/// A simple (Gregorian) calendar date, the parseable core of a GEDCOM `DATE` — modifiers such as
/// `ABT`/`BEF`/`BET` are dropped to a best-effort year for now (GEDCOM date grammar is a refinement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    /// The year (negative for BCE).
    pub year: i32,
    /// The month, 1–12, if given.
    pub month: Option<u8>,
    /// The day, 1–31, if given.
    pub day: Option<u8>,
}

/// An event (`BIRT`, `DEAT`, `MARR`, …) with its optional `DATE` and `PLAC`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The kind of event.
    pub kind: EventKind,
    /// The event date, if parseable.
    pub date: Option<Date>,
    /// The place name (the `PLAC` text), if present.
    pub place: Option<String>,
}

/// An `INDI` record: an id, one name, and `SEX`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Individual {
    /// The GEDCOM cross-reference id (without the surrounding `@`).
    pub xref: String,
    /// The stable `_UID` (a GUID on MyHeritage/Gramps exports), if present — the cross-file identity
    /// preferred over `xref` for re-import (data-model §11).
    pub uid: Option<String>,
    /// The given name from the primary `NAME`, if present.
    pub given: Option<String>,
    /// The surname (the text between the slashes of `NAME`), if present.
    pub surname: Option<String>,
    /// The recorded `SEX`, if present.
    pub sex: Option<Sex>,
    /// Individual events (`BIRT`, `DEAT`, `CHR`, `BURI`, …), in document order.
    pub events: Vec<Event>,
    /// Source citations (`SOUR @S..@`) attached directly to the individual, in document order.
    pub citations: Vec<Citation>,
    /// Inline media objects (`OBJE`) attached to the individual, in document order.
    pub media: Vec<MediaObject>,
    /// Note texts (`NOTE`) attached to the individual, in document order.
    pub notes: Vec<String>,
}

/// A `FAM` record: partners (`HUSB`/`WIFE`) and children (`CHIL`), referenced by xref.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Family {
    /// The GEDCOM cross-reference id (without the surrounding `@`).
    pub xref: String,
    /// The stable `_UID` (a GUID on MyHeritage/Gramps exports), if present — the cross-file identity
    /// preferred over `xref` for re-import (data-model §11).
    pub uid: Option<String>,
    /// Partner xrefs, in the order `HUSB` then `WIFE`.
    pub partners: Vec<String>,
    /// Child xrefs (`CHIL`), in document order.
    pub children: Vec<String>,
    /// Family events (`MARR`, …), in document order.
    pub events: Vec<Event>,
}
