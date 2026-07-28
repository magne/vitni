//! The bulk-import wizard's session state machine and source/target parsing (issue #191).
//!
//! [`BulkImportSession`] is the framework-free heart of the bulk-import mode on `Tool::Import`: it
//! mirrors [`crate::view_model::export::ExportSession`] exactly (same stage shape, same terminal-stage
//! guard) but for a local-file import rather than an export — the GUI counterpart of `genealogy import
//! <plugin> <file> (--new NAME PATH | --into NAME) [--yes]`.
//!
//! [`ImportSourcePath`] classifies what the operator typed as the source file, lexically (no
//! filesystem access) — mirroring [`crate::view_model::export::ExportDestination`]'s parsing, but a
//! bulk import always names one file, never a directory. [`ImportTargetChoice`] is the other half of
//! the CLI's target selection (`--new`/`--into`), with a pure validator against the already-registered
//! workspace names so the wizard can reject an empty or duplicate new-workspace name before ever
//! opening a store.

use std::path::{Component, Path, PathBuf};

/// A progress report from a running bulk import — the framework-free mirror of the plugin host's
/// `ProgressUpdate`. `total` is absent until the plugin knows the record count.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BulkImportProgress {
    /// The phase the plugin is in, in the plugin's own vocabulary.
    pub step: String,
    /// How many records the plugin has imported so far.
    pub processed: u32,
    /// The total it expects, if known.
    pub total: Option<u32>,
}

/// What a finished bulk import read and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkImportSummary {
    /// How many records the plugin imported.
    pub records: u32,
    /// The source file the operator picked, for display.
    pub source: String,
}

/// Where a bulk-import session currently is. It starts at [`Source`](Self::Source) and runs through
/// [`Running`](Self::Running) to one of the three terminal stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BulkImportStage {
    /// The initial stage: the operator picks a plugin, a source file, and a target workspace.
    Source,
    /// The import is running; the payload is its latest progress report.
    Running(BulkImportProgress),
    /// The import finished and read records.
    Summary(BulkImportSummary),
    /// The import failed; the payload is the localized message to show.
    Error(String),
    /// The operator cancelled the import.
    Cancelled,
}

/// The bulk-import wizard's session: the current [`BulkImportStage`], advanced by the running
/// invocation. Identical shape to [`crate::view_model::export::ExportSession`] — see its docs for the
/// terminal-stage guard rationale (a cancelled run's trailing reports must not resurrect it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkImportSession {
    stage: BulkImportStage,
}

impl Default for BulkImportSession {
    fn default() -> Self {
        Self::new()
    }
}

impl BulkImportSession {
    /// A fresh session at the [`Source`](BulkImportStage::Source) stage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stage: BulkImportStage::Source,
        }
    }

    /// The current stage.
    #[must_use]
    pub fn stage(&self) -> &BulkImportStage {
        &self.stage
    }

    /// Whether the session has reached a terminal stage (summary, error, or cancelled) — the wizard
    /// stops following the run.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(
            self.stage,
            BulkImportStage::Summary(_) | BulkImportStage::Error(_) | BulkImportStage::Cancelled
        )
    }

    /// Moves the session to [`Running`](BulkImportStage::Running) with no progress reported yet.
    pub fn start(&mut self) {
        self.stage = BulkImportStage::Running(BulkImportProgress::default());
    }

    /// Replaces the running stage's progress with the plugin's latest report. Ignored once the
    /// session is finished: a cancelled run keeps reporting until the guest stops, and that trailing
    /// progress must not resurrect it.
    pub fn on_progress(&mut self, progress: BulkImportProgress) {
        if self.is_finished() {
            return;
        }
        self.stage = BulkImportStage::Running(progress);
    }

    /// Records a successful run. Ignored once the session is finished.
    pub fn on_success(&mut self, summary: BulkImportSummary) {
        if self.is_finished() {
            return;
        }
        self.stage = BulkImportStage::Summary(summary);
    }

    /// Records a failed run with its localized message. Ignored once the session is finished — a
    /// cancelled run fails on the way out, and "Cancelled" is the truer thing to show.
    pub fn on_failure(&mut self, message: String) {
        if self.is_finished() {
            return;
        }
        self.stage = BulkImportStage::Error(message);
    }

    /// Cancels the session from any stage.
    pub fn cancel(&mut self) {
        self.stage = BulkImportStage::Cancelled;
    }
}

