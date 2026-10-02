//! What a parsed record means: its residence or church-book event, the census municipality, a
//! birth value, a census family position and a church-book role.
//!
//! The parsers keep the page's text verbatim; these read it into crate-local types the plugin maps
//! onto the host's vocabulary. A value no rule recognizes reads as `None`, never as a guess.

use crate::classify::record_id;
use crate::model::{Heading, PersonRecord};

/// The census residence a person record belongs to, from its `Bosted …` heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residence {
    /// The residence record id, e.g. `"bf01099901000100"`.
    pub id: String,
    /// The residence's name, its leading residence number dropped, e.g. `"Fjellstue"`.
    pub name: String,
    /// True for a rural residence (`/census/rural-residence/`), a farm or holding.
    pub rural: bool,
}

/// The municipality a census covers, from a title like `Folketelling 1920 for 9901 Eksempelvik herred`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Municipality {
    /// The municipality number, e.g. `"9901"`.
    pub code: String,
    /// The municipality's name without its kind, e.g. `"Eksempelvik"`.
    pub name: String,
}

/// The kinds of church-book event a heading names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChurchbookEventKind {
    /// `Fødte og døpte`.
    Baptism,
    /// `Konfirmerte`.
    Confirmation,
    /// `Viede`.
    Marriage,
    /// `Døde og begravde`.
    Burial,
    /// `Innflyttede`.
    Immigration,
    /// `Utflyttede`.
    Emigration,
}

/// The church-book event a record belongs to, from its event heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChurchbookEvent {
    /// The event record id, e.g. `"hd00000099901000"`.
    pub id: String,
    /// The kind of event the heading names.
    pub kind: ChurchbookEventKind,
    /// The heading's value, the event's date as transcribed, e.g. `"1925-02-15"`.
    pub date: String,
}

/// A birth value as transcribed in `Alder/født`, `Fødselsdato` or `Fødselsår`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BirthValue {
    /// A full date.
    Date {
        /// The year.
        year: i32,
        /// The month, 1–12.
        month: u8,
        /// The day, 1–31.
        day: u8,
    },
    /// A year alone.
    Year(i32),
    /// An age at the record's date, from which a birth year is estimated.
    Age {
        /// Whole years, when given.
        years: Option<u16>,
        /// Months, when given (an infant's `3 mnd`).
        months: Option<u16>,
    },
    /// Text no rule reads, kept verbatim.
    Text(String),
}

/// A census family position (`Familiestilling`) that places a person in the household's family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HouseholdPosition {
    /// The head of the household.
    Head,
    /// The head's spouse.
    Spouse,
    /// A child of the household.
    Child,
}

/// A church-book participant role (`Rolle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChurchbookRole {
    /// The person the event is about: the child baptized, the confirmand, the deceased, the mover.
    Primary,
    /// The father.
    Father,
    /// The mother.
    Mother,
    /// A godparent.
    Godparent,
    /// The bride.
    Bride,
    /// The groom.
    Groom,
    /// The officiating priest.
    Clergy,
    /// A witness or best man.
    Witness,
}

/// The census residence `record` belongs to: the heading linking to a residence page.
#[must_use]
pub fn residence(record: &PersonRecord) -> Option<Residence> {
    for heading in &record.source.headings {
        let Some(url) = heading.url.as_deref() else {
            continue;
        };
        let rural = url.contains("/census/rural-residence/");
        if !rural && !url.contains("/census/urban-residence/") {
            continue;
        }
        let id = record_id(url)?;
        return Some(Residence {
            id,
            name: without_number(&heading.value).to_owned(),
            rural,
        });
    }
    None
}

/// The church-book event `record` belongs to: the heading linking to a `/view/<n>/hd…` page whose
/// label names a known kind of event.
#[must_use]
pub fn churchbook_event(record: &PersonRecord) -> Option<ChurchbookEvent> {
    for heading in &record.source.headings {
        let Some(id) = event_id(heading) else {
            continue;
        };
        let kind = churchbook_event_kind(&heading.key)?;
        return Some(ChurchbookEvent {
            id,
            kind,
            date: heading.value.clone(),
        });
    }
    None
}

