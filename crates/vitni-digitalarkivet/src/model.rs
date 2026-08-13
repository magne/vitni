//! Typed records produced by the parsers.
//!
//! These are plain data — no host, WASM, or `vitni-core` types — so the crate
//! builds for `wasm32-wasip2`. The import plugin maps them onto core aggregates.

/// The kind of Digitalarkivet page a URL points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    /// `/census/person/…` — one person in a census household.
    CensusPerson,
    /// `/census/rural-residence/…` or `/census/urban-residence/…` — a household.
    CensusResidence,
    /// `/view/<n>/pd…` — a church-book record (an event's participant list).
    ChurchbookRecord,
    /// Anything else, including non-Digitalarkivet hosts.
    Unknown,
}

/// A stable external identifier back to the source archive (data-model §11).
///
/// `authority` is always [`AUTHORITY`](crate::AUTHORITY) for this crate; the plugin
/// resolves-or-creates by it so re-import is idempotent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalId {
    /// The archive authority, e.g. `"digitalarkivet"`.
    pub authority: String,
    /// The record identifier within that authority, e.g. `"pf01073902000464"`.
    pub value: String,
}

impl ExternalId {
    /// A Digitalarkivet external id from a record identifier.
    #[must_use]
    pub fn digitalarkivet(value: impl Into<String>) -> Self {
        Self {
            authority: crate::AUTHORITY.to_owned(),
            value: value.into(),
        }
    }
}

/// One transcribed key/value row, in document order.
///
/// `key` is the source label with any trailing colon removed (e.g. `"Fødested"`);
/// labels are resolved to display text by the plugin's own catalogue, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The source label, colon-stripped and whitespace-normalized.
    pub key: String,
    /// The transcribed value, whitespace-normalized.
    pub value: String,
}

/// Source/citation metadata suggested for a record, for Source/Citation/Repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    /// The source title (census or church-book), e.g.
    /// `"Folketelling 1920 for 1017 Greipstad herred"`.
    pub title: Option<String>,
    /// A four-digit year extracted from the title, when present.
    pub year: Option<String>,
    /// The managing repository name for the citation (free-reuse attribution).
    pub repository: &'static str,
    /// Context headings from the page (census `Tellingskrets`/`Bosted land`,
    /// church-book event heading) as generic label/value pairs.
    pub headings: Vec<Field>,
}

/// A person/record page: the focal person's transcribed fields plus household
/// links, scan-viewer URL, and source metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRecord {
    /// Which page kind produced this record.
    pub page_kind: PageKind,
    /// The permanent record page URL (from `og:url`).
    pub record_url: String,
    /// The stable external id (record id under `digitalarkivet`).
    pub external_id: ExternalId,
    /// The focal person's name.
    pub name: String,
    /// Every transcribed field of the focal person, in document order.
    pub fields: Vec<Field>,
    /// Birth date or year (`Alder/født`, `Fødselsdato`, `Fødselsår`).
    pub birth: Option<String>,
    /// Birthplace (`Fødested`).
    pub birthplace: Option<String>,
    /// Residence (`Bosted`).
    pub residence: Option<String>,
    /// Family position / role (`Familiestilling`, `Rolle`).
    pub role: Option<String>,
    /// Marital status (`Sivilstand`).
    pub marital_status: Option<String>,
    /// Occupation (`Yrke`, `Stilling/stand`).
    pub occupation: Option<String>,
    /// The scan-viewer URL resolved from the page, when present.
    pub scan_viewer_url: Option<String>,
    /// Absolute, de-duplicated links to every household/participant record.
    pub household: Vec<String>,
    /// Source/citation metadata for this record.
    pub source: SourceMetadata,
}

/// A residence/household page: the person links it lists plus source metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidenceRecord {
    /// The permanent record page URL (from `og:url`).
    pub record_url: String,
    /// The stable external id (residence id under `digitalarkivet`).
    pub external_id: ExternalId,
    /// Absolute, de-duplicated `/census/person/` links in the household.
    pub person_links: Vec<String>,
    /// Source/citation metadata for the residence.
    pub source: SourceMetadata,
}
