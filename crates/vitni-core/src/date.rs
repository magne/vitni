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

/// A time of day on an exact date (GEDCOM 7 `TIME`; data-model §7.1).
///
/// Optional on a [`GenealogicalDate`] — most genealogical dates have no time, but vital records
/// and timestamps do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeOfDay {
    /// The hour, 0–23.
    pub hour: u8,
    /// The minute, 0–59.
    pub minute: u8,
    /// The second, 0–59. `None` when only hour and minute are recorded.
    pub second: Option<u8>,
}

/// How a date is qualified: exact, open-ended, approximate, or a range/span (Gramps `MOD_*`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// A date interpreted from a free-text phrase (GEDCOM `INT`): the editor's structured reading
    /// plus the verbatim phrase it was interpreted from.
    Interpreted {
        /// The interpreted, structured date.
        date: DatePoint,
        /// The verbatim phrase the date was interpreted from.
        phrase: String,
    },
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
    /// An optional time of day on an exact date (GEDCOM 7 `TIME`). Defaults to `None` so events
    /// recorded before this field existed still decode (ADR 0004 §4).
    #[serde(default)]
    pub time: Option<TimeOfDay>,
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
    use super::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody, TimeOfDay};

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
            time: None,
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
            time: None,
            new_year_begins: None,
            sort_value: 0,
            original_text: Some("harvest time, the year of the great flood".to_owned()),
        };
        let json = serde_json::to_string(&date).unwrap();
        let back: GenealogicalDate = serde_json::from_str(&json).unwrap();
        assert_eq!(date, back);
    }

    #[test]
    fn date_with_time_round_trips() {
        let date = GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(day(1847, 3, 12))),
            time: Some(TimeOfDay {
                hour: 14,
                minute: 5,
                second: Some(30),
            }),
            new_year_begins: None,
            sort_value: 18_470_312,
            original_text: Some("12 March 1847, 14:05:30".to_owned()),
        };
        let json = serde_json::to_string(&date).unwrap();
        let back: GenealogicalDate = serde_json::from_str(&json).unwrap();
        assert_eq!(date, back);
    }

    #[test]
    fn interpreted_modifier_round_trips() {
        let modifier = DateModifier::Interpreted {
            date: day(1944, 6, 6),
            phrase: "the day of the landings".to_owned(),
        };
        let back: DateModifier = serde_json::from_str(&serde_json::to_string(&modifier).unwrap()).unwrap();
        assert_eq!(modifier, back);
    }

    #[test]
    fn historical_date_without_time_field_decodes() {
        // An event stored before `time` existed has no `time` key (ADR 0004 §4 additive rule).
        let json = r#"{
            "calendar": "Gregorian",
            "quality": "Normal",
            "modifier": { "modifier": "None", "year": 1847, "month": 3, "day": 12 },
            "new_year_begins": null,
            "sort_value": 18470312,
            "original_text": "12 March 1847"
        }"#;
        let date: GenealogicalDate = serde_json::from_str(json).unwrap();
        assert_eq!(date.time, None);
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
