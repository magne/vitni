//! The nested-draft state machine for a record link: a field that references another record can be
//! left [`Empty`](RecordLink::Empty), point at an [`Existing`](RecordLink::Existing) record (picked
//! via the record picker), or hold the fields of a [`New`](RecordLink::New) record created inline
//! (`docs/mockups/record-editing.html` §6b). Recursion is by composition: a new citation's
//! `NewCitationFields` carries its own `RecordLink<NewSourceFields>`, so a "New citation → New source"
//! cascade is one nested value the parent draft owns whole — dirtiness, validity, and Save flow
//! through the existing record-edit machinery unchanged.

use vitni_app::{EventType, NameType, PersonNameParts, PlaceType};

use super::non_blank;
use crate::navigation::{
    Category, CitationChangeSetRequest, CitationSourceRequest, EventChangeSetRequest, EventPlaceRequest,
    MediaChangeSetRequest, NewRecordRequest, NoteChangeSetRequest, PersonChangeSetRequest, PlaceChangeSetRequest,
    RepositoryChangeSetRequest, SourceChangeSetRequest,
};
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

/// The inline fields of a new note (an attach picker's "+ New note").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewNoteFields {
    /// The note's Markdown content (blank ⇒ no content; a note with no text is invalid).
    pub text: String,
}

/// The inline fields of a new media object (an attach picker's "+ New media").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewMediaFields {
    /// The local file path (blank ⇒ no path; a media object with no path is invalid).
    pub file_path: String,
}

/// The inline fields of a new event (an attach picker's "+ New event", e.g. adding a participant to
/// an event that does not exist yet). `event_type` starts unset — deliberately no default, so the
/// draft stays invalid until the operator picks one rather than silently writing the wrong claim.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewEventFields {
    /// The event type (required; `None` until chosen).
    pub event_type: Option<EventType>,
    /// The free-text description (blank ⇒ no description).
    pub description: String,
}

/// The inline fields of a new repository (an attach picker's "+ New repository").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewRepositoryFields {
    /// The repository's name (blank ⇒ no name; a repository with no name is invalid).
    pub name: String,
}

/// The nested draft an attach picker's "+ New …" row opens (issue #314): the find-or-create attach
/// mechanism's uncommitted half. One enum rather than a generic over the create request, because a
/// third of the picker call sites resolve their category from a runtime `Category`/`field: String`
/// value, not a compile-time type (`person.rs`, `family.rs`, `source.rs`, `media.rs`, `citation.rs`,
/// `event.rs` pick it from a field name; `research_note.rs` takes a `Category` prop over any category).
///
/// [`Self::seed`] builds one from a category and the picker's typed query; [`Self::to_request`] is the
/// single validity rule ([`Self::is_valid`] just asks whether it returns `Some`); [`Self::summary`] is
/// the typed text, shown as the chip title the instant the create commits (before a refetch can supply
/// the record's real title).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewRecordDraft {
    /// A new person.
    Person(NewPersonFields),
    /// A new place.
    Place(NewPlaceFields),
    /// A new source.
    Source(NewSourceFields),
    /// A new citation.
    Citation(NewCitationFields),
    /// A new note.
    Note(NewNoteFields),
    /// A new media object.
    Media(NewMediaFields),
    /// A new event.
    Event(NewEventFields),
    /// A new repository.
    Repository(NewRepositoryFields),
}

