//! **Temporary.** The pre-1.0 backup formats and their upgraders (ADR 0041 §4).
//!
//! Before 1.0 a restore reads the current format and the two before it. At 1.0 the format in force
//! is frozen as v1 and this whole module is deleted, together with the older `v0.*` fixtures, keeping
//! only the permanent `0.K` → v1 alias. `cargo xtask check` fails while this file exists at a
//! workspace version of 1.0.0 or later, so the deletion cannot be forgotten (#392).
//!
//! Bumping the format: append a [`FormatRecord`] for the new version with `upgrade: None`, give the
//! previous last record its `vN → vN+1` [`Upgrader`] (pure functions over `serde_json::Value`, in
//! this module), and add the new golden fixture under `crates/vitni-app/tests/fixtures/backup/`.

use serde_json::{Map, Value, json};

use super::{FormatRecord, Upgrader};
use crate::backup::format::FormatVersion;
use vitni_interchange::place_type_key_from_gramps_label;

/// Every pre-1.0 format, oldest first; the last is the one this version writes.
///
/// Formats 0.1 and 0.2 both carry `first_app_version` "0.1.0": the workspace version is still 0.1.0
/// and no release has been tagged, so the `readable_before` message of `chain_for` names it for both.
pub(super) const HISTORY: &[FormatRecord] = &[
    FormatRecord {
        version: FormatVersion::pre_release(1),
        first_app_version: "0.1.0",
        upgrade: Some(Upgrader {
            manifest: keep_manifest,
            row: place_type_row,
        }),
    },
    FormatRecord {
        version: FormatVersion::pre_release(2),
        first_app_version: "0.1.0",
        upgrade: None,
    },
];

/// The kind and level of each standard Gramps place type as format 0.1 upgrades it (ADR 0031 §10),
/// frozen here: the live table may change without changing how an old archive reads.
const STD_KINDS: [(&str, &str, Option<&str>); 20] = [
    ("country", "Administrative", Some("Country")),
    ("state", "Administrative", Some("Adm1")),
    ("county", "Administrative", None),
    ("city", "Settlement", Some("Settlement")),
    ("parish", "Ecclesiastical", None),
    ("locality", "Settlement", Some("SubSettlement")),
    ("street", "Settlement", Some("SubSettlement")),
    ("province", "Administrative", Some("Adm1")),
    ("region", "Administrative", None),
    ("department", "Administrative", None),
    ("neighborhood", "Settlement", Some("SubSettlement")),
    ("district", "Administrative", None),
    ("borough", "Administrative", None),
    ("municipality", "Administrative", None),
    ("town", "Settlement", Some("Settlement")),
    ("village", "Settlement", Some("Settlement")),
    ("hamlet", "Settlement", Some("Settlement")),
    ("farm", "Settlement", None),
    ("building", "Building", None),
    ("number", "Building", None),
];

/// The 0.1 → 0.2 manifest step: the manifest keeps its shape.
#[expect(clippy::unnecessary_wraps, reason = "the `Upgrader::manifest` step signature")]
fn keep_manifest(manifest: Value) -> Result<Value, String> {
    Ok(manifest)
}

/// The 0.1 → 0.2 row step: `PlaceCreated` and `PlaceTypeSet` carry a `PlaceTypeStatement` in
/// `type_statement` (event version "2.0") in place of the closed `place_type`. Other rows pass through.
///
/// The row's `metadata` and `context.origin.digest` stay as stamped over the old body: the origin gate
/// recomputes the live digest from the decoded payload, and `record_origins` digests are recomputed
/// from the upgraded body on restore. Place types have no list-valued origin rows.
fn place_type_row(mut row: Value) -> Result<Value, String> {
    let is_place = row.get("aggregate_type").and_then(Value::as_str) == Some("place");
    let event_type = row
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if !is_place || !matches!(event_type.as_str(), "PlaceCreated" | "PlaceTypeSet") {
        return Ok(row);
    }
    let payload = row
        .get_mut("payload")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| format!("{event_type} row has no payload object"))?;
    let old = payload
        .remove("place_type")
        .ok_or_else(|| format!("{event_type} row has no `place_type` to upgrade"))?;
    let statement = statement_from_old(&old).map_err(|why| format!("{event_type} row: {why}"))?;
    payload.insert("type_statement".to_owned(), statement);
    row["event_version"] = json!("2.0");
    Ok(row)
}

