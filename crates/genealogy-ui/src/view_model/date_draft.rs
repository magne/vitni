//! Structured date editing (view-model): a free-typed date field parsed into a [`DatePoint`], and
//! the [`DateDraft`] the whole-record editor edits (the `event.html` control cluster — modifier ·
//! date · quality · calendar · original text).
//!
//! The parser mirrors the GEDCOM importer's day/month/year reading (`genealogy-gedcom`) without
//! taking a dependency on it: a `DAY MON YEAR` phrase, an ISO `YYYY-MM-DD`, a bare year, or a BCE
//! `-YYYY`. Day-in-month is validated for the Gregorian calendar only; the other calendars carry
//! different month lengths, so a day up to 31 is accepted there unchecked.

use genealogy_app::{
    Calendar, DateInput, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody, TimeOfDay,
};

use crate::view_model::common::non_blank;

/// The three-letter month abbreviations, title-cased for display (index + 1 = month number).
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The date qualities the editor offers, in display order (the `event.html` Quality select).
pub const DATE_QUALITIES: [DateQuality; 3] = [DateQuality::Normal, DateQuality::Estimated, DateQuality::Calculated];

/// The calendars the editor offers, in display order (the `event.html` Calendar select).
pub const DATE_CALENDARS: [Calendar; 6] = [
    Calendar::Gregorian,
    Calendar::Julian,
    Calendar::Hebrew,
    Calendar::FrenchRepublican,
    Calendar::Islamic,
    Calendar::Swedish,
];

/// Why a typed date could not be read into a [`DatePoint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateEntryError {
    /// The text carried no recognisable year (garbage, or an empty field).
    Unparseable,
    /// The day is out of range for the month (e.g. `31 Feb`) on the Gregorian calendar.
    ImpossibleDay,
}

/// Parses a typed date into a (possibly partial) [`DatePoint`]; the year is required.
///
/// Accepts `DAY MON YEAR` / `MON YEAR` / `YEAR` phrases, an ISO `YYYY-MM-DD` / `YYYY-MM` / `YYYY`,
/// and a BCE `-YYYY`. Case-insensitive on the month. Day-in-month is checked only for
/// [`Calendar::Gregorian`].
///
/// # Errors
///
/// [`DateEntryError::Unparseable`] when no year is found, [`DateEntryError::ImpossibleDay`] when a
/// Gregorian day exceeds its month's length.
pub fn parse_date_point(text: &str, calendar: Calendar) -> Result<DatePoint, DateEntryError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(DateEntryError::Unparseable);
    }
    let point = if is_numeric_iso(trimmed) {
        parse_iso(trimmed)
    } else {
        parse_phrase(trimmed)
    }
    .ok_or(DateEntryError::Unparseable)?;
    validate(point, calendar)?;
    Ok(point)
}

/// Renders a [`DatePoint`] back to the field's `DAY MON YEAR` / `MON YEAR` / `YEAR` text (empty when
/// the year is unknown). Round-trips with [`parse_date_point`].
#[must_use]
pub fn format_date_point(point: &DatePoint) -> String {
    let Some(year) = point.year else {
        return String::new();
    };
    let mut text = String::new();
    if let Some(day) = point.day
        && point.month.is_some()
    {
        text.push_str(&day.to_string());
        text.push(' ');
    }
    if let Some(month) = point.month
        && let Some(name) = MONTHS.get(usize::from(month.saturating_sub(1)))
    {
        text.push_str(name);
        text.push(' ');
    }
    text.push_str(&year.to_string());
    text
}

/// Whether the text is a numeric/ISO form (digits and `-` only) rather than a `DAY MON YEAR` phrase.
fn is_numeric_iso(text: &str) -> bool {
    text.chars().all(|c| c.is_ascii_digit() || c == '-')
}

