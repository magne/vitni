//! The Explorer filter bar's view-model (ADR 0052 §2–3): the clauses a list query carries, as the editor
//! holds them while they are typed and as the chips show them once applied.
//!
//! A [`FilterDraft`] is one clause in the editor: a column of the kind's catalogue, one of the operators
//! its value type takes, and the raw value inputs — two text fields (the second for *between*) and the
//! chosen keys or ids of a choice, tags or record value. [`FilterDraft::apply`] reads it into the store's
//! [`ListFilter`], or says which input is wrong. An [`ActiveFilter`] keeps the draft beside the clause,
//! so editing a chip reopens exactly what was typed and a tag or record shows by the name it was picked
//! by, while the query holds only its id (ADR 0052 §3).

use std::collections::{BTreeMap, BTreeSet};

use vitni_app::{
    Calendar, ColumnType, DateParts, DnaProvider, DnaTestType, EventType, FilterOp, FilterValue, GenealogicalDate,
    ListFilter, ListedKind, MatchStatus, NoteType, PlaceKind, PlaceLevel, RepositoryType, SavedSearchId,
    SavedSearchList, Sex, UnreadableSavedSearch, choice_key, gregorian_date, same_saved_name,
};

use crate::i18n::Localizer;
use crate::presentation::ConfidenceLevel;
use crate::view_model::{DateDraft, parse_date_point};

/// One catalogue column, labelled, as the editor's column select offers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnChoice {
    /// The column id.
    pub id: &'static str,
    /// The localized label.
    pub label: String,
    /// The column's value type, which picks its operators and value editor.
    pub ty: ColumnType,
}

/// One value of a choice column: the key the query holds and its localized label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOption {
    /// The kebab-case key (`female`, `twenty-three-and-me`).
    pub key: String,
    /// The localized label.
    pub label: String,
}

/// Why a [`FilterDraft`] cannot be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDraftError {
    /// A value the operator needs is blank, or nothing is chosen.
    MissingValue,
    /// A date input names no date.
    BadDate,
    /// A number input is not a number.
    BadNumber,
}

/// One clause as the filter editor holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterDraft {
    /// The column id.
    pub column: String,
    /// The operator.
    pub op: FilterOp,
    /// The first text input: the text, date or number compared with, or a range's start.
    pub first: String,
    /// The second text input: a *between* range's end.
    pub second: String,
    /// The chosen choice keys, tag ids, or the one record id.
    pub values: Vec<String>,
    /// The name each chosen tag or record id was picked by.
    pub labels: BTreeMap<String, String>,
    /// The chosen record ids that name no record any more; their label is display text only.
    pub deleted_records: BTreeSet<String>,
}

/// An applied clause: what the query holds, and the draft it was read from.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveFilter {
    /// The clause the store evaluates.
    pub filter: ListFilter,
    /// The draft, for reopening the clause in the editor and naming its values.
    pub draft: FilterDraft,
}

impl FilterDraft {
    /// A new clause over `kind`'s title column, with that column's first operator.
    #[must_use]
    pub fn new(kind: ListedKind) -> Self {
        let title = kind.title_column();
        Self::blank(title.id, title.ty)
    }

    /// This clause moved to `column`, with that column's first operator and no value; unchanged when
    /// `kind` has no such column.
    #[must_use]
    pub fn with_column(self, kind: ListedKind, column: &str) -> Self {
        match kind.column(column) {
            Some(found) => Self::blank(found.id, found.ty),
            None => self,
        }
    }

    /// This clause with operator `op`, keeping the values it still takes.
    #[must_use]
    pub fn with_op(self, op: FilterOp) -> Self {
        let second = if op == FilterOp::Between {
            self.second
        } else {
            String::new()
        };
        Self { op, second, ..self }
    }

    fn blank(column: &str, ty: ColumnType) -> Self {
        Self {
            column: column.to_owned(),
            op: FilterOp::for_type(ty).first().copied().unwrap_or(FilterOp::Is),
            first: String::new(),
            second: String::new(),
            values: Vec::new(),
            labels: BTreeMap::new(),
            deleted_records: BTreeSet::new(),
        }
    }

