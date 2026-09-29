//! Date comparison for record matching (ADR 0038 §4): genealogical dates as day intervals, compared
//! on a decaying curve whose width follows the date's quality and provenance.
//!
//! A date becomes a closed interval of Julian Day Numbers: a year-only date spans its year, `About`
//! widens by two years, `Before`/`After` open a century to one side. Two intervals that overlap
//! agree; otherwise the gap between them decays the similarity, slowly for a birth year computed from
//! a census age and quickly for an exact recorded date. Only a gap beyond the tolerance's limit is a
//! disagreement.

use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};

/// Days in a (mean Gregorian) year, for the tolerance constants.
const YEAR: i32 = 365;

/// How far `About` widens a date to either side.
const ABOUT_DAYS: i32 = 2 * YEAR;

/// How far an open-ended `Before`/`After` date reaches.
const OPEN_DAYS: i32 = 100 * YEAR;

/// How a date was arrived at — the provenance that widens its tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateBasis {
    /// The date as the source records it.
    Recorded,
    /// A birth date computed from a stated age, such as a census age.
    FromAge,
}

/// A closed interval of Julian Day Numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DayInterval {
    pub lo: i32,
    pub hi: i32,
}

impl DayInterval {
    /// The number of days between two intervals, `0` when they overlap.
    pub fn gap(self, other: Self) -> i32 {
        if self.hi < other.lo {
            other.lo - self.hi
        } else if other.hi < self.lo {
            self.lo - other.hi
        } else {
            0
        }
    }

    /// The interval widened by `before` days at its start and `after` days at its end.
    pub fn widened(self, before: i32, after: i32) -> Self {
        Self {
            lo: self.lo - before,
            hi: self.hi + after,
        }
    }
}

/// How tolerant a comparison is: the more lenient of the two dates' tolerances applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Tolerance {
    /// A recorded date of normal quality: decays over days and months.
    Exact,
    /// An estimated or calculated date.
    Estimated,
    /// A birth year computed from an age: about ±5 years, decaying slowly.
    Age,
}

impl Tolerance {
    /// The tolerance of one date, from its quality and basis.
    pub fn of(date: &GenealogicalDate, basis: DateBasis) -> Self {
        match (basis, date.quality) {
            (DateBasis::FromAge, _) => Self::Age,
            (DateBasis::Recorded, DateQuality::Estimated | DateQuality::Calculated) => Self::Estimated,
            (DateBasis::Recorded, DateQuality::Normal) => Self::Exact,
        }
    }

    /// The half-widths, in days, of the near and far components of the decay curve.
    fn half_widths(self) -> (f64, f64) {
        match self {
            Self::Exact => (30.0, 730.0),
            Self::Estimated => (365.0, 1826.0),
            Self::Age => (1826.0, 3652.0),
        }
    }

    /// The gap, in days, beyond which two dates disagree.
    pub fn limit_days(self) -> i32 {
        match self {
            Self::Exact => 6 * YEAR,
            Self::Estimated | Self::Age => 10 * YEAR,
        }
    }

    /// How far one date's death may precede another's birth before the pair is impossible.
    pub fn slack_days(self) -> i32 {
        match self {
            Self::Exact => 0,
            Self::Estimated => 2 * YEAR,
            Self::Age => 5 * YEAR,
        }
    }
}

/// The similarity in `0..=1` of two dates `gap_days` apart: `1` at no gap, never increasing with the
/// gap, and `0` beyond the tolerance's limit.
pub(crate) fn similarity(gap_days: i32, tolerance: Tolerance) -> f64 {
    if gap_days > tolerance.limit_days() {
        return 0.0;
    }
    let gap = f64::from(gap_days);
    let (near, far) = tolerance.half_widths();
    0.5 / (1.0 + (gap / near).powi(2)) + 0.5 / (1.0 + (gap / far).powi(2))
}

/// The similarity given to an exact-date near miss that is recognisably a clerical slip: a
/// transposed day and month, or a Julian date recorded as Gregorian.
pub(crate) const SLIP_SIMILARITY: f64 = 0.97;

/// Whether two exact dates differ by a recognisable slip — a transposed day and month, or the
/// Julian↔Gregorian offset of their era.
pub(crate) fn is_clerical_slip(a: &GenealogicalDate, b: &GenealogicalDate) -> bool {
    let (Some(pa), Some(pb)) = (exact_point(a), exact_point(b)) else {
        return false;
    };
    let transposed = pa.year == pb.year && pa.month == pb.day && pa.day == pb.month && pa != pb;
    transposed || is_calendar_offset(a.calendar, pa, b.calendar, pb)
}