/// Parses an ISO `YYYY-MM-DD` / `YYYY-MM` / `YYYY` or a BCE `-YYYY` (a leading `-` is the year sign).
fn parse_iso(text: &str) -> Option<DatePoint> {
    let (negative, rest) = text.strip_prefix('-').map_or((false, text), |rest| (true, rest));
    let mut fields = rest.split('-');
    let year: i32 = fields.next()?.parse().ok()?;
    let month = match fields.next() {
        None => None,
        Some(value) => Some(value.parse::<u8>().ok()?),
    };
    let day = match fields.next() {
        None => None,
        Some(value) => Some(value.parse::<u8>().ok()?),
    };
    if fields.next().is_some() {
        return None;
    }
    Some(DatePoint {
        year: Some(if negative { -year } else { year }),
        month,
        day,
    })
}

/// Parses a `DAY MON YEAR` phrase (any order of the recognised tokens); the year is required.
fn parse_phrase(text: &str) -> Option<DatePoint> {
    let mut point = DatePoint {
        year: None,
        month: None,
        day: None,
    };
    for token in text.split_whitespace() {
        if let Some(month) = month_number(token) {
            point.month = Some(month);
        } else if let Some(year) = parse_year(token) {
            point.year = Some(year);
        } else if let Ok(day) = token.parse::<u8>()
            && (1..=31).contains(&day)
            && point.day.is_none()
            && point.year.is_none()
        {
            point.day = Some(day);
        }
    }
    point.year.map(|_| point)
}

/// Maps a three-letter month abbreviation to its number (case-insensitive).
fn month_number(token: &str) -> Option<u8> {
    MONTHS
        .iter()
        .position(|month| month.eq_ignore_ascii_case(token))
        .and_then(|index| u8::try_from(index + 1).ok())
}

/// Parses a year token; a bare 1–31 is a day, not a year.
fn parse_year(token: &str) -> Option<i32> {
    let year = token.parse::<i32>().ok()?;
    if (1..=31).contains(&year) { None } else { Some(year) }
}

/// Rejects an out-of-range day for its Gregorian month; other calendars are accepted unchecked.
fn validate(point: DatePoint, calendar: Calendar) -> Result<(), DateEntryError> {
    let (Some(month), Some(day)) = (point.month, point.day) else {
        return Ok(());
    };
    match calendar {
        Calendar::Gregorian => {
            let limit = gregorian_days_in_month(point.year.unwrap_or(0), month);
            if day >= 1 && day <= limit {
                Ok(())
            } else {
                Err(DateEntryError::ImpossibleDay)
            }
        }
        Calendar::Julian | Calendar::Hebrew | Calendar::FrenchRepublican | Calendar::Islamic | Calendar::Swedish => {
            Ok(())
        }
    }
}

/// The number of days in a Gregorian month, honouring leap years for February.
fn gregorian_days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Whether a Gregorian year is a leap year.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Which date modifier the editor is expressing — the `event.html` Modifier select. `Interpreted`
/// is not one of the nine offered options: it only appears when a seeded date already carries an
/// interpreted phrase, and switching away from it drops the phrase (an explicit user action).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateModifierKind {
    /// An exact (single) date.
    #[default]
    Exact,
    /// Before the given date.
    Before,
    /// After the given date.
    After,
    /// Approximately the given date.
    About,
    /// Somewhere between two dates (uncertainty) — uses the end field.
    Range,
    /// A span covering a stretch of time (duration) — uses the end field.
    Span,
    /// From the given date (open-ended period start).
    From,
    /// To the given date (open-ended period end).
    To,
    /// A free-text date supplied through the Original-text field.
    TextOnly,
    /// A date interpreted from a free-text phrase (GEDCOM `INT`); only offered when seeded.
    Interpreted,
}

impl DateModifierKind {
    /// The nine modifier options the editor always offers (the `event.html` Modifier select).
    #[must_use]
    pub fn all_offered() -> Vec<Self> {
        vec![
            Self::Exact,
            Self::Before,
            Self::After,
            Self::About,
            Self::Range,
            Self::Span,
            Self::From,
            Self::To,
            Self::TextOnly,
        ]
    }

