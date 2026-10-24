//! The `std` place-type vocabulary table and the prefill and merge helpers over it (ADR 0031 §3, §10).

use std::sync::LazyLock;

use vitni_core::date::GenealogicalDate;
use vitni_core::place_type::{PlaceKind, PlaceLevel, PlaceTypeKey, PlaceTypeStatement};

const STD_NAMESPACE: &str = "std";

type StdRow = (&'static str, PlaceKind, Option<PlaceLevel>);

/// The twenty Gramps standard place types in Gramps order, with the kind and level each suggests (ADR 0031 §10).
const STD_TABLE: [StdRow; 20] = [
    ("country", PlaceKind::Administrative, Some(PlaceLevel::Country)),
    ("state", PlaceKind::Administrative, Some(PlaceLevel::Adm1)),
    ("county", PlaceKind::Administrative, None),
    ("city", PlaceKind::Settlement, Some(PlaceLevel::Settlement)),
    ("parish", PlaceKind::Ecclesiastical, None),
    ("locality", PlaceKind::Settlement, Some(PlaceLevel::SubSettlement)),
    ("street", PlaceKind::Settlement, Some(PlaceLevel::SubSettlement)),
    ("province", PlaceKind::Administrative, Some(PlaceLevel::Adm1)),
    ("region", PlaceKind::Administrative, None),
    ("department", PlaceKind::Administrative, None),
    ("neighborhood", PlaceKind::Settlement, Some(PlaceLevel::SubSettlement)),
    ("district", PlaceKind::Administrative, None),
    ("borough", PlaceKind::Administrative, None),
    ("municipality", PlaceKind::Administrative, None),
    ("town", PlaceKind::Settlement, Some(PlaceLevel::Settlement)),
    ("village", PlaceKind::Settlement, Some(PlaceLevel::Settlement)),
    ("hamlet", PlaceKind::Settlement, Some(PlaceLevel::Settlement)),
    ("farm", PlaceKind::Settlement, None),
    ("building", PlaceKind::Building, None),
    ("number", PlaceKind::Building, None),
];

fn statement_of(row: &StdRow) -> Option<PlaceTypeStatement> {
    let (name, kind, level) = *row;
    let type_key = PlaceTypeKey::new(&format!("{STD_NAMESPACE}:{name}")).ok()?;
    Some(PlaceTypeStatement {
        kind: Some(kind),
        level,
        type_key: Some(type_key),
        date: None,
    })
}

static STD_STATEMENTS: LazyLock<Vec<PlaceTypeStatement>> =
    LazyLock::new(|| STD_TABLE.iter().filter_map(statement_of).collect());

/// The twenty pre-filled, undated `std` statements in table order.
#[must_use]
pub fn std_statements() -> &'static [PlaceTypeStatement] {
    &STD_STATEMENTS
}

/// The pre-filled `std` statement for `name` (`"city"`), if the table has that row.
#[must_use]
pub fn std_statement(name: &str) -> Option<PlaceTypeStatement> {
    let row = STD_TABLE.iter().position(|row| row.0 == name)?;
    STD_STATEMENTS.get(row).cloned()
}

/// The kind and level the table suggests for a `std` key; `None` for any other key or a `std` key with no row.
#[must_use]
fn suggest(key: &PlaceTypeKey) -> Option<(PlaceKind, Option<PlaceLevel>)> {
    if key.namespace() != STD_NAMESPACE {
        return None;
    }
    let row = STD_TABLE.iter().find(|row| row.0 == key.name())?;
    Some((row.1, row.2))
}

/// Fills kind and level from the table when both are absent and the key is a `std` key with a row.
#[must_use]
pub fn prefill_from_std(mut statement: PlaceTypeStatement) -> PlaceTypeStatement {
    if statement.kind.is_some() || statement.level.is_some() {
        return statement;
    }
    let Some((kind, level)) = statement.type_key.as_ref().and_then(suggest) else {
        return statement;
    };
    statement.kind = Some(kind);
    statement.level = level;
    statement
}

/// The fields a caller states when setting a place type; the date picks the base and is passed separately.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceTypeGiven {
    /// The type key, if given.
    pub type_key: Option<PlaceTypeKey>,
    /// The kind, if given.
    pub kind: Option<PlaceKind>,
    /// The level, if given.
    pub level: Option<PlaceLevel>,
}

