//! The command palette's framework-neutral model (`docs/mockups/search-palette.html`, ADR 0008).
//!
//! The palette blends four kinds of result — records (the loaded entity lists, filtered by the
//! query), commands (create-a-record, review-matches, open-a-tool/help), saved searches, and the
//! recently-opened records — into grouped, keyboard-navigable options. All of the decision logic lives here as pure
//! functions ([`palette_groups`], [`move_active`], [`activate`]) so it is unit-testable without a
//! renderer; the Dioxus layer only loads the rows, resolves labels, and draws the listbox.

use vitni_app::{SavedSearchId, SavedSearchList};

use crate::list::RowVm;
use crate::listed::listed_category;
use crate::navigation::{Category, RecordRef, Tool};

/// The most options one record group shows; the operator narrows with the query rather than
/// scrolling (mirrors [`picker::PICKER_MAX_ROWS`](crate::picker::PICKER_MAX_ROWS)).
pub const PALETTE_GROUP_MAX: usize = 5;

/// A command the palette can run — the non-record actions, each mapped by the renderer onto an
/// existing [`NavState`](../../vitni_ui_dioxus/shell/nav_state) action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteCommand {
    /// Create a new record of a category (→ `request_new_for`).
    Create(Category),
    /// Open the Matches tool's possible-matches queue (→ `go_to(Tool::Matches)`).
    ReviewMatches,
    /// Open a tool screen (→ `go_to(Tool)`), for the tools without a bespoke command.
    OpenTool(Tool),
    /// Open the in-app help browser (→ `go_to(Destination::Help)`).
    OpenHelp,
}

/// A command paired with its already-localized label (the renderer resolves the label; the query
/// matches against it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteCommandVm {
    /// The command to run when this option is activated.
    pub command: PaletteCommand,
    /// The already-localized label shown in the row (e.g. `Create person…`).
    pub label: String,
}

/// One palette option: a record hit, a command, a saved search, or a recently-opened record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteEntry {
    /// A record from a loaded entity list.
    Record {
        /// The record's category.
        category: Category,
        /// The list row (its id, title, subtitle, avatar).
        row: RowVm,
    },
    /// A command.
    Command(PaletteCommandVm),
    /// A saved search, run in the list of its kind's category.
    SavedSearch {
        /// The saved search.
        id: SavedSearchId,
        /// Its name.
        name: String,
        /// The category that lists its kind, shown beside the name.
        category: Category,
    },
    /// A recently-opened record (the "Jump back in" list, newest first).
    Recent {
        /// The record's category.
        category: Category,
        /// The record's user-facing id.
        human_id: String,
        /// The display label captured when it was opened.
        label: String,
    },
}

/// Which heading a [`PaletteGroup`] carries — the renderer resolves it to localized text (record
/// groups reuse the rail label; Commands/Recent have their own chrome keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteGroupKind {
    /// A record category (heading = the rail label).
    Category(Category),
    /// The Commands group.
    Commands,
    /// The Saved searches group.
    SavedSearches,
    /// The Recent group.
    Recent,
}

/// A labelled run of palette options shown under one heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteGroup {
    /// The group's heading kind.
    pub kind: PaletteGroupKind,
    /// The group's options, in display order.
    pub entries: Vec<PaletteEntry>,
}

/// What activating a palette option does: open a record, run a command, or run a saved search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    /// Open a record as a tab.
    Open(RecordRef),
    /// Run a command.
    Run(PaletteCommand),
    /// Open a saved search's list with its query applied.
    RunSavedSearch {
        /// The saved search.
        id: SavedSearchId,
        /// The category whose list it applies to.
        category: Category,
    },
}