impl NewRecordDraft {
    /// Seeds a new draft for `category` from the picker's typed `query`, or `None` when `category`
    /// does not support inline creation from an attach picker — exhaustive over every [`Category`], so
    /// a new aggregate cannot become pickable-but-uncreatable by omission.
    ///
    /// The typed query lands in the created record's own primary free-text field (stated once, here):
    /// a person's lone-token query becomes the surname (the genealogy sort key, and what a picker
    /// query usually is), a two-token query splits given/surname; every other supported category has
    /// exactly one free-text field the query seeds directly.
    #[must_use]
    #[expect(
        clippy::match_same_arms,
        reason = "six of the fourteen categories are excluded for six different reasons; kept as separate arms, \
                  each with its own doc comment, rather than merged into one, so the reason for each stays legible"
    )]
    pub fn seed(category: Category, query: &str) -> Option<Self> {
        match category {
            // Not a record — nothing to seed.
            Category::Dashboard => None,
            Category::People => {
                let (given, surname) = split_new_person_name(query);
                Some(Self::Person(NewPersonFields { given, surname }))
            }
            // A family needs partners; there is nothing for a bare query to seed.
            Category::Families => None,
            Category::Events => Some(Self::Event(NewEventFields {
                event_type: None,
                description: query.trim().to_owned(),
            })),
            Category::Places => Some(Self::Place(NewPlaceFields {
                place_type: PlaceType::City,
                name: query.trim().to_owned(),
            })),
            Category::Sources => Some(Self::Source(NewSourceFields {
                title: query.trim().to_owned(),
            })),
            Category::Citations => Some(Self::Citation(NewCitationFields {
                source: RecordLink::Empty,
                page: query.trim().to_owned(),
            })),
            Category::Repositories => Some(Self::Repository(NewRepositoryFields {
                name: query.trim().to_owned(),
            })),
            Category::Media => Some(Self::Media(NewMediaFields {
                file_path: query.trim().to_owned(),
            })),
            Category::Notes => Some(Self::Note(NewNoteFields {
                text: query.trim().to_owned(),
            })),
            // A research note needs at least one subject (ADR 0028 §2); a bare query names none.
            Category::ResearchNotes => None,
            // Not picker-backed at all — a tag field is a `Select` over existing tags.
            Category::Tags => None,
            // A DNA test needs an anchoring person; a bare query names none.
            Category::DnaTests => None,
            // A DNA match needs two tests plus a shared-cM measurement; a bare query supplies neither.
            Category::DnaMatches => None,
        }
    }

    /// Whether `category` supports inline creation from an attach picker's "+ New …" row.
    #[must_use]
    pub fn supports(category: Category) -> bool {
        Self::seed(category, "").is_some()
    }

    /// Builds the [`NewRecordRequest`] this draft describes, or `None` when it is not yet valid to
    /// save — the single rule [`Self::is_valid`] and every "+ New …" Save button defer to.
    #[must_use]
    pub fn to_request(&self) -> Option<NewRecordRequest> {
        match self {
            Self::Person(fields) => {
                let given = non_blank(&fields.given);
                let surname = non_blank(&fields.surname);
                if given.is_none() && surname.is_none() {
                    return None;
                }
                Some(NewRecordRequest::Person(PersonChangeSetRequest {
                    existing_human_id: None,
                    human_id_override: None,
                    name: Some(PersonNameParts {
                        name_type: NameType::BirthName,
                        given,
                        surname_prefix: None,
                        surname,
                        nickname: None,
                        prefix: None,
                        suffix: None,
                    }),
                    name_citation: None,
                    sex: None,
                    tags: Vec::new(),
                    new_sources: Vec::new(),
                    new_citations: Vec::new(),
                }))
            }
            Self::Place(fields) => {
                let name = non_blank(&fields.name)?;
                Some(NewRecordRequest::Place(PlaceChangeSetRequest {
                    human_id: None,
                    place_type: fields.place_type.clone(),
                    name: Some(name),
                    coordinates: None,
                    code: None,
                }))
            }
            Self::Source(fields) => {
                let title = non_blank(&fields.title)?;
                Some(NewRecordRequest::Source(SourceChangeSetRequest {
                    human_id: None,
                    title: Some(title),
                    author: None,
                    publication: None,
                    abbreviation: None,
                }))
            }
            Self::Citation(fields) => {
                let source = match &fields.source {
                    RecordLink::Existing(selection) => CitationSourceRequest::Existing(selection.human_id.clone()),
                    RecordLink::New(source_fields) => CitationSourceRequest::New {
                        title: non_blank(&source_fields.title),
                    },
                    // An unset source is the one thing a citation cannot save without.
                    RecordLink::Empty => return None,
                };
                Some(NewRecordRequest::Citation(CitationChangeSetRequest {
                    source,
                    page: non_blank(&fields.page),
                    confidence: None,
                    evidence: None,
                    date: None,
                }))
            }
            Self::Note(fields) => {
                let text = non_blank(&fields.text)?;
                Some(NewRecordRequest::Note(NoteChangeSetRequest {
                    human_id: None,
                    note_type: None,
                    text: Some(text),
                    language: None,
                }))
            }
            Self::Media(fields) => {
                let file_path = non_blank(&fields.file_path)?;
                Some(NewRecordRequest::Media(MediaChangeSetRequest {
                    human_id: None,
                    file_path: Some(file_path),
                    web_path: None,
                    mime: None,
                    date: None,
                }))
            }
            Self::Event(fields) => {
                let event_type = fields.event_type.clone()?;
                Some(NewRecordRequest::Event(EventChangeSetRequest {
                    event_type,
                    description: non_blank(&fields.description),
                    place: EventPlaceRequest::None,
                    date: None,
                }))
            }
            Self::Repository(fields) => {
                let name = non_blank(&fields.name)?;
                Some(NewRecordRequest::Repository(RepositoryChangeSetRequest {
                    human_id: None,
                    repository_type: None,
                    name: Some(name),
                }))
            }
        }
    }

    /// Whether this draft is valid to save — [`Self::to_request`] returning `Some`, so there is no
    /// second validity rule to drift out of step with the one Save actually uses.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.to_request().is_some()
    }

    /// The typed text this draft was seeded from, for the chip title shown the instant a create
    /// commits (before a refetch can supply the record's own title). `None` when every relevant field
    /// is still blank.
    #[must_use]
    pub fn summary(&self) -> Option<String> {
        match self {
            Self::Person(fields) => {
                let parts: Vec<&str> = [fields.given.trim(), fields.surname.trim()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect();
                (!parts.is_empty()).then(|| parts.join(" "))
            }
            Self::Place(fields) => non_blank(&fields.name),
            Self::Source(fields) => non_blank(&fields.title),
            Self::Citation(fields) => non_blank(&fields.page),
            Self::Note(fields) => non_blank(&fields.text),
            Self::Media(fields) => non_blank(&fields.file_path),
            Self::Event(fields) => non_blank(&fields.description),
            Self::Repository(fields) => non_blank(&fields.name),
        }
    }
}

