//! Fixed-point decimal parsing and formatting for the numeric value objects (data-model §7, §12).
//!
//! Decimal quantities — centimorgans, shared-percent, geographic coordinates — are stored as
//! **scaled integers** so their value objects keep `Eq`/`Ord` and serialize byte-identically (the
//! projection-rebuild test compares serialized views exactly; the §15 Rust that sketches `f64` is
//! explicitly illustrative). Conversion goes straight from the decimal string to the scaled integer
//! — never through `f64` — so it is exact and never a lossy float cast.

use serde::{Deserialize, Serialize};

/// The error of parsing a fixed-point decimal string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ParseFixedError {
    /// The string is not a decimal number.
    #[error("not a valid decimal number: {0:?}")]
    Invalid(String),
    /// The string has more fractional digits than the scale allows.
    #[error("{input:?} has more than {scale} fractional digits")]
    TooPrecise {
        /// The number of fractional digits the target stores.
        scale: u32,
        /// The offending input.
        input: String,
    },
    /// The value does not fit the target integer.
    #[error("{0:?} is out of range")]
    OutOfRange(String),
}

/// Parses `input` as a decimal with `scale` fractional digits into a scaled `i64`.
///
/// `parse_decimal("850.5", 2)` is `85050`. Leading sign, an empty integer part (`.5`), and fewer
/// fractional digits than `scale` are accepted; more fractional digits than `scale`, non-digit
/// characters, and overflow are rejected.
///
/// # Errors
///
/// [`ParseFixedError`] if the string is not a decimal, is too precise for `scale`, or overflows.
pub fn parse_decimal(input: &str, scale: u32) -> Result<i64, ParseFixedError> {
    let trimmed = input.trim();
    let (negative, rest) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let (int_str, frac_str) = match rest.split_once('.') {
        Some((int_part, frac_part)) => (int_part, frac_part),
        None => (rest, ""),
    };
    if (int_str.is_empty() && frac_str.is_empty())
        || !int_str.bytes().all(|b| b.is_ascii_digit())
        || !frac_str.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ParseFixedError::Invalid(input.to_owned()));
    }
    let scale_width = usize::try_from(scale).unwrap_or(usize::MAX);
    if frac_str.len() > scale_width {
        return Err(ParseFixedError::TooPrecise {
            scale,
            input: input.to_owned(),
        });
    }

    let int_value: i64 = if int_str.is_empty() {
        0
    } else {
        int_str
            .parse()
            .map_err(|_| ParseFixedError::OutOfRange(input.to_owned()))?
    };
    let mut frac_padded = String::with_capacity(scale_width);
    frac_padded.push_str(frac_str);
    while frac_padded.len() < scale_width {
        frac_padded.push('0');
    }
    let frac_value: i64 = if frac_padded.is_empty() {
        0
    } else {
        frac_padded
            .parse()
            .map_err(|_| ParseFixedError::OutOfRange(input.to_owned()))?
    };

    let scale_factor = 10_i64
        .checked_pow(scale)
        .ok_or_else(|| ParseFixedError::OutOfRange(input.to_owned()))?;
    let magnitude = int_value
        .checked_mul(scale_factor)
        .and_then(|scaled| scaled.checked_add(frac_value))
        .ok_or_else(|| ParseFixedError::OutOfRange(input.to_owned()))?;
    Ok(if negative { -magnitude } else { magnitude })
}

/// Renders a scaled `i64` as a decimal string with `scale` fractional digits (`8550, 2` → `85.50`).
#[must_use]
pub fn format_decimal(value: i64, scale: u32) -> String {
    if scale == 0 {
        return value.to_string();
    }
    let scale_width = usize::try_from(scale).unwrap_or(usize::MAX);
    let scale_factor = 10_i64.pow(scale);
    let sign = if value < 0 { "-" } else { "" };
    let magnitude = value.checked_abs().unwrap_or(i64::MAX);
    let whole = magnitude / scale_factor;
    let frac = magnitude % scale_factor;
    let mut rendered = format!("{sign}{whole}.{frac:0>scale_width$}");
    while rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.pop();
    }
    rendered
}

/// Implements `Display` and `FromStr` for a scaled-integer decimal newtype.
macro_rules! fixed_decimal_display {
    ($name:ty, $scale:expr) => {
        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}", $crate::fixed::format_decimal(i64::from(self.0), $scale))
            }
        }
    };
}
pub(crate) use fixed_decimal_display;

#[cfg(test)]
mod tests {
    use super::{ParseFixedError, format_decimal, parse_decimal};

    #[test]
    fn parses_whole_and_fractional_parts() {
        assert_eq!(parse_decimal("850.5", 2), Ok(85050));
        assert_eq!(parse_decimal("850.05", 2), Ok(85005));
        assert_eq!(parse_decimal("850", 2), Ok(85000));
        assert_eq!(parse_decimal(".5", 2), Ok(50));
        assert_eq!(parse_decimal("-12.34", 2), Ok(-1234));
    }

    #[test]
    fn rejects_too_many_fractional_digits() {
        assert_eq!(
            parse_decimal("1.234", 2),
            Err(ParseFixedError::TooPrecise {
                scale: 2,
                input: "1.234".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_non_numeric() {
        assert_eq!(parse_decimal("abc", 2), Err(ParseFixedError::Invalid("abc".to_owned())));
        assert_eq!(parse_decimal("", 2), Err(ParseFixedError::Invalid(String::new())));
    }

    #[test]
    fn formats_with_trailing_zeros_trimmed() {
        assert_eq!(format_decimal(85050, 2), "850.5");
        assert_eq!(format_decimal(85000, 2), "850");
        assert_eq!(format_decimal(85005, 2), "850.05");
        assert_eq!(format_decimal(-1234, 2), "-12.34");
    }

    #[test]
    fn round_trips_through_string() {
        for input in ["0", "1.5", "4400.99", "-90.123456"] {
            let scale = 6;
            let parsed = parse_decimal(input, scale).unwrap();
            assert_eq!(parse_decimal(&format_decimal(parsed, scale), scale), Ok(parsed));
        }
    }
}