    /// The choices to render for this draft: the nine offered options, plus `Interpreted` appended
    /// when the current kind is `Interpreted` (a seeded value that survives an untouched save).
    #[must_use]
    pub fn choices_for(&self) -> Vec<Self> {
        let mut choices = Self::all_offered();
        if *self == Self::Interpreted {
            choices.push(Self::Interpreted);
        }
        choices
    }

    /// Whether this kind uses the end (second) date field.
    #[must_use]
    pub fn uses_end(&self) -> bool {
        matches!(self, Self::Range | Self::Span)
    }
}

/// The whole-record editor's structured date (the `event.html` control cluster): a modifier, one or
/// two typed date points, a quality, a calendar, and the always-retained Original-text field.
///
/// Passthrough fields (`interpreted_phrase`, `new_year_begins`, `time`) ride through an untouched
/// edit so a seeded date round-trips unchanged (a `from_value` → `to_input` → `build_genealogical_date`
/// reproduces an equal [`GenealogicalDate`]). `display` is the localized read-box string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateDraft {
    /// The modifier the editor is expressing.
    pub kind: DateModifierKind,
    /// The (first) typed date point — `DAY MON YEAR` / ISO / bare year.
    pub start: String,
    /// The end date point (Range/Span only; ignored otherwise).
    pub end: String,
    /// The date's reliability.
    pub quality: DateQuality,
    /// The calendar the date is expressed in.
    pub calendar: Calendar,
    /// The verbatim source string — always retained, and the sole date input when kind is `TextOnly`.
    pub original_text: String,
    /// The interpreted phrase a seeded `Interpreted` date carries; dropped if the user switches kind.
    pub interpreted_phrase: Option<String>,
    /// Month in which the year begins, for dual / old-style dating; rides through untouched.
    pub new_year_begins: Option<u8>,
    /// An optional time of day on an exact date; rides through untouched.
    pub time: Option<TimeOfDay>,
    /// The localized read-box display string (seeded from the record; not editable here).
    pub display: String,
}

impl Default for DateDraft {
    fn default() -> Self {
        Self {
            kind: DateModifierKind::default(),
            start: String::new(),
            end: String::new(),
            quality: DateQuality::Normal,
            calendar: Calendar::Gregorian,
            original_text: String::new(),
            interpreted_phrase: None,
            new_year_begins: None,
            time: None,
            display: String::new(),
        }
    }
}

impl DateDraft {
    /// Seeds a draft from an existing [`GenealogicalDate`] and its localized `display` string.
    #[must_use]
    pub fn from_value(value: &GenealogicalDate, display: String) -> Self {
        let mut draft = Self {
            quality: value.quality,
            calendar: value.calendar,
            original_text: value.original_text.clone().unwrap_or_default(),
            new_year_begins: value.new_year_begins,
            time: value.time,
            display,
            ..Self::default()
        };
        match &value.modifier {
            GenealogicalDateBody::Structured(modifier) => seed_from_modifier(&mut draft, modifier),
            GenealogicalDateBody::TextOnly { text } => {
                draft.kind = DateModifierKind::TextOnly;
                if draft.original_text.is_empty() {
                    text.clone_into(&mut draft.original_text);
                }
            }
        }
        draft
    }