/// The default command list, in palette display order: a Create for every creatable category, then
/// Review-possible-matches, the tool screens (Pedigree/Import/Export/Plugins/Preferences), and Help. The
/// renderer resolves each command's label. Matches is reached through
/// [`PaletteCommand::ReviewMatches`], so it is not an [`PaletteCommand::OpenTool`]. Import is the
/// assisted-import wizard (ADR 0017); Export is the bulk-export wizard (ADR 0013).
#[must_use]
pub fn palette_commands() -> Vec<PaletteCommand> {
    let mut commands = Vec::new();
    for category in Category::creatable() {
        commands.push(PaletteCommand::Create(category));
    }
    commands.push(PaletteCommand::ReviewMatches);
    commands.push(PaletteCommand::OpenTool(Tool::Pedigree));
    commands.push(PaletteCommand::OpenTool(Tool::Import));
    commands.push(PaletteCommand::OpenTool(Tool::Export));
    commands.push(PaletteCommand::OpenTool(Tool::Plugins));
    commands.push(PaletteCommand::OpenTool(Tool::Preferences));
    commands.push(PaletteCommand::OpenHelp);
    commands
}

/// Builds the palette's groups for `query`, in display order (record categories in
/// [`Category::all`] order, then Commands, Saved searches, then Recent).
///
/// An empty query shows Commands, Saved searches and Recent only (the record lists are not dumped
/// wholesale). A non-empty query filters each record category by [`RowVm::matches`] (capped at
/// [`PALETTE_GROUP_MAX`] per group), the commands and saved searches by their name, and the recent
/// records by their label. Like commands, saved searches are never capped. Every empty group is
/// omitted.
#[must_use]
pub fn palette_groups(
    records: &[(Category, Vec<RowVm>)],
    commands: &[PaletteCommandVm],
    saved: &[PaletteEntry],
    recent: &[PaletteEntry],
    query: &str,
) -> Vec<PaletteGroup> {
    let query = query.trim();
    let mut groups: Vec<PaletteGroup> = Vec::new();
    if !query.is_empty() {
        for (category, rows) in records {
            let mut entries: Vec<PaletteEntry> = Vec::new();
            for row in rows {
                if row.matches(query) {
                    entries.push(PaletteEntry::Record {
                        category: *category,
                        row: row.clone(),
                    });
                    if entries.len() >= PALETTE_GROUP_MAX {
                        break;
                    }
                }
            }
            if !entries.is_empty() {
                groups.push(PaletteGroup {
                    kind: PaletteGroupKind::Category(*category),
                    entries,
                });
            }
        }
    }
    let command_entries: Vec<PaletteEntry> = commands
        .iter()
        .filter(|command| query.is_empty() || command_matches(command, query))
        .cloned()
        .map(PaletteEntry::Command)
        .collect();
    if !command_entries.is_empty() {
        groups.push(PaletteGroup {
            kind: PaletteGroupKind::Commands,
            entries: command_entries,
        });
    }
    let saved_entries: Vec<PaletteEntry> = saved
        .iter()
        .filter(|entry| query.is_empty() || saved_matches(entry, query))
        .cloned()
        .collect();
    if !saved_entries.is_empty() {
        groups.push(PaletteGroup {
            kind: PaletteGroupKind::SavedSearches,
            entries: saved_entries,
        });
    }
    let recent_entries: Vec<PaletteEntry> = recent
        .iter()
        .filter(|entry| query.is_empty() || recent_matches(entry, query))
        .cloned()
        .collect();
    if !recent_entries.is_empty() {
        groups.push(PaletteGroup {
            kind: PaletteGroupKind::Recent,
            entries: recent_entries,
        });
    }
    groups
}

/// Whether a command's label contains `query` (case-insensitive).
fn command_matches(command: &PaletteCommandVm, query: &str) -> bool {
    command.label.to_lowercase().contains(&query.to_lowercase())
}

/// Whether a saved search's name contains `query` (case-insensitive).
fn saved_matches(entry: &PaletteEntry, query: &str) -> bool {
    match entry {
        PaletteEntry::SavedSearch { name, .. } => name.to_lowercase().contains(&query.to_lowercase()),
        PaletteEntry::Record { .. } | PaletteEntry::Command(_) | PaletteEntry::Recent { .. } => false,
    }
}

/// Whether a recent entry's label or id contains `query` (case-insensitive).
fn recent_matches(entry: &PaletteEntry, query: &str) -> bool {
    let query = query.to_lowercase();
    match entry {
        PaletteEntry::Recent { human_id, label, .. } => {
            label.to_lowercase().contains(&query) || human_id.to_lowercase().contains(&query)
        }
        PaletteEntry::Record { .. } | PaletteEntry::Command(_) | PaletteEntry::SavedSearch { .. } => false,
    }
}

