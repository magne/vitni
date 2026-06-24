//! Navigation: the rail's category/tool taxonomy, the active destination, and the data-loading
//! intents the renderer dispatches.
//!
//! Two concepts are deliberately separate (ADR 0008 keeps both framework-free):
//! - [`Destination`] — every place the rail or a `g`-prefix shortcut can navigate to. Exhaustive
//!   from day one so the chrome is complete; most destinations are placeholders this milestone.
//! - [`Screen`] — what the renderer actually mounts. One variant per *buildable* screen; it grows as
//!   each aggregate slice lands (PR4, PR6–12). A renderer maps a [`Destination`] to a [`Screen`].
//!
//! [`Intent`] is a request to load the app data a screen needs; the renderer turns it into a
//! use-case call via [`dispatch`](crate::intent::dispatch). Running a plugin is **not** an intent
//! here: the plugin host sits above this crate (ADR 0008), so a renderer orchestrates it directly
//! and hands the result to [`vocabulary::parse`](crate::vocabulary::parse).

use genealogy_app::{
    Address, AssociationRole, ChildParentRelationship, DateParts, EvidenceAnalysis, FactType, NoteType,
    ParticipantRole, PersonNameParts, Sex, SourceMediaType, Url,
};
use serde::{Deserialize, Serialize};

use crate::presentation::{ConfidenceLevel, RestrictionKind};

/// A primary entity category — the rail's "Entities" group: the 12 Gramps primaries plus a
/// workspace dashboard.
///
/// Order is the rail's display order. Eleven categories carry a `g`-prefix navigation key (see
/// [`Self::nav_key`]); the two DNA aggregates appear in the rail but have no nav key yet (the
/// shortcut spec leaves them off the `g`-map).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Workspace overview.
    Dashboard,
    /// Persons.
    People,
    /// Families.
    Families,
    /// Events.
    Events,
    /// Places.
    Places,
    /// Sources.
    Sources,
    /// Citations.
    Citations,
    /// Repositories.
    Repositories,
    /// Media objects.
    Media,
    /// Notes.
    Notes,
    /// Tags.
    Tags,
    /// DNA tests (aggregate; reachable by click, no `g`-key).
    DnaTests,
    /// DNA matches (aggregate; reachable by click, no `g`-key).
    DnaMatches,
}

impl Category {
    /// Every category in rail display order.
    #[must_use]
    pub const fn all() -> [Self; 13] {
        [
            Self::Dashboard,
            Self::People,
            Self::Families,
            Self::Events,
            Self::Places,
            Self::Sources,
            Self::Citations,
            Self::Repositories,
            Self::Media,
            Self::Notes,
            Self::Tags,
            Self::DnaTests,
            Self::DnaMatches,
        ]
    }