/// Whether an attach picker's link is savable: an existing selection always is, a new draft only once
/// it validates ([`NewRecordDraft::is_valid`]), and an unset link never is. The single rule the
/// find-or-create attach panel's Save button disables on — kept as a free fn over
/// `RecordLink<NewRecordDraft>` specifically (not [`RecordLink::is_set`], which an unvalidated new
/// draft would also satisfy).
#[must_use]
pub fn link_is_savable(link: &RecordLink<NewRecordDraft>) -> bool {
    match link {
        RecordLink::Existing(_) => true,
        RecordLink::New(draft) => draft.is_valid(),
        RecordLink::Empty => false,
    }
}

/// Splits a "+ New person" picker query into given/surname: the last whitespace-separated token is
/// the surname (the genealogy sort key, and what a picker query usually is) and everything before it
/// is the given name; a lone token becomes the surname alone.
fn split_new_person_name(query: &str) -> (String, String) {
    let trimmed = query.trim();
    match trimmed.rsplit_once(char::is_whitespace) {
        Some((given, surname)) => (given.trim().to_owned(), surname.trim().to_owned()),
        None => (String::new(), trimmed.to_owned()),
    }
}

/// What pressing Save on a find-or-create attach link means (issue #314) — the framework-free half of
/// `screens::shared::use_attach_save`, so the decision is testable with no dioxus signal and no
/// workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachSaveAction {
    /// Dispatch the attach immediately against this existing record's `human_id`.
    Attach(String),
    /// Commit this validated draft first (create-then-attach), carrying the typed-text summary for the
    /// chip title the instant the create lands. Boxed: `NewRecordRequest` is far larger than `Attach`'s
    /// `String`, and boxing it keeps [`AttachSaveAction`] itself small to pass around.
    Create {
        /// The request to commit.
        request: Box<NewRecordRequest>,
        /// The draft's own summary ([`NewRecordDraft::summary`]).
        summary: Option<String>,
    },
    /// The link is not yet resolvable (still [`RecordLink::Empty`]) — Save should have stayed disabled.
    Blocked,
}

