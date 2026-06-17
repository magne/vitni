//! [`GenealogicalDate`] — a structured, calendar-aware, uncertainty-aware date (data-model §7.1).
//!
//! Dates are the most error-prone part of any genealogy model, so we keep a structured form
//! (never a bare string): an explicit calendar, a modifier for ranges/approximations, a quality,
//! optionally-partial components, honest dual-dating, a precomputed sort key, and the verbatim
//! source text so an unparseable date is never lost.

use serde::{Deserialize, Serialize};

/// The calendar a date is expressed in (Gramps' set — data-model §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Calendar {
    /// The proleptic Gregorian calendar.
    Gregorian,
    /// The Julian calendar.
    Julian,
    /// The Hebrew calendar.
    Hebrew,
    /// The French Republican calendar.
    FrenchRepublican,
    /// The Islamic (Hijri) calendar.
    Islamic,
    /// The Swedish calendar.
    Swedish,
}

/// How reliable the date is (Gramps `QUAL_*` — data-model §7.1).
///
/// `QUAL_INTERPRETED` is defined-but-unused in Gramps; we omit it rather than carry a dead variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateQuality {
    /// A normal, asserted date.
    Normal,
    /// An estimated date.
    Estimated,
    /// A date calculated from other facts.
    Calculated,
}

/// A single, possibly-partial point on a calendar; `year` may be negative for BCE (data-model §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatePoint {
    /// The year; negative for BCE. `None` if unknown.
    pub year: Option<i32>,
    /// The month, 1–12. `None` if unknown.
    pub month: Option<u8>,
    /// The day, 1–31. `None` if unknown.
    pub day: Option<u8>,
}

/// How a date is qualified: exact, open-ended, approximate, or a range/span (Gramps `MOD_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "modifier")]
pub enum DateModifier {
    /// An exact (single) date.
    None(DatePoint),
    /// Before the given date.
    Before(DatePoint),
    /// After the given date.
    After(DatePoint),
    /// Approximately the given date.
    About(DatePoint),
    /// Somewhere between two dates (uncertainty).
    Range {
        /// The earliest possible date.
        start: DatePoint,
        /// The latest possible date.
        end: DatePoint,
    },
    /// A span covering a stretch of time (duration).
    Span {
        /// The start of the span.
        start: DatePoint,
        /// The end of the span.
        end: DatePoint,
    },
    /// From the given date (open-ended period start).
    From(DatePoint),
    /// To the given date (open-ended period end).
    To(DatePoint),
}

/// A structured genealogical date (data-model §7.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenealogicalDate {
    /// The calendar the date is expressed in.
    pub calendar: Calendar,
    /// The reliability of the date.
    pub quality: DateQuality,
    /// The date itself, possibly a range/span/approximation, or free text if unparseable.
    pub modifier: GenealogicalDateBody,
    /// Month in which the year begins, for dual / old-style dating (e.g. 1735/6).
    pub new_year_begins: Option<u8>,
    /// A precomputed integer ordering key (supplied by the application — data-model §7.1).
    pub sort_value: i64,
    /// The verbatim source text, always retained even when unparseable (GEDCOM 7 date phrase).
    pub original_text: Option<String>,
}

/// Either a structured [`DateModifier`] or, when parsing failed, free text (Gramps `MOD_TEXTONLY`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GenealogicalDateBody {
    /// A structured, ordered date.
    Structured(DateModifier),
    /// An unparseable date kept verbatim as text.
    TextOnly {
        /// The free-text date.
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};

    fn day(year: i32, month: u8, day: u8) -> DatePoint {
        DatePoint {
            year: Some(year),
            month: Some(month),
            day: Some(day),
        }
    }

    #[test]
    fn exact_date_round_trips_through_json() {
        let date = GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(day(1847, 3, 12))),
            new_year_begins: None,
            sort_value: 18_470_312,
            original_text: Some("12 March 1847".to_owned()),
        };
        let json = serde_json::to_string(&date).unwrap();
        let back: GenealogicalDate = serde_json::from_str(&json).unwrap();
        assert_eq!(date, back);
    }

    #[test]
    fn unparseable_date_is_retained_as_text() {
        let date = GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Estimated,
            modifier: GenealogicalDateBody::TextOnly {
                text: "harvest time, the year of the great flood".to_owned(),
            },
            new_year_begins: None,
            sort_value: 0,
            original_text: Some("harvest time, the year of the great flood".to_owned()),
        };
        let json = serde_json::to_string(&date).unwrap();
        let back: GenealogicalDate = serde_json::from_str(&json).unwrap();
        assert_eq!(date, back);
    }

    #[test]
    fn partial_date_allows_missing_components() {
        let point = DatePoint {
            year: Some(1847),
            month: None,
            day: None,
        };
        assert_eq!(point.year, Some(1847));
        assert_eq!(point.month, None);
    }
}
