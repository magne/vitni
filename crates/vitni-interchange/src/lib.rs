//! `vitni-interchange` — the format-neutral value vocabulary shared by import/export formats.
//!
//! These are the *leaf* value types every record-interchange format (`vitni-gedcom`,
//! `vitni-gramps-xml`, the assisted-import `vitni-digitalarkivet` crate, a future GEDCOM X
//! crate) and the WASM plugin bridge speak: sex, name parts, the date grammar, postal addresses,
//! and the event/fact/association/name kinds. They are deliberately *simple and serde-free*: no
//! `Custom(String)` escapes, no event-payload tagging, no provenance, no sort keys. Those richer
//! concerns belong to `vitni-core`'s domain types; this crate is the lossy interchange shape the
//! plugins convert to and from the host's WIT types.
//!
//! Container/reference types (a format's document tree, its xref/handle links) stay in the format
//! crate — only the value vocabulary lives here.

mod age;

pub use age::{Age, AgeBound, age_value, parse_age};

/// Biological sex as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    /// Male (`M`).
    Male,
    /// Female (`F`).
    Female,
    /// Does not fit a binary classification (GEDCOM 7 `X`).
    Intersex,
    /// Unknown or unrecognized (`U`).
    Unknown,
}

/// A privacy restriction on a record (GEDCOM v7 `RESN` — data-model §6), mirroring
/// `vitni_core::enums::Restriction`. A record carries a set of these; empty = unrestricted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Restriction {
    /// Hide from general view.
    Confidential,
    /// Protected from edits.
    Locked,
    /// Living-person privacy.
    Privacy,
}

/// How a source is held at a repository (GEDCOM `SOUR.REPO.CALN.MEDI`, Gramps `<reporef medium>`),
/// mirroring `vitni_core::enums::SourceMediaType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMediaKind {
    /// `AUDIO`.
    Audio,
    /// `BOOK`.
    Book,
    /// `CARD`.
    Card,
    /// `ELECTRONIC`.
    Electronic,
    /// `FICHE`.
    Fiche,
    /// `FILM`.
    Film,
    /// `MAGAZINE`.
    Magazine,
    /// `MANUSCRIPT`.
    Manuscript,
    /// `MAP`.
    Map,
    /// `NEWSPAPER`.
    Newspaper,
    /// `PHOTO`.
    Photo,
    /// `TOMBSTONE`.
    Tombstone,
    /// `VIDEO`.
    Video,
    /// An unrecognized medium kept verbatim (GEDCOM `OTHER` + `PHRASE`).
    Other(String),
}

/// The kind of a shared event, mirroring the first-class `vitni_core::enums::EventType` set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// `BIRT`.
    Birth,
    /// `DEAT`.
    Death,
    /// `MARR`.
    Marriage,
    /// `BAPM`.
    Baptism,
    /// `CHR`.
    Christening,
    /// `BURI`.
    Burial,
    /// `CREM`.
    Cremation,
    /// `CENS`.
    Census,
    /// `RESI`.
    Residence,
    /// `IMMI`.
    Immigration,
    /// `EMIG`.
    Emigration,
    /// `ADOP`.
    Adoption,
    /// `CONF`.
    Confirmation,
    /// `BARM`.
    BarMitzvah,
    /// `BASM`.
    BasMitzvah,
    /// `FCOM`.
    FirstCommunion,
    /// `GRAD`.
    Graduation,
    /// `NATU`.
    Naturalization,
    /// `ORDN`.
    Ordination,
    /// `PROB`.
    Probate,
    /// `RETI`.
    Retirement,
    /// `WILL`.
    Will,
    /// `ENGA`.
    Engagement,
    /// `ANUL`.
    Annulment,
    /// `DIV`.
    Divorce,
    /// `DIVF`.
    DivorceFiled,
    /// `MARB`.
    MarriageBanns,
    /// `MARC`.
    MarriageContract,
    /// `MARL`.
    MarriageLicense,
    /// `MARS`.
    MarriageSettlement,
}

/// The kind of a single-person fact (a GEDCOM INDI attribute) — the attribute-shaped subset of
/// `vitni_core::enums::FactType`. Vital types (birth/death/baptism/burial) are not here: they
/// are Events, not facts (ADR 0021 §2). `Residence` is also absent today — a residence carried
/// through interchange is modelled as an `EventKind`, not a fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactKind {
    /// `OCCU`.
    Occupation,
    /// `RELI`.
    Religion,
    /// `EDUC`.
    Education,
    /// `CAST`.
    Caste,
    /// `DSCR`.
    PhysicalDescription,
    /// `ETHN`.
    Ethnicity,
    /// `IDNO`.
    NationalId,
    /// `NATI`.
    Nationality,
    /// `NCHI`.
    NumberOfChildren,
    /// `NMR`.
    NumberOfMarriages,
    /// `PROP`.
    Property,
    /// `SSN`.
    SocialSecurityNumber,
    /// `TITL` (under `INDI`) — a title of nobility.
    NobilityTitle,
}

/// The kind of a person-to-person association (GEDCOM `ASSO.ROLE`), mirroring
/// `vitni_core::enums::AssociationRole`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssociationKind {
    /// `CLERGY`.
    Clergy,
    /// `FRIEND`.
    Friend,
    /// `GODP`.
    Godparent,
    /// `NGHBR`.
    Neighbour,
    /// `OFFICIATOR`.
    Officiator,
    /// `WITN`.
    Witness,
    /// `CHIL`.
    Child,
    /// `FATH`.
    Father,
    /// `MOTH`.
    Mother,
    /// `PARENT`.
    Parent,
    /// `HUSB`.
    Husband,
    /// `WIFE`.
    Wife,
    /// `SPOU`.
    Spouse,
    /// `MULTIPLE`.
    Multiple,
    /// An unrecognized role kept verbatim.
    Other(String),
}