    /// The value editor this clause needs: none for *is empty*.
    #[must_use]
    pub fn value_type(&self, kind: ListedKind) -> Option<ColumnType> {
        if self.op == FilterOp::IsEmpty {
            return None;
        }
        kind.column(&self.column).map(|column| column.ty)
    }

    /// The clause this draft reads as over `kind`.
    ///
    /// # Errors
    ///
    /// The [`FilterDraftError`] of the first input that does not read.
    pub fn apply(&self, kind: ListedKind) -> Result<ActiveFilter, FilterDraftError> {
        let value = match self.value_type(kind) {
            None => FilterValue::None,
            Some(ColumnType::Text) => FilterValue::Text(required(&self.first)?.to_owned()),
            Some(ColumnType::Date) => self.date_value()?,
            Some(ColumnType::Number) => self.number_value()?,
            Some(ColumnType::Choice | ColumnType::Tags) => FilterValue::Values(self.chosen()?.to_vec()),
            Some(ColumnType::Record) => FilterValue::Text(self.chosen()?.first().cloned().unwrap_or_default()),
        };
        Ok(ActiveFilter {
            filter: ListFilter {
                column: self.column.clone(),
                op: self.op,
                value,
            },
            draft: self.clone(),
        })
    }

    /// The draft that reads back as `filter` over `kind`, the inverse of [`Self::apply`] for the values
    /// a clause holds as typed: text, dates and numbers. The ids of a tag, choice or record value, and a
    /// record column's text, are left for the caller to label.
    #[must_use]
    pub fn from_filter(kind: ListedKind, filter: &ListFilter) -> Self {
        let mut draft = Self::new(kind).with_column(kind, &filter.column).with_op(filter.op);
        match &filter.value {
            FilterValue::None | FilterValue::Values(_) => {}
            FilterValue::Text(text) => {
                if draft.value_type(kind) != Some(ColumnType::Record) {
                    draft.first.clone_from(text);
                }
            }
            FilterValue::Date(date) => draft.first = DateDraft::from_value(date, String::new()).start,
            FilterValue::DateRange { from, to } => {
                draft.first = DateDraft::from_value(from, String::new()).start;
                draft.second = DateDraft::from_value(to, String::new()).start;
            }
            FilterValue::Number(number) => draft.first = number.to_string(),
            FilterValue::NumberRange { from, to } => {
                draft.first = from.to_string();
                draft.second = to.to_string();
            }
        }
        draft
    }

    fn date_value(&self) -> Result<FilterValue, FilterDraftError> {
        let from = date(&self.first)?;
        if self.op != FilterOp::Between {
            return Ok(FilterValue::Date(from));
        }
        Ok(FilterValue::DateRange {
            from,
            to: date(&self.second)?,
        })
    }

    fn number_value(&self) -> Result<FilterValue, FilterDraftError> {
        let from = number(&self.first)?;
        if self.op != FilterOp::Between {
            return Ok(FilterValue::Number(from));
        }
        Ok(FilterValue::NumberRange {
            from,
            to: number(&self.second)?,
        })
    }

    fn chosen(&self) -> Result<&[String], FilterDraftError> {
        if self.values.is_empty() {
            return Err(FilterDraftError::MissingValue);
        }
        Ok(&self.values)
    }
}

/// `text` trimmed, unless it is blank.
fn required(text: &str) -> Result<&str, FilterDraftError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(FilterDraftError::MissingValue);
    }
    Ok(text)
}

/// A typed date (any form the date fields read), on the Gregorian calendar.
fn date(text: &str) -> Result<GenealogicalDate, FilterDraftError> {
    let point = parse_date_point(required(text)?, Calendar::Gregorian).map_err(|_| FilterDraftError::BadDate)?;
    let year = point.year.ok_or(FilterDraftError::BadDate)?;
    Ok(gregorian_date(DateParts {
        year,
        month: point.month,
        day: point.day,
    }))
}