    /// The stable id token used for `data-nav` attributes, test keys, and telemetry.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::People => "people",
            Self::Families => "families",
            Self::Events => "events",
            Self::Places => "places",
            Self::Sources => "sources",
            Self::Citations => "citations",
            Self::Repositories => "repositories",
            Self::Media => "media",
            Self::Notes => "notes",
            Self::Tags => "tags",
            Self::DnaTests => "dna-tests",
            Self::DnaMatches => "dna-matches",
        }
    }

    /// The `g`-prefix navigation key for this category, if any (`g` then `<key>` navigates here).
    ///
    /// Returns `None` for the DNA aggregates, which are reachable via the rail but not the `g`-map.
    #[must_use]
    pub const fn nav_key(self) -> Option<char> {
        match self {
            Self::Dashboard => Some('d'),
            Self::People => Some('p'),
            Self::Families => Some('f'),
            Self::Events => Some('e'),
            Self::Places => Some('l'),
            Self::Sources => Some('s'),
            Self::Citations => Some('c'),
            Self::Repositories => Some('r'),
            Self::Media => Some('m'),
            Self::Notes => Some('n'),
            Self::Tags => Some('t'),
            Self::DnaTests | Self::DnaMatches => None,
        }
    }

    /// The category a `g`-prefix second key navigates to, if the key is bound.
    #[must_use]
    pub fn from_nav_key(key: char) -> Option<Self> {
        Self::all().into_iter().find(|category| category.nav_key() == Some(key))
    }

    /// The decorative emoji icon (rendered `aria-hidden`; the label is the accessible name).
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "⌂",
            Self::People => "👤",
            Self::Families => "👪",
            Self::Events => "📅",
            Self::Places => "📍",
            Self::Sources => "📚",
            Self::Citations => "❝",
            Self::Repositories => "🏛",
            Self::Media => "🖼",
            Self::Notes => "🗒",
            Self::Tags => "🏷",
            Self::DnaTests => "🧬",
            Self::DnaMatches => "🔗",
        }
    }

    /// The Fluent message id for this category's label (resolved by the renderer's chrome catalogue).
    #[must_use]
    pub const fn label_id(self) -> &'static str {
        match self {
            Self::Dashboard => "nav-dashboard",
            Self::People => "nav-people",
            Self::Families => "nav-families",
            Self::Events => "nav-events",
            Self::Places => "nav-places",
            Self::Sources => "nav-sources",
            Self::Citations => "nav-citations",
            Self::Repositories => "nav-repositories",
            Self::Media => "nav-media",
            Self::Notes => "nav-notes",
            Self::Tags => "nav-tags",
            Self::DnaTests => "nav-dna-tests",
            Self::DnaMatches => "nav-dna-matches",
        }
    }
}

/// A tool — the rail's "Tools" group: actions/functions, kept apart from the entity lists.
///
/// Plugins lives here per the locked design even though the rendered plugin form is a
/// [`Screen::PluginPanel`] — the rail entry and the render state are different concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    /// Ancestor/descendant chart (PR13).
    Pedigree,
    /// Split-view compare + non-destructive merge wizard (PR14).
    Merge,
    /// Plugin manager / plugin form host (ADR 0012).
    Plugins,
    /// Preferences / configuration (PR15).
    Preferences,
}

impl Tool {
    /// Every tool in rail display order.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Pedigree, Self::Merge, Self::Plugins, Self::Preferences]
    }

    /// The stable id token.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pedigree => "pedigree",
            Self::Merge => "merge",
            Self::Plugins => "plugins",
            Self::Preferences => "preferences",
        }
    }

    /// The decorative emoji icon (rendered `aria-hidden`).
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Pedigree => "🌳",
            Self::Merge => "⇄",
            Self::Plugins => "🧩",
            Self::Preferences => "⚙",
        }
    }

    /// The Fluent message id for this tool's label.
    #[must_use]
    pub const fn label_id(self) -> &'static str {
        match self {
            Self::Pedigree => "nav-pedigree",
            Self::Merge => "nav-merge",
            Self::Plugins => "nav-plugins",
            Self::Preferences => "nav-preferences",
        }
    }
}

/// Every place the rail or a `g`-prefix shortcut can navigate to.
///
/// Exhaustive across categories and tools; a renderer maps it to a [`Screen`] (a real screen this
/// milestone only for [`Category::People`] and [`Tool::Plugins`]; everything else is a placeholder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Destination {
    /// An entity category list.
    Category(Category),
    /// A tool.
    Tool(Tool),
}

impl Destination {
    /// The Fluent message id for this destination's label (the category's or tool's `label_id`).
    #[must_use]
    pub const fn label_id(self) -> &'static str {
        match self {
            Self::Category(category) => category.label_id(),
            Self::Tool(tool) => tool.label_id(),
        }
    }
}

/// An open record in the in-app tabstrip: a category-scoped entity reference with a display label.
///
/// Distinct from [`Destination`] (the rail's `Copy` category/tool navigation, which drives which
/// *screen* mounts): a `RecordRef` is what the tabstrip holds and the detail pane shows. It owns a
/// `human_id` + `label`, so it is **not** `Copy`. The `label` is the already-localized display name
/// (built by the renderer from a [`RowVm`](crate::list::RowVm)/[`PersonDetail`]), carried as data —
/// the tabstrip never resolves a chrome message id for a record tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRef {
    /// Which aggregate this record belongs to (only `People` this milestone; modelled to generalize).
    pub category: Category,
    /// The record's stable user-facing id (e.g. `I0001`) — the detail pane's resource key.
    pub human_id: String,
    /// The already-localized display label shown on the tab (the record's name; data, not chrome).
    pub label: String,
}

