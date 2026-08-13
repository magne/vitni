//! The date-aware resolution rule shared by Place name/enclosure/geometry temporal reads (ADR 0026
//! §1).
//!
//! A place's name, enclosing parent, and geometry are each dated, accumulating assertions (never
//! last-writer-wins — `place_name.rs`, `place_ref.rs`, `place_geometry.rs`). Reading "the" value as
//! of some date needs one rule, used identically everywhere: treat each assertion's date as
//! **effective from**, and pick the one with the latest effective date that is still `<=` the target,
//! falling back to the first undated ("primary") assertion when none qualifies. Assertions stay
//! unordered in the log; this is a pure read over the already-accumulated set — no `[from, until)`
//! validity interval is introduced (deferred per ADR 0026 §1).
//!
//! This one rule drives the generated place title, the transitive hierarchy walk
//! (`vitni-app/src/place.rs`), and — later — the geography view's time slider (ADR 0025).

/// Resolves the item in `items` valid **as of** `target_sort_value` (a [`GenealogicalDate`]'s
/// precomputed [`sort_value`](crate::date::GenealogicalDate::sort_value)): the item whose
/// `effective_from` date is the latest one `<= target_sort_value`, or — when none qualifies — the
/// first item with no date at all (the undated/primary case).
///
/// `effective_from` extracts each item's optional date's `sort_value`; items are read in the order
/// given (accumulation/assertion order). Among dated candidates tied on `sort_value`, the
/// later-encountered one wins (the most-recently-asserted takes precedence). Returns `None` only when
/// `items` has neither a qualifying dated item nor any undated item.
pub fn resolve_as_of<'a, T>(
    items: impl Iterator<Item = &'a T>,
    target_sort_value: i64,
    effective_from: impl Fn(&T) -> Option<i64>,
) -> Option<&'a T> {
    let mut best: Option<(&'a T, i64)> = None;
    let mut fallback: Option<&'a T> = None;
    for item in items {
        match effective_from(item) {
            Some(sort_value) if sort_value <= target_sort_value => {
                let take = best.is_none_or(|(_, best_sort_value)| sort_value >= best_sort_value);
                if take {
                    best = Some((item, sort_value));
                }
            }
            Some(_) => {}
            None => {
                if fallback.is_none() {
                    fallback = Some(item);
                }
            }
        }
    }
    best.map(|(item, _)| item).or(fallback)
}

#[cfg(test)]
mod tests {
    use super::resolve_as_of;

    #[derive(Debug, PartialEq, Eq)]
    struct Dated {
        label: &'static str,
        sort_value: Option<i64>,
    }

    fn dated(label: &'static str, sort_value: i64) -> Dated {
        Dated {
            label,
            sort_value: Some(sort_value),
        }
    }

    fn undated(label: &'static str) -> Dated {
        Dated {
            label,
            sort_value: None,
        }
    }

    fn resolve(items: &[Dated], target: i64) -> Option<&Dated> {
        resolve_as_of(items.iter(), target, |item| item.sort_value)
    }

    #[test]
    fn picks_the_latest_dated_item_at_or_before_the_target() {
        let items = vec![dated("1801", 1801), dated("1900", 1900), dated("1950", 1950)];
        assert_eq!(resolve(&items, 1920).unwrap().label, "1900");
    }

    #[test]
    fn an_exact_match_on_the_target_date_wins() {
        let items = vec![dated("1900", 1900), dated("1950", 1950)];
        assert_eq!(resolve(&items, 1900).unwrap().label, "1900");
    }

    #[test]
    fn falls_back_to_the_undated_item_when_no_dated_item_qualifies() {
        let items = vec![undated("current"), dated("1950", 1950)];
        assert_eq!(resolve(&items, 1900).unwrap().label, "current");
    }

    #[test]
    fn returns_none_when_nothing_qualifies_and_nothing_is_undated() {
        let items = vec![dated("1950", 1950)];
        assert!(resolve(&items, 1900).is_none());
    }

    #[test]
    fn the_first_undated_item_is_the_primary_fallback() {
        let items = vec![undated("first"), undated("second")];
        assert_eq!(resolve(&items, 1900).unwrap().label, "first");
    }

    #[test]
    fn ties_on_sort_value_are_broken_by_the_later_item() {
        let items = vec![dated("earlier-asserted", 1900), dated("later-asserted", 1900)];
        assert_eq!(resolve(&items, 1900).unwrap().label, "later-asserted");
    }

    #[test]
    fn an_empty_set_resolves_to_none() {
        let items: Vec<Dated> = Vec::new();
        assert!(resolve(&items, 1900).is_none());
    }
}