/// The kind of a name (`NAME.TYPE`), mirroring `vitni_core::name::NameType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameKind {
    /// `BIRTH`.
    BirthName,
    /// `MARRIED`.
    MarriedName,
    /// `MAIDEN`.
    Maiden,
    /// `IMMIGRANT`.
    Immigrant,
    /// `PROFESSIONAL`.
    Professional,
    /// `AKA`.
    AlsoKnownAs,
    /// `RELIGIOUS`.
    ReligiousName,
    /// An unrecognized type kept verbatim.
    Other(String),
}

/// A personal name and its structured parts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Name {
    /// The name type (`TYPE`).
    pub name_type: Option<NameKind>,
    /// The given name (`GIVN`, or the part before the first slash).
    pub given: Option<String>,
    /// The surname prefix (`SPFX`, e.g. `van`).
    pub surname_prefix: Option<String>,
    /// The surname (`SURN`, or the part between slashes).
    pub surname: Option<String>,
    /// The nickname (`NICK`).
    pub nickname: Option<String>,
    /// The name prefix / title (`NPFX`).
    pub prefix: Option<String>,
    /// The name suffix (`NSFX`).
    pub suffix: Option<String>,
}

/// A calendar a date is expressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Calendar {
    /// The Gregorian calendar (`@#DGREGORIAN@`, the default).
    #[default]
    Gregorian,
    /// The Julian calendar (`@#DJULIAN@`).
    Julian,
    /// The Hebrew calendar (`@#DHEBREW@`).
    Hebrew,
    /// The French Republican calendar (`@#DFRENCH R@`).
    FrenchRepublican,
    /// The Islamic calendar (`@#DISLAMIC@`).
    Islamic,
    /// The Swedish calendar (`@#DSWEDISH@`).
    Swedish,
}

/// How reliable a date is (GEDCOM `EST`/`CAL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateQuality {
    /// A normal, asserted date.
    #[default]
    Normal,
    /// An estimated date (`EST`).
    Estimated,
    /// A date calculated from other facts (`CAL`).
    Calculated,
}

/// A single, possibly-partial point on a calendar; `year` may be negative for BCE.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DatePoint {
    /// The year. `None` if unknown.
    pub year: Option<i32>,
    /// The month, 1–12. `None` if unknown.
    pub month: Option<u8>,
    /// The day, 1–31. `None` if unknown.
    pub day: Option<u8>,
}

/// How a date is qualified: exact, open-ended, approximate, a range/span, or free text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateModifier {
    /// An exact (single) date.
    Exact(DatePoint),
    /// Before the given date (`BEF`).
    Before(DatePoint),
    /// After the given date (`AFT`).
    After(DatePoint),
    /// Approximately the given date (`ABT`).
    About(DatePoint),
    /// Somewhere between two dates (`BET … AND …`).
    Range {
        /// The earliest possible date.
        start: DatePoint,
        /// The latest possible date.
        end: DatePoint,
    },
    /// A span covering a stretch of time (`FROM … TO …`).
    Span {
        /// The start of the span.
        start: DatePoint,
        /// The end of the span.
        end: DatePoint,
    },
    /// From the given date (`FROM`, open-ended).
    From(DatePoint),
    /// To the given date (`TO`, open-ended).
    To(DatePoint),
    /// A date interpreted from a free-text phrase (`INT`).
    Interpreted {
        /// The interpreted, structured date.
        date: DatePoint,
        /// The verbatim phrase it was interpreted from.
        phrase: String,
    },
    /// An unparseable date kept verbatim.
    TextOnly(String),
}

/// A structured genealogical date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Date {
    /// The calendar the date is expressed in.
    pub calendar: Calendar,
    /// The reliability of the date.
    pub quality: DateQuality,
    /// The date itself.
    pub modifier: DateModifier,
    /// Month in which the year begins, for dual / old-style dating (e.g. 1735/6).
    pub new_year_begins: Option<u8>,
    /// The verbatim source text, always retained.
    pub original: String,
}

/// A postal address (`ADDR` plus its structured subtags and the contact subtags beside it).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Address {
    /// The street address lines (`ADR1`/`ADR2`/`ADR3`, or the `ADDR` value lines).
    pub lines: Vec<String>,
    /// The city / locality (`CITY`).
    pub locality: Option<String>,
    /// The state / region (`STAE`).
    pub region: Option<String>,
    /// The postal code (`POST`).
    pub postal_code: Option<String>,
    /// The country (`CTRY`).
    pub country: Option<String>,
    /// A phone number (`PHON`).
    pub phone: Option<String>,
    /// An email address (`EMAIL`).
    pub email: Option<String>,
    /// A fax number (`FAX`).
    pub fax: Option<String>,
    /// A web address (`WWW`).
    pub www: Option<String>,
    /// The verbatim `ADDR` payload, when it could not be split into fields.
    pub original_text: Option<String>,
}

impl Address {
    /// Whether every field is absent.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            && self.locality.is_none()
            && self.region.is_none()
            && self.postal_code.is_none()
            && self.country.is_none()
            && self.phone.is_none()
            && self.email.is_none()
            && self.fax.is_none()
            && self.www.is_none()
            && self.original_text.is_none()
    }
}