/// Which screen the GUI is showing — one variant per *buildable* screen (grows per slice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// The workspace dashboard: stat cards, recent activity, and quick entry points.
    Dashboard,
    /// The list of persons in the workspace.
    PersonList,
    /// One person's detail view.
    PersonDetail {
        /// The person's user-facing id (e.g. `I0001`).
        human_id: String,
    },
    /// One source's detail view.
    SourceDetail {
        /// The source's user-facing id (e.g. `S0001`).
        human_id: String,
    },
    /// One repository's detail view.
    RepositoryDetail {
        /// The repository's user-facing id (e.g. `R0001`).
        human_id: String,
    },
    /// One media object's detail view.
    MediaDetail {
        /// The media object's user-facing id (e.g. `O0001`).
        human_id: String,
    },
    /// One note's detail view.
    NoteDetail {
        /// The note's user-facing id (e.g. `N0001`).
        human_id: String,
    },
    /// A panel rendering a form a plugin supplied (ADR 0012).
    PluginPanel,
    /// A not-yet-built [`Destination`], shown as an "under construction" placeholder.
    ///
    /// PR2 renders 12 of 13 categories and three of four tools through this; later PRs replace each
    /// with a real variant. Carrying the [`Destination`] lets the placeholder name what it becomes.
    Placeholder {
        /// The destination this placeholder stands in for.
        destination: Destination,
    },
}

/// A request to load the app data a screen needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Load the workspace dashboard (counts + recent activity).
    ShowDashboard,
    /// Load the person list.
    ShowList,
    /// Load one person's detail.
    ShowPerson {
        /// The person's user-facing id (e.g. `I0001`).
        human_id: String,
    },
    /// Load the citation list.
    ShowCitationList,
    /// Load one citation's detail.
    ShowCitation {
        /// The citation's user-facing id (e.g. `C0001`).
        human_id: String,
    },
    /// Load the family list.
    ShowFamilyList,
    /// Load one family's detail.
    ShowFamily {
        /// The family's user-facing id (e.g. `F0001`).
        human_id: String,
    },
    /// Load the event list.
    ShowEventList,
    /// Load one event's detail.
    ShowEvent {
        /// The event's user-facing id (e.g. `E0001`).
        human_id: String,
    },
    /// Load the place list.
    ShowPlaceList,
    /// Load one place's detail.
    ShowPlace {
        /// The place's user-facing id (e.g. `P0001`).
        human_id: String,
    },
    /// Load the source list.
    ShowSourceList,
    /// Load one source's detail.
    ShowSource {
        /// The source's user-facing id (e.g. `S0001`).
        human_id: String,
    },
    /// Load the repository list.
    ShowRepositoryList,
    /// Load one repository's detail.
    ShowRepository {
        /// The repository's user-facing id (e.g. `R0001`).
        human_id: String,
    },
    /// Load the media list.
    ShowMediaList,
    /// Load one media object's detail.
    ShowMedia {
        /// The media object's user-facing id (e.g. `O0001`).
        human_id: String,
    },
    /// Load the note list.
    ShowNoteList,
    /// Load one note's detail.
    ShowNote {
        /// The note's user-facing id (e.g. `N0001`).
        human_id: String,
    },
}

