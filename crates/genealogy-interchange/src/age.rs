//! [`Age`] — a participant's age at an event, and the GEDCOM `AGE` grammar shared by the GEDCOM
//! `AGE` tag and the Gramps `"Age"` eventref attribute.
//!
//! An age is a *span* (how old someone was), not a calendar point, so it is its own value object
//! rather than a reuse of the date grammar (ADR 0019). It mirrors `genealogy_core::age::Age`: the
//! decomposed years/months/days a source records, an optional [`AgeBound`] for GEDCOM's `<`/`>`
//! qualifiers, and a free-text `phrase` for an age that does not decompose. Weeks are deliberately
//! absent from the value object: GEDCOM's `w` unit is normalized to days on parse (ADR 0019), so a
//! `3w` round-trips back out as `21d`.

/// A one-sided bound on an age (GEDCOM `AGE` `<` / `>` qualifiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeBound {
    /// The age is strictly less than the stated value (GEDCOM `<`).
    LessThan,
    /// The age is strictly greater than the stated value (GEDCOM `>`).
    GreaterThan,
}

/// A participant's age at an event, expressed as a duration (ADR 0019). Every field is optional; an
/// all-`None` age is [`is_empty`](Age::is_empty).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Age {
    /// A one-sided bound qualifying the age (GEDCOM `<` / `>`), if any.
    pub bound: Option<AgeBound>,
    /// Whole years.
    pub years: Option<u16>,
    /// Whole months.
    pub months: Option<u16>,
    /// Whole days (GEDCOM weeks are normalized to days on parse — ADR 0019).
    pub days: Option<u16>,
    /// A free-text age that does not decompose into parts (GEDCOM `AGE` phrase).
    pub phrase: Option<String>,
}

impl Age {
    /// Whether every field is absent, so no age should be recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bound.is_none()
            && self.years.is_none()
            && self.months.is_none()
            && self.days.is_none()
            && self.phrase.is_none()
    }
}

/// Parses a GEDCOM `AGE` value (also the Gramps `"Age"` attribute) into an [`Age`].
///
/// Grammar: an optional leading `<` / `>` bound, then whitespace-separated `NNy` / `NNm` / `NNw` /
/// `NNd` tokens (case-insensitive). Weeks are converted to days and folded into `days` (ADR 0019).
/// A value that does not decompose (`INFANT`, `STILLBORN`, …) is kept verbatim in `phrase`. An empty
/// value is `None`.
#[must_use]
pub fn parse_age(input: &str) -> Option<Age> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (bound, rest) = strip_bound(trimmed);
    let mut age = Age {
        bound,
        ..Age::default()
    };
    let mut any = false;
    for token in rest.split_whitespace() {
        let Some((number, unit)) = split_number_unit(token) else {
            return Some(phrase_age(trimmed));
        };
        match unit {
            'y' => age.years = Some(age.years.unwrap_or(0).saturating_add(number)),
            'm' => age.months = Some(age.months.unwrap_or(0).saturating_add(number)),
            'w' => age.days = Some(age.days.unwrap_or(0).saturating_add(number.saturating_mul(7))),
            'd' => age.days = Some(age.days.unwrap_or(0).saturating_add(number)),
            _ => return Some(phrase_age(trimmed)),
        }
        any = true;
    }
    if any { Some(age) } else { Some(phrase_age(trimmed)) }
}

