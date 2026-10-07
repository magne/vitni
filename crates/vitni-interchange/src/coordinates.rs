//! A geographic point as interchange formats state it: GEDCOM `PLAC.MAP.LATI`/`LONG` (`N58.028`) and
//! Gramps `<coord lat long>` (`58.028`).

/// A point in decimal degrees, north and east positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinates {
    /// Latitude, in −90..=90.
    pub latitude: f64,
    /// Longitude, in −180..=180.
    pub longitude: f64,
}

impl Coordinates {
    /// The point `latitude` and `longitude` state: each a decimal degree count, signed or carrying
    /// its hemisphere letter (`N`/`S`, `E`/`W`) before or after it. `None` when either half is not
    /// such a number, or lies outside its range — degrees-minutes-seconds included.
    #[must_use]
    pub fn parse(latitude: &str, longitude: &str) -> Option<Self> {
        let latitude = degrees(latitude, ('N', 'S'), 90.0)?;
        let longitude = degrees(longitude, ('E', 'W'), 180.0)?;
        Some(Self { latitude, longitude })
    }
}

/// One half of a point: a decimal degree count no further from zero than `limit`, signed, or carrying
/// the `positive`/`negative` hemisphere letter at either end.
fn degrees(text: &str, (positive, negative): (char, char), limit: f64) -> Option<f64> {
    let text = text.trim();
    let (sign, number) = match hemisphere(text, positive, negative) {
        Some((sign, number)) => (sign, number),
        None => match text.strip_prefix('-') {
            Some(number) => (-1.0, number),
            None => (1.0, text.strip_prefix('+').unwrap_or(text)),
        },
    };
    let value = sign * decimal(number)?;
    (value.abs() <= limit).then_some(value)
}

/// The sign a leading or trailing hemisphere letter gives, and the number it marks.
fn hemisphere(text: &str, positive: char, negative: char) -> Option<(f64, &str)> {
    let first = text.chars().next()?;
    let last = text.chars().next_back()?;
    for (letter, sign) in [(positive, 1.0), (negative, -1.0)] {
        if first.eq_ignore_ascii_case(&letter) {
            return Some((sign, &text[first.len_utf8()..]));
        }
        if last.eq_ignore_ascii_case(&letter) {
            return Some((sign, &text[..text.len() - last.len_utf8()]));
        }
    }
    None
}

/// An unsigned decimal — digits with at most one point — as a number.
fn decimal(text: &str) -> Option<f64> {
    let mut digits = 0;
    let mut points = 0;
    for character in text.chars() {
        match character {
            '0'..='9' => digits += 1,
            '.' => points += 1,
            _ => return None,
        }
    }
    if digits == 0 || points > 1 {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::Coordinates;

    fn point(latitude: &str, longitude: &str) -> Option<(f64, f64)> {
        Coordinates::parse(latitude, longitude).map(|point| (point.latitude, point.longitude))
    }

    #[test]
    fn a_hemisphere_letter_signs_the_degrees() {
        assert_eq!(point("N58.028", "E7.46"), Some((58.028, 7.46)));
        assert_eq!(point("S33.9", "W0.12"), Some((-33.9, -0.12)));
        assert_eq!(point("58.028N", "7.46E"), Some((58.028, 7.46)));
        assert_eq!(point("n58", "w7"), Some((58.0, -7.0)));
    }

    #[test]
    fn a_signed_decimal_is_read_as_is() {
        assert_eq!(point("40.7128", "-74.006"), Some((40.7128, -74.006)));
        assert_eq!(point(" -90 ", "+180"), Some((-90.0, 180.0)));
    }

    #[test]
    fn a_value_out_of_range_is_no_point() {
        assert_eq!(point("N91", "E7"), None);
        assert_eq!(point("N58", "E181"), None);
        assert_eq!(point("-90.5", "0"), None);
    }

    #[test]
    fn a_value_that_is_not_decimal_degrees_is_no_point() {
        for bad in [
            "",
            " ",
            "nan",
            "inf",
            "abc",
            "58°1'40\"N",
            "N",
            "N-58",
            "NS58",
            "58 N 2",
            "1e1",
        ] {
            assert_eq!(point(bad, "7"), None, "latitude {bad:?}");
            assert_eq!(point("58", bad), None, "longitude {bad:?}");
        }
    }

    #[test]
    fn a_hemisphere_letter_of_the_other_axis_is_no_point() {
        assert_eq!(point("E58", "E7"), None);
        assert_eq!(point("N58", "N7"), None);
    }
}
