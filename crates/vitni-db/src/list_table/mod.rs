//! The list tables (ADR 0052 §5) and the list query evaluated over them (§4).
//!
//! A list shows one kind's records, filtered, sorted and paged. The values it filters and sorts by are
//! not all in the record's own projection: a person's birth date is its Birth event's, an event's place
//! name the place's. Each listed kind therefore has a derived, rebuildable table (ADR 0010) holding one
//! row per record and one typed SQL column per catalogue column, written after the projection and
//! recomputed for the records [`record_links`](crate::record_links) names as referring to a changed one.
//! A tag column is not stored: the `tagged` links answer it.
//!
//! The catalogue lives here, beside the table it describes, because the projection pipeline that fills
//! the table is this crate's; `vitni-app` re-exports it as the column catalogue its `ListQuery` names.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
mod rows;
mod sql;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use serde::{Deserialize, Serialize};
use vitni_core::date::GenealogicalDate;
use vitni_core::ids::TagId;
use vitni_core::matching::MatchableKind;

use crate::store::DbError;

pub(crate) use rows::ListedRecord;

/// A kind of record a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ListedKind {
    /// Persons.
    Person,
    /// Families.
    Family,
    /// Events.
    Event,
    /// Places.
    Place,
    /// Sources.
    Source,
    /// Citations.
    Citation,
    /// Repositories.
    Repository,
    /// Media.
    Media,
    /// Notes.
    Note,
    /// Research notes.
    ResearchNote,
    /// DNA tests.
    DnaTest,
    /// DNA matches.
    DnaMatch,
    /// Tags.
    Tag,
}

/// The type of a column's values, which fixes the tests a filter on it may use (ADR 0052 §1–2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnType {
    /// A string.
    Text,
    /// A genealogical date, compared by its sort key.
    Date,
    /// An integer or decimal.
    Number,
    /// A value of a closed core enum, held as its kebab-case variant name (`male`, `very-high`).
    Choice,
    /// The record's tags.
    Tags,
    /// A reference to another record, shown by that record's title.
    Record,
}

/// One column of a kind's catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListColumn {
    /// The stable column id a query names (`birth`, `surname`).
    pub id: &'static str,
    /// The type of its values.
    pub ty: ColumnType,
}

const fn column(id: &'static str, ty: ColumnType) -> ListColumn {
    ListColumn { id, ty }
}

const TAGS: ListColumn = column("tags", ColumnType::Tags);

const PERSON: &[ListColumn] = &[
    column("name", ColumnType::Text),
    column("given", ColumnType::Text),
    column("surname", ColumnType::Text),
    column("sex", ColumnType::Choice),
    column("birth", ColumnType::Date),
    column("death", ColumnType::Date),
    TAGS,
];
const FAMILY: &[ListColumn] = &[
    column("partners", ColumnType::Text),
    column("marriage", ColumnType::Date),
    column("children", ColumnType::Number),
    TAGS,
];
const EVENT: &[ListColumn] = &[
    column("type", ColumnType::Choice),
    column("date", ColumnType::Date),
    column("place", ColumnType::Record),
    column("description", ColumnType::Text),
    TAGS,
];
const PLACE: &[ListColumn] = &[
    column("name", ColumnType::Text),
    column("type", ColumnType::Choice),
    column("enclosed-by", ColumnType::Record),
    TAGS,
];
const SOURCE: &[ListColumn] = &[
    column("title", ColumnType::Text),
    column("author", ColumnType::Text),
    column("abbreviation", ColumnType::Text),
    TAGS,
];
const CITATION: &[ListColumn] = &[
    column("source", ColumnType::Record),
    column("page", ColumnType::Text),
    column("date", ColumnType::Date),
    column("confidence", ColumnType::Choice),
    TAGS,
];
const REPOSITORY: &[ListColumn] = &[
    column("name", ColumnType::Text),
    column("type", ColumnType::Choice),
    TAGS,
];
const MEDIA: &[ListColumn] = &[
    column("path", ColumnType::Text),
    column("mime", ColumnType::Text),
    column("date", ColumnType::Date),
    TAGS,
];
const NOTE: &[ListColumn] = &[
    column("text", ColumnType::Text),
    column("type", ColumnType::Choice),
    TAGS,
];
const RESEARCH_NOTE: &[ListColumn] = &[
    column("title", ColumnType::Text),
    column("text", ColumnType::Text),
    TAGS,
];
const DNA_TEST: &[ListColumn] = &[
    column("person", ColumnType::Record),
    column("provider", ColumnType::Choice),
    column("test-type", ColumnType::Choice),
    column("kit", ColumnType::Text),
    TAGS,
];
const DNA_MATCH: &[ListColumn] = &[
    column("test-a", ColumnType::Record),
    column("test-b", ColumnType::Record),
    column("shared-cm", ColumnType::Number),
    column("status", ColumnType::Choice),
    column("relationship", ColumnType::Text),
    TAGS,
];
const TAG: &[ListColumn] = &[
    column("name", ColumnType::Text),
    column("priority", ColumnType::Number),
    column("color", ColumnType::Text),
];

