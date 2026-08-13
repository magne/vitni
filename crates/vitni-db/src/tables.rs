//! The conclusion-projection table names, generated from the canonical [`registry`](crate::registry).
//!
//! Table names are engine-neutral — both backends write the same `*_view` tables and the
//! cross-aggregate [`resolver`](crate::resolver)s read them — so they live here, in one
//! always-compiled place, rather than inside any one backend's wiring.

use crate::registry::{for_each_db_aggregate, for_each_db_human_id_aggregate};

/// Defines one `pub(crate) const <TABLE>: &str = "<name>"` per aggregate plus [`ALL_VIEW_TABLES`],
/// using only the `table_const`/`table_str` columns of each registry row (the rest is ignored).
macro_rules! db_view_tables {
    ($(($snake:ident, $State:ty, $View:ty, $Cmd:ty, $Err:ty, $table_const:ident, $table_str:literal, $($rest:tt)*)),+ $(,)?) => {
        $(
            #[doc = concat!("The ", stringify!($snake), " conclusion projection table written by the `GenericQuery`.")]
            pub(crate) const $table_const: &str = $table_str;
        )+

        /// Every projection table, in aggregate order — created at `open()` and rebuilt together.
        pub(crate) const ALL_VIEW_TABLES: &[&str] = &[$($table_const),+];
    };
}

for_each_db_aggregate!(db_view_tables);

/// Defines [`HUMAN_ID_VIEW_TABLES`], the subset of [`ALL_VIEW_TABLES`] that carry a `human_id`
/// (every aggregate but Tag) — the tables the `human_id` indexes (ADR 0032) are created for.
macro_rules! db_human_id_view_tables {
    ($(($snake:ident, $next:ident, $table_const:ident)),+ $(,)?) => {
        /// Every projection table with a `human_id` column indexed (ADR 0032) — all 12 aggregates
        /// but Tag, which has no `human_id`.
        pub(crate) const HUMAN_ID_VIEW_TABLES: &[&str] = &[$($table_const),+];
    };
}

for_each_db_human_id_aggregate!(db_human_id_view_tables);

/// Defines [`view_table_for`], mapping an `events.aggregate_type` value (the snake aggregate name,
/// e.g. `person`) to its projection table, using the `snake`/`table_const` columns of each row.
macro_rules! db_view_table_lookup {
    ($(($snake:ident, $State:ty, $View:ty, $Cmd:ty, $Err:ty, $table_const:ident, $table_str:literal, $($rest:tt)*)),+ $(,)?) => {
        /// The projection table for `aggregate_type` (the stored `Aggregate::TYPE`), or `None` if it
        /// is not one of the 12 aggregates.
        pub(crate) fn view_table_for(aggregate_type: &str) -> Option<&'static str> {
            match aggregate_type {
                $(stringify!($snake) => Some($table_const),)+
                _ => None,
            }
        }
    };
}

for_each_db_aggregate!(db_view_table_lookup);