/// The source file a bulk import should read, as parsed from what the operator typed (lexical,
/// filesystem-free — mirrors [`crate::view_model::export::ExportDestination::parse`]). Unlike an
/// export destination, a bulk-import source must name one file: there is no directory default, so an
/// empty field or one that names a directory is simply not usable yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSourcePath {
    /// The operator has not typed anything (or only whitespace) yet.
    None,
    /// The typed path's last component is a directory marker, or resolves with no file name.
    Directory(PathBuf),
    /// A path that names a file — the only source a run can use.
    File(PathBuf),
}

impl ImportSourcePath {
    /// Parses the operator's `input` against `default_dir` (the workspace's own directory, for a
    /// relative path). An input ending in a path separator, or whose last component is `.` or `..`,
    /// names a directory; anything else names a file. `.` and `..` components resolve lexically.
    #[must_use]
    pub fn parse(input: &str, default_dir: &Path) -> Self {
        let input = input.trim();
        if input.is_empty() {
            return Self::None;
        }
        let typed = Path::new(input);
        let resolved = if typed.is_absolute() {
            normalize(typed)
        } else {
            normalize(&default_dir.join(typed))
        };
        if names_a_directory(input) || resolved.file_name().is_none() {
            Self::Directory(resolved)
        } else {
            Self::File(resolved)
        }
    }

    /// The resolved path, for the live preview — absent only when nothing has been typed yet.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::None => None,
            Self::Directory(path) | Self::File(path) => Some(path),
        }
    }

    /// Whether this path can be run as a bulk-import source: it must name a file.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::File(_))
    }

    /// Whether the typed input names a directory rather than a file (the Run-disabling reason
    /// distinct from an empty field).
    #[must_use]
    pub fn names_a_directory(&self) -> bool {
        matches!(self, Self::Directory(_))
    }
}

/// Whether the operator's raw input names a directory: it ends in a path separator, or its last
/// component is `.` or `..`. Mirrors `export.rs`'s private helper of the same name.
fn names_a_directory(input: &str) -> bool {
    if input.ends_with(std::path::is_separator) {
        return true;
    }
    match Path::new(input).components().next_back() {
        Some(Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_)) => true,
        Some(Component::Normal(_)) | None => false,
    }
}

/// Resolves `.` and `..` lexically, without consulting the filesystem. A `..` that would climb past
/// the root is dropped; one that climbs past a relative path's start is kept. Mirrors `export.rs`'s
/// private helper of the same name.
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !normalized.has_root() {
                    normalized.push(Component::ParentDir);
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => normalized.push(component),
        }
    }
    normalized
}

/// Why a new-workspace [`ImportTargetChoice`] cannot be run yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportTargetError {
    /// The new-workspace name is empty (after trimming).
    EmptyName,
    /// The new-workspace name already names a registered workspace.
    NameTaken,
}

/// The bulk-import target the operator picked — the GUI shape of the CLI's `--new NAME PATH` /
/// `--into NAME` choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportTargetChoice {
    /// Import into an already-registered workspace.
    Existing {
        /// The registered workspace's name.
        workspace: String,
    },
    /// Register and import into a freshly created workspace.
    New {
        /// The new workspace's name (trimmed before use).
        name: String,
        /// The workspace directory; `None` uses the default data directory.
        directory: Option<PathBuf>,
        /// An opt-in Postgres connection URL; `None` keeps the default SQLite engine.
        database_url: Option<String>,
    },
}

