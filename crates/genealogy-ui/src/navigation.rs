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
    Address, AssociationRole, ChildParentRelationship, DateParts, DnaGenomeBuild, DnaProvider, DnaTestType, EventType,
    EvidenceAnalysis, FactType, GeoCoordinates, NoteType, ParticipantRole, PersonNameParts, PlaceType, RepositoryType,
    Sex, SourceMediaType, Url,
};
use serde::{Deserialize, Serialize};

use crate::help::HelpTopicId;
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

    /// The aggregate categories a new record can be created for (all except the workspace
    /// Dashboard), in rail order.
    #[must_use]
    pub const fn creatable() -> [Self; 12] {
        [
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

    /// The stored aggregate-type string (the `Aggregate::TYPE` the event store keys on) for this
    /// category, or `None` for [`Self::Dashboard`] (not an aggregate). Inverse of
    /// [`Self::from_aggregate_kind`].
    #[must_use]
    pub const fn aggregate_kind(self) -> Option<&'static str> {
        match self {
            Self::Dashboard => None,
            Self::People => Some("person"),
            Self::Families => Some("family"),
            Self::Events => Some("event"),
            Self::Places => Some("place"),
            Self::Sources => Some("source"),
            Self::Citations => Some("citation"),
            Self::Repositories => Some("repository"),
            Self::Media => Some("media"),
            Self::Notes => Some("note"),
            Self::Tags => Some("tag"),
            Self::DnaTests => Some("dna_test"),
            Self::DnaMatches => Some("dna_match"),
        }
    }

    /// The entity category for a stored aggregate-type string (the `Aggregate::TYPE` the event store
    /// keys on), or `None` for [`Self::Dashboard`] and any non-aggregate kind.
    #[must_use]
    pub fn from_aggregate_kind(kind: &str) -> Option<Self> {
        match kind {
            "person" => Some(Self::People),
            "family" => Some(Self::Families),
            "event" => Some(Self::Events),
            "place" => Some(Self::Places),
            "source" => Some(Self::Sources),
            "citation" => Some(Self::Citations),
            "repository" => Some(Self::Repositories),
            "media" => Some(Self::Media),
            "note" => Some(Self::Notes),
            "tag" => Some(Self::Tags),
            "dna_test" => Some(Self::DnaTests),
            "dna_match" => Some(Self::DnaMatches),
            _ => None,
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

    /// The tool for a stable id token (the inverse of [`Self::id`]), or `None` for an unknown token.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "pedigree" => Some(Self::Pedigree),
            "merge" => Some(Self::Merge),
            "plugins" => Some(Self::Plugins),
            "preferences" => Some(Self::Preferences),
            _ => None,
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
    /// The in-app help browser. `None` shows the default/landing topic; `Some` shows a specific
    /// article (e.g. a widget's contextual help target).
    Help {
        /// The article to show, or `None` for the default topic.
        topic: Option<HelpTopicId>,
    },
}

impl Destination {
    /// The Fluent message id for this destination's label (the category's or tool's `label_id`, or
    /// the help browser's `nav-help`).
    #[must_use]
    pub const fn label_id(self) -> &'static str {
        match self {
            Self::Category(category) => category.label_id(),
            Self::Tool(tool) => tool.label_id(),
            Self::Help { .. } => "nav-help",
        }
    }
}

/// One visited navigation location: the mounted destination plus the focused record (if any),
/// identified by its `(category, human id)` so history survives tab reordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavLocation {
    /// The destination mounted at this point in history.
    pub destination: Destination,
    /// The record focused within that destination, if any, identified by category + human id.
    pub record: Option<(Category, String)>,
}

/// A linear back/forward navigation history with a cursor (browser semantics).
///
/// `push` drops any forward entries beyond the cursor before appending, matching how a browser
/// history behaves after navigating back and then to a new location.
#[derive(Debug, Clone, Default)]
pub struct NavHistory {
    entries: Vec<NavLocation>,
    cursor: Option<usize>,
}

