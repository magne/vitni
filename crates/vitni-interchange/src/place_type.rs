//! Gramps place-type labels and the `type_key` text they map to (ADR 0031 §10, ADR 0054).
//!
//! Gramps names a place's type by a label. The twenty standard labels map to `std:<lowercase label>`;
//! any other non-blank text, except Gramps' own "Unknown" and "place" placeholders, becomes
//! `custom:<text>`. Text that merely looks namespaced (`std:foo`, `custom:x`) is wrapped, never trusted.
//!
//! The mapping is stored in backups, so changing a label, a key or this rule is a backup format change.

/// The twenty standard Gramps place-type labels, in Gramps' order.
pub const GRAMPS_PLACE_TYPE_LABELS: [&str; 20] = [
    "Country",
    "State",
    "County",
    "City",
    "Parish",
    "Locality",
    "Street",
    "Province",
    "Region",
    "Department",
    "Neighborhood",
    "District",
    "Borough",
    "Municipality",
    "Town",
    "Village",
    "Hamlet",
    "Farm",
    "Building",
    "Number",
];

const STANDARD_PREFIX: &str = "std:";
const CUSTOM_PREFIX: &str = "custom:";
const UNKNOWN_LABEL: &str = "Unknown";
const PLACEHOLDER_LABEL: &str = "place";

/// The `type_key` for a Gramps place-type label, or `None` when the label states no type.
///
/// The label is trimmed and matched case-sensitively.
#[must_use]
pub fn place_type_key_from_gramps_label(label: &str) -> Option<String> {
    let label = label.trim();
    if label.is_empty() || label == UNKNOWN_LABEL || label == PLACEHOLDER_LABEL {
        return None;
    }
    if GRAMPS_PLACE_TYPE_LABELS.contains(&label) {
        return Some(format!("{STANDARD_PREFIX}{}", label.to_lowercase()));
    }
    Some(format!("{CUSTOM_PREFIX}{label}"))
}

/// The Gramps label for a `type_key`: "Unknown" for no key, the standard label for a `std:` key,
/// the text after exactly one `custom:` prefix, and any other key as it stands.
#[must_use]
pub fn gramps_label_from_place_type_key(key: Option<&str>) -> String {
    let Some(key) = key else {
        return UNKNOWN_LABEL.to_owned();
    };
    if let Some(label) = key.strip_prefix(CUSTOM_PREFIX) {
        return label.to_owned();
    }
    if let Some(name) = key.strip_prefix(STANDARD_PREFIX) {
        for label in GRAMPS_PLACE_TYPE_LABELS {
            if label.to_lowercase() == name {
                return label.to_owned();
            }
        }
    }
    key.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{GRAMPS_PLACE_TYPE_LABELS, gramps_label_from_place_type_key, place_type_key_from_gramps_label};
    use proptest::prelude::*;

    const EXPECTED_LABELS: [&str; 20] = [
        "Country",
        "State",
        "County",
        "City",
        "Parish",
        "Locality",
        "Street",
        "Province",
        "Region",
        "Department",
        "Neighborhood",
        "District",
        "Borough",
        "Municipality",
        "Town",
        "Village",
        "Hamlet",
        "Farm",
        "Building",
        "Number",
    ];

    #[test]
    fn label_table_lists_the_twenty_gramps_labels_in_order() {
        assert_eq!(GRAMPS_PLACE_TYPE_LABELS, EXPECTED_LABELS);
    }

    #[test]
    fn every_standard_label_maps_to_its_lowercased_std_key() {
        for label in EXPECTED_LABELS {
            let expected = format!("std:{}", label.to_lowercase());
            assert_eq!(place_type_key_from_gramps_label(label), Some(expected), "{label}");
        }
    }

    #[test]
    fn unknown_place_empty_and_blank_give_no_key() {
        for label in ["Unknown", "place", "", "   ", "\t\n"] {
            assert_eq!(place_type_key_from_gramps_label(label), None, "{label:?}");
        }
    }

    #[test]
    fn other_text_becomes_a_custom_key() {
        assert_eq!(place_type_key_from_gramps_label("Sokn"), Some("custom:Sokn".to_owned()));
    }

    #[test]
    fn namespace_lookalikes_are_wrapped_not_trusted() {
        assert_eq!(
            place_type_key_from_gramps_label("std:foo"),
            Some("custom:std:foo".to_owned())
        );
        assert_eq!(
            place_type_key_from_gramps_label("custom:Sokn"),
            Some("custom:custom:Sokn".to_owned())
        );
    }

    #[test]
    fn input_is_trimmed() {
        assert_eq!(
            place_type_key_from_gramps_label(" Hamlet "),
            Some("std:hamlet".to_owned())
        );
        assert_eq!(
            place_type_key_from_gramps_label("  Sokn\t"),
            Some("custom:Sokn".to_owned())
        );
    }

    #[test]
    fn case_is_exact() {
        assert_eq!(
            place_type_key_from_gramps_label("hamlet"),
            Some("custom:hamlet".to_owned())
        );
        assert_eq!(
            place_type_key_from_gramps_label("HAMLET"),
            Some("custom:HAMLET".to_owned())
        );
        assert_eq!(
            place_type_key_from_gramps_label("unknown"),
            Some("custom:unknown".to_owned())
        );
        assert_eq!(
            place_type_key_from_gramps_label("Place"),
            Some("custom:Place".to_owned())
        );
    }

    #[test]
    fn inverse_maps_no_key_to_unknown() {
        assert_eq!(gramps_label_from_place_type_key(None), "Unknown");
    }

    #[test]
    fn inverse_strips_exactly_one_custom_prefix() {
        assert_eq!(gramps_label_from_place_type_key(Some("custom:Sokn")), "Sokn");
        assert_eq!(
            gramps_label_from_place_type_key(Some("custom:custom:Sokn")),
            "custom:Sokn"
        );
        assert_eq!(gramps_label_from_place_type_key(Some("custom:std:foo")), "std:foo");
    }

    #[test]
    fn inverse_returns_any_other_key_as_text() {
        assert_eq!(gramps_label_from_place_type_key(Some("std:foo")), "std:foo");
        assert_eq!(gramps_label_from_place_type_key(Some("no:fylke")), "no:fylke");
    }

    #[test]
    fn standard_rows_are_a_bijection() {
        for label in GRAMPS_PLACE_TYPE_LABELS {
            let key = place_type_key_from_gramps_label(label).expect("a standard label has a key");
            assert_eq!(gramps_label_from_place_type_key(Some(&key)), label, "label {label}");
            let back = place_type_key_from_gramps_label(&gramps_label_from_place_type_key(Some(&key)));
            assert_eq!(back, Some(key), "key round trip for {label}");
        }
    }

    proptest! {
        #[test]
        fn trimmed_custom_text_round_trips(text in "[^\\s](.*[^\\s])?") {
            prop_assume!(!GRAMPS_PLACE_TYPE_LABELS.contains(&text.as_str()));
            prop_assume!(text != "Unknown" && text != "place");
            let key = place_type_key_from_gramps_label(&text);
            let expected = format!("custom:{text}");
            prop_assert_eq!(key.as_deref(), Some(expected.as_str()));
            prop_assert_eq!(gramps_label_from_place_type_key(key.as_deref()), text);
        }
    }
}