/// The statement for a 0.1 `PlaceType` value: `{"type": "Farm"}` or `{"type": "Custom", "value": text}`.
fn statement_from_old(old: &Value) -> Result<Value, String> {
    let tag = old
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("the place type {old} is not a tagged value"))?;
    if tag == "Custom" {
        let text = old
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("the custom place type {old} has no text"))?;
        return Ok(statement_from_label(text));
    }
    let name = tag.to_lowercase();
    let known = [
        "country",
        "county",
        "municipality",
        "parish",
        "city",
        "town",
        "village",
        "farm",
        "building",
    ];
    if !known.contains(&name.as_str()) {
        return Err(format!("unknown place type {tag:?}"));
    }
    Ok(frozen_std_statement(&name))
}

/// The statement for Gramps-style label text: none for a placeholder, a `std` statement for a standard
/// label, else a bare `custom:` key.
fn statement_from_label(text: &str) -> Value {
    let Some(key) = place_type_key_from_gramps_label(text) else {
        return Value::Object(Map::new());
    };
    match key.strip_prefix("std:") {
        Some(name) => frozen_std_statement(name),
        None => json!({ "type_key": key }),
    }
}

/// The frozen statement of the standard row `name`.
fn frozen_std_statement(name: &str) -> Value {
    let key = format!("std:{name}");
    let Some((_, kind, level)) = STD_KINDS.iter().find(|row| row.0 == name) else {
        return json!({ "type_key": key });
    };
    let mut statement = json!({ "kind": kind, "type_key": key });
    if let Some(level) = level {
        statement["level"] = json!(level);
    }
    statement
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::HISTORY;
    use crate::backup::format::FormatVersion;
    use crate::place_vocabulary::std_statement;
    use vitni_interchange::GRAMPS_PLACE_TYPE_LABELS;

    fn upgrade_row(row: Value) -> Result<Value, String> {
        let Some(step) = HISTORY[0].upgrade else {
            return Err("format 0.1 has no upgrader".to_owned());
        };
        (step.row)(row)
    }

    fn place_row(event_type: &str, old_type: &Value) -> Value {
        json!({
            "aggregate_type": "place",
            "aggregate_id": "00000000-0000-7000-8000-000000000001",
            "sequence": 1,
            "event_type": event_type,
            "event_version": "1.0",
            "payload": {
                "type": event_type,
                "assertion_id": "00000000-0000-7000-8000-0000000000a1",
                "place_id": "00000000-0000-7000-8000-000000000001",
                "human_id": "P0001",
                "place_type": old_type,
                "context": { "confidence": "Normal" },
            },
            "metadata": {},
        })
    }

    fn upgraded_statement(event_type: &str, old_type: &Value) -> Value {
        let row = upgrade_row(place_row(event_type, old_type)).expect("upgrade");
        assert_eq!(row["event_version"], "2.0");
        assert_eq!(row["payload"].get("place_type"), None, "the old field is gone");
        row["payload"]["type_statement"].clone()
    }

    fn farm_like(kind: &str, level: Option<&str>, key: &str) -> Value {
        let mut statement = json!({ "kind": kind, "type_key": key });
        if let Some(level) = level {
            statement["level"] = json!(level);
        }
        statement
    }

    #[test]
    fn the_history_has_a_second_format_and_the_first_upgrades_to_it() {
        assert_eq!(HISTORY.len(), 2);
        assert_eq!(HISTORY[0].version, FormatVersion::pre_release(1));
        assert_eq!(HISTORY[1].version, FormatVersion::pre_release(2));
        assert!(HISTORY[0].upgrade.is_some());
        assert!(HISTORY[1].upgrade.is_none());
    }

    #[test]
    fn every_row_of_the_adr_table_upgrades_to_its_statement() {
        let table = [
            ("Country", farm_like("Administrative", Some("Country"), "std:country")),
            ("County", farm_like("Administrative", None, "std:county")),
            ("Municipality", farm_like("Administrative", None, "std:municipality")),
            ("Parish", farm_like("Ecclesiastical", None, "std:parish")),
            ("City", farm_like("Settlement", Some("Settlement"), "std:city")),
            ("Town", farm_like("Settlement", Some("Settlement"), "std:town")),
            ("Village", farm_like("Settlement", Some("Settlement"), "std:village")),
            ("Farm", farm_like("Settlement", None, "std:farm")),
            ("Building", farm_like("Building", None, "std:building")),
        ];
        for (old, expected) in table {
            for event_type in ["PlaceCreated", "PlaceTypeSet"] {
                let got = upgraded_statement(event_type, &json!({ "type": old }));
                assert_eq!(got, expected, "{event_type} {old}");
            }
        }
    }

    #[test]
    fn a_placeholder_blank_or_unknown_custom_type_upgrades_to_an_empty_statement() {
        for text in ["place", "", "   ", "Unknown"] {
            let got = upgraded_statement("PlaceTypeSet", &json!({ "type": "Custom", "value": text }));
            assert_eq!(got, json!({}), "{text:?}");
        }
    }

    #[test]
    fn a_custom_type_naming_a_gramps_label_takes_that_labels_key_kind_and_level() {
        let got = upgraded_statement("PlaceCreated", &json!({ "type": "Custom", "value": "Hamlet" }));
        assert_eq!(got, farm_like("Settlement", Some("Settlement"), "std:hamlet"));
    }

    #[test]
    fn any_other_custom_text_upgrades_to_a_custom_key_with_no_kind_or_level() {
        let got = upgraded_statement("PlaceCreated", &json!({ "type": "Custom", "value": "Sokn" }));
        assert_eq!(got, json!({ "type_key": "custom:Sokn" }));
    }

    #[test]
    fn a_place_row_without_the_old_field_is_an_error_naming_the_row() {
        let mut row = place_row("PlaceCreated", &json!({ "type": "Farm" }));
        row["payload"].as_object_mut().expect("payload").remove("place_type");
        let error = upgrade_row(row).expect_err("a missing field");
        assert!(error.contains("place_type"), "{error}");
        assert!(error.contains("PlaceCreated"), "{error}");
    }

    #[test]
    fn a_malformed_old_value_is_an_error_not_a_panic() {
        for old in [
            json!("Farm"),
            json!({ "type": "Hovel" }),
            json!({ "type": "Custom" }),
            json!(7),
        ] {
            let row = place_row("PlaceTypeSet", &old);
            assert!(upgrade_row(row).is_err(), "{old}");
        }
    }

    #[test]
    fn other_rows_pass_through_untouched() {
        let rows = [
            place_row("NameAsserted", &json!({ "type": "Farm" })),
            json!({
                "aggregate_type": "person",
                "aggregate_id": "00000000-0000-7000-8000-000000000002",
                "sequence": 1,
                "event_type": "PersonCreated",
                "event_version": "1.0",
                "payload": { "type": "PersonCreated", "place_type": "not ours" },
                "metadata": {},
            }),
        ];
        for row in rows {
            assert_eq!(upgrade_row(row.clone()).expect("upgrade"), row);
        }
    }

    #[test]
    fn the_manifest_step_changes_nothing_but_is_there() {
        let step = HISTORY[0].upgrade.expect("format 0.1 has an upgrader");
        let manifest = json!({ "format_version": "0.1", "event_count": 3 });
        assert_eq!((step.manifest)(manifest.clone()).expect("manifest"), manifest);
    }

    #[test]
    fn the_frozen_kind_and_level_table_equals_the_live_std_table_on_all_twenty_rows() {
        for label in GRAMPS_PLACE_TYPE_LABELS {
            let got = upgraded_statement("PlaceCreated", &json!({ "type": "Custom", "value": label }));
            let live = std_statement(&label.to_lowercase()).expect("a std row");
            let expected = serde_json::to_value(live).expect("statement");
            assert_eq!(got, expected, "{label}");
        }
    }
}