impl ListedKind {
    /// Every listed kind.
    pub const ALL: &'static [Self] = &[
        Self::Person,
        Self::Family,
        Self::Event,
        Self::Place,
        Self::Source,
        Self::Citation,
        Self::Repository,
        Self::Media,
        Self::Note,
        Self::ResearchNote,
        Self::DnaTest,
        Self::DnaMatch,
        Self::Tag,
    ];

    /// The kind's column catalogue. The first column is its **title column**, which every row shows.
    #[must_use]
    pub const fn columns(self) -> &'static [ListColumn] {
        match self {
            Self::Person => PERSON,
            Self::Family => FAMILY,
            Self::Event => EVENT,
            Self::Place => PLACE,
            Self::Source => SOURCE,
            Self::Citation => CITATION,
            Self::Repository => REPOSITORY,
            Self::Media => MEDIA,
            Self::Note => NOTE,
            Self::ResearchNote => RESEARCH_NOTE,
            Self::DnaTest => DNA_TEST,
            Self::DnaMatch => DNA_MATCH,
            Self::Tag => TAG,
        }
    }

    /// The kind's title column, the one every row shows: its catalogue's first.
    #[must_use]
    pub const fn title_column(self) -> ListColumn {
        match self.columns() {
            [title, ..] => *title,
            // Every catalogue above names a first column; a test holds each to it.
            [] => TAGS,
        }
    }

    /// The kind's name as a query and the CLI spell it (`person`, `dna-test`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Family => "family",
            Self::Event => "event",
            Self::Place => "place",
            Self::Source => "source",
            Self::Citation => "citation",
            Self::Repository => "repository",
            Self::Media => "media",
            Self::Note => "note",
            Self::ResearchNote => "research-note",
            Self::DnaTest => "dna-test",
            Self::DnaMatch => "dna-match",
            Self::Tag => "tag",
        }
    }

    /// The column named `id`, if the kind has one.
    #[must_use]
    pub fn column(self, id: &str) -> Option<ListColumn> {
        self.columns().iter().find(|column| column.id == id).copied()
    }

    /// The kind's list table.
    #[must_use]
    pub const fn table(self) -> &'static str {
        match self {
            Self::Person => "person_list",
            Self::Family => "family_list",
            Self::Event => "event_list",
            Self::Place => "place_list",
            Self::Source => "source_list",
            Self::Citation => "citation_list",
            Self::Repository => "repository_list",
            Self::Media => "media_list",
            Self::Note => "note_list",
            Self::ResearchNote => "research_note_list",
            Self::DnaTest => "dna_test_list",
            Self::DnaMatch => "dna_match_list",
            Self::Tag => "tag_list",
        }
    }

    /// The kind whose merged records the list leaves out (ADR 0039 §5), `None` where no merge applies.
    const fn merged_kind(self) -> Option<MatchableKind> {
        match self {
            Self::Person => Some(MatchableKind::Person),
            Self::Family => Some(MatchableKind::Family),
            Self::Event => Some(MatchableKind::Event),
            Self::Place => Some(MatchableKind::Place),
            Self::Source => Some(MatchableKind::Source),
            Self::Citation => Some(MatchableKind::Citation),
            Self::Repository => Some(MatchableKind::Repository),
            Self::Media => Some(MatchableKind::Media),
            Self::Note => Some(MatchableKind::Note),
            Self::ResearchNote | Self::DnaTest | Self::DnaMatch | Self::Tag => None,
        }
    }
}

/// One typed cell of a row.
#[derive(Debug, Clone, PartialEq)]
pub enum ListCell {
    /// A text value.
    Text(String),
    /// A date.
    Date(GenealogicalDate),
    /// A number.
    Number(f64),
    /// A choice, as its kebab-case variant name.
    Choice(String),
    /// The record's tags, in id order.
    Tags(Vec<TagId>),
    /// A referenced record: its id, and its title when it has one.
    Record {
        /// The referenced record's aggregate id.
        id: String,
        /// The referenced record's title.
        title: Option<String>,
    },
}