    /// Whether the draft carries no date at all — an untouched default (the seed had no date). A
    /// blank draft emits no `SetDate` (`to_input` returns `Ok(None)`).
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.kind == DateModifierKind::Exact
            && self.start.trim().is_empty()
            && self.end.trim().is_empty()
            && self.original_text.trim().is_empty()
            && self.interpreted_phrase.is_none()
    }

    /// Whether the draft is invalid — a non-blank draft whose date text does not parse (or a
    /// `TextOnly` kind with a blank Original-text field). Drives `aria-invalid` + the field error.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.to_input().is_err()
    }

    /// Builds the [`DateInput`] the app asserts on Save, or `Ok(None)` when the draft is blank (no
    /// `SetDate` is then emitted).
    ///
    /// # Errors
    ///
    /// [`DateEntryError`] when a required date point does not parse, or a `TextOnly` kind has a
    /// blank Original-text field.
    pub fn to_input(&self) -> Result<Option<DateInput>, DateEntryError> {
        if self.is_blank() {
            return Ok(None);
        }
        let (body, original_text) = self.build_body()?;
        Ok(Some(DateInput {
            calendar: self.calendar,
            quality: self.quality,
            body,
            new_year_begins: self.new_year_begins,
            original_text,
            time: self.time,
        }))
    }

    /// Builds the date body + the retained original text for the current kind.
    fn build_body(&self) -> Result<(GenealogicalDateBody, Option<String>), DateEntryError> {
        if self.kind == DateModifierKind::TextOnly {
            let text = self.original_text.trim();
            if text.is_empty() {
                return Err(DateEntryError::Unparseable);
            }
            return Ok((
                GenealogicalDateBody::TextOnly { text: text.to_owned() },
                Some(text.to_owned()),
            ));
        }
        let start = parse_date_point(&self.start, self.calendar)?;
        let modifier = match self.kind {
            DateModifierKind::Exact => DateModifier::None(start),
            DateModifierKind::Before => DateModifier::Before(start),
            DateModifierKind::After => DateModifier::After(start),
            DateModifierKind::About => DateModifier::About(start),
            DateModifierKind::From => DateModifier::From(start),
            DateModifierKind::To => DateModifier::To(start),
            DateModifierKind::Range => DateModifier::Range {
                start,
                end: parse_date_point(&self.end, self.calendar)?,
            },
            DateModifierKind::Span => DateModifier::Span {
                start,
                end: parse_date_point(&self.end, self.calendar)?,
            },
            DateModifierKind::Interpreted => DateModifier::Interpreted {
                date: start,
                phrase: self.interpreted_phrase.clone().unwrap_or_default(),
            },
            DateModifierKind::TextOnly => unreachable!("handled above"),
        };
        Ok((
            GenealogicalDateBody::Structured(modifier),
            non_blank(&self.original_text),
        ))
    }
}

