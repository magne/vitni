//! [`IdFormat`] — a configurable [`HumanId`](crate::ids::HumanId) numbering pattern (data-model §7).
//!
//! Mirrors Gramps' printf-style id formats (`I%04d` for persons, `F%04d` for families, …). The
//! numeric field is a single `%d` / `%0Nd` conversion that may carry a literal **prefix and
//! suffix** (e.g. `I%04d`, `P-%04d`, `%05d-X`). The format lives in the workspace config; the core
//! owns the parsing/rendering so the database layer stays both engine-neutral and format-agnostic.

use std::fmt;

/// A `HumanId` numbering pattern: a literal prefix, a zero-padded numeric width, and a suffix.
///
/// `width == 0` means a bare `%d` (no zero-padding). Renders as `{prefix}{number}{suffix}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdFormat {
    prefix: String,
    width: usize,
    suffix: String,
}

/// A pattern that is not a valid [`IdFormat`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid id format {pattern:?}: {reason}")]
pub struct IdFormatError {
    /// The offending pattern.
    pub pattern: String,
    /// Why it was rejected.
    pub reason: &'static str,
}

impl IdFormat {
    /// Parses a Gramps-style pattern: `{prefix}%d{suffix}` or `{prefix}%0{width}d{suffix}`.
    ///
    /// Exactly one `%…d` conversion is required; the prefix and suffix are arbitrary literal text
    /// (but may not themselves contain `%`). A `%0Nd` field is zero-padded to `N`; a bare `%d` is
    /// unpadded.
    ///
    /// # Errors
    ///
    /// Returns [`IdFormatError`] if there is not exactly one `%`, the conversion is not `d`, or the
    /// width spec is malformed.
    pub fn parse(pattern: &str) -> Result<Self, IdFormatError> {
        let err = |reason: &'static str| IdFormatError {
            pattern: pattern.to_owned(),
            reason,
        };

        if pattern.matches('%').count() != 1 {
            return Err(err("expected exactly one %d / %0Nd conversion"));
        }
        let (prefix, rest) = pattern.split_once('%').ok_or_else(|| err("missing % conversion"))?;
        let Some(d_index) = rest.find('d') else {
            return Err(err("conversion must end in 'd'"));
        };
        let (spec, after) = rest.split_at(d_index);
        let suffix = &after['d'.len_utf8()..];

        let width = parse_width(spec).ok_or_else(|| err("width spec must be empty or 0 followed by digits"))?;

        Ok(Self {
            prefix: prefix.to_owned(),
            width,
            suffix: suffix.to_owned(),
        })
    }

    /// Renders `number` into the pattern (zero-padded to the width; longer numbers are not truncated).
    #[must_use]
    pub fn render(&self, number: u64) -> String {
        let width = self.width;
        format!("{}{number:0width$}{}", self.prefix, self.suffix)
    }

    /// Extracts the numeric value from an id that matches this pattern, or `None` if it does not.
    #[must_use]
    pub fn extract_number(&self, id: &str) -> Option<u64> {
        let middle = id.strip_prefix(&self.prefix)?.strip_suffix(&self.suffix)?;
        if middle.is_empty() {
            return None;
        }
        middle.parse().ok()
    }
}

/// Parses the chars between `%` and `d`: empty (`%d`, width 0) or `0` then digits (`%0Nd`).
fn parse_width(spec: &str) -> Option<usize> {
    if spec.is_empty() {
        return Some(0);
    }
    let digits = spec.strip_prefix('0')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

impl fmt::Display for IdFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.width == 0 {
            write!(f, "{}%d{}", self.prefix, self.suffix)
        } else {
            write!(f, "{}%0{}d{}", self.prefix, self.width, self.suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IdFormat;

    #[test]
    fn parses_and_renders_prefix_and_width() {
        let format = IdFormat::parse("I%04d").expect("valid");
        assert_eq!(format.render(1), "I0001");
        assert_eq!(format.render(42), "I0042");
    }

    #[test]
    fn supports_a_suffix() {
        let format = IdFormat::parse("%04d-X").expect("valid");
        assert_eq!(format.render(7), "0007-X");
        assert_eq!(format.extract_number("0007-X"), Some(7));

        let dashed = IdFormat::parse("P-%03d").expect("valid");
        assert_eq!(dashed.render(12), "P-012");
        assert_eq!(dashed.extract_number("P-012"), Some(12));
    }

    #[test]
    fn bare_conversion_has_no_padding() {
        let format = IdFormat::parse("I%d").expect("valid");
        assert_eq!(format.render(5), "I5");
        assert_eq!(format.extract_number("I5"), Some(5));
    }

    #[test]
    fn render_does_not_truncate_past_the_width() {
        let format = IdFormat::parse("I%04d").expect("valid");
        assert_eq!(format.render(10_000), "I10000");
    }

    #[test]
    fn extract_number_round_trips_and_rejects_mismatches() {
        let format = IdFormat::parse("I%04d").expect("valid");
        assert_eq!(format.extract_number("I0042"), Some(42));
        assert_eq!(format.extract_number("F0042"), None, "wrong prefix");
        assert_eq!(format.extract_number("I"), None, "no number");
        assert_eq!(format.extract_number("Ixyz"), None, "not numeric");
    }

    #[test]
    fn rejects_malformed_patterns() {
        assert!(IdFormat::parse("I0004").is_err(), "no conversion");
        assert!(IdFormat::parse("I%04d%02d").is_err(), "two conversions");
        assert!(IdFormat::parse("I%04x").is_err(), "not a 'd' conversion");
        assert!(IdFormat::parse("I%4d").is_err(), "width without zero flag");
    }
}