/// The event record id a heading links to, when it links to a church-book event page.
fn event_id(heading: &Heading) -> Option<String> {
    let url = heading.url.as_deref()?;
    if !url.contains("/view/") {
        return None;
    }
    record_id(url).filter(|id| id.starts_with("hd"))
}

/// The kind of event a church-book heading label names. Stillbirths (`Dødfødte`) name none.
#[must_use]
pub fn churchbook_event_kind(label: &str) -> Option<ChurchbookEventKind> {
    let label = label.to_lowercase();
    if label.contains("dødfødt") {
        return None;
    }
    let rules: [(&[&str], ChurchbookEventKind); 6] = [
        (&["døpte", "fødte"], ChurchbookEventKind::Baptism),
        (&["konfirm"], ChurchbookEventKind::Confirmation),
        (&["viede", "vigde", "vielse"], ChurchbookEventKind::Marriage),
        (&["begrav", "døde"], ChurchbookEventKind::Burial),
        (&["innflytt"], ChurchbookEventKind::Immigration),
        (&["utflytt"], ChurchbookEventKind::Emigration),
    ];
    for (needles, kind) in rules {
        if needles.iter().any(|needle| label.contains(needle)) {
            return Some(kind);
        }
    }
    None
}

/// The words a census title names a municipality's kind with.
const MUNICIPALITY_KINDS: [&str; 6] = ["herred", "by", "kommune", "landsogn", "kjøpstad", "ladested"];

/// The municipality a census title names after `for`: its number, then its name, then its kind.
#[must_use]
pub fn municipality(title: &str) -> Option<Municipality> {
    let (_, rest) = title.split_once(" for ")?;
    let mut words: Vec<&str> = rest.split_whitespace().collect();
    let code = words.first().filter(|word| word.chars().all(|c| c.is_ascii_digit()))?;
    let code = (*code).to_owned();
    words.remove(0);
    if words
        .last()
        .is_some_and(|kind| MUNICIPALITY_KINDS.contains(&kind.to_lowercase().as_str()))
    {
        words.pop();
    }
    if words.is_empty() {
        return None;
    }
    Some(Municipality {
        code,
        name: words.join(" "),
    })
}

/// `value` without a leading residence number: `0012 Fjellstue` is `Fjellstue`.
fn without_number(value: &str) -> &str {
    let value = value.trim();
    match value.split_once(char::is_whitespace) {
        Some((number, rest)) if number.chars().all(|c| c.is_ascii_digit()) && !rest.trim().is_empty() => rest.trim(),
        _ => value,
    }
}

/// Reads a transcribed birth value: `1887-03-14`, `14.03.1887`, `1887`, `33`, `33 år` or `3 mnd`.
/// An empty value or the archive's `-` placeholder reads as `None`.
#[must_use]
pub fn birth_value(value: &str) -> Option<BirthValue> {
    let value = value.trim();
    if value.is_empty() || value == "-" {
        return None;
    }
    if let Some(date) = iso_date(value).or_else(|| dotted_date(value)) {
        return Some(date);
    }
    if value.len() == 4
        && let Ok(year) = value.parse::<i32>()
    {
        return Some(BirthValue::Year(year));
    }
    Some(age(value).unwrap_or_else(|| BirthValue::Text(value.to_owned())))
}

/// A `YYYY-MM-DD` date.
fn iso_date(value: &str) -> Option<BirthValue> {
    let mut parts = value.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 {
        return None;
    }
    date(year, month, day)
}

/// A `DD.MM.YYYY` date.
fn dotted_date(value: &str) -> Option<BirthValue> {
    let mut parts = value.split('.');
    let (day, month, year) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 {
        return None;
    }
    date(year, month, day)
}

/// A date from its parts, when each is a number in range.
fn date(year: &str, month: &str, day: &str) -> Option<BirthValue> {
    let year = year.parse().ok()?;
    let month: u8 = month.parse().ok()?;
    let day: u8 = day.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(BirthValue::Date { year, month, day })
}

