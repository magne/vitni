//! Saved-search subcommands (ADR 0052 §8): a named list query stored with the workspace.

use clap::Subcommand;
use vitni_app::{
    AppError, FileConfigStore, Session, Workspace, delete_saved_search, list_saved_searches, resolve_saved_search,
    save_saved_search,
};

use crate::args::QueryArgs;
use crate::i18n::Localizer;

/// Saved-search subcommands.
#[derive(Subcommand)]
pub enum SavedSearchCmd {
    /// List the saved searches with their kind, and mark the entries this version cannot read.
    List,
    /// Save a query under a name (names are unique, ignoring case).
    Save {
        /// The name to save it under.
        name: String,
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Delete a saved search by its name (any case).
    Delete {
        /// The saved search's name.
        name: String,
    },
}

/// Runs a saved-search subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: SavedSearchCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let store = FileConfigStore::for_workspace(workspace.dir().to_path_buf());
    match command {
        SavedSearchCmd::List => list(&store, localizer),
        SavedSearchCmd::Save { name, query } => {
            let query = query.query(workspace).await?;
            save_saved_search(&store, session, &name, &query)?;
            println!("{}", localizer.saved_search_saved(name.trim()));
            Ok(())
        }
        SavedSearchCmd::Delete { name } => {
            let resolution = resolve_saved_search(&store, &name, None)?;
            delete_saved_search(&store, resolution.search.id)?;
            println!("{}", localizer.saved_search_deleted(&resolution.search.name));
            Ok(())
        }
    }
}

/// Prints each saved search with its kind, then the entries set aside.
fn list(store: &FileConfigStore, localizer: &Localizer) -> Result<(), AppError> {
    let list = list_saved_searches(store)?;
    if list.searches.is_empty() && list.unreadable.is_empty() {
        println!("{}", localizer.saved_search_list_empty());
    }
    for search in &list.searches {
        println!("{}", localizer.saved_search_line(&search.name, search.kind.as_str()));
    }
    for entry in &list.unreadable {
        println!("{}", localizer.saved_search_unreadable_line(entry));
    }
    Ok(())
}