impl NavHistory {
    /// Records a visit to `location`.
    ///
    /// A no-op when `location` equals the current entry (avoids duplicate back-stops for
    /// re-renders of the same place). Otherwise drops any forward entries (everything after the
    /// cursor) and appends `location`, moving the cursor to the new last entry.
    pub fn push(&mut self, location: NavLocation) {
        if self.current() == Some(&location) {
            return;
        }
        let next_index = self.cursor.map_or(0, |cursor| cursor + 1);
        self.entries.truncate(next_index);
        self.entries.push(location);
        self.cursor = Some(self.entries.len() - 1);
    }

    /// The currently focused location, if the history is non-empty.
    #[must_use]
    pub fn current(&self) -> Option<&NavLocation> {
        self.cursor.and_then(|cursor| self.entries.get(cursor))
    }

    /// Moves the cursor one step back and returns a clone of the now-current location, or `None`
    /// when [`Self::can_back`] is `false`.
    pub fn back(&mut self) -> Option<NavLocation> {
        if !self.can_back() {
            return None;
        }
        self.cursor = self.cursor.map(|cursor| cursor - 1);
        self.current().cloned()
    }

    /// Moves the cursor one step forward and returns a clone of the now-current location, or
    /// `None` when [`Self::can_forward`] is `false`.
    pub fn forward(&mut self) -> Option<NavLocation> {
        if !self.can_forward() {
            return None;
        }
        self.cursor = self.cursor.map(|cursor| cursor + 1);
        self.current().cloned()
    }

    /// Whether [`Self::back`] would move the cursor (there is an earlier entry).
    #[must_use]
    pub fn can_back(&self) -> bool {
        self.cursor.is_some_and(|cursor| cursor > 0)
    }

    /// Whether [`Self::forward`] would move the cursor (there is a later entry).
    #[must_use]
    pub fn can_forward(&self) -> bool {
        self.cursor.is_some_and(|cursor| cursor + 1 < self.entries.len())
    }
}