/// An age: whole years (`33`, `33 år`) or months (`3 mnd`).
fn age(value: &str) -> Option<BirthValue> {
    let mut words = value.split_whitespace();
    let number: u16 = words.next()?.parse().ok()?;
    let unit = words.next().map(str::to_lowercase);
    if words.next().is_some() {
        return None;
    }
    match unit.as_deref() {
        None | Some("år") => Some(BirthValue::Age {
            years: Some(number),
            months: None,
        }),
        Some("mnd" | "mnd." | "måneder") => Some(BirthValue::Age {
            years: None,
            months: Some(number),
        }),
        Some(_) => None,
    }
}

/// The household family position a census `Familiestilling` code names. A code for anyone else in
/// the household — a servant, a lodger, a relative — reads as `None`.
#[must_use]
pub fn household_position(code: &str) -> Option<HouseholdPosition> {
    let code = code.trim().trim_end_matches('.').to_lowercase();
    match code.as_str() {
        "hp" | "hf" | "husfar" | "husbond" | "hovedperson" => Some(HouseholdPosition::Head),
        "hm" | "hu" | "husmor" | "hustru" | "kone" => Some(HouseholdPosition::Spouse),
        "s" | "d" | "sønn" | "søn" | "datter" | "barn" => Some(HouseholdPosition::Child),
        _ => None,
    }
}

/// Where `record`'s person joins its household's family: its family position, a head only when the
/// household lists someone besides them (a head living alone founds no family).
#[must_use]
pub fn family_position(record: &PersonRecord) -> Option<HouseholdPosition> {
    let position = household_position(record.role.as_deref()?)?;
    match position {
        HouseholdPosition::Head if record.household.len() < 2 => None,
        HouseholdPosition::Head | HouseholdPosition::Spouse | HouseholdPosition::Child => Some(position),
    }
}