/// Moves the active option index by `delta` within `total` options, clamped at both ends (no wrap).
/// Returns `0` when there are no options.
#[must_use]
pub fn move_active(active: usize, total: usize, delta: isize) -> usize {
    let Some(last) = total.checked_sub(1) else {
        return 0;
    };
    let current = isize::try_from(active).unwrap_or(0);
    let last_index = isize::try_from(last).unwrap_or(0);
    let stepped = (current + delta).clamp(0, last_index);
    usize::try_from(stepped).unwrap_or(0).min(last)
}

/// The action activating `entry` performs: open the record (record + recent hits) or run the command or saved search.
#[must_use]
pub fn activate(entry: &PaletteEntry) -> PaletteAction {
    match entry {
        PaletteEntry::Record { category, row } => PaletteAction::Open(RecordRef {
            category: *category,
            human_id: row.id.clone(),
            label: row.title.clone(),
        }),
        PaletteEntry::Recent {
            category,
            human_id,
            label,
        } => PaletteAction::Open(RecordRef {
            category: *category,
            human_id: human_id.clone(),
            label: label.clone(),
        }),
        PaletteEntry::Command(vm) => PaletteAction::Run(vm.command.clone()),
        PaletteEntry::SavedSearch { id, category, .. } => PaletteAction::RunSavedSearch {
            id: *id,
            category: *category,
        },
    }
}