/// Whether two dates tagged with the same calendar differ by exactly the Julian↔Gregorian offset
/// at their date — the mark of a date copied from a Julian source without conversion.
fn is_calendar_offset(ca: Calendar, a: ExactDate, cb: Calendar, b: ExactDate) -> bool {
    if ca != cb {
        return false;
    }
    let (Some(ja), Some(jb)) = (
        gregorian_jdn(a.year, a.month, a.day),
        gregorian_jdn(b.year, b.month, b.day),
    ) else {
        return false;
    };
    let (Some(julian_a), Some(julian_b)) = (julian_jdn(a.year, a.month, a.day), julian_jdn(b.year, b.month, b.day))
    else {
        return false;
    };
    let gap = (ja - jb).abs();
    gap != 0 && (gap == julian_a - ja || gap == julian_b - jb)
}

/// A fully specified calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExactDate {
    year: i32,
    month: u8,
    day: u8,
}

/// A date that names one day exactly.
fn exact_point(date: &GenealogicalDate) -> Option<ExactDate> {
    let GenealogicalDateBody::Structured(DateModifier::None(point)) = &date.modifier else {
        return None;
    };
    Some(ExactDate {
        year: point.year?,
        month: point.month?,
        day: point.day?,
    })
}

/// The day interval a date covers, or `None` for a text-only date, a date without a year, or a
/// calendar the engine does not convert.
pub(crate) fn interval(date: &GenealogicalDate) -> Option<DayInterval> {
    let GenealogicalDateBody::Structured(modifier) = &date.modifier else {
        return None;
    };
    let point = |p: &DatePoint| point_interval(date.calendar, p);
    match modifier {
        DateModifier::None(p) | DateModifier::Interpreted { date: p, .. } => point(p),
        DateModifier::About(p) => point(p).map(|i| i.widened(ABOUT_DAYS, ABOUT_DAYS)),
        DateModifier::Before(p) | DateModifier::To(p) => point(p).map(|i| i.widened(OPEN_DAYS, 0)),
        DateModifier::After(p) | DateModifier::From(p) => point(p).map(|i| i.widened(0, OPEN_DAYS)),
        DateModifier::Range { start, end } | DateModifier::Span { start, end } => match (point(start), point(end)) {
            (Some(s), Some(e)) => Some(DayInterval {
                lo: s.lo.min(e.lo),
                hi: s.hi.max(e.hi),
            }),
            (Some(one), None) | (None, Some(one)) => Some(one),
            (None, None) => None,
        },
    }
}

/// The year a date falls in, for choosing a region's period.
pub(crate) fn year(date: &GenealogicalDate) -> Option<i32> {
    let GenealogicalDateBody::Structured(modifier) = &date.modifier else {
        return None;
    };
    match modifier {
        DateModifier::None(p)
        | DateModifier::Interpreted { date: p, .. }
        | DateModifier::About(p)
        | DateModifier::Before(p)
        | DateModifier::To(p)
        | DateModifier::After(p)
        | DateModifier::From(p) => p.year,
        DateModifier::Range { start, end } | DateModifier::Span { start, end } => start.year.or(end.year),
    }
}

/// The days a partial date point covers: its whole year, its whole month, or its day.
fn point_interval(calendar: Calendar, point: &DatePoint) -> Option<DayInterval> {
    let year = point.year?;
    let day = |y: i32, m: u8, d: u8| calendar_jdn(calendar, y, m, d);
    match (point.month, point.day) {
        (Some(month), Some(d)) => day(year, month, d).map(|j| DayInterval { lo: j, hi: j }),
        (Some(month), None) => {
            let lo = day(year, month, 1)?;
            let (next_year, next_month) = if month >= 12 { (year + 1, 1) } else { (year, month + 1) };
            Some(DayInterval {
                lo,
                hi: day(next_year, next_month, 1)? - 1,
            })
        }
        (None, _) => Some(DayInterval {
            lo: day(year, 1, 1)?,
            hi: day(year + 1, 1, 1)? - 1,
        }),
    }
}

/// The Julian Day Number of a date in `calendar`, or `None` for a calendar the engine does not convert.
fn calendar_jdn(calendar: Calendar, year: i32, month: u8, day: u8) -> Option<i32> {
    match calendar {
        Calendar::Gregorian => gregorian_jdn(year, month, day),
        Calendar::Julian => julian_jdn(year, month, day),
        Calendar::Swedish => julian_jdn(year, month, day).map(|j| j - 1),
        Calendar::Hebrew | Calendar::FrenchRepublican | Calendar::Islamic => None,
    }
}

/// The largest year magnitude the engine converts: far past any genealogical date, and small enough
/// that the day-number arithmetic (and a century of widening) cannot overflow an `i32`.
const MAX_YEAR: i32 = 1_000_000;

