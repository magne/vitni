//! The corpus file schema and its mapping onto the engine's person profiles.
//!
//! A corpus file is a list of `[[pair]]` tables, each with two records written as the few fields a
//! transcription gives: names, sex, vital events as year/month/day with the country and place, and the
//! relatives' names and birth years. A census gives an age, not a birth date, so a vital event says
//! whether its year was computed from one (`basis = "age"`).

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::enums::Sex;
use vitni_core::matching::DateBasis;
use vitni_core::matching::profile::{PersonProfile, PlaceProfile, Relative, VitalEvent, VitalKind};
use vitni_core::name::{NameType, PersonName, Surname};
use vitni_core::place_name::PlaceName;

/// Whether the two records of a pair describe one individual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    /// One individual.
    Same,
    /// Two individuals.
    Distinct,
}

/// The hard true matches whose recall the gate holds (ADR 0038 §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HardCase {
    /// A given name or surname spelled differently.
    SpellingVariant,
    /// A farm name or surname that changed after a move.
    SurnameAfterMove,
    /// A birth year computed from a census age, one to five years off.
    CensusAge,
    /// A baptism standing in for a birth.
    BaptismForBirth,
}

impl HardCase {
    /// Every hard case, each of which the corpus must hold.
    pub const ALL: [Self; 4] = [
        Self::SpellingVariant,
        Self::SurnameAfterMove,
        Self::CensusAge,
        Self::BaptismForBirth,
    ];

    /// The name a corpus file spells the case with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SpellingVariant => "spelling-variant",
            Self::SurnameAfterMove => "surname-after-move",
            Self::CensusAge => "census-age",
            Self::BaptismForBirth => "baptism-for-birth",
        }
    }
}