/// A typed number, a decimal comma read as a point.
fn number(text: &str) -> Result<f64, FilterDraftError> {
    required(text)?
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or(FilterDraftError::BadNumber)
}

/// Every column of `kind`'s catalogue, labelled, title column first.
#[must_use]
pub fn filter_columns(loc: &Localizer, kind: ListedKind) -> Vec<ColumnChoice> {
    let mut columns = Vec::with_capacity(kind.columns().len());
    for column in kind.columns() {
        columns.push(ColumnChoice {
            id: column.id,
            label: loc.list_column_label(column.id),
            ty: column.ty,
        });
    }
    columns
}

/// The operators `ty` takes, labelled.
#[must_use]
pub fn filter_ops(loc: &Localizer, ty: ColumnType) -> Vec<(FilterOp, String)> {
    let mut ops = Vec::new();
    for op in FilterOp::for_type(ty) {
        ops.push((*op, loc.filter_op_label(*op)));
    }
    ops
}

/// The values a choice column of `kind` takes, labelled; empty for any other column. A variant that
/// carries a value (`Custom`, `Other`) is not filterable, so it is not offered.
#[must_use]
pub fn choice_options(loc: &Localizer, kind: ListedKind, column: &str) -> Vec<ChoiceOption> {
    match (kind, column) {
        (ListedKind::Person, "sex") => options(&[Sex::Male, Sex::Female, Sex::Unknown, Sex::Intersex], |sex| {
            loc.sex_label(Some(sex))
        }),
        (ListedKind::Event, "type") => options(&EVENT_TYPES, |ty| loc.event_type_label(ty)),
        (ListedKind::Citation, "confidence") => {
            options(&ConfidenceLevel::all().map(vitni_app::Confidence::from), |confidence| {
                loc.confidence_label((*confidence).into())
            })
        }
        (ListedKind::Repository, "type") => options(&REPOSITORY_TYPES, |ty| loc.repository_type_label(ty)),
        (ListedKind::Note, "type") => options(&NOTE_TYPES, |ty| loc.note_type_label(ty)),
        (ListedKind::Place, "kind") => options(&PlaceKind::ALL, |kind| loc.place_kind_label(*kind)),
        (ListedKind::Place, "level") => options(&PlaceLevel::ALL, |level| loc.place_level_label(*level)),
        (ListedKind::DnaTest, "provider") => options(&DNA_PROVIDERS, |provider| loc.dna_provider_label(provider)),
        (ListedKind::DnaTest, "test-type") => options(&DNA_TEST_TYPES, |ty| loc.dna_test_type_label(*ty)),
        (ListedKind::DnaMatch, "status") => options(&[MatchStatus::Confirmed, MatchStatus::Rejected], |status| {
            loc.match_status_label(Some(*status))
        }),
        _ => Vec::new(),
    }
}

/// The chip text of an applied clause over `kind`: `Birth before 1800`, `Sex is any of Female, Male`.
#[must_use]
pub fn filter_chip_label(loc: &Localizer, kind: ListedKind, active: &ActiveFilter) -> String {
    let column = loc.list_column_label(&active.filter.column);
    let op = loc.filter_op_label(active.filter.op);
    let draft = &active.draft;
    let value = match draft.value_type(kind) {
        None => return loc.filter_chip_bare(&column, &op),
        Some(ColumnType::Text | ColumnType::Date | ColumnType::Number) if draft.op == FilterOp::Between => {
            loc.filter_range(draft.first.trim(), draft.second.trim())
        }
        Some(ColumnType::Text | ColumnType::Date | ColumnType::Number) => draft.first.trim().to_owned(),
        Some(ColumnType::Choice) => {
            let labels = choice_options(loc, kind, &draft.column);
            let mut names = Vec::with_capacity(draft.values.len());
            for value in &draft.values {
                let label = labels.iter().find(|option| option.key == *value);
                names.push(label.map_or_else(|| value.clone(), |option| option.label.clone()));
            }
            names.join(", ")
        }
        Some(ColumnType::Tags | ColumnType::Record) => {
            let mut names = Vec::with_capacity(draft.values.len());
            for value in &draft.values {
                names.push(draft.labels.get(value).cloned().unwrap_or_else(|| value.clone()));
            }
            names.join(", ")
        }
    };
    loc.filter_chip(&column, &op, &value)
}

