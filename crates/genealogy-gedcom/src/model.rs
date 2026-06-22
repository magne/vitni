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
}