/// A labelled pair, its records mapped to profiles.
#[derive(Debug, Clone)]
pub struct Pair {
    /// The pair's id, unique across the corpus.
    pub id: String,
    /// Whether the records describe one individual.
    pub label: Label,
    /// The hard case the pair exercises, if any.
    pub hard: Option<HardCase>,
    /// The left record.
    pub left: PersonProfile,
    /// The right record.
    pub right: PersonProfile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    pair: Vec<RawPair>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPair {
    id: String,
    label: Label,
    hard: Option<HardCase>,
    /// Where the records come from: `invented`, or the records' references and the linkage evidence.
    source: String,
    left: Record,
    right: Record,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    given: Option<String>,
    surname: Option<String>,
    sex: Option<RecordSex>,
    #[serde(default)]
    occupations: Vec<String>,
    birth: Option<Vital>,
    baptism: Option<Vital>,
    death: Option<Vital>,
    burial: Option<Vital>,
    #[serde(default)]
    parents: Vec<Kin>,
    #[serde(default)]
    partners: Vec<Kin>,
    #[serde(default)]
    children: Vec<Kin>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RecordSex {
    Male,
    Female,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vital {
    year: i32,
    month: Option<u8>,
    day: Option<u8>,
    #[serde(default)]
    basis: Basis,
    country: Option<String>,
    place: Option<String>,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Basis {
    #[default]
    Recorded,
    Age,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Kin {
    given: Option<String>,
    surname: Option<String>,
    sex: Option<RecordSex>,
    born: Option<i32>,
}

/// Parses one corpus file, `file` naming it in every error.
pub fn parse(file: &str, text: &str) -> Result<Vec<Pair>> {
    let parsed: File = toml::from_str(text).with_context(|| format!("parsing corpus file {file}"))?;
    let mut pairs = Vec::new();
    for raw in parsed.pair {
        pairs.push(pair(raw).with_context(|| format!("in corpus file {file}"))?);
    }
    Ok(pairs)
}

fn pair(raw: RawPair) -> Result<Pair> {
    let id = raw.id;
    ensure!(!raw.source.trim().is_empty(), "pair {id:?} names no source");
    if raw.hard.is_some() && raw.label == Label::Distinct {
        bail!("pair {id:?} is a hard case but labelled distinct; a hard case is a true match");
    }
    let left = profile(raw.left).with_context(|| format!("pair {id:?}, left record"))?;
    let right = profile(raw.right).with_context(|| format!("pair {id:?}, right record"))?;
    Ok(Pair {
        id,
        label: raw.label,
        hard: raw.hard,
        left,
        right,
    })
}

fn profile(record: Record) -> Result<PersonProfile> {
    let name = name(record.given, record.surname).context("a record needs a given name or a surname")?;
    let mut vitals = Vec::new();
    for (kind, event) in [
        (VitalKind::Birth, record.birth),
        (VitalKind::Baptism, record.baptism),
        (VitalKind::Death, record.death),
        (VitalKind::Burial, record.burial),
    ] {
        if let Some(event) = event {
            vitals.push(vital(kind, event)?);
        }
    }
    Ok(PersonProfile {
        names: vec![name],
        sex: record.sex.map(sex),
        vitals,
        occupations: record.occupations,
        parents: relatives(record.parents)?,
        partners: relatives(record.partners)?,
        children: relatives(record.children)?,
        ..PersonProfile::default()
    })
}

fn relatives(kin: Vec<Kin>) -> Result<Vec<Relative>> {
    let mut relatives = Vec::new();
    for relative in kin {
        let name = name(relative.given, relative.surname).context("a relative needs a given name or a surname")?;
        relatives.push(Relative {
            names: vec![name],
            sex: relative.sex.map(sex),
            birth: relative.born.map(|year| VitalEvent {
                kind: VitalKind::Birth,
                date: Some(date(year, None, None)),
                basis: DateBasis::Recorded,
                place: None,
            }),
        });
    }
    Ok(relatives)
}

fn name(given: Option<String>, surname: Option<String>) -> Option<PersonName> {
    if given.is_none() && surname.is_none() {
        return None;
    }
    let mut surnames = Vec::new();
    if let Some(surname) = surname {
        surnames.push(Surname {
            prefix: None,
            surname,
            primary: true,
            connector: None,
        });
    }
    Some(PersonName {
        name_type: NameType::BirthName,
        given,
        surnames,
        suffix: None,
        title: None,
        nickname: None,
        call_name: None,
        date: None,
        language: None,
        transliterations: Vec::new(),
    })
}

fn sex(sex: RecordSex) -> Sex {
    match sex {
        RecordSex::Male => Sex::Male,
        RecordSex::Female => Sex::Female,
    }
}

/// The vital event `event` records, rejecting a date the engine would silently drop or widen: a month
/// outside 1–12, a day outside 1–31, or a day without its month.
fn vital(kind: VitalKind, event: Vital) -> Result<VitalEvent> {
    let year = event.year;
    ensure!(
        event.month.is_none_or(|month| (1..=12).contains(&month)),
        "{kind:?} {year}: month {:?} is not 1-12",
        event.month
    );
    ensure!(
        event.day.is_none_or(|day| (1..=31).contains(&day)),
        "{kind:?} {year}: day {:?} is not 1-31",
        event.day
    );
    ensure!(
        event.day.is_none() || event.month.is_some(),
        "{kind:?} {year}: a day needs a month"
    );
    let basis = match event.basis {
        Basis::Recorded => DateBasis::Recorded,
        Basis::Age => DateBasis::FromAge,
    };
    let place = (event.country.is_some() || event.place.is_some()).then(|| PlaceProfile {
        names: event.place.map_or_else(Vec::new, |text| {
            vec![PlaceName {
                text,
                language: None,
                date: None,
            }]
        }),
        country: event.country,
        ..PlaceProfile::default()
    });
    Ok(VitalEvent {
        kind,
        date: Some(date(event.year, event.month, event.day)),
        basis,
        place,
    })
}

/// A Gregorian date of the given precision; the engine reads its point, never its sort value.
fn date(year: i32, month: Option<u8>, day: Option<u8>) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(year),
            month,
            day,
        })),
        time: None,
        new_year_begins: None,
        sort_value: 0,
        original_text: None,
    }
}
