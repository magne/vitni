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

/// An `INDI` record reduced to the minimal spike subset: an id and one name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Individual {
    /// The GEDCOM cross-reference id (without the surrounding `@`).
    pub xref: String,
    /// The given name from the primary `NAME`, if present.
    pub given: Option<String>,
    /// The surname (the text between the slashes of `NAME`), if present.
    pub surname: Option<String>,
}

/// A `FAM` record: partners (`HUSB`/`WIFE`) and children (`CHIL`), referenced by xref.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Family {
    /// The GEDCOM cross-reference id (without the surrounding `@`).
    pub xref: String,
    /// Partner xrefs, in the order `HUSB` then `WIFE`.
    pub partners: Vec<String>,
    /// Child xrefs (`CHIL`), in document order.
    pub children: Vec<String>,
}