/// The tab-title rule for an open record: the record's name when present and non-blank after
/// trimming, otherwise its `human_id`.
#[must_use]
pub fn tab_label(name: Option<&str>, human_id: &str) -> String {
    match name {
        Some(name) if !name.trim().is_empty() => name.to_string(),
        _ => human_id.to_string(),
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
    /// One tag's detail view.
    TagDetail {
        /// The tag's stable id (a UUID string; tags have no `human_id`).
        id: String,
    },
    /// One DNA test's detail view.
    DnaTestDetail {
        /// The test's user-facing id (e.g. `D0001`).
        human_id: String,
    },
    /// One DNA match's detail view.
    DnaMatchDetail {
        /// The match's user-facing id (e.g. `X0001`).
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
    /// Load the tag list.
    ShowTagList,
    /// Load one tag's detail.
    ShowTag {
        /// The tag's stable id (a UUID string).
        id: String,
    },
    /// Load the DNA-test list.
    ShowDnaTestList,
    /// Load one DNA test's detail.
    ShowDnaTest {
        /// The test's user-facing id (e.g. `D0001`).
        human_id: String,
    },
    /// Load the DNA-match list.
    ShowDnaMatchList,
    /// Load one DNA match's detail.
    ShowDnaMatch {
        /// The match's user-facing id (e.g. `X0001`).
        human_id: String,
    },
    /// Load the Pedigree tool's ancestor and descendant charts for one focus person.
    ShowPedigree {
        /// The focus person's user-facing id.
        human_id: String,
        /// How many generations to show on each side.
        depth: u32,
    },
    /// Compute the kinship between two people (the Pedigree tool's Relationships view).
    ComputeRelationship {
        /// The first person's user-facing id.
        human_id_a: String,
        /// The second person's user-facing id.
        human_id_b: String,
    },
    /// Scan the workspace for possible-duplicate person pairs (the Merge tool's landing table).
    ListDuplicateCandidates,
    /// Load both people's summaries for the Merge tool's compare/merge wizard.
    MergeCompare {
        /// The surviving person's `human_id` (keeps their id after a merge).
        surviving_human_id: String,
        /// The person who would become a persona of the survivor.
        merged_human_id: String,
    },
}

/// A request to merge two persons, dispatched to `genealogy_app::merge_persons` via
/// [`dispatch_merge`](crate::intent::dispatch_merge). Distinct from [`Intent`] (a read): a merge
/// emits an event and the renderer shows the outcome/reloads the duplicates list afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePersons {
    /// The surviving person's `human_id`.
    pub surviving_human_id: String,
    /// The person to merge into the survivor (becomes a persona; their own record is untouched).
    pub merged_human_id: String,
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
    /// Set (or change) the event's type.
    SetType {
        /// The event to edit.
        human_id: String,
        /// The event type to set.
        event_type: EventType,
    },
    /// Set (or change) the event's date.
    SetDate {
        /// The event to edit.
        human_id: String,
        /// The date parts to assert.
        date: DateParts,
    },
    /// Set (or change) the event's free-text description.
    SetDescription {
        /// The event to edit.
        human_id: String,
        /// The description to set.
        description: String,
    },
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
            Self::SetType { human_id, .. }
            | Self::SetDate { human_id, .. }
            | Self::SetDescription { human_id, .. }
            | Self::AddParticipant { human_id, .. }
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
    /// Set (or change) the place's type.
    SetType {
        /// The place to edit.
        human_id: String,
        /// The place type to set.
        place_type: PlaceType,
    },
    /// Set (or change) the place's geographic coordinates.
    SetCoordinates {
        /// The place to edit.
        human_id: String,
        /// The coordinates to assert.
        coordinates: GeoCoordinates,
    },
    /// Set (or change) the place's jurisdiction code.
    SetCode {
        /// The place to edit.
        human_id: String,
        /// The code to set.
        code: String,
    },
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
            Self::SetType { human_id, .. }
            | Self::SetCoordinates { human_id, .. }
            | Self::SetCode { human_id, .. }
            | Self::AddName { human_id, .. }
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
    /// Set (or change) the source's author.
    SetAuthor {
        /// The source to edit.
        human_id: String,
        /// The author to set.
        author: String,
    },
    /// Set (or change) the source's publication info.
    SetPubInfo {
        /// The source to edit.
        human_id: String,
        /// The publication info to set.
        pub_info: String,
    },
    /// Set (or change) the source's abbreviation.
    SetAbbrev {
        /// The source to edit.
        human_id: String,
        /// The abbreviation to set.
        abbrev: String,
    },
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
            Self::SetAuthor { human_id, .. }
            | Self::SetPubInfo { human_id, .. }
            | Self::SetAbbrev { human_id, .. }
            | Self::LinkRepository { human_id, .. }
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
    /// Set (or change) the repository's name.
    SetName {
        /// The repository to edit.
        human_id: String,
        /// The name to set.
        name: String,
    },
    /// Set (or change) the repository's type.
    SetType {
        /// The repository to edit.
        human_id: String,
        /// The repository type to set.
        repository_type: RepositoryType,
    },
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
            Self::SetName { human_id, .. }
            | Self::SetType { human_id, .. }
            | Self::AddAddress { human_id, .. }
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
    /// Set (or change) the media object's file path.
    SetFilePath {
        /// The media object to edit.
        human_id: String,
        /// The file path to set.
        path: String,
    },
    /// Set (or change) the media object's web path.
    SetWebPath {
        /// The media object to edit.
        human_id: String,
        /// The web path / URL to set.
        href: String,
    },
    /// Set (or change) the media object's checksum.
    SetChecksum {
        /// The media object to edit.
        human_id: String,
        /// The checksum to set.
        checksum: String,
    },
    /// Set (or change) the media object's date.
    SetDate {
        /// The media object to edit.
        human_id: String,
        /// The date parts to assert.
        date: DateParts,
    },
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
            Self::SetFilePath { human_id, .. }
            | Self::SetWebPath { human_id, .. }
            | Self::SetChecksum { human_id, .. }
            | Self::SetDate { human_id, .. }
            | Self::AttachCitation { human_id, .. }
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

/// A request to mutate a tag, dispatched via [`dispatch_tag_edit`](crate::intent::dispatch_tag_edit).
/// A tag is identified by its stable id (no `human_id`); the renderer reloads it after the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagEdit {
    /// Rename the tag.
    SetName {
        /// The tag's stable id.
        id: String,
        /// The new name.
        name: String,
    },
    /// Set (or change) the tag's sort priority.
    SetPriority {
        /// The tag's stable id.
        id: String,
        /// The new priority.
        priority: i32,
    },
    /// Set (or change) the tag's colour (a CSS hex string).
    SetColor {
        /// The tag's stable id.
        id: String,
        /// The new colour.
        color: String,
    },
}

impl TagEdit {
    /// The stable id of the tag this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::SetName { id, .. } | Self::SetPriority { id, .. } | Self::SetColor { id, .. } => id,
        }
    }
}

