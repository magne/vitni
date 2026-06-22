//! The conclusion-projection table names, generated from the canonical [`registry`](crate::registry).
//!
//! Table names are engine-neutral — both backends write the same `*_view` tables and the
//! cross-aggregate [`resolver`](crate::resolver)s read them — so they live here, in one
//! always-compiled place, rather than inside any one backend's wiring.

use crate::registry::for_each_db_aggregate;

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