/// One saved search in the Explorer's *Saved* menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedEntryVm {
    /// The saved search.
    pub id: SavedSearchId,
    /// Its name.
    pub name: String,
    /// Whether it is the one applied to the list now.
    pub applied: bool,
}

/// The Explorer's *Saved* menu for one kind (ADR 0052 §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedMenuVm {
    /// This kind's readable saved searches, in stored order.
    pub entries: Vec<SavedEntryVm>,
    /// The entries this version cannot read, listed disabled by name.
    pub unreadable: Vec<UnreadableSavedSearch>,
    /// Whether *Update*, *Rename* and *Delete* are offered: the applied saved search is still listed.
    pub acts_on_applied: bool,
}

/// The *Saved* menu of `kind`'s list, with `applied` marked. *Update*, *Rename* and *Delete* act on the
/// applied search, so they are offered only while it is still in the list.
#[must_use]
pub fn saved_menu(list: &SavedSearchList, kind: ListedKind, applied: Option<SavedSearchId>) -> SavedMenuVm {
    let mut entries = Vec::new();
    for search in &list.searches {
        if search.kind == kind {
            entries.push(SavedEntryVm {
                id: search.id,
                name: search.name.clone(),
                applied: applied == Some(search.id),
            });
        }
    }
    let acts_on_applied = entries.iter().any(|entry| entry.applied);
    SavedMenuVm {
        entries,
        unreadable: list.unreadable.clone(),
        acts_on_applied,
    }
}

/// What the inline name field is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamingMode {
    /// *Save as new*.
    New,
    /// *Rename* of the applied search.
    Rename,
}

/// Why a name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProblem {
    /// Nothing but whitespace.
    Empty,
    /// Another saved search has it, compared ignoring case.
    Duplicate,
}

/// The inline name field's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedNaming {
    /// Whether it names a new search or renames the applied one.
    pub mode: NamingMode,
    /// What has been typed.
    pub name: String,
    /// Why the name was refused, once it was tried.
    pub problem: Option<NameProblem>,
}

/// Why `name` cannot name a saved search: it is empty, or another one has it (ignoring case and
/// surrounding space); an entry this version cannot read still holds its name. `own` is the search being
/// renamed, which may keep its own name.
#[must_use]
pub fn name_problem(list: &SavedSearchList, name: &str, own: Option<SavedSearchId>) -> Option<NameProblem> {
    if name.trim().is_empty() {
        return Some(NameProblem::Empty);
    }
    for search in &list.searches {
        if Some(search.id) != own && same_saved_name(&search.name, name) {
            return Some(NameProblem::Duplicate);
        }
    }
    for unreadable in &list.unreadable {
        if unreadable
            .name
            .as_deref()
            .is_some_and(|taken| same_saved_name(taken, name))
        {
            return Some(NameProblem::Duplicate);
        }
    }
    None
}

/// `values` as options, keyed as the list table stores them ([`choice_key`]).
fn options<T: serde::Serialize>(values: &[T], label: impl Fn(&T) -> String) -> Vec<ChoiceOption> {
    let mut options = Vec::with_capacity(values.len());
    for value in values {
        let Some(key) = choice_key(value) else {
            continue;
        };
        options.push(ChoiceOption {
            key,
            label: label(value),
        });
    }
    options
}