/// What a filter tests a column for (ADR 0052 §2). Text and choice values compare case-insensitively.
#[derive(Debug, Clone, PartialEq)]
pub enum ListTest {
    /// Text: the value contains the string.
    Contains(String),
    /// Text: the value is the string. Record: the reference is the record with this id.
    Is(String),
    /// Text, date or record: the record has no value.
    IsEmpty,
    /// Date: the sort key is less than this one.
    Before(i64),
    /// Date: the sort key is greater than this one.
    After(i64),
    /// Date: the sort key lies in this inclusive range.
    DatesBetween(i64, i64),
    /// Number: the value is less than this one.
    LessThan(f64),
    /// Number: the value is greater than this one.
    GreaterThan(f64),
    /// Number: the value lies in this inclusive range.
    NumbersBetween(f64, f64),
    /// Choice: the value is one of these. Tags: the record has one of these tag ids.
    AnyOf(Vec<String>),
    /// Choice: the value is none of these (or absent). Tags: the record has none of these tag ids.
    NoneOf(Vec<String>),
    /// Tags: the record has every one of these tag ids.
    AllOf(Vec<String>),
}

/// One filter: a column and the test its value must pass.
#[derive(Debug, Clone, PartialEq)]
pub struct ListPredicate {
    /// The column id.
    pub column: String,
    /// The test.
    pub test: ListTest,
}

/// A sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortDirection {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
}

/// A sort: one column and a direction. Rows with no value sort last either way; ties break by
/// `human_id`, then the record id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListSort {
    /// The column id.
    pub column: String,
    /// The direction.
    pub direction: SortDirection,
}

/// The value a row sorted by — part of the [`ListCursor`] a keyset read continues from.
#[derive(Debug, Clone, PartialEq)]
pub enum SortKey {
    /// A folded text value.
    Text(String),
    /// A date sort key.
    Int(i64),
    /// A number.
    Real(f64),
}

/// Where a row stands in its query's order: the read after it continues from here (ADR 0052 §4).
#[derive(Debug, Clone, PartialEq)]
pub struct ListCursor {
    /// The row's sort value, `None` when the query is unsorted or the row has no value.
    pub sort: Option<SortKey>,
    /// The row's `human_id`.
    pub human_id: Option<String>,
    /// The row's record id.
    pub id: String,
}

/// Which rows of the ordered matches a query returns.
#[derive(Debug, Clone, PartialEq)]
pub enum ListWindow {
    /// The rows from `offset`, at most `limit` of them (all with `None`).
    Offset {
        /// Rows skipped.
        offset: u32,
        /// Rows returned at most.
        limit: Option<u32>,
    },
    /// The rows after `cursor`, at most `limit` of them: deep in a list, a keyset read stays as fast
    /// as the first page, where a large offset does not.
    After {
        /// The row the read continues after.
        cursor: ListCursor,
        /// Rows returned at most.
        limit: Option<u32>,
    },
}

/// A list query, in store terms: the app's `ListQuery` once validated against the catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct ListSpec {
    /// The kind listed.
    pub kind: ListedKind,
    /// A substring matched case-insensitively against the text columns and the `human_id`.
    pub search: Option<String>,
    /// Filters, all of which a row must pass.
    pub filters: Vec<ListPredicate>,
    /// The sort; `None` orders by `human_id`, then id.
    pub sort: Option<ListSort>,
    /// The column ids whose cells each row carries, in this order.
    pub columns: Vec<String>,
    /// The rows wanted.
    pub window: ListWindow,
}

/// One row of a query's result.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    /// The record's aggregate id.
    pub id: String,
    /// The record's `human_id`, `None` for a kind that has none (tags).
    pub human_id: Option<String>,
    /// One cell per requested column, `None` where the record has no value.
    pub cells: Vec<Option<ListCell>>,
    /// The row's place in the order, for a keyset read after it.
    pub cursor: ListCursor,
}

/// A query's rows and how many records match it in all.
#[derive(Debug, Clone, PartialEq)]
pub struct ListResult {
    /// The rows in the window.
    pub rows: Vec<ListRow>,
    /// Every record matching the query, the window aside.
    pub total: u64,
}

/// The tag ids of a comma-joined `tagged` list, in id order.
fn tag_ids(joined: Option<&str>) -> Result<Vec<TagId>, DbError> {
    let mut tags = Vec::new();
    for id in joined.unwrap_or_default().split(',').filter(|id| !id.is_empty()) {
        let uuid = uuid::Uuid::parse_str(id).map_err(|e| DbError::Backend(format!("decoding a tag id `{id}`: {e}")))?;
        tags.push(TagId::from_uuid(uuid));
    }
    tags.sort_by_key(TagId::as_uuid);
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::{ColumnType, ListedKind};

    #[test]
    fn every_kind_has_a_text_or_record_title_and_unique_column_ids() {
        for kind in ListedKind::ALL {
            let title = kind.title_column();
            assert_eq!(kind.columns().first(), Some(&title), "{kind:?}");
            assert_ne!(title.ty, ColumnType::Tags, "{kind:?}");
            let mut ids: Vec<&str> = kind.columns().iter().map(|column| column.id).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), kind.columns().len(), "{kind:?}");
        }
    }
}