/// Renders an [`Age`] back to a GEDCOM `AGE` value. A decomposed age emits `NNy NNm NNd` (weeks were
/// normalized to days on parse, so none appear); a phrase age emits its verbatim text.
#[must_use]
pub fn age_value(age: &Age) -> String {
    if let Some(phrase) = &age.phrase {
        return phrase.clone();
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(years) = age.years {
        parts.push(format!("{years}y"));
    }
    if let Some(months) = age.months {
        parts.push(format!("{months}m"));
    }
    if let Some(days) = age.days {
        parts.push(format!("{days}d"));
    }
    let body = parts.join(" ");
    match age.bound {
        Some(AgeBound::LessThan) => format!("< {body}"),
        Some(AgeBound::GreaterThan) => format!("> {body}"),
        None => body,
    }
}

/// An age that only carries a verbatim `phrase` (the unparseable fallback).
fn phrase_age(text: &str) -> Age {
    Age {
        phrase: Some(text.to_owned()),
        ..Age::default()
    }
}

/// Strips a leading `<` / `>` bound, returning it and the remaining (trimmed) text.
fn strip_bound(text: &str) -> (Option<AgeBound>, &str) {
    if let Some(rest) = text.strip_prefix('<') {
        (Some(AgeBound::LessThan), rest.trim_start())
    } else if let Some(rest) = text.strip_prefix('>') {
        (Some(AgeBound::GreaterThan), rest.trim_start())
    } else {
        (None, text)
    }
}

/// Splits a `NN<unit>` token into its number and its lowercased unit letter, or `None` when the
/// token is not `digits` + a single trailing letter.
fn split_number_unit(token: &str) -> Option<(u16, char)> {
    let unit = token.chars().next_back()?;
    if !unit.is_ascii_alphabetic() {
        return None;
    }
    let number = &token[..token.len() - unit.len_utf8()];
    let number: u16 = number.parse().ok()?;
    Some((number, unit.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::{Age, AgeBound, age_value, parse_age};

    #[test]
    fn parses_a_full_decomposed_age() {
        assert_eq!(
            parse_age("25y 3m 10d"),
            Some(Age {
                years: Some(25),
                months: Some(3),
                days: Some(10),
                ..Age::default()
            })
        );
    }

    #[test]
    fn parses_a_less_than_bound() {
        assert_eq!(
            parse_age("< 8y"),
            Some(Age {
                bound: Some(AgeBound::LessThan),
                years: Some(8),
                ..Age::default()
            })
        );
    }

    #[test]
    fn parses_a_greater_than_bound() {
        assert_eq!(
            parse_age("> 90y"),
            Some(Age {
                bound: Some(AgeBound::GreaterThan),
                years: Some(90),
                ..Age::default()
            })
        );
    }

    #[test]
    fn normalizes_weeks_to_days() {
        assert_eq!(
            parse_age("3w"),
            Some(Age {
                days: Some(21),
                ..Age::default()
            })
        );
    }

    #[test]
    fn accumulates_weeks_and_days() {
        assert_eq!(
            parse_age("2w 3d"),
            Some(Age {
                days: Some(17),
                ..Age::default()
            })
        );
    }

    #[test]
    fn keeps_an_unparseable_age_as_a_phrase() {
        assert_eq!(
            parse_age("INFANT"),
            Some(Age {
                phrase: Some("INFANT".to_owned()),
                ..Age::default()
            })
        );
    }

    #[test]
    fn an_empty_age_is_none() {
        assert_eq!(parse_age(""), None);
        assert_eq!(parse_age("   "), None);
        assert!(Age::default().is_empty());
    }

    #[test]
    fn weeks_re_emit_as_days() {
        let age = parse_age("3w").expect("age");
        assert_eq!(age_value(&age), "21d");
    }

    #[test]
    fn a_bounded_age_round_trips_through_emit_and_parse() {
        for input in ["25y 3m 10d", "< 8y", "> 90y", "40y", "6m"] {
            let age = parse_age(input).expect("age");
            let re_parsed = parse_age(&age_value(&age)).expect("re-parse");
            assert_eq!(age, re_parsed, "round-trip of {input}");
        }
    }

    #[test]
    fn a_phrase_age_round_trips() {
        let age = parse_age("stillborn").expect("age");
        assert_eq!(age_value(&age), "stillborn");
        assert_eq!(parse_age(&age_value(&age)), Some(age));
    }
}