/// The participant role a church-book `Rolle` names.
#[must_use]
pub fn churchbook_role(role: &str) -> Option<ChurchbookRole> {
    let role = role.trim().to_lowercase();
    match role.as_str() {
        "barn" | "dåpsbarn" | "konfirmant" | "død" | "avdød" | "døde" | "innflytter" | "utflytter" => {
            Some(ChurchbookRole::Primary)
        }
        "far" => Some(ChurchbookRole::Father),
        "mor" => Some(ChurchbookRole::Mother),
        "fadder" => Some(ChurchbookRole::Godparent),
        "brud" => Some(ChurchbookRole::Bride),
        "brudgom" => Some(ChurchbookRole::Groom),
        "prest" => Some(ChurchbookRole::Clergy),
        "vitne" | "forlover" => Some(ChurchbookRole::Witness),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::interpret::{
        BirthValue, ChurchbookEventKind, ChurchbookRole, HouseholdPosition, Municipality, birth_value,
        churchbook_event_kind, churchbook_role, household_position, municipality, without_number,
    };

    #[test]
    fn birth_value_reads_dates_years_and_ages() {
        let date = BirthValue::Date {
            year: 1887,
            month: 3,
            day: 14,
        };
        assert_eq!(birth_value("1887-03-14"), Some(date.clone()));
        assert_eq!(birth_value(" 14.03.1887 "), Some(date));
        assert_eq!(birth_value("1887"), Some(BirthValue::Year(1887)));
        let years = BirthValue::Age {
            years: Some(33),
            months: None,
        };
        assert_eq!(birth_value("33"), Some(years.clone()));
        assert_eq!(birth_value("33 år"), Some(years));
        assert_eq!(
            birth_value("3 mnd"),
            Some(BirthValue::Age {
                years: None,
                months: Some(3),
            })
        );
    }

    #[test]
    fn birth_value_keeps_what_it_cannot_read() {
        assert_eq!(birth_value(""), None);
        assert_eq!(birth_value(" - "), None);
        assert_eq!(birth_value("ca. 1887"), Some(BirthValue::Text("ca. 1887".to_owned())));
        assert_eq!(
            birth_value("1887-13-01"),
            Some(BirthValue::Text("1887-13-01".to_owned()))
        );
        assert_eq!(birth_value("3 uker"), Some(BirthValue::Text("3 uker".to_owned())));
        assert_eq!(birth_value("33 år gl"), Some(BirthValue::Text("33 år gl".to_owned())));
    }

    #[test]
    fn household_position_reads_head_spouse_and_child_codes() {
        assert_eq!(household_position("hp"), Some(HouseholdPosition::Head));
        assert_eq!(household_position(" Husfar "), Some(HouseholdPosition::Head));
        assert_eq!(household_position("hm"), Some(HouseholdPosition::Spouse));
        assert_eq!(household_position("Hustru"), Some(HouseholdPosition::Spouse));
        assert_eq!(household_position("s"), Some(HouseholdPosition::Child));
        assert_eq!(household_position("d."), Some(HouseholdPosition::Child));
        assert_eq!(household_position("Datter"), Some(HouseholdPosition::Child));
    }

    #[test]
    fn household_position_leaves_the_rest_of_the_household_out() {
        assert_eq!(household_position("tj"), None);
        assert_eq!(household_position("fl"), None);
        assert_eq!(household_position(""), None);
    }

    #[test]
    fn churchbook_role_reads_known_roles_only() {
        assert_eq!(churchbook_role("far"), Some(ChurchbookRole::Father));
        assert_eq!(churchbook_role(" Mor"), Some(ChurchbookRole::Mother));
        assert_eq!(churchbook_role("barn"), Some(ChurchbookRole::Primary));
        assert_eq!(churchbook_role("Fadder"), Some(ChurchbookRole::Godparent));
        assert_eq!(churchbook_role("brudgom"), Some(ChurchbookRole::Groom));
        assert_eq!(churchbook_role("brud"), Some(ChurchbookRole::Bride));
        assert_eq!(churchbook_role("prest"), Some(ChurchbookRole::Clergy));
        assert_eq!(churchbook_role("forlover"), Some(ChurchbookRole::Witness));
        assert_eq!(churchbook_role("husbonde"), None);
        assert_eq!(churchbook_role(""), None);
    }

    #[test]
    fn churchbook_event_kind_reads_the_heading_label() {
        assert_eq!(
            churchbook_event_kind("Fødte og døpte"),
            Some(ChurchbookEventKind::Baptism)
        );
        assert_eq!(
            churchbook_event_kind("Konfirmerte"),
            Some(ChurchbookEventKind::Confirmation)
        );
        assert_eq!(churchbook_event_kind("Viede"), Some(ChurchbookEventKind::Marriage));
        assert_eq!(
            churchbook_event_kind("Døde og begravde"),
            Some(ChurchbookEventKind::Burial)
        );
        assert_eq!(
            churchbook_event_kind("Innflyttede"),
            Some(ChurchbookEventKind::Immigration)
        );
        assert_eq!(
            churchbook_event_kind("Utflyttede"),
            Some(ChurchbookEventKind::Emigration)
        );
        assert_eq!(churchbook_event_kind("Tellingskrets"), None);
        assert_eq!(churchbook_event_kind("Dødfødte"), None);
    }

    #[test]
    fn municipality_reads_the_census_title() {
        assert_eq!(
            municipality("Folketelling 1920 for 9901 Eksempelvik herred"),
            Some(Municipality {
                code: "9901".to_owned(),
                name: "Eksempelvik".to_owned(),
            })
        );
        assert_eq!(
            municipality("Folketelling 1910 for 0301 Kristiania kjøpstad"),
            Some(Municipality {
                code: "0301".to_owned(),
                name: "Kristiania".to_owned(),
            })
        );
        assert_eq!(municipality("Klokkerbok for Eksempelvik prestegjeld"), None);
        assert_eq!(municipality("Folketelling 1920 for 9901 herred"), None);
        assert_eq!(municipality("Folketelling 1920"), None);
    }

    #[test]
    fn without_number_drops_a_leading_residence_number() {
        assert_eq!(without_number("0012 Fjellstue"), "Fjellstue");
        assert_eq!(without_number("Fjellstue"), "Fjellstue");
        assert_eq!(without_number("0012"), "0012");
        assert_eq!(without_number("Nedre Fjellstue"), "Nedre Fjellstue");
    }
}