/// Merges what a caller `given` over `base`: unstated fields keep the base, and a given `std` key re-derives
/// kind and level from the table unless they are given too.
#[must_use]
pub fn merge_place_type(
    base: Option<&PlaceTypeStatement>,
    given: PlaceTypeGiven,
    date: Option<GenealogicalDate>,
) -> PlaceTypeStatement {
    let base_kind = base.and_then(|base| base.kind);
    let base_level = base.and_then(|base| base.level);
    let base_key = base.and_then(|base| base.type_key.clone());
    let suggested = given.type_key.as_ref().and_then(suggest);
    let (kind, level) = match suggested {
        Some((kind, level)) => (given.kind.or(Some(kind)), given.level.or(level)),
        None => (given.kind.or(base_kind), given.level.or(base_level)),
    };
    PlaceTypeStatement {
        kind,
        level,
        type_key: given.type_key.or(base_key),
        date,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use vitni_core::date::GenealogicalDate;
    use vitni_core::place_type::{PlaceKind, PlaceLevel, PlaceTypeKey, PlaceTypeStatement};
    use vitni_interchange::GRAMPS_PLACE_TYPE_LABELS;

    use super::{PlaceTypeGiven, merge_place_type, prefill_from_std, std_statement, std_statements, suggest};
    use crate::event::{DateParts, gregorian_date};

    fn key(text: &str) -> PlaceTypeKey {
        PlaceTypeKey::new(text).expect("test key is valid")
    }

    fn year(y: i32) -> GenealogicalDate {
        gregorian_date(DateParts {
            year: y,
            month: None,
            day: None,
        })
    }

    fn statement(
        type_key: Option<&str>,
        kind: Option<PlaceKind>,
        level: Option<PlaceLevel>,
        date: Option<GenealogicalDate>,
    ) -> PlaceTypeStatement {
        PlaceTypeStatement {
            kind,
            level,
            type_key: type_key.map(key),
            date,
        }
    }

    fn given(type_key: Option<&str>, kind: Option<PlaceKind>, level: Option<PlaceLevel>) -> PlaceTypeGiven {
        PlaceTypeGiven {
            type_key: type_key.map(key),
            kind,
            level,
        }
    }

    fn row(name: &str) -> (Option<PlaceKind>, Option<PlaceLevel>) {
        let statement = std_statement(name).expect("row exists");
        (statement.kind, statement.level)
    }

    #[test]
    fn table_has_twenty_unique_rows_and_all_build() {
        let statements = std_statements();
        assert_eq!(statements.len(), 20);
        let keys: HashSet<String> = statements
            .iter()
            .map(|s| s.type_key.as_ref().expect("every row has a key").as_str().to_owned())
            .collect();
        assert_eq!(keys.len(), 20);
        for statement in statements {
            assert!(statement.date.is_none());
            assert_eq!(statement.type_key.as_ref().map(PlaceTypeKey::namespace), Some("std"));
        }
    }

    #[test]
    fn table_key_set_equals_gramps_labels_lowercased() {
        let table: Vec<String> = std_statements()
            .iter()
            .map(|s| s.type_key.as_ref().expect("key").name().to_owned())
            .collect();
        let labels: Vec<String> = GRAMPS_PLACE_TYPE_LABELS.iter().map(|l| l.to_lowercase()).collect();
        assert_eq!(table, labels, "same keys in Gramps order");
    }

    #[test]
    fn adr_section_10_rows_are_fixed() {
        use PlaceKind::{Administrative, Building, Ecclesiastical, Settlement};
        assert_eq!(row("country"), (Some(Administrative), Some(PlaceLevel::Country)));
        assert_eq!(row("county"), (Some(Administrative), None));
        assert_eq!(row("municipality"), (Some(Administrative), None));
        assert_eq!(row("parish"), (Some(Ecclesiastical), None));
        for name in ["city", "town", "village"] {
            assert_eq!(row(name), (Some(Settlement), Some(PlaceLevel::Settlement)), "{name}");
        }
        assert_eq!(row("farm"), (Some(Settlement), None));
        assert_eq!(row("building"), (Some(Building), None));
    }

    #[test]
    fn std_statement_finds_defaults_and_rejects_unknown_names() {
        assert!(std_statement("city").is_some());
        assert!(std_statement("farm").is_some());
        assert!(std_statement("prestegjeld").is_none());
    }

    #[test]
    fn suggest_returns_the_row_for_std_keys_only() {
        assert_eq!(suggest(&key("std:farm")), Some((PlaceKind::Settlement, None)));
        assert_eq!(
            suggest(&key("std:city")),
            Some((PlaceKind::Settlement, Some(PlaceLevel::Settlement)))
        );
        assert_eq!(suggest(&key("no:prestegjeld")), None);
        assert_eq!(suggest(&key("custom:Sokn")), None);
        assert_eq!(suggest(&key("std:foo")), None);
    }

    #[test]
    fn prefill_fills_a_bare_std_key_and_keeps_the_date() {
        let filled = prefill_from_std(statement(Some("std:farm"), None, None, Some(year(1838))));
        assert_eq!(filled.kind, Some(PlaceKind::Settlement));
        assert_eq!(filled.level, None);
        assert_eq!(filled.type_key, Some(key("std:farm")));
        assert_eq!(filled.date, Some(year(1838)));
    }

    #[test]
    fn prefill_leaves_explicit_kind_or_level_unchanged() {
        let explicit = statement(Some("std:farm"), Some(PlaceKind::Cadastral), None, None);
        assert_eq!(prefill_from_std(explicit.clone()), explicit);
        let with_level = statement(Some("std:farm"), None, Some(PlaceLevel::Adm3), None);
        assert_eq!(prefill_from_std(with_level.clone()), with_level);
    }

    #[test]
    fn prefill_leaves_non_std_and_keyless_statements_unchanged() {
        for text in ["no:prestegjeld", "custom:Sokn", "std:foo"] {
            let bare = statement(Some(text), None, None, None);
            assert_eq!(prefill_from_std(bare.clone()), bare, "{text}");
        }
        let empty = PlaceTypeStatement::default();
        assert_eq!(prefill_from_std(empty.clone()), empty);
    }

    #[test]
    fn merge_level_only_keeps_key_and_kind() {
        let base = std_statement("county").expect("row");
        let merged = merge_place_type(Some(&base), given(None, None, Some(PlaceLevel::Adm1)), None);
        assert_eq!(merged.type_key, Some(key("std:county")));
        assert_eq!(merged.kind, Some(PlaceKind::Administrative));
        assert_eq!(merged.level, Some(PlaceLevel::Adm1));
        assert_eq!(merged.date, None);
    }

    #[test]
    fn merge_kind_only_on_untyped_place_stores_kind_without_key() {
        let merged = merge_place_type(None, given(None, Some(PlaceKind::Ecclesiastical), None), None);
        assert_eq!(merged, statement(None, Some(PlaceKind::Ecclesiastical), None, None));
    }

    #[test]
    fn merge_all_three_given_stores_all_three() {
        let merged = merge_place_type(
            None,
            given(
                Some("no:prestegjeld"),
                Some(PlaceKind::Ecclesiastical),
                Some(PlaceLevel::Adm3),
            ),
            None,
        );
        assert_eq!(
            merged,
            statement(
                Some("no:prestegjeld"),
                Some(PlaceKind::Ecclesiastical),
                Some(PlaceLevel::Adm3),
                None
            )
        );
    }

    #[test]
    fn merge_std_type_rederives_kind_and_level_from_the_table() {
        let base = std_statement("farm").expect("row");
        let merged = merge_place_type(Some(&base), given(Some("std:parish"), None, None), None);
        assert_eq!(merged.type_key, Some(key("std:parish")));
        assert_eq!(merged.kind, Some(PlaceKind::Ecclesiastical));
        assert_eq!(merged.level, None);
    }

    #[test]
    fn merge_std_type_with_explicit_kind_lets_kind_win_and_derives_level() {
        let base = std_statement("farm").expect("row");
        let merged = merge_place_type(
            Some(&base),
            given(Some("std:city"), Some(PlaceKind::Cadastral), None),
            None,
        );
        assert_eq!(merged.kind, Some(PlaceKind::Cadastral));
        assert_eq!(merged.level, Some(PlaceLevel::Settlement));
    }

    #[test]
    fn merge_non_std_type_keeps_base_kind_and_level() {
        let base = statement(
            Some("std:county"),
            Some(PlaceKind::Administrative),
            Some(PlaceLevel::Adm2),
            None,
        );
        let merged = merge_place_type(Some(&base), given(Some("no:fylke"), None, None), None);
        assert_eq!(merged.type_key, Some(key("no:fylke")));
        assert_eq!(merged.kind, Some(PlaceKind::Administrative));
        assert_eq!(merged.level, Some(PlaceLevel::Adm2));
    }

    #[test]
    fn merge_dated_level_keeps_the_dated_base_key_and_kind() {
        let base = statement(
            Some("no:prestegjeld"),
            Some(PlaceKind::Ecclesiastical),
            None,
            Some(year(1838)),
        );
        let merged = merge_place_type(Some(&base), given(None, None, Some(PlaceLevel::Adm1)), Some(year(1838)));
        assert_eq!(merged.type_key, Some(key("no:prestegjeld")));
        assert_eq!(merged.kind, Some(PlaceKind::Ecclesiastical));
        assert_eq!(merged.level, Some(PlaceLevel::Adm1));
        assert_eq!(merged.date, Some(year(1838)));
    }

    #[test]
    fn merge_with_empty_base_and_a_date_adds_a_dated_entry() {
        let merged = merge_place_type(None, given(Some("std:parish"), None, None), Some(year(1838)));
        assert_eq!(merged.kind, Some(PlaceKind::Ecclesiastical));
        assert_eq!(merged.date, Some(year(1838)));
    }
}
