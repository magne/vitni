//! Structured date editing (view-model): a free-typed date field parsed into a [`DatePoint`], and
//! the [`DateDraft`] the whole-record editor edits (the `event.html` control cluster — modifier ·
//! date · quality · calendar · original text).
//!
//! The parser mirrors the GEDCOM importer's day/month/year reading (`genealogy-gedcom`) without
//! taking a dependency on it: a `DAY MON YEAR` phrase, an ISO `YYYY-MM-DD`, a bare year, or a BCE
//! `-YYYY`. Day-in-month is validated for the Gregorian calendar only; the other calendars carry
//! different month lengths, so a day up to 31 is accepted there unchecked.

use genealogy_app::{Calendar, DatePoint};

/// The three-letter month abbreviations, title-cased for display (index + 1 = month number).
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
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
