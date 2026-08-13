//! [`Address`] — a postal address on a [`Repository`](crate::repository) (data-model §6, §7).

use serde::{Deserialize, Serialize};

/// A postal address (data-model §7, GEDCOM `ADDR`). Every part is optional — sources rarely give
/// all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    /// The street address lines, in order (GEDCOM `ADR1`/`ADR2`/`ADR3` in 5.5.1, the multi-line
    /// `ADDR` payload in 7.0). A single-line address is one element.
    #[serde(default)]
    pub lines: Vec<String>,
    /// The locality (city / town / village — GEDCOM `CITY`).
    pub locality: Option<String>,
    /// The region (county / state / province — GEDCOM `STAE`).
    pub region: Option<String>,
    /// The postal / ZIP code (GEDCOM `POST`).
    pub postal_code: Option<String>,
    /// The country (GEDCOM `CTRY`).
    pub country: Option<String>,
    /// A telephone number (GEDCOM `PHON`).
    #[serde(default)]
    pub phone: Option<String>,
    /// An email address (GEDCOM `EMAIL`).
    #[serde(default)]
    pub email: Option<String>,
    /// A fax number (GEDCOM `FAX`).
    #[serde(default)]
    pub fax: Option<String>,
    /// A web address (GEDCOM `WWW`).
    #[serde(default)]
    pub www: Option<String>,
    /// The verbatim `ADDR` payload, retained when it cannot be split into the fields above so it
    /// is never lost (mirrors [`GenealogicalDate`](crate::date::GenealogicalDate) original text).
    #[serde(default)]
    pub original_text: Option<String>,
}

impl Address {
    /// Returns `true` if no address part is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            && self.locality.is_none()
            && self.region.is_none()
            && self.postal_code.is_none()
            && self.country.is_none()
            && self.phone.is_none()
            && self.email.is_none()
            && self.fax.is_none()
            && self.www.is_none()
            && self.original_text.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::Address;

    #[test]
    fn empty_address_is_empty() {
        assert!(Address::default().is_empty());
    }

    #[test]
    fn address_round_trips_through_json() {
        let address = Address {
            lines: vec!["Kong Oscars gate 1".to_owned(), "c/o Statsarkivet".to_owned()],
            locality: Some("Bergen".to_owned()),
            country: Some("Norway".to_owned()),
            email: Some("post@example.no".to_owned()),
            ..Address::default()
        };
        assert!(!address.is_empty());
        let json = serde_json::to_string(&address).unwrap();
        let back: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(address, back);
        assert_eq!(back.lines.len(), 2);
    }

    #[test]
    fn address_with_only_verbatim_text_is_not_empty() {
        let address = Address {
            original_text: Some("somewhere near the old mill".to_owned()),
            ..Address::default()
        };
        assert!(!address.is_empty());
    }
}