const EVENT_TYPES: [EventType; 30] = [
    EventType::Birth,
    EventType::Death,
    EventType::Marriage,
    EventType::Baptism,
    EventType::Christening,
    EventType::Burial,
    EventType::Cremation,
    EventType::Census,
    EventType::Residence,
    EventType::Immigration,
    EventType::Emigration,
    EventType::Adoption,
    EventType::Confirmation,
    EventType::BarMitzvah,
    EventType::BasMitzvah,
    EventType::FirstCommunion,
    EventType::Graduation,
    EventType::Naturalization,
    EventType::Ordination,
    EventType::Probate,
    EventType::Retirement,
    EventType::Will,
    EventType::Engagement,
    EventType::Annulment,
    EventType::Divorce,
    EventType::DivorceFiled,
    EventType::MarriageBanns,
    EventType::MarriageContract,
    EventType::MarriageLicense,
    EventType::MarriageSettlement,
];

const REPOSITORY_TYPES: [RepositoryType; 7] = [
    RepositoryType::Library,
    RepositoryType::Archive,
    RepositoryType::Church,
    RepositoryType::Cemetery,
    RepositoryType::Museum,
    RepositoryType::Website,
    RepositoryType::Collection,
];

const NOTE_TYPES: [NoteType; 4] = [
    NoteType::General,
    NoteType::Research,
    NoteType::Transcript,
    NoteType::Citation,
];

const DNA_PROVIDERS: [DnaProvider; 6] = [
    DnaProvider::AncestryDna,
    DnaProvider::TwentyThreeAndMe,
    DnaProvider::MyHeritage,
    DnaProvider::FamilyTreeDna,
    DnaProvider::GedMatch,
    DnaProvider::LivingDna,
];