/// A request to mutate a DNA test, dispatched via
/// [`dispatch_dna_test_edit`](crate::intent::dispatch_dna_test_edit). The renderer reloads the test
/// after the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnaTestEdit {
    /// Set (or change) the test's provider.
    SetProvider {
        /// The DNA test to edit.
        human_id: String,
        /// The provider to set.
        provider: DnaProvider,
    },
    /// Set (or change) the test's kit id.
    SetKitId {
        /// The DNA test to edit.
        human_id: String,
        /// The kit id to set.
        kit_id: String,
    },
    /// Set (or change) the test's type.
    SetType {
        /// The DNA test to edit.
        human_id: String,
        /// The test type to set.
        test_type: DnaTestType,
    },
    /// Set (or change) the test's genome build.
    SetGenomeBuild {
        /// The DNA test to edit.
        human_id: String,
        /// The genome build to set.
        genome_build: DnaGenomeBuild,
    },
    /// Assert an additional haplogroup.
    AddHaplogroup {
        /// The test to edit.
        human_id: String,
        /// The haplogroup (e.g. `R-M269`).
        haplogroup: String,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The test to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The test to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the test's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The test to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The test whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl DnaTestEdit {
    /// The `human_id` of the DNA test this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::SetProvider { human_id, .. }
            | Self::SetKitId { human_id, .. }
            | Self::SetType { human_id, .. }
            | Self::SetGenomeBuild { human_id, .. }
            | Self::AddHaplogroup { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

/// A request to mutate a DNA match, dispatched via
/// [`dispatch_dna_match_edit`](crate::intent::dispatch_dna_match_edit). The renderer reloads the match
/// after the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnaMatchEdit {
    /// Confirm or reject the match (the inferred-relationship conclusion's status).
    SetStatus {
        /// The match to edit.
        human_id: String,
        /// Whether to confirm (`true`) rather than reject (`false`).
        confirmed: bool,
    },
    /// Attach an existing note (by `human_id`).
    AttachNote {
        /// The match to edit.
        human_id: String,
        /// The note's `human_id`.
        note_id: String,
    },
    /// Apply or remove a tag. The `tag_id` is resolved from a tag picked by name; never shown.
    Tag {
        /// The match to edit.
        human_id: String,
        /// The tag's aggregate id (a UUID string) — never rendered.
        tag_id: String,
        /// Whether to remove (`true`) rather than apply (`false`) the tag.
        remove: bool,
    },
    /// Set the match's privacy restrictions (an empty set clears them).
    SetRestrictions {
        /// The match to edit.
        human_id: String,
        /// The restrictions to set.
        restrictions: Vec<RestrictionKind>,
    },
    /// Undo a prior assertion by retracting it (non-destructive — the event log is append-only).
    UndoAssertion {
        /// The match whose change log holds the assertion.
        human_id: String,
        /// The assertion to retract (its `AssertionId`, a UUID string).
        assertion_id: String,
    },
}

impl DnaMatchEdit {
    /// The `human_id` of the DNA match this edit targets (the detail to reload afterwards).
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::SetStatus { human_id, .. }
            | Self::AttachNote { human_id, .. }
            | Self::Tag { human_id, .. }
            | Self::SetRestrictions { human_id, .. }
            | Self::UndoAssertion { human_id, .. } => human_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Category, Destination, NavHistory, NavLocation, Tool, tab_label};

    fn location(category: Category) -> NavLocation {
        NavLocation {
            destination: Destination::Category(category),
            record: None,
        }
    }

    fn location_with_record(category: Category, human_id: &str) -> NavLocation {
        NavLocation {
            destination: Destination::Category(category),
            record: Some((category, human_id.to_string())),
        }
    }

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

    #[test]
    fn creatable_excludes_dashboard() {
        let creatable = Category::creatable();
        assert_eq!(creatable.len(), 12);
        assert!(!creatable.contains(&Category::Dashboard));
    }

    #[test]
    fn creatable_matches_all_minus_dashboard_in_order() {
        let expected: Vec<Category> = Category::all()
            .into_iter()
            .filter(|category| *category != Category::Dashboard)
            .collect();
        assert_eq!(Category::creatable().to_vec(), expected);
    }

    #[test]
    fn empty_history_cannot_move() {
        let history = NavHistory::default();
        assert_eq!(history.current(), None);
        assert!(!history.can_back());
        assert!(!history.can_forward());
    }

    #[test]
    fn push_deduplicates_current_location() {
        let mut history = NavHistory::default();
        history.push(location(Category::People));
        history.push(location(Category::People));
        assert_eq!(history.current(), Some(&location(Category::People)));
        assert!(!history.can_back());
        assert!(!history.can_forward());
    }

    #[test]
    fn push_tracks_cursor_and_appends() {
        let mut history = NavHistory::default();
        history.push(location(Category::Dashboard));
        history.push(location(Category::People));
        history.push(location(Category::Families));
        assert_eq!(history.current(), Some(&location(Category::Families)));
        assert!(history.can_back());
        assert!(!history.can_forward());
    }

    #[test]
    fn back_and_forward_return_expected_locations() {
        let mut history = NavHistory::default();
        history.push(location(Category::Dashboard));
        history.push(location(Category::People));
        history.push(location(Category::Families));

        assert_eq!(history.back(), Some(location(Category::People)));
        assert!(history.can_back());
        assert!(history.can_forward());

        assert_eq!(history.back(), Some(location(Category::Dashboard)));
        assert!(!history.can_back());
        assert!(history.can_forward());
        assert_eq!(history.back(), None);

        assert_eq!(history.forward(), Some(location(Category::People)));
        assert_eq!(history.forward(), Some(location(Category::Families)));
        assert!(!history.can_forward());
        assert_eq!(history.forward(), None);
    }

    #[test]
    fn divergent_push_after_back_truncates_forward_tail() {
        let mut history = NavHistory::default();
        history.push(location(Category::Dashboard));
        history.push(location(Category::People));
        history.push(location(Category::Families));

        history.back();
        history.push(location_with_record(Category::Events, "E0001"));

        assert_eq!(
            history.current(),
            Some(&location_with_record(Category::Events, "E0001"))
        );
        assert!(!history.can_forward());
        assert!(history.can_back());
        assert_eq!(history.back(), Some(location(Category::People)));
    }

    #[test]
    fn tab_label_prefers_non_blank_name() {
        assert_eq!(tab_label(Some("Ada Lovelace"), "I0001"), "Ada Lovelace");
    }

    #[test]
    fn tab_label_falls_back_to_human_id() {
        assert_eq!(tab_label(None, "I0001"), "I0001");
        assert_eq!(tab_label(Some(""), "I0001"), "I0001");
        assert_eq!(tab_label(Some("   "), "I0001"), "I0001");
    }
}