/// Seeds the kind + date text fields from a structured [`DateModifier`].
fn seed_from_modifier(draft: &mut DateDraft, modifier: &DateModifier) {
    match modifier {
        DateModifier::None(point) => {
            draft.kind = DateModifierKind::Exact;
            draft.start = format_date_point(point);
        }
        DateModifier::Before(point) => {
            draft.kind = DateModifierKind::Before;
            draft.start = format_date_point(point);
        }
        DateModifier::After(point) => {
            draft.kind = DateModifierKind::After;
            draft.start = format_date_point(point);
        }
        DateModifier::About(point) => {
            draft.kind = DateModifierKind::About;
            draft.start = format_date_point(point);
        }
        DateModifier::From(point) => {
            draft.kind = DateModifierKind::From;
            draft.start = format_date_point(point);
        }
        DateModifier::To(point) => {
            draft.kind = DateModifierKind::To;
            draft.start = format_date_point(point);
        }
        DateModifier::Range { start, end } => {
            draft.kind = DateModifierKind::Range;
            draft.start = format_date_point(start);
            draft.end = format_date_point(end);
        }
        DateModifier::Span { start, end } => {
            draft.kind = DateModifierKind::Span;
            draft.start = format_date_point(start);
            draft.end = format_date_point(end);
        }
        DateModifier::Interpreted { date, phrase } => {
            draft.kind = DateModifierKind::Interpreted;
            draft.start = format_date_point(date);
            draft.interpreted_phrase = Some(phrase.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use genealogy_app::{Calendar, DatePoint};

    use crate::view_model::date_draft::{DateEntryError, format_date_point, parse_date_point};

    fn point(year: i32, month: Option<u8>, day: Option<u8>) -> DatePoint {
        DatePoint {
            year: Some(year),
            month,
            day,
        }
    }

    #[test]
    fn parses_day_month_year() {
        let parsed = parse_date_point("14 Jun 1876", Calendar::Gregorian).unwrap();
        assert_eq!(parsed, point(1876, Some(6), Some(14)));
    }

    #[test]
    fn parses_month_year() {
        let parsed = parse_date_point("Jun 1876", Calendar::Gregorian).unwrap();
        assert_eq!(parsed, point(1876, Some(6), None));
    }

    #[test]
    fn parses_year_only() {
        let parsed = parse_date_point("1876", Calendar::Gregorian).unwrap();
        assert_eq!(parsed, point(1876, None, None));
    }

    #[test]
    fn parses_iso_numeric() {
        assert_eq!(
            parse_date_point("1876-06-14", Calendar::Gregorian).unwrap(),
            point(1876, Some(6), Some(14))
        );
        assert_eq!(
            parse_date_point("1876-06", Calendar::Gregorian).unwrap(),
            point(1876, Some(6), None)
        );
    }

    #[test]
    fn parses_bce_year() {
        assert_eq!(
            parse_date_point("-44", Calendar::Gregorian).unwrap(),
            point(-44, None, None)
        );
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(
            parse_date_point("14 jun 1876", Calendar::Gregorian).unwrap(),
            parse_date_point("14 JUN 1876", Calendar::Gregorian).unwrap()
        );
    }

    #[test]
    fn rejects_impossible_day_in_month() {
        assert_eq!(
            parse_date_point("31 Feb 1850", Calendar::Gregorian),
            Err(DateEntryError::ImpossibleDay)
        );
    }

    #[test]
    fn accepts_leap_day() {
        assert_eq!(
            parse_date_point("29 Feb 1848", Calendar::Gregorian).unwrap(),
            point(1848, Some(2), Some(29))
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(
            parse_date_point("not a date at all", Calendar::Gregorian),
            Err(DateEntryError::Unparseable)
        );
    }

    #[test]
    fn allows_day_31_on_non_gregorian_calendars() {
        assert_eq!(
            parse_date_point("31 Feb 1850", Calendar::Hebrew).unwrap(),
            point(1850, Some(2), Some(31))
        );
    }

    #[test]
    fn formatting_then_parsing_round_trips() {
        for original in [
            point(1876, Some(6), Some(14)),
            point(1876, Some(6), None),
            point(1876, None, None),
        ] {
            let text = format_date_point(&original);
            let parsed = parse_date_point(&text, Calendar::Gregorian).unwrap();
            assert_eq!(parsed, original, "round-trip of {text:?}");
        }
    }
}

#[cfg(test)]
mod draft_tests {
    use genealogy_app::{
        Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody, TimeOfDay,
        build_genealogical_date,
    };

    use crate::view_model::date_draft::{DateDraft, DateModifierKind};

    fn point(year: i32, month: u8, day: u8) -> DatePoint {
        DatePoint {
            year: Some(year),
            month: Some(month),
            day: Some(day),
        }
    }

    fn structured(modifier: DateModifier) -> GenealogicalDate {
        build_genealogical_date(genealogy_app::DateInput {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            body: GenealogicalDateBody::Structured(modifier),
            new_year_begins: None,
            original_text: Some("14 June 1876".to_owned()),
            time: None,
        })
    }

    fn round_trip(seed: &GenealogicalDate) -> GenealogicalDate {
        let draft = DateDraft::from_value(seed, "display".to_owned());
        let input = draft.to_input().expect("valid").expect("some input");
        build_genealogical_date(input)
    }

    #[test]
    fn an_empty_draft_builds_no_input() {
        assert_eq!(DateDraft::default().to_input(), Ok(None));
        assert!(!DateDraft::default().is_invalid());
    }

    #[test]
    fn exact_date_round_trips() {
        let seed = structured(DateModifier::None(point(1876, 6, 14)));
        assert_eq!(round_trip(&seed), seed);
    }

    #[test]
    fn range_requires_both_points() {
        let only_start = DateDraft {
            kind: DateModifierKind::Range,
            start: "1876".to_owned(),
            ..DateDraft::default()
        };
        assert!(only_start.is_invalid(), "a range with no end is invalid");
        let both = DateDraft {
            kind: DateModifierKind::Range,
            start: "1876".to_owned(),
            end: "1880".to_owned(),
            ..DateDraft::default()
        };
        assert!(!both.is_invalid(), "a range with both points is valid");
    }

    #[test]
    fn span_round_trips() {
        let seed = structured(DateModifier::Span {
            start: point(1876, 6, 14),
            end: point(1880, 1, 1),
        });
        assert_eq!(round_trip(&seed), seed);
    }

    #[test]
    fn before_after_about_from_to_round_trip() {
        let cases = [
            DateModifier::Before(point(1876, 6, 14)),
            DateModifier::After(point(1876, 6, 14)),
            DateModifier::About(point(1876, 6, 14)),
            DateModifier::From(point(1876, 6, 14)),
            DateModifier::To(point(1876, 6, 14)),
        ];
        for modifier in cases {
            let seed = structured(modifier.clone());
            assert_eq!(round_trip(&seed), seed, "round-trip of {modifier:?}");
        }
    }

    #[test]
    fn text_only_takes_the_original_text() {
        let draft = DateDraft {
            kind: DateModifierKind::TextOnly,
            original_text: "  harvest time, 1850  ".to_owned(),
            ..DateDraft::default()
        };
        let input = draft.to_input().expect("valid").expect("some input");
        assert_eq!(
            input.body,
            GenealogicalDateBody::TextOnly {
                text: "harvest time, 1850".to_owned()
            }
        );
        assert_eq!(input.original_text.as_deref(), Some("harvest time, 1850"));
    }

    #[test]
    fn text_only_with_blank_text_is_invalid() {
        let draft = DateDraft {
            kind: DateModifierKind::TextOnly,
            original_text: "   ".to_owned(),
            ..DateDraft::default()
        };
        assert!(draft.is_invalid());
    }

    #[test]
    fn interpreted_round_trips_untouched() {
        let seed = structured(DateModifier::Interpreted {
            date: point(1944, 6, 6),
            phrase: "the day of the landings".to_owned(),
        });
        let draft = DateDraft::from_value(&seed, "display".to_owned());
        assert_eq!(draft.kind, DateModifierKind::Interpreted);
        assert_eq!(draft.kind.choices_for().last(), Some(&DateModifierKind::Interpreted));
        assert_eq!(round_trip(&seed), seed);
    }

    #[test]
    fn new_year_begins_survives_an_untouched_edit() {
        let mut seed = structured(DateModifier::None(point(1735, 3, 25)));
        seed.new_year_begins = Some(3);
        let seed = build_genealogical_date(genealogy_app::DateInput {
            calendar: seed.calendar,
            quality: seed.quality,
            body: seed.modifier,
            new_year_begins: Some(3),
            original_text: seed.original_text,
            time: None,
        });
        assert_eq!(round_trip(&seed).new_year_begins, Some(3));
    }

    #[test]
    fn time_survives_an_untouched_edit() {
        let time = TimeOfDay {
            hour: 9,
            minute: 30,
            second: None,
        };
        let seed = build_genealogical_date(genealogy_app::DateInput {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            body: GenealogicalDateBody::Structured(DateModifier::None(point(1876, 6, 14))),
            new_year_begins: None,
            original_text: None,
            time: Some(time),
        });
        assert_eq!(round_trip(&seed).time, Some(time));
    }

    #[test]
    fn invalid_start_text_reports_invalid() {
        let draft = DateDraft {
            kind: DateModifierKind::Exact,
            start: "gibberish".to_owned(),
            ..DateDraft::default()
        };
        assert!(draft.is_invalid());
    }

    #[test]
    fn end_field_only_used_by_range_and_span() {
        let exact_with_garbage_end = DateDraft {
            kind: DateModifierKind::Exact,
            start: "1876".to_owned(),
            end: "gibberish".to_owned(),
            ..DateDraft::default()
        };
        assert!(
            !exact_with_garbage_end.is_invalid(),
            "the end field is ignored for a non-range kind"
        );
        let range_with_garbage_end = DateDraft {
            kind: DateModifierKind::Range,
            start: "1876".to_owned(),
            end: "gibberish".to_owned(),
            ..DateDraft::default()
        };
        assert!(range_with_garbage_end.is_invalid(), "a range parses its end field");
    }
}
