//! The nested-draft state machine for a record link: a field that references another record can be
//! left [`Empty`](RecordLink::Empty), point at an [`Existing`](RecordLink::Existing) record (picked
//! via the record picker), or hold the fields of a [`New`](RecordLink::New) record created inline
//! (`docs/mockups/record-editing.html` §6b). Recursion is by composition: a new citation's
//! `NewCitationFields` carries its own `RecordLink<NewSourceFields>`, so a "New citation → New source"
//! cascade is one nested value the parent draft owns whole — dirtiness, validity, and Save flow
//! through the existing record-edit machinery unchanged.

use genealogy_app::PlaceType;

use crate::picker::PickerSelection;

/// A field that links to another record: unset, an existing record (its picker selection), or a new
/// record's inline fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RecordLink<N> {
    /// No record linked.
    #[default]
    Empty,
    /// An existing record, by the picker selection (its `human_id` + display title).
    Existing(PickerSelection),
    /// A new record being created inline, holding its fields.
    New(N),
}

impl<N> RecordLink<N> {
    /// The linked existing record's `human_id`, or `None` when empty or holding a new record.
    #[must_use]
    pub fn existing_id(&self) -> Option<&str> {
        match self {
            Self::Existing(selection) => Some(selection.human_id.as_str()),
            Self::Empty | Self::New(_) => None,
        }
    }

    /// Whether the link points at a record (existing or new) rather than being empty.
    #[must_use]
    pub fn is_set(&self) -> bool {
        match self {
            Self::Empty => false,
            Self::Existing(_) | Self::New(_) => true,
        }
    }
}

/// The inline fields of a new person (a family partner created from the picker's "+ New person").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewPersonFields {
    /// The given name (blank ⇒ no given name).
    pub given: String,
    /// The surname (blank ⇒ no surname).
    pub surname: String,
}

/// The inline fields of a new place (an event place created from the picker's "+ New place").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlaceFields {
    /// The place's type (required; defaults to [`PlaceType::City`]).
    pub place_type: PlaceType,
    /// The place's name (blank ⇒ no name).
    pub name: String,
}

impl Default for NewPlaceFields {
    fn default() -> Self {
        Self {
            place_type: PlaceType::City,
            name: String::new(),
        }
    }
}

/// The inline fields of a new source (a citation source created from the picker's "+ New source").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewSourceFields {
    /// The source's title (blank ⇒ no title).
    pub title: String,
}

/// The inline fields of a new citation (a name citation created from the picker's "+ New citation").
/// A citation cites exactly one source (data-model §7), so its source is itself a [`RecordLink`] — an
/// existing source or a nested new one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewCitationFields {
    /// The source this citation cites (existing or a nested new source).
    pub source: RecordLink<NewSourceFields>,
    /// The page / locator within the source (blank ⇒ no page).
    pub page: String,
}

#[cfg(test)]
mod tests {
    use super::{NewCitationFields, NewPersonFields, NewPlaceFields, NewSourceFields, RecordLink};
    use crate::picker::PickerSelection;
    use genealogy_app::PlaceType;

    #[test]
    fn a_default_link_is_empty_and_unset() {
        let link: RecordLink<NewPersonFields> = RecordLink::default();
        assert_eq!(link, RecordLink::Empty);
        assert!(!link.is_set());
        assert!(link.existing_id().is_none());
    }

    #[test]
    fn an_existing_link_exposes_its_id_and_is_set() {
        let link: RecordLink<NewPersonFields> = RecordLink::Existing(PickerSelection {
            human_id: "I0042".to_owned(),
            title: "Ada Lovelace".to_owned(),
        });
        assert!(link.is_set());
        assert_eq!(link.existing_id(), Some("I0042"));
    }

    #[test]
    fn a_new_link_is_set_but_has_no_existing_id() {
        let link = RecordLink::New(NewPersonFields {
            given: "Ada".to_owned(),
            surname: "Lovelace".to_owned(),
        });
        assert!(link.is_set());
        assert!(link.existing_id().is_none());
    }

    #[test]
    fn a_new_place_defaults_to_a_city_with_no_name() {
        let fields = NewPlaceFields::default();
        assert_eq!(fields.place_type, PlaceType::City);
        assert!(fields.name.is_empty());
    }

    #[test]
    fn a_new_citation_nests_a_source_link_that_starts_empty() {
        let citation = NewCitationFields::default();
        assert_eq!(citation.source, RecordLink::Empty);
        assert!(citation.page.is_empty());
        let with_new_source = NewCitationFields {
            source: RecordLink::New(NewSourceFields {
                title: "Baptism register".to_owned(),
            }),
            page: "p. 14".to_owned(),
        };
        assert!(with_new_source.source.is_set());
    }
}