const DNA_TEST_TYPES: [DnaTestType; 4] = [
    DnaTestType::Autosomal,
    DnaTestType::YDna,
    DnaTestType::MtDna,
    DnaTestType::XDna,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn loc() -> Localizer {
        Localizer::for_test("en")
    }

    fn draft(kind: ListedKind, column: &str, op: FilterOp) -> FilterDraft {
        FilterDraft::new(kind).with_column(kind, column).with_op(op)
    }

    fn year(year: i32) -> GenealogicalDate {
        gregorian_date(DateParts {
            year,
            month: None,
            day: None,
        })
    }

    #[test]
    fn a_new_clause_starts_on_the_title_column_with_its_first_operator() {
        let draft = FilterDraft::new(ListedKind::Person);

        assert_eq!((draft.column.as_str(), draft.op), ("name", FilterOp::Contains));
    }

    #[test]
    fn changing_the_column_takes_its_types_first_operator_and_clears_the_value() {
        let mut draft = FilterDraft::new(ListedKind::Person);
        draft.first = "Olsen".to_owned();

        let draft = draft.with_column(ListedKind::Person, "birth");

        assert_eq!((draft.op, draft.first.as_str()), (FilterOp::Before, ""));
        assert_eq!(
            FilterDraft::new(ListedKind::Person)
                .with_column(ListedKind::Person, "nope")
                .column,
            "name",
            "a column the kind lacks changes nothing"
        );
    }

    #[test]
    fn a_text_clause_reads_its_trimmed_text() {
        let mut draft = draft(ListedKind::Person, "surname", FilterOp::Is);
        draft.first = "  Olsen ".to_owned();

        let active = draft.apply(ListedKind::Person).expect("applies");

        assert_eq!(active.filter.value, FilterValue::Text("Olsen".to_owned()));
        assert_eq!(active.draft, draft);
    }

    #[test]
    fn a_blank_value_is_missing() {
        let draft = draft(ListedKind::Person, "surname", FilterOp::Contains);

        assert_eq!(draft.apply(ListedKind::Person), Err(FilterDraftError::MissingValue));
    }

    #[test]
    fn is_empty_needs_no_value() {
        let draft = draft(ListedKind::Person, "birth", FilterOp::IsEmpty);

        assert_eq!(draft.value_type(ListedKind::Person), None);
        assert_eq!(
            draft.apply(ListedKind::Person).map(|active| active.filter.value),
            Ok(FilterValue::None)
        );
    }

    #[test]
    fn a_date_clause_reads_a_year_and_a_range_reads_both_ends() {
        let mut before = draft(ListedKind::Person, "birth", FilterOp::Before);
        before.first = "1800".to_owned();
        let mut between = draft(ListedKind::Person, "birth", FilterOp::Between);
        between.first = "1800".to_owned();
        between.second = "1850".to_owned();

        assert_eq!(
            before.apply(ListedKind::Person).map(|active| active.filter.value),
            Ok(FilterValue::Date(year(1800)))
        );
        assert_eq!(
            between.apply(ListedKind::Person).map(|active| active.filter.value),
            Ok(FilterValue::DateRange {
                from: year(1800),
                to: year(1850),
            })
        );
    }

    fn filter(column: &str, op: FilterOp, value: FilterValue) -> ListFilter {
        ListFilter {
            column: column.to_owned(),
            op,
            value,
        }
    }

    #[test]
    fn a_saved_text_clause_reads_back_as_its_text() {
        let saved = filter("surname", FilterOp::Contains, FilterValue::Text("anne".to_owned()));

        let draft = FilterDraft::from_filter(ListedKind::Person, &saved);

        assert_eq!(
            (draft.column.as_str(), draft.op, draft.first.as_str()),
            ("surname", FilterOp::Contains, "anne")
        );
    }

    #[test]
    fn a_saved_record_clause_leaves_its_text_for_the_caller_to_label() {
        let saved = filter("place", FilterOp::Is, FilterValue::Text("some-id".to_owned()));

        let draft = FilterDraft::from_filter(ListedKind::Event, &saved);

        assert_eq!(draft.first, "");
    }

    #[test]
    fn a_saved_date_clause_reads_back_as_its_year() {
        let saved = filter("birth", FilterOp::Before, FilterValue::Date(year(1800)));

        let draft = FilterDraft::from_filter(ListedKind::Person, &saved);

        assert_eq!((draft.op, draft.first.as_str()), (FilterOp::Before, "1800"));
        assert_eq!(draft.apply(ListedKind::Person).map(|active| active.filter), Ok(saved));
    }

    #[test]
    fn a_saved_number_clause_reads_back_as_its_number() {
        let saved = filter("children", FilterOp::GreaterThan, FilterValue::Number(2.0));

        let draft = FilterDraft::from_filter(ListedKind::Family, &saved);

        assert_eq!((draft.op, draft.first.as_str()), (FilterOp::GreaterThan, "2"));
        assert_eq!(draft.apply(ListedKind::Family).map(|active| active.filter), Ok(saved));
    }

    #[test]
    fn a_date_that_does_not_read_is_bad() {
        let mut bad = draft(ListedKind::Person, "birth", FilterOp::After);
        bad.first = "soon".to_owned();
        let mut half = draft(ListedKind::Person, "birth", FilterOp::Between);
        half.first = "1800".to_owned();

        assert_eq!(bad.apply(ListedKind::Person), Err(FilterDraftError::BadDate));
        assert_eq!(half.apply(ListedKind::Person), Err(FilterDraftError::MissingValue));
    }

    #[test]
    fn a_number_clause_reads_a_decimal_comma() {
        let mut more = draft(ListedKind::DnaMatch, "shared-cm", FilterOp::GreaterThan);
        more.first = "42,5".to_owned();
        let mut bad = draft(ListedKind::DnaMatch, "shared-cm", FilterOp::LessThan);
        bad.first = "lots".to_owned();

        assert_eq!(
            more.apply(ListedKind::DnaMatch).map(|active| active.filter.value),
            Ok(FilterValue::Number(42.5))
        );
        assert_eq!(bad.apply(ListedKind::DnaMatch), Err(FilterDraftError::BadNumber));
    }

    #[test]
    fn a_choice_or_tags_clause_needs_something_chosen() {
        let mut sex = draft(ListedKind::Person, "sex", FilterOp::IsAnyOf);

        assert_eq!(sex.apply(ListedKind::Person), Err(FilterDraftError::MissingValue));
        sex.values = vec!["female".to_owned()];
        assert_eq!(
            sex.apply(ListedKind::Person).map(|active| active.filter.value),
            Ok(FilterValue::Values(vec!["female".to_owned()]))
        );
    }

    #[test]
    fn a_record_clause_holds_the_picked_id() {
        let mut place = draft(ListedKind::Event, "place", FilterOp::Is);
        place.values = vec!["0190-place".to_owned()];

        assert_eq!(
            place.apply(ListedKind::Event).map(|active| active.filter.value),
            Ok(FilterValue::Text("0190-place".to_owned()))
        );
    }

    #[test]
    fn leaving_between_drops_the_ranges_end() {
        let mut range = draft(ListedKind::Person, "birth", FilterOp::Between);
        range.second = "1850".to_owned();

        assert_eq!(range.with_op(FilterOp::After).second, "");
    }

    #[test]
    fn every_catalogue_column_has_a_label() {
        let loc = loc();
        for kind in ListedKind::ALL {
            for column in filter_columns(&loc, *kind) {
                assert_ne!(column.label, column.id, "{} {} has no label", kind.as_str(), column.id);
            }
        }
    }

    #[test]
    fn every_type_offers_its_operators_labelled() {
        let ops = filter_ops(&loc(), ColumnType::Date);

        assert_eq!(
            ops,
            [
                (FilterOp::Before, "before".to_owned()),
                (FilterOp::After, "after".to_owned()),
                (FilterOp::Between, "between".to_owned()),
                (FilterOp::IsEmpty, "is empty".to_owned()),
            ]
        );
    }

    #[test]
    fn a_choice_column_offers_its_unit_variants_by_key() {
        let loc = loc();
        let sex = choice_options(&loc, ListedKind::Person, "sex");
        let providers = choice_options(&loc, ListedKind::DnaTest, "provider");
        let confidence = choice_options(&loc, ListedKind::Citation, "confidence");

        assert_eq!(
            sex.first().map(|option| (option.key.as_str(), option.label.as_str())),
            Some(("male", "male"))
        );
        assert!(providers.iter().any(|option| option.key == "twenty-three-and-me"));
        assert_eq!(confidence.first().map(|option| option.key.as_str()), Some("very-low"));
        assert_eq!(choice_options(&loc, ListedKind::Person, "name"), Vec::new());
    }

    #[test]
    fn the_place_kind_and_level_columns_offer_every_variant_localized() {
        for lang in ["en", "no"] {
            let loc = Localizer::for_test(lang);
            let kinds = choice_options(&loc, ListedKind::Place, "kind");
            let levels = choice_options(&loc, ListedKind::Place, "level");

            let kind_labels: Vec<String> = vitni_app::PlaceKind::ALL
                .map(|kind| loc.place_kind_label(kind))
                .to_vec();
            let level_labels: Vec<String> = vitni_app::PlaceLevel::ALL
                .map(|level| loc.place_level_label(level))
                .to_vec();
            assert_eq!(
                kinds.iter().map(|o| o.label.clone()).collect::<Vec<_>>(),
                kind_labels,
                "{lang}"
            );
            assert_eq!(
                levels.iter().map(|o| o.label.clone()).collect::<Vec<_>>(),
                level_labels,
                "{lang}"
            );
            assert_eq!(kinds.len(), vitni_app::PlaceKind::ALL.len());
            assert_eq!(levels.len(), vitni_app::PlaceLevel::ALL.len());
            assert!(kinds.iter().any(|option| option.key == "ecclesiastical"));
            assert!(levels.iter().any(|option| option.key == "adm1"));
            assert!(levels.iter().any(|option| option.key == "sub-settlement"));
        }
        assert_eq!(choice_options(&loc(), ListedKind::Person, "kind"), Vec::new());
    }

    #[test]
    fn the_place_type_column_stays_text_so_it_offers_no_choices() {
        assert_eq!(choice_options(&loc(), ListedKind::Place, "type"), Vec::new());
    }

    #[test]
    fn a_chip_names_the_column_operator_and_value() {
        let loc = loc();
        let mut birth = draft(ListedKind::Person, "birth", FilterOp::Before);
        birth.first = "1800".to_owned();
        let mut sex = draft(ListedKind::Person, "sex", FilterOp::IsAnyOf);
        sex.values = vec!["female".to_owned(), "male".to_owned()];
        let mut tags = draft(ListedKind::Person, "tags", FilterOp::HasNoneOf);
        tags.values = vec!["tag-id".to_owned()];
        tags.labels.insert("tag-id".to_owned(), "To verify".to_owned());
        let mut range = draft(ListedKind::Family, "children", FilterOp::Between);
        range.first = "2".to_owned();
        range.second = "5".to_owned();
        let empty = draft(ListedKind::Person, "death", FilterOp::IsEmpty);
        let chip = |kind, draft: &FilterDraft| {
            let active = draft.apply(kind).expect("applies");
            filter_chip_label(&loc, kind, &active)
        };

        assert_eq!(chip(ListedKind::Person, &birth), "Birth before 1800");
        assert_eq!(chip(ListedKind::Person, &sex), "Sex is any of female, male");
        assert_eq!(chip(ListedKind::Person, &tags), "Tags has none of To verify");
        assert_eq!(chip(ListedKind::Family, &range), "Children between 2 – 5");
        assert_eq!(chip(ListedKind::Person, &empty), "Death is empty");
    }

    fn summary(n: u128, name: &str, kind: ListedKind) -> vitni_app::SavedSearchSummary {
        vitni_app::SavedSearchSummary {
            id: vitni_app::SavedSearchId::from_uuid(uuid::Uuid::from_u128(n)),
            name: name.to_owned(),
            kind,
        }
    }

    fn list_of(searches: Vec<vitni_app::SavedSearchSummary>) -> vitni_app::SavedSearchList {
        vitni_app::SavedSearchList {
            searches,
            unreadable: vec![vitni_app::UnreadableSavedSearch {
                position: 2,
                name: Some("Broken".to_owned()),
            }],
        }
    }

    #[test]
    fn the_saved_menu_disables_update_rename_and_delete_with_nothing_applied() {
        let list = list_of(vec![summary(1, "Born before 1800", ListedKind::Person)]);
        let menu = saved_menu(&list, ListedKind::Person, None);
        assert!(!menu.acts_on_applied, "{menu:?}");
        assert_eq!(menu.entries.len(), 1);
        assert_eq!(menu.entries[0].name, "Born before 1800");
        assert!(!menu.entries[0].applied);
    }

    #[test]
    fn the_saved_menu_enables_them_for_an_applied_search_and_marks_it() {
        let list = list_of(vec![
            summary(1, "Born before 1800", ListedKind::Person),
            summary(2, "Untagged places", ListedKind::Place),
        ]);
        let applied = vitni_app::SavedSearchId::from_uuid(uuid::Uuid::from_u128(1));
        let menu = saved_menu(&list, ListedKind::Person, Some(applied));
        assert!(menu.acts_on_applied, "{menu:?}");
        assert_eq!(menu.entries.len(), 1, "only this kind's searches");
        assert!(menu.entries[0].applied);
    }

    #[test]
    fn the_saved_menu_offers_no_actions_on_an_applied_search_that_is_gone() {
        let list = list_of(vec![summary(1, "Born before 1800", ListedKind::Person)]);
        let gone = vitni_app::SavedSearchId::from_uuid(uuid::Uuid::from_u128(9));
        let menu = saved_menu(&list, ListedKind::Person, Some(gone));
        assert!(!menu.acts_on_applied, "{menu:?}");
    }

    #[test]
    fn the_saved_menu_lists_an_unreadable_entry_by_name() {
        let menu = saved_menu(&list_of(Vec::new()), ListedKind::Person, None);
        assert_eq!(menu.entries.len(), 0);
        assert_eq!(menu.unreadable.len(), 1);
        assert_eq!(menu.unreadable[0].position, 2);
    }
}