/// Resolves `link` into the [`AttachSaveAction`] pressing Save should take. Mirrors [`link_is_savable`]
/// exactly: `Blocked` here is `!link_is_savable(link)` there, so the two cannot drift apart.
#[must_use]
pub fn resolve_attach_save(link: &RecordLink<NewRecordDraft>) -> AttachSaveAction {
    match link {
        RecordLink::Existing(selection) => AttachSaveAction::Attach(selection.human_id.clone()),
        RecordLink::New(draft) => match draft.to_request() {
            Some(request) => AttachSaveAction::Create {
                request: Box::new(request),
                summary: draft.summary(),
            },
            None => AttachSaveAction::Blocked,
        },
        RecordLink::Empty => AttachSaveAction::Blocked,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttachSaveAction, NewCitationFields, NewEventFields, NewMediaFields, NewNoteFields, NewPersonFields,
        NewPlaceFields, NewRecordDraft, NewRepositoryFields, NewSourceFields, RecordLink, link_is_savable,
        resolve_attach_save,
    };
    use crate::navigation::{Category, NewRecordRequest};
    use crate::picker::PickerSelection;
    use vitni_app::{EventType, PlaceType};

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

    #[test]
    fn seed_covers_every_pickable_category() {
        // Eight categories support inline creation from an attach picker; the rest are excluded for a
        // structural reason (not a record, not enough seedable fields, or not picker-backed at all).
        for category in Category::all() {
            let supported = matches!(
                category,
                Category::People
                    | Category::Events
                    | Category::Places
                    | Category::Sources
                    | Category::Citations
                    | Category::Repositories
                    | Category::Media
                    | Category::Notes
            );
            assert_eq!(
                NewRecordDraft::supports(category),
                supported,
                "{category:?} disagrees with the supported/excluded table"
            );
        }
    }

    #[test]
    fn the_typed_query_lands_in_each_categorys_primary_field() {
        let cases = [
            Category::Places,
            Category::Sources,
            Category::Citations,
            Category::Repositories,
            Category::Media,
            Category::Notes,
            Category::Events,
        ];
        for category in cases {
            let draft = NewRecordDraft::seed(category, "Ellis Island")
                .unwrap_or_else(|| panic!("{category:?} is a supported category and must seed a draft"));
            let field = match &draft {
                NewRecordDraft::Place(fields) => &fields.name,
                NewRecordDraft::Source(fields) => &fields.title,
                NewRecordDraft::Citation(fields) => &fields.page,
                NewRecordDraft::Repository(fields) => &fields.name,
                NewRecordDraft::Media(fields) => &fields.file_path,
                NewRecordDraft::Note(fields) => &fields.text,
                NewRecordDraft::Event(fields) => &fields.description,
                NewRecordDraft::Person(_) => unreachable!("People is asserted separately (name splitting)"),
            };
            assert_eq!(field, "Ellis Island", "{category:?} did not seed its primary field");
        }
    }

    #[test]
    fn a_lone_token_person_query_becomes_the_surname() {
        let Some(NewRecordDraft::Person(fields)) = NewRecordDraft::seed(Category::People, "Lovelace") else {
            panic!("People always seeds a person draft");
        };
        assert!(fields.given.is_empty());
        assert_eq!(fields.surname, "Lovelace");
    }

    #[test]
    fn a_two_token_person_query_splits_given_and_surname() {
        let Some(NewRecordDraft::Person(fields)) = NewRecordDraft::seed(Category::People, "Ada Lovelace") else {
            panic!("People always seeds a person draft");
        };
        assert_eq!(fields.given, "Ada");
        assert_eq!(fields.surname, "Lovelace");
    }

    #[test]
    fn an_event_without_a_type_is_invalid() {
        let draft = NewRecordDraft::Event(NewEventFields {
            event_type: None,
            description: "Family reunion".to_owned(),
        });
        assert!(!draft.is_valid(), "no default type is chosen for a new event");
        assert!(draft.to_request().is_none());
        let with_type = NewRecordDraft::Event(NewEventFields {
            event_type: Some(EventType::Census),
            description: "Family reunion".to_owned(),
        });
        assert!(with_type.is_valid());
    }

    #[test]
    fn a_citation_with_an_empty_source_is_invalid() {
        let draft = NewRecordDraft::Citation(NewCitationFields {
            source: RecordLink::Empty,
            page: "p. 14".to_owned(),
        });
        assert!(!draft.is_valid(), "a citation cannot save without a source");
        assert!(draft.to_request().is_none());
    }

    #[test]
    fn each_valid_draft_maps_to_its_request_variant() {
        let drafts = [
            NewRecordDraft::Person(NewPersonFields {
                given: "Ada".to_owned(),
                surname: "Lovelace".to_owned(),
            }),
            NewRecordDraft::Place(NewPlaceFields {
                place_type: PlaceType::City,
                name: "Ellis Island".to_owned(),
            }),
            NewRecordDraft::Source(NewSourceFields {
                title: "Baptism register".to_owned(),
            }),
            NewRecordDraft::Citation(NewCitationFields {
                source: RecordLink::Existing(PickerSelection {
                    human_id: "S0001".to_owned(),
                    title: "Baptism register".to_owned(),
                }),
                page: "p. 14".to_owned(),
            }),
            NewRecordDraft::Note(NewNoteFields {
                text: "A research note".to_owned(),
            }),
            NewRecordDraft::Media(NewMediaFields {
                file_path: "/photos/ada.jpg".to_owned(),
            }),
            NewRecordDraft::Event(NewEventFields {
                event_type: Some(EventType::Census),
                description: "1911 census".to_owned(),
            }),
            NewRecordDraft::Repository(NewRepositoryFields {
                name: "National Archives".to_owned(),
            }),
        ];
        for draft in drafts {
            let request = draft
                .to_request()
                .unwrap_or_else(|| panic!("{draft:?} should be valid"));
            let matches = matches!(
                (&draft, &request),
                (NewRecordDraft::Person(_), NewRecordRequest::Person(_))
                    | (NewRecordDraft::Place(_), NewRecordRequest::Place(_))
                    | (NewRecordDraft::Source(_), NewRecordRequest::Source(_))
                    | (NewRecordDraft::Citation(_), NewRecordRequest::Citation(_))
                    | (NewRecordDraft::Note(_), NewRecordRequest::Note(_))
                    | (NewRecordDraft::Media(_), NewRecordRequest::Media(_))
                    | (NewRecordDraft::Event(_), NewRecordRequest::Event(_))
                    | (NewRecordDraft::Repository(_), NewRecordRequest::Repository(_))
            );
            assert!(matches, "{draft:?} mapped to the wrong request variant: {request:?}");
        }
    }

    #[test]
    fn link_is_savable_agrees_with_each_variant() {
        let empty: RecordLink<NewRecordDraft> = RecordLink::Empty;
        assert!(!link_is_savable(&empty), "an unset link is never savable");

        let existing = RecordLink::Existing(PickerSelection {
            human_id: "N0007".to_owned(),
            title: "Baptism note".to_owned(),
        });
        assert!(link_is_savable(&existing), "an existing selection is always savable");

        let invalid_new = RecordLink::New(NewRecordDraft::Note(NewNoteFields::default()));
        assert!(!link_is_savable(&invalid_new), "an invalid draft is not savable");

        let valid_new = RecordLink::New(NewRecordDraft::Note(NewNoteFields {
            text: "A research note".to_owned(),
        }));
        assert!(link_is_savable(&valid_new), "a valid draft is savable");
    }

    #[test]
    fn an_existing_selection_needs_no_create_and_dispatches_directly() {
        let link = RecordLink::Existing(PickerSelection {
            human_id: "N0007".to_owned(),
            title: "Baptism note".to_owned(),
        });
        assert_eq!(resolve_attach_save(&link), AttachSaveAction::Attach("N0007".to_owned()));
    }

    #[test]
    fn a_valid_draft_resolves_to_create_carrying_its_summary() {
        let link = RecordLink::New(NewRecordDraft::Note(NewNoteFields {
            text: "A research note".to_owned(),
        }));
        let AttachSaveAction::Create { summary, .. } = resolve_attach_save(&link) else {
            panic!("a valid draft resolves to Create");
        };
        assert_eq!(summary, Some("A research note".to_owned()));
    }

    #[test]
    fn an_empty_or_invalid_link_is_blocked() {
        let empty: RecordLink<NewRecordDraft> = RecordLink::Empty;
        assert_eq!(resolve_attach_save(&empty), AttachSaveAction::Blocked);

        let invalid_new = RecordLink::New(NewRecordDraft::Note(NewNoteFields::default()));
        assert_eq!(resolve_attach_save(&invalid_new), AttachSaveAction::Blocked);
    }
}