impl ImportTargetChoice {
    /// Validates the choice against the already-registered workspace `names`. An `Existing` choice is
    /// always valid — the operator picked it from the registry, so it names a real workspace by
    /// construction; only a `New` choice's name can be empty or collide.
    ///
    /// # Errors
    /// [`ImportTargetError::EmptyName`] for a blank (after trimming) new-workspace name, or
    /// [`ImportTargetError::NameTaken`] when it already names a registered workspace.
    pub fn validate(&self, names: &[String]) -> Result<(), ImportTargetError> {
        match self {
            Self::Existing { .. } => Ok(()),
            Self::New { name, .. } => {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    return Err(ImportTargetError::EmptyName);
                }
                if names.iter().any(|existing| existing == trimmed) {
                    return Err(ImportTargetError::NameTaken);
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        BulkImportProgress, BulkImportSession, BulkImportStage, BulkImportSummary, ImportSourcePath,
        ImportTargetChoice, ImportTargetError,
    };

    fn progress(step: &str, processed: u32) -> BulkImportProgress {
        BulkImportProgress {
            step: step.to_owned(),
            processed,
            total: Some(40),
        }
    }

    fn summary() -> BulkImportSummary {
        BulkImportSummary {
            records: 40,
            source: "/ws/family.ged".to_owned(),
        }
    }

    #[test]
    fn a_new_session_starts_at_the_source_stage() {
        let session = BulkImportSession::new();
        assert_eq!(*session.stage(), BulkImportStage::Source);
        assert!(!session.is_finished());
    }

    #[test]
    fn a_run_moves_through_running_to_the_summary() {
        let mut session = BulkImportSession::new();

        session.start();
        assert_eq!(
            *session.stage(),
            BulkImportStage::Running(BulkImportProgress::default())
        );
        assert!(!session.is_finished());

        session.on_progress(progress("persons", 10));
        assert_eq!(*session.stage(), BulkImportStage::Running(progress("persons", 10)));

        // A later report replaces the earlier one rather than accumulating.
        session.on_progress(progress("families", 30));
        assert_eq!(*session.stage(), BulkImportStage::Running(progress("families", 30)));
        assert!(!session.is_finished());

        session.on_success(summary());
        assert_eq!(*session.stage(), BulkImportStage::Summary(summary()));
        assert!(session.is_finished());
    }

    #[test]
    fn a_failed_run_lands_in_the_error_stage() {
        let mut session = BulkImportSession::new();
        session.start();
        session.on_failure("plugin trapped: out of fuel".to_owned());
        assert_eq!(
            *session.stage(),
            BulkImportStage::Error("plugin trapped: out of fuel".to_owned())
        );
        assert!(session.is_finished());
    }

    #[test]
    fn cancel_from_any_stage_moves_to_cancelled() {
        let mut session = BulkImportSession::new();
        session.cancel();
        assert_eq!(*session.stage(), BulkImportStage::Cancelled);

        let mut session = BulkImportSession::new();
        session.start();
        session.on_progress(progress("persons", 4));
        session.cancel();
        assert_eq!(*session.stage(), BulkImportStage::Cancelled);
        assert!(session.is_finished());
    }

    #[test]
    fn a_cancelled_run_is_not_resurrected_by_its_trailing_reports() {
        // The guest only stops at its next progress report, so a cancelled run keeps reporting and
        // then fails on the way out. Neither may overwrite the operator's decision.
        let mut session = BulkImportSession::new();
        session.start();
        session.cancel();

        session.on_progress(progress("families", 22));
        assert_eq!(*session.stage(), BulkImportStage::Cancelled);

        session.on_failure("cancelled by the frontend".to_owned());
        assert_eq!(*session.stage(), BulkImportStage::Cancelled);

        session.on_success(summary());
        assert_eq!(*session.stage(), BulkImportStage::Cancelled);
    }

    #[test]
    fn every_terminal_stage_finishes_the_session() {
        let mut summary_session = BulkImportSession::new();
        summary_session.on_success(summary());
        assert!(summary_session.is_finished());

        let mut error_session = BulkImportSession::new();
        error_session.on_failure("boom".to_owned());
        assert!(error_session.is_finished());

        let mut cancelled_session = BulkImportSession::new();
        cancelled_session.cancel();
        assert!(cancelled_session.is_finished());
    }

    fn default_dir() -> PathBuf {
        PathBuf::from("/ws")
    }

    #[test]
    fn an_empty_source_is_none() {
        let source = ImportSourcePath::parse("", &default_dir());
        assert_eq!(source, ImportSourcePath::None);
        assert!(!source.is_usable());
        assert!(!source.names_a_directory());
        assert_eq!(source.path(), None);

        // Whitespace-only input is the same as empty.
        assert_eq!(ImportSourcePath::parse("   ", &default_dir()), ImportSourcePath::None);
    }

    #[test]
    fn a_relative_path_resolves_against_the_default_directory() {
        let source = ImportSourcePath::parse("imports/family.ged", &default_dir());
        assert_eq!(source, ImportSourcePath::File(PathBuf::from("/ws/imports/family.ged")));
        assert!(source.is_usable());
        assert_eq!(source.path(), Some(Path::new("/ws/imports/family.ged")));
    }

    #[test]
    fn an_absolute_path_is_taken_as_typed() {
        assert_eq!(
            ImportSourcePath::parse("/srv/archive/family.ged", &default_dir()),
            ImportSourcePath::File(PathBuf::from("/srv/archive/family.ged"))
        );
    }

    #[test]
    fn a_trailing_separator_names_a_directory_and_is_not_usable() {
        let source = ImportSourcePath::parse("imports/", &default_dir());
        assert_eq!(source, ImportSourcePath::Directory(PathBuf::from("/ws/imports")));
        assert!(!source.is_usable());
        assert!(source.names_a_directory());
    }

    #[test]
    fn dot_and_dot_dot_components_resolve_lexically() {
        assert_eq!(
            ImportSourcePath::parse("../family.ged", &default_dir()),
            ImportSourcePath::File(PathBuf::from("/family.ged"))
        );
        assert_eq!(
            ImportSourcePath::parse("./imports/./family.ged", &default_dir()),
            ImportSourcePath::File(PathBuf::from("/ws/imports/family.ged"))
        );
        // A trailing `..` (or `.`) is a directory, not a file called "..".
        assert_eq!(
            ImportSourcePath::parse("..", &default_dir()),
            ImportSourcePath::Directory(PathBuf::from("/"))
        );
        assert_eq!(
            ImportSourcePath::parse(".", &default_dir()),
            ImportSourcePath::Directory(default_dir())
        );
        // Climbing past the root cannot escape it.
        assert_eq!(
            ImportSourcePath::parse("/../../family.ged", &default_dir()),
            ImportSourcePath::File(PathBuf::from("/family.ged"))
        );
    }

    #[test]
    fn the_root_directory_has_no_file_name_so_it_is_a_directory() {
        assert_eq!(
            ImportSourcePath::parse("/", &default_dir()),
            ImportSourcePath::Directory(PathBuf::from("/"))
        );
    }

    #[test]
    fn an_existing_target_is_always_valid() {
        let target = ImportTargetChoice::Existing {
            workspace: "family".to_owned(),
        };
        assert_eq!(target.validate(&["family".to_owned()]), Ok(()));
        assert_eq!(target.validate(&[]), Ok(()));
    }

    #[test]
    fn a_new_target_rejects_an_empty_name() {
        let target = ImportTargetChoice::New {
            name: "   ".to_owned(),
            directory: None,
            database_url: None,
        };
        assert_eq!(target.validate(&[]), Err(ImportTargetError::EmptyName));
    }

    #[test]
    fn a_new_target_rejects_a_name_already_registered() {
        let target = ImportTargetChoice::New {
            name: "family".to_owned(),
            directory: None,
            database_url: None,
        };
        assert_eq!(
            target.validate(&["other".to_owned(), "family".to_owned()]),
            Err(ImportTargetError::NameTaken)
        );
    }

    #[test]
    fn a_new_target_with_a_fresh_trimmed_name_is_valid() {
        let target = ImportTargetChoice::New {
            name: "  family  ".to_owned(),
            directory: Some(PathBuf::from("/data/family")),
            database_url: None,
        };
        assert_eq!(target.validate(&["other".to_owned()]), Ok(()));
    }
}
