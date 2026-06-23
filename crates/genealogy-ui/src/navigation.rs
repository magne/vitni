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

use genealogy_app::{AssociationRole, FactType, PersonNameParts, Sex};
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

/// Which screen the GUI is showing — one variant per *buildable* screen (grows per slice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// The list of persons in the workspace.
    PersonList,
    /// One person's detail view.
    PersonDetail {
        /// The person's user-facing id (e.g. `I0001`).
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
    /// Load the person list.
    ShowList,
    /// Load one person's detail.
    ShowPerson {
        /// The person's user-facing id (e.g. `I0001`).
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
            | Self::AssertAssociation { human_id, .. } => human_id,
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
