//! The intermediate model a GEDCOM document parses into and emits from.
//!
//! This is the plugin-neutral shape both the import and export glue map between the host's
//! `commands`/`query` DTOs and GEDCOM text. References between records use the GEDCOM cross-reference
//! id (`xref`, e.g. `I0001`); the plugin glue maps those to workspace human ids.

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
}

/// Biological sex as recorded (`SEX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    /// `M`.
    Male,
    /// `F`.
    Female,
    /// `X` — does not fit a binary classification (GEDCOM 7).
    Intersex,
    /// `U` or unrecognized.
    Unknown,
}

/// The kind of a shared event, mirroring the first-class `genealogy_core::enums::EventType` set.
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

/// An event under an `INDI` or `FAM` (`BIRT`/`DEAT`/`MARR`/…) with its date, place, and address.
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
}

/// The kind of a single-person fact (a GEDCOM INDI attribute), mirroring the first-class
/// `genealogy_core::enums::FactType` set.
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

/// The kind of a person-to-person association (GEDCOM `ASSO.ROLE`), mirroring
/// `genealogy_core::enums::AssociationRole`.
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

/// A person-to-person association (`ASSO` on an `INDI`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Association {
    /// The associated person's xref.
    pub other_xref: String,
    /// The role (`ROLE`); `None` when unspecified.
    pub role: Option<AssociationKind>,
}

/// The kind of a name (`NAME.TYPE`), mirroring `genealogy_core::name::NameType`.
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

/// A personal name (`NAME` and its sub-records).
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
    /// The child xrefs (`CHIL`).
    pub children: Vec<String>,
    /// The family's events.
    pub events: Vec<Event>,
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

/// A structured GEDCOM date.
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