/// The readable saved searches as palette entries, each with its kind's category; the unreadable ones
/// are not offered.
#[must_use]
pub fn saved_search_entries(list: &SavedSearchList) -> Vec<PaletteEntry> {
    let mut entries = Vec::with_capacity(list.searches.len());
    for search in &list.searches {
        entries.push(PaletteEntry::SavedSearch {
            id: search.id,
            name: search.name.clone(),
            category: listed_category(search.kind),
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use vitni_app::{ListedKind, SavedSearchId, SavedSearchList, SavedSearchSummary, UnreadableSavedSearch};

    use super::{
        PALETTE_GROUP_MAX, PaletteAction, PaletteCommand, PaletteCommandVm, PaletteEntry, PaletteGroupKind, activate,
        move_active, palette_commands, palette_groups, saved_search_entries,
    };
    use crate::list::RowVm;
    use crate::navigation::{Category, Tool};

    fn row(id: &str, title: &str) -> RowVm {
        RowVm {
            id: id.to_owned(),
            title: title.to_owned(),
            ..RowVm::default()
        }
    }

    fn command(command: PaletteCommand, label: &str) -> PaletteCommandVm {
        PaletteCommandVm {
            command,
            label: label.to_owned(),
        }
    }

    fn recent(category: Category, human_id: &str, label: &str) -> PaletteEntry {
        PaletteEntry::Recent {
            category,
            human_id: human_id.to_owned(),
            label: label.to_owned(),
        }
    }

    fn saved(n: u128, name: &str, category: Category) -> PaletteEntry {
        PaletteEntry::SavedSearch {
            id: SavedSearchId::from_uuid(Uuid::from_u128(n)),
            name: name.to_owned(),
            category,
        }
    }

    fn saved_searches() -> Vec<PaletteEntry> {
        vec![
            saved(1, "Born before 1800", Category::People),
            saved(2, "Untagged places", Category::Places),
        ]
    }

    fn people() -> (Category, Vec<RowVm>) {
        (
            Category::People,
            vec![
                row("I0042", "Smith, John"),
                row("I0118", "Smith, Jane"),
                row("I0061", "Jones, Mary"),
            ],
        )
    }

    fn commands() -> Vec<PaletteCommandVm> {
        vec![
            command(PaletteCommand::Create(Category::People), "Create person…"),
            command(PaletteCommand::ReviewMatches, "Review possible matches"),
            command(PaletteCommand::OpenHelp, "Open help"),
        ]
    }

    #[test]
    fn empty_query_shows_commands_and_recent_without_saved_searches() {
        let records = vec![people()];
        let recent = vec![recent(Category::Families, "F0017", "Smith–Doe family")];
        let groups = palette_groups(&records, &commands(), &[], &recent, "");
        let kinds: Vec<PaletteGroupKind> = groups.iter().map(|group| group.kind).collect();
        assert_eq!(kinds, vec![PaletteGroupKind::Commands, PaletteGroupKind::Recent]);
        assert_eq!(groups[0].entries.len(), 3, "all commands on an empty query");
        assert_eq!(groups[1].entries.len(), 1, "the recent record");
    }

    #[test]
    fn a_query_filters_records_commands_and_recent() {
        let records = vec![people()];
        let recent = vec![
            recent(Category::Families, "F0017", "Smith–Doe family"),
            recent(Category::Places, "L0009", "Boston"),
        ];
        let groups = palette_groups(&records, &commands(), &[], &recent, "smi");
        // People group first (two Smith/Smyth rows), then Recent (the Smith–Doe family). No command
        // matches "smi", so the Commands group is omitted.
        let kinds: Vec<PaletteGroupKind> = groups.iter().map(|group| group.kind).collect();
        assert_eq!(
            kinds,
            vec![PaletteGroupKind::Category(Category::People), PaletteGroupKind::Recent]
        );
        assert_eq!(groups[0].entries.len(), 2, "Smith + Smyth match, Jones does not");
        assert_eq!(groups[1].entries.len(), 1, "only the Smith–Doe recent matches");
    }

    #[test]
    fn record_groups_are_capped_and_follow_category_order() {
        let many: Vec<RowVm> = (0..20).map(|n| row(&format!("I{n:04}"), "Ada Match")).collect();
        let places: Vec<RowVm> = vec![row("L0001", "Match Place")];
        let records = vec![(Category::People, many), (Category::Places, places)];
        let groups = palette_groups(&records, &[], &[], &[], "match");
        assert_eq!(groups[0].kind, PaletteGroupKind::Category(Category::People));
        assert_eq!(groups[0].entries.len(), PALETTE_GROUP_MAX, "the people group is capped");
        assert_eq!(groups[1].kind, PaletteGroupKind::Category(Category::Places));
    }

    #[test]
    fn command_label_matching_is_case_insensitive() {
        let groups = palette_groups(&[], &commands(), &[], &[], "MATCHES");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, PaletteGroupKind::Commands);
        assert_eq!(groups[0].entries.len(), 1);
    }

    #[test]
    fn empty_groups_are_omitted() {
        let groups = palette_groups(&[people()], &commands(), &[], &[], "no-such-token");
        assert!(groups.is_empty(), "nothing matches, so no groups: {groups:?}");
    }

    #[test]
    fn move_active_clamps_without_wrapping() {
        assert_eq!(move_active(0, 3, -1), 0, "does not wrap past the top");
        assert_eq!(move_active(2, 3, 1), 2, "does not wrap past the bottom");
        assert_eq!(move_active(1, 3, 1), 2);
        assert_eq!(move_active(1, 3, -1), 0);
        assert_eq!(move_active(0, 0, 1), 0, "no options");
    }

    #[test]
    fn activate_opens_records_and_recents_and_runs_commands() {
        let record = PaletteEntry::Record {
            category: Category::People,
            row: row("I0042", "Smith, John"),
        };
        match activate(&record) {
            PaletteAction::Open(reference) => {
                assert_eq!(reference.category, Category::People);
                assert_eq!(reference.human_id, "I0042");
                assert_eq!(reference.label, "Smith, John");
            }
            PaletteAction::Run(_) | PaletteAction::RunSavedSearch { .. } => panic!("a record entry opens a record"),
        }
        match activate(&recent(Category::Families, "F0017", "Smith–Doe family")) {
            PaletteAction::Open(reference) => assert_eq!(reference.human_id, "F0017"),
            PaletteAction::Run(_) | PaletteAction::RunSavedSearch { .. } => panic!("a recent entry opens a record"),
        }
        let run = PaletteEntry::Command(command(PaletteCommand::ReviewMatches, "Review possible matches"));
        assert_eq!(activate(&run), PaletteAction::Run(PaletteCommand::ReviewMatches));
    }

    #[test]
    fn every_creatable_category_has_a_create_command() {
        let commands = palette_commands();
        for category in Category::creatable() {
            assert!(
                commands.contains(&PaletteCommand::Create(category)),
                "missing Create command for {category:?}"
            );
        }
        assert!(commands.contains(&PaletteCommand::ReviewMatches));
        assert!(commands.contains(&PaletteCommand::OpenTool(Tool::Export)));
        assert!(commands.contains(&PaletteCommand::OpenTool(Tool::Preferences)));
        assert!(commands.contains(&PaletteCommand::OpenHelp));
        assert!(
            !commands.contains(&PaletteCommand::OpenTool(Tool::Matches)),
            "Matches is reached via Review possible matches, not an OpenTool command"
        );
    }

    #[test]
    fn saved_searches_follow_commands_and_precede_recent_on_an_empty_query() {
        let recent = vec![recent(Category::Families, "F0017", "Smith–Doe family")];
        let groups = palette_groups(&[people()], &commands(), &saved_searches(), &recent, "");
        let kinds: Vec<PaletteGroupKind> = groups.iter().map(|group| group.kind).collect();
        assert_eq!(
            kinds,
            vec![
                PaletteGroupKind::Commands,
                PaletteGroupKind::SavedSearches,
                PaletteGroupKind::Recent
            ]
        );
        assert_eq!(
            groups[1].entries,
            saved_searches(),
            "every saved search, each with its category"
        );
    }

    #[test]
    fn saved_searches_are_filtered_by_name_and_omitted_when_none_match() {
        let groups = palette_groups(&[], &[], &saved_searches(), &[], "UNTAGGED");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, PaletteGroupKind::SavedSearches);
        assert_eq!(groups[0].entries, vec![saved(2, "Untagged places", Category::Places)]);
        let none = palette_groups(&[], &[], &saved_searches(), &[], "no-such-token");
        assert!(none.is_empty(), "{none:?}");
    }

    #[test]
    fn every_saved_search_is_listed_beyond_the_record_group_cap() {
        let many: Vec<PaletteEntry> = (1..=PALETTE_GROUP_MAX as u128 + 1)
            .map(|n| saved(n, &format!("Search {n}"), Category::People))
            .collect();
        let groups = palette_groups(&[], &[], &many, &[], "");
        assert_eq!(groups[0].entries, many);
    }

    #[test]
    fn the_saved_searches_group_is_omitted_when_there_are_none() {
        let groups = palette_groups(&[], &commands(), &[], &[], "");
        let kinds: Vec<PaletteGroupKind> = groups.iter().map(|group| group.kind).collect();
        assert_eq!(kinds, vec![PaletteGroupKind::Commands]);
    }

    #[test]
    fn activating_a_saved_search_runs_it_in_its_category() {
        let entry = saved(7, "Born before 1800", Category::People);
        assert_eq!(
            activate(&entry),
            PaletteAction::RunSavedSearch {
                id: SavedSearchId::from_uuid(Uuid::from_u128(7)),
                category: Category::People,
            }
        );
    }

    #[test]
    fn saved_search_entries_carry_the_kinds_category_and_skip_unreadable() {
        let id = SavedSearchId::from_uuid(Uuid::from_u128(1));
        let place_id = SavedSearchId::from_uuid(Uuid::from_u128(2));
        let list = SavedSearchList {
            searches: vec![
                SavedSearchSummary {
                    id,
                    name: "Born in Oslo".to_owned(),
                    kind: ListedKind::Person,
                },
                SavedSearchSummary {
                    id: place_id,
                    name: "Farms".to_owned(),
                    kind: ListedKind::Place,
                },
            ],
            unreadable: vec![UnreadableSavedSearch {
                position: 1,
                name: None,
            }],
        };
        assert_eq!(
            saved_search_entries(&list),
            vec![
                PaletteEntry::SavedSearch {
                    id,
                    name: "Born in Oslo".to_owned(),
                    category: Category::People,
                },
                PaletteEntry::SavedSearch {
                    id: place_id,
                    name: "Farms".to_owned(),
                    category: Category::Places,
                },
            ]
        );
    }
}