/// A request to mutate a person, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_edit`](crate::intent::dispatch_edit). Distinct from [`Intent`] (a read): an edit emits
/// an event and the renderer reloads the detail afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonEdit {
    /// Assert an additional name.
    AssertName {
        /// The person to edit.
        human_id: String,
        /// The structured name parts.
        name: PersonNameParts,
    },
    /// Assert the person's sex.
    AssertSex {
        /// The person to edit.
        human_id: String,
        /// The sex to assert.
        sex: Sex,
    },
    /// Set the person's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The person to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Assert a fact, with its confidence and an optional backing citation.
    AssertFact {
        /// The person to edit.
        human_id: String,
        /// The fact's type.
        fact_type: FactType,
        /// The fact's free-text value, if any.
        value: Option<String>,
        /// The operator's surety.
        confidence: ConfidenceLevel,
        /// A backing citation's `human_id`, if supplied.
        citation: Option<String>,
    },
    /// Attach an existing citation (by `human_id`).
    AttachCitation {
        /// The person to edit.
        human_id: String,
        /// The citation's `human_id`.
        citation_id: String,
    },
    /// Attach an existing media object (by `human_id`).
    AttachMedia {
        /// The person to edit.
        human_id: String,
        /// The media object's `human_id`.
        media_id: String,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The person to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Assert a person-to-person association with a role.
    AssertAssociation {
        /// The asserting person.
        human_id: String,
        /// The other person's `human_id`.
        other_id: String,
        /// The association role.
        role: AssociationRole,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The person whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl PersonEdit {
    /// The `human_id` of the person this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::AssertName { human_id, .. }
            | Self::AssertSex { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::AssertFact { human_id, .. }
            | Self::AttachCitation { human_id, .. }
            | Self::AttachMedia { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::AssertAssociation { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a citation, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_citation_edit`](crate::intent::dispatch_citation_edit). Mirrors [`PersonEdit`] for the
/// Citation slice; covers the full citation command surface (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationEdit {
    /// Set the page / locator within the cited source.
    SetPage {
        /// The citation to edit.
        human_id: String,
        /// The page text.
        page: String,
    },
    /// Assert the date of the cited record.
    SetDate {
        /// The citation to edit.
        human_id: String,
        /// The structured date parts.
        parts: DateParts,
    },
    /// Set the operator's confidence in the citation.
    SetConfidence {
        /// The citation to edit.
        human_id: String,
        /// The surety level.
        confidence: ConfidenceLevel,
    },
    /// Set the Evidence Explained analysis (the three axes).
    SetEvidenceAnalysis {
        /// The citation to edit.
        human_id: String,
        /// The analysis to record.
        analysis: EvidenceAnalysis,
    },
    /// Add a typed attribute.
    AddAttribute {
        /// The citation to edit.
        human_id: String,
        /// The attribute's type.
        attribute_type: String,
        /// The attribute's value.
        value: String,
    },
    /// Attach an existing media object (by its `human_id`).
    AttachMedia {
        /// The citation to edit.
        human_id: String,
        /// The media object's `human_id`.
        media_id: String,
    },
    /// Attach an existing note (by its `human_id`).
    AttachNote {
        /// The citation to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag the user picked by name; it is
    /// carried for the command but never shown to the user (data-model §9).
    Tag {
        /// The citation to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the citation's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The citation to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The citation whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl CitationEdit {
    /// The `human_id` of the citation this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::SetPage { human_id, .. }
            | Self::SetDate { human_id, .. }
            | Self::SetConfidence { human_id, .. }
            | Self::SetEvidenceAnalysis { human_id, .. }
            | Self::AddAttribute { human_id, .. }
            | Self::AttachMedia { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a family, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_family_edit`](crate::intent::dispatch_family_edit). Mirrors [`CitationEdit`] for the
/// Family slice; covers the family command surface the screen exposes (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FamilyEdit {
    /// Add an existing person as a partner (neutral role), by `human_id`.
    AddPartner {
        /// The family to edit.
        human_id: String,
        /// The partner's person `human_id`.
        person_id: String,
    },
    /// Add an existing person as a child, with a relationship to each family partner (by `human_id`).
    AddChild {
        /// The family to edit.
        human_id: String,
        /// The child's person `human_id`.
        person_id: String,
        /// The child's relationship to each family partner (partner `human_id` → relationship).
        relationships: Vec<(String, ChildParentRelationship)>,
    },
    /// Link an existing event (e.g. a marriage) to the family, by `human_id`.
    LinkFamilyEvent {
        /// The family to edit.
        human_id: String,
        /// The event's `human_id`.
        event_id: String,
    },
    /// Attach an existing media object (by `human_id`).
    AttachMedia {
        /// The family to edit.
        human_id: String,
        /// The media object's `human_id`.
        media_id: String,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The family to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag the user picked by name; it is
    /// carried for the command but never shown to the user (data-model §9).
    Tag {
        /// The family to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the family's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The family to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The family whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl FamilyEdit {
    /// The `human_id` of the family this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::AddPartner { human_id, .. }
            | Self::AddChild { human_id, .. }
            | Self::LinkFamilyEvent { human_id, .. }
            | Self::AttachMedia { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate an event, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_event_edit`](crate::intent::dispatch_event_edit). Mirrors [`FamilyEdit`] for the Event
/// slice (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventEdit {
    /// Add an existing person as a participant, with a role.
    AddParticipant {
        /// The event to edit.
        human_id: String,
        /// The participant's person `human_id`.
        person_id: String,
        /// The participant's role.
        role: ParticipantRole,
    },
    /// Attach an existing citation (by `human_id`).
    AttachCitation {
        /// The event to edit.
        human_id: String,
        /// The citation's `human_id`.
        citation_id: String,
    },
    /// Attach an existing media object (by `human_id`).
    AttachMedia {
        /// The event to edit.
        human_id: String,
        /// The media object's `human_id`.
        media_id: String,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The event to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The event to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the event's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The event to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The event whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl EventEdit {
    /// The `human_id` of the event this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::AddParticipant { human_id, .. }
            | Self::AttachCitation { human_id, .. }
            | Self::AttachMedia { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a place, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_place_edit`](crate::intent::dispatch_place_edit). Mirrors [`FamilyEdit`] for the Place
/// slice (data-model §14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceEdit {
    /// Assert an additional name (text only; language/date are collected by a later slice).
    AddName {
        /// The place to edit.
        human_id: String,
        /// The name text.
        text: String,
    },
    /// Assert that the place is enclosed by another place, by its `human_id`.
    AddEnclosing {
        /// The place to edit.
        human_id: String,
        /// The enclosing place's `human_id`.
        enclosing_id: String,
    },
    /// Attach an existing citation (by `human_id`).
    AttachCitation {
        /// The place to edit.
        human_id: String,
        /// The citation's `human_id`.
        citation_id: String,
    },
    /// Attach an existing media object (by `human_id`).
    AttachMedia {
        /// The place to edit.
        human_id: String,
        /// The media object's `human_id`.
        media_id: String,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The place to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The place to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the place's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The place to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The place whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl PlaceEdit {
    /// The `human_id` of the place this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::AddName { human_id, .. }
            | Self::AddEnclosing { human_id, .. }
            | Self::AttachCitation { human_id, .. }
            | Self::AttachMedia { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a source, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_source_edit`](crate::intent::dispatch_source_edit). Mirrors [`EventEdit`] for the
/// Source slice (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceEdit {
    /// Link an existing repository (by `human_id`) that holds this source, with a call number/medium.
    LinkRepository {
        /// The source to edit.
        human_id: String,
        /// The repository's `human_id`.
        repository_id: String,
        /// The source's call number / shelf mark in that repository, if recorded.
        call_number: Option<String>,
        /// How the source is held there.
        media_type: SourceMediaType,
    },
    /// Add a typed attribute.
    AddAttribute {
        /// The source to edit.
        human_id: String,
        /// The attribute's type / key.
        attribute_type: String,
        /// The attribute's value.
        value: String,
    },
    /// Attach an existing media object (by `human_id`).
    AttachMedia {
        /// The source to edit.
        human_id: String,
        /// The media object's `human_id`.
        media_id: String,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The source to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The source to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the source's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The source to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The source whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl SourceEdit {
    /// The `human_id` of the source this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::LinkRepository { human_id, .. }
            | Self::AddAttribute { human_id, .. }
            | Self::AttachMedia { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a repository, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_repository_edit`](crate::intent::dispatch_repository_edit). Mirrors [`SourceEdit`] for
/// the Repository slice (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryEdit {
    /// Add a postal address.
    AddAddress {
        /// The repository to edit.
        human_id: String,
        /// The address to add.
        address: Address,
    },
    /// Add a contact URL.
    AddUrl {
        /// The repository to edit.
        human_id: String,
        /// The URL to add.
        url: Url,
    },
    /// Link an existing source (by `human_id`) as held here, with a call number/medium. This emits a
    /// `LinkRepository` command against the *source*, with this repository as the target.
    LinkSource {
        /// The repository to edit (and reload afterwards).
        human_id: String,
        /// The source's `human_id` to link.
        source_id: String,
        /// The source's call number / shelf mark here, if recorded.
        call_number: Option<String>,
        /// How the source is held here.
        media_type: SourceMediaType,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The repository to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The repository to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the repository's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The repository to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The repository whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl RepositoryEdit {
    /// The `human_id` of the repository this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::AddAddress { human_id, .. }
            | Self::AddUrl { human_id, .. }
            | Self::LinkSource { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a media object, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_media_edit`](crate::intent::dispatch_media_edit). Mirrors [`SourceEdit`] for the Media
/// slice (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaEdit {
    /// Attach an existing citation (by `human_id`) backing the media's claims.
    AttachCitation {
        /// The media object to edit.
        human_id: String,
        /// The citation's `human_id`.
        citation_id: String,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The media object to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The media object to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the media object's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The media object to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The media object whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl MediaEdit {
    /// The `human_id` of the media object this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::AttachCitation { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a note, dispatched to a `genealogy-app` command use-case via
/// [`dispatch_note_edit`](crate::intent::dispatch_note_edit). Mirrors [`SourceEdit`] for the Note
/// slice (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteEdit {
    /// Set (or change) the note's type.
    SetType {
        /// The note to edit.
        human_id: String,
        /// The note type to set.
        note_type: NoteType,
    },
    /// Set (or change) the note's primary Markdown text.
    SetText {
        /// The note to edit.
        human_id: String,
        /// The Markdown body.
        text: String,
    },
    /// Add (or replace) a translation of the note's content into another language.
    AddTranslation {
        /// The note to edit.
        human_id: String,
        /// The translation's language (a BCP-47 tag).
        language: String,
        /// The translated text.
        text: String,
        /// Who produced the translation, if recorded.
        translator: Option<String>,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The note to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the note's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The note to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The note whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl NoteEdit {
    /// The `human_id` of the note this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::SetType { human_id, .. }
            | Self::SetText { human_id, .. }
            | Self::AddTranslation { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Category, Tool};

    #[test]
    fn category_all_has_thirteen_unique_ids() {
        let ids: BTreeSet<&str> = Category::all().iter().map(|category| category.id()).collect();
        assert_eq!(Category::all().len(), 13);
        assert_eq!(ids.len(), 13);
    }

    #[test]
    fn eleven_categories_have_unique_nav_keys() {
        let keys: Vec<char> = Category::all()
            .iter()
            .filter_map(|category| category.nav_key())
            .collect();
        let unique: BTreeSet<char> = keys.iter().copied().collect();
        assert_eq!(keys.len(), 11);
        assert_eq!(unique.len(), 11);
    }

    #[test]
    fn nav_keys_match_spec() {
        let keys: BTreeSet<char> = Category::all()
            .iter()
            .filter_map(|category| category.nav_key())
            .collect();
        let expected: BTreeSet<char> = "dpfelscrmnt".chars().collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn dna_categories_have_no_nav_key() {
        assert_eq!(Category::DnaTests.nav_key(), None);
        assert_eq!(Category::DnaMatches.nav_key(), None);
    }

    #[test]
    fn tool_all_has_four_unique_ids() {
        let ids: BTreeSet<&str> = Tool::all().iter().map(|tool| tool.id()).collect();
        assert_eq!(Tool::all().len(), 4);
        assert_eq!(ids.len(), 4);
    }
}