/// The year and month shifted to a March-based year, as the day-number formulas need.
fn march_based(year: i32, month: u8, day: u8) -> Option<(i32, i32, i32)> {
    if !(-MAX_YEAR..=MAX_YEAR).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let month = i32::from(month);
    let a = (14 - month) / 12;
    Some((year + 4800 - a, month + 12 * a - 3, i32::from(day)))
}

/// The Julian Day Number of a proleptic Gregorian date.
pub(crate) fn gregorian_jdn(year: i32, month: u8, day: u8) -> Option<i32> {
    let (y, m, d) = march_based(year, month, day)?;
    Some(d + (153 * m + 2) / 5 + 365 * y + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400) - 32045)
}

/// The Julian Day Number of a Julian-calendar date.
pub(crate) fn julian_jdn(year: i32, month: u8, day: u8) -> Option<i32> {
    let (y, m, d) = march_based(year, month, day)?;
    Some(d + (153 * m + 2) / 5 + 365 * y + y.div_euclid(4) - 32083)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::{prop_assert, proptest};

    use super::{
        DateBasis, DayInterval, Tolerance, gregorian_jdn, interval, is_clerical_slip, julian_jdn, similarity, year,
    };
    use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};

    fn date(calendar: Calendar, modifier: DateModifier) -> GenealogicalDate {
        GenealogicalDate {
            calendar,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(modifier),
            time: None,
            new_year_begins: None,
            sort_value: 0,
            original_text: None,
        }
    }

    fn point(year: i32, month: Option<u8>, day: Option<u8>) -> DatePoint {
        DatePoint {
            year: Some(year),
            month,
            day,
        }
    }

    fn on(year: i32, month: u8, day: u8) -> GenealogicalDate {
        date(
            Calendar::Gregorian,
            DateModifier::None(point(year, Some(month), Some(day))),
        )
    }

    #[test]
    fn the_denmark_norway_calendar_switch_is_one_day() {
        let last_julian = julian_jdn(1700, 2, 18).unwrap();
        let first_gregorian = gregorian_jdn(1700, 3, 1).unwrap();
        assert_eq!(first_gregorian - last_julian, 1);
    }

    #[test]
    fn a_known_day_number() {
        assert_eq!(gregorian_jdn(2000, 1, 1), Some(2_451_545));
        assert_eq!(julian_jdn(1582, 10, 4).map(|j| j + 1), gregorian_jdn(1582, 10, 15));
    }

    #[test]
    fn an_out_of_range_month_or_day_has_no_day_number() {
        assert_eq!(gregorian_jdn(1850, 13, 1), None);
        assert_eq!(gregorian_jdn(1850, 1, 0), None);
    }

    #[test]
    fn an_absurd_year_has_no_day_number_rather_than_overflowing() {
        assert_eq!(gregorian_jdn(2_000_000_000, 1, 1), None);
        assert_eq!(julian_jdn(i32::MIN, 1, 1), None);
        let absurd = date(Calendar::Gregorian, DateModifier::None(point(i32::MAX, None, None)));
        assert_eq!(interval(&absurd), None);
    }

    #[test]
    fn a_julian_and_a_gregorian_date_of_one_day_share_an_interval() {
        let julian = date(Calendar::Julian, DateModifier::None(point(1690, Some(3), Some(1))));
        let gregorian = on(1690, 3, 11);
        assert_eq!(interval(&julian), interval(&gregorian));
    }

    #[test]
    fn a_year_only_date_spans_its_year() {
        let d = date(Calendar::Gregorian, DateModifier::None(point(1852, None, None)));
        let i = interval(&d).unwrap();
        assert_eq!(i.hi - i.lo, 365);
        assert_eq!(Some(i.lo), gregorian_jdn(1852, 1, 1));
    }

    #[test]
    fn a_month_date_spans_its_month() {
        let d = date(Calendar::Gregorian, DateModifier::None(point(1852, Some(2), None)));
        let i = interval(&d).unwrap();
        assert_eq!(i.hi - i.lo, 28, "1852 is a leap year");
        let december = date(Calendar::Gregorian, DateModifier::None(point(1852, Some(12), None)));
        let i = interval(&december).unwrap();
        assert_eq!(i.hi - i.lo, 30);
    }

    #[test]
    fn about_widens_and_before_opens_to_the_past() {
        let exact = interval(&on(1850, 6, 1)).unwrap();
        let about = interval(&date(
            Calendar::Gregorian,
            DateModifier::About(point(1850, Some(6), Some(1))),
        ))
        .unwrap();
        assert!(about.lo < exact.lo && about.hi > exact.hi);
        let before = interval(&date(
            Calendar::Gregorian,
            DateModifier::Before(point(1850, Some(6), Some(1))),
        ))
        .unwrap();
        assert_eq!(before.hi, exact.hi);
        assert!(before.lo < exact.lo - 30 * 365);
    }

    #[test]
    fn a_range_covers_both_ends() {
        let range = DateModifier::Range {
            start: point(1840, None, None),
            end: point(1845, None, None),
        };
        let i = interval(&date(Calendar::Gregorian, range)).unwrap();
        assert_eq!(Some(i.lo), gregorian_jdn(1840, 1, 1));
        assert_eq!(Some(i.hi), gregorian_jdn(1845, 12, 31));
    }

    #[test]
    fn a_text_only_or_yearless_or_unconverted_date_has_no_interval() {
        let text = GenealogicalDate {
            modifier: GenealogicalDateBody::TextOnly {
                text: "2 Pascha".to_owned(),
            },
            ..on(1850, 1, 1)
        };
        assert_eq!(interval(&text), None);
        assert_eq!(year(&text), None);
        let yearless = date(
            Calendar::Gregorian,
            DateModifier::None(DatePoint {
                year: None,
                month: Some(3),
                day: None,
            }),
        );
        assert_eq!(interval(&yearless), None);
        let hebrew = date(Calendar::Hebrew, DateModifier::None(point(5610, None, None)));
        assert_eq!(interval(&hebrew), None);
    }

    #[test]
    fn overlapping_intervals_have_no_gap() {
        let a = DayInterval { lo: 10, hi: 20 };
        assert_eq!(a.gap(DayInterval { lo: 15, hi: 30 }), 0);
        assert_eq!(a.gap(DayInterval { lo: 25, hi: 30 }), 5);
        assert_eq!(DayInterval { lo: 25, hi: 30 }.gap(a), 5);
    }

    #[test]
    fn the_tolerance_follows_quality_and_basis() {
        let normal = on(1850, 1, 1);
        let estimated = GenealogicalDate {
            quality: DateQuality::Estimated,
            ..normal.clone()
        };
        assert_eq!(Tolerance::of(&normal, DateBasis::Recorded), Tolerance::Exact);
        assert_eq!(Tolerance::of(&estimated, DateBasis::Recorded), Tolerance::Estimated);
        assert_eq!(Tolerance::of(&normal, DateBasis::FromAge), Tolerance::Age);
    }

    #[test]
    fn a_census_age_tolerates_five_years_and_disagrees_past_ten() {
        assert!(similarity(5 * 365, Tolerance::Age) > 0.6);
        assert!(similarity(10 * 365, Tolerance::Age) > 0.0);
        assert!(similarity(10 * 365 + 1, Tolerance::Age).abs() < f64::EPSILON);
    }

    #[test]
    fn an_exact_date_decays_over_months() {
        assert!(similarity(10, Tolerance::Exact) > 0.9);
        assert!(similarity(180, Tolerance::Exact) < 0.5);
        assert!(similarity(180, Tolerance::Exact) < similarity(180, Tolerance::Age));
    }

    #[test]
    fn a_transposed_day_and_month_is_a_slip() {
        assert!(is_clerical_slip(&on(1850, 3, 5), &on(1850, 5, 3)));
        assert!(!is_clerical_slip(&on(1850, 3, 5), &on(1850, 3, 5)));
        assert!(!is_clerical_slip(&on(1850, 3, 5), &on(1851, 5, 3)));
    }

    #[test]
    fn the_julian_offset_of_the_era_is_a_slip() {
        assert!(
            is_clerical_slip(&on(1750, 4, 1), &on(1750, 4, 12)),
            "11 days in the 1700s"
        );
        assert!(
            is_clerical_slip(&on(1850, 4, 13), &on(1850, 4, 1)),
            "12 days in the 1800s"
        );
        assert!(
            !is_clerical_slip(&on(1850, 4, 1), &on(1850, 4, 12)),
            "11 days is not the 1800s offset"
        );
    }

    proptest! {
        #[test]
        fn similarity_never_rises_with_the_gap(a in 0i32..8000, b in 0i32..8000, t in 0usize..3) {
            let tolerance = [Tolerance::Exact, Tolerance::Estimated, Tolerance::Age][t];
            let (near, far) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(similarity(near, tolerance) >= similarity(far, tolerance));
            prop_assert!((0.0..=1.0).contains(&similarity(near, tolerance)));
        }

        #[test]
        fn a_looser_tolerance_is_never_stricter(gap in 0i32..8000) {
            prop_assert!(similarity(gap, Tolerance::Exact) <= similarity(gap, Tolerance::Estimated));
            prop_assert!(similarity(gap, Tolerance::Estimated) <= similarity(gap, Tolerance::Age));
        }
    }
}
