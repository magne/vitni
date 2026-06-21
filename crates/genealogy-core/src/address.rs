//! [`Address`] — a postal address on a [`Repository`](crate::repository) (data-model §6, §7).

use serde::{Deserialize, Serialize};

/// A postal address (data-model §7). Every part is optional — sources rarely give all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    /// The street address line.
    pub street: Option<String>,
    /// The locality (city / town / village).
    pub locality: Option<String>,
    /// The region (county / state / province).
    pub region: Option<String>,
    /// The postal / ZIP code.
    pub postal_code: Option<String>,
    /// The country.
    pub country: Option<String>,
}

impl Address {
    /// Returns `true` if no address part is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.street.is_none()
            && self.locality.is_none()
            && self.region.is_none()
            && self.postal_code.is_none()
            && self.country.is_none()
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
            locality: Some("Bergen".to_owned()),
            country: Some("Norway".to_owned()),
            ..Address::default()
        };
        assert!(!address.is_empty());
        let json = serde_json::to_string(&address).unwrap();
        let back: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(address, back);
    }
}
