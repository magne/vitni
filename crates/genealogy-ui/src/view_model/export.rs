//! The bulk-export wizard's session state machine and destination parsing (ADR 0013).
//!
//! [`ExportSession`] is the framework-free heart of the `Tool::Export` wizard: it holds the current
//! [`ExportStage`] and advances it as the host reports progress and, finally, an outcome. One plugin
//! invocation drives the whole session (pick a destination → run → summary), so the session simply
//! mirrors whatever the run last reported.
//!
//! The stages carry this crate's own [`ExportProgress`] rather than the plugin host's
//! `ProgressUpdate`: `genealogy-ui` sits below the renderer and must stay free of plugin-host types
//! (ADR 0008), so the renderer maps each host update into [`ExportProgress`] on the way in.
//!
//! Terminal stages are sticky. Cancelling is the operator's decision, but the run only stops at the
//! plugin's next progress report, so a cancelled run still emits trailing progress and finally fails
//! — none of that may overwrite [`ExportStage::Cancelled`].
//!
//! [`ExportDestination`] turns what the operator typed into a file-or-directory target without
//! touching the filesystem, so the renderer can preview the resolved path live and hand the same
//! decision to the host as an `ExportTarget`.

use std::path::{Component, Path, PathBuf};

/// A progress report from a running bulk export — the framework-free mirror of the plugin host's
/// `ProgressUpdate` (ADR 0013). `total` is absent until the plugin knows the record count.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportProgress {
    /// The phase the plugin is in (e.g. `"persons"`, `"families"`), in the plugin's own vocabulary.
    pub step: String,
    /// How many records the plugin has written so far.
    pub processed: u32,
    /// The total it expects, if known.
    pub total: Option<u32>,
}

/// What a finished export wrote and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    /// How many records the plugin wrote.
    pub records: u32,
    /// The destination the host resolved, for display.
    pub destination: String,
}

/// Where a bulk-export session currently is. It starts at [`Destination`](Self::Destination) and runs
/// through [`Running`](Self::Running) to one of the three terminal stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportStage {
    /// The initial stage: the operator picks a plugin and a destination.
    Destination,
    /// The export is running; the payload is its latest progress report.
    Running(ExportProgress),
    /// The export finished and wrote records.
    Summary(ExportSummary),
    /// The export failed; the payload is the localized message to show.
    Error(String),
    /// The operator cancelled the export.
    Cancelled,
}

/// The bulk-export wizard's session: the current [`ExportStage`], advanced by the running invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSession {
    stage: ExportStage,
}

impl Default for ExportSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportSession {
    /// A fresh session at the [`Destination`](ExportStage::Destination) stage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stage: ExportStage::Destination,
        }
    }

    /// The current stage.
    #[must_use]
    pub fn stage(&self) -> &ExportStage {
        &self.stage
    }

    /// Whether the session has reached a terminal stage (summary, error, or cancelled) — the wizard
    /// stops following the run.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(
            self.stage,
            ExportStage::Summary(_) | ExportStage::Error(_) | ExportStage::Cancelled
        )
    }

    /// Moves the session to [`Running`](ExportStage::Running) with no progress reported yet.
    pub fn start(&mut self) {
        self.stage = ExportStage::Running(ExportProgress::default());
    }

    /// Replaces the running stage's progress with the plugin's latest report. Ignored once the
    /// session is finished: a cancelled run keeps reporting until the guest stops, and that trailing
    /// progress must not resurrect it.
    pub fn on_progress(&mut self, progress: ExportProgress) {
        if self.is_finished() {
            return;
        }
        self.stage = ExportStage::Running(progress);
    }

    /// Records a successful run. Ignored once the session is finished.
    pub fn on_success(&mut self, summary: ExportSummary) {
        if self.is_finished() {
            return;
        }
        self.stage = ExportStage::Summary(summary);
    }

    /// Records a failed run with its localized message. Ignored once the session is finished — a
    /// cancelled run fails on the way out, and "Cancelled" is the truer thing to show.
    pub fn on_failure(&mut self, message: String) {
        if self.is_finished() {
            return;
        }
        self.stage = ExportStage::Error(message);
    }

    /// Cancels the session from any stage.
    pub fn cancel(&mut self) {
        self.stage = ExportStage::Cancelled;
    }
}

/// Where an export should write, as parsed from what the operator typed (ADR 0013's `ExportTarget`,
/// decided before the renderer is involved so it is unit-testable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportDestination {
    /// Write into this directory under the plugin's suggested file name.
    Directory(PathBuf),
    /// Write exactly this file, ignoring the plugin's suggested name.
    File(PathBuf),
}

impl ExportDestination {
    /// Parses the operator's destination `input` against `default_dir` (the workspace's `exports/`).
    ///
    /// An empty input is the default directory. A relative input resolves against `default_dir`; an
    /// absolute one is taken as typed. An input that ends in a separator, or whose last component is
    /// `.` or `..`, is a directory; anything else names a file. `.` and `..` components are resolved
    /// lexically, so nothing here touches the filesystem.
    #[must_use]
    pub fn parse(input: &str, default_dir: &Path) -> Self {
        let input = input.trim();
        if input.is_empty() {
            return Self::Directory(normalize(default_dir));
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

    /// The resolved path, for the live preview.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Directory(path) | Self::File(path) => path,
        }
    }

    /// Whether the plugin's suggested file name still decides the final file name.
    #[must_use]
    pub fn is_directory(&self) -> bool {
        match self {
            Self::Directory(_) => true,
            Self::File(_) => false,
        }
    }
}

/// Whether the operator's raw input names a directory: it ends in a path separator, or its last
/// component is `.` or `..`.
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
/// the root is dropped; one that climbs past a relative path's start is kept.
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{ExportDestination, ExportProgress, ExportSession, ExportStage, ExportSummary};

    fn progress(step: &str, processed: u32) -> ExportProgress {
        ExportProgress {
            step: step.to_owned(),
            processed,
            total: Some(120),
        }
    }

    fn summary() -> ExportSummary {
        ExportSummary {
            records: 120,
            destination: "/ws/exports/family.ged".to_owned(),
        }
    }

    #[test]
    fn a_new_session_starts_at_the_destination_stage() {
        let session = ExportSession::new();
        assert_eq!(*session.stage(), ExportStage::Destination);
        assert!(!session.is_finished());
    }

    #[test]
    fn a_run_moves_through_running_to_the_summary() {
        let mut session = ExportSession::new();

        session.start();
        assert_eq!(*session.stage(), ExportStage::Running(ExportProgress::default()));
        assert!(!session.is_finished());

        session.on_progress(progress("persons", 40));
        assert_eq!(*session.stage(), ExportStage::Running(progress("persons", 40)));

        // A later report replaces the earlier one rather than accumulating.
        session.on_progress(progress("families", 90));
        assert_eq!(*session.stage(), ExportStage::Running(progress("families", 90)));
        assert!(!session.is_finished());

        session.on_success(summary());
        assert_eq!(*session.stage(), ExportStage::Summary(summary()));
        assert!(session.is_finished());
    }

    #[test]
    fn a_failed_run_lands_in_the_error_stage() {
        let mut session = ExportSession::new();
        session.start();
        session.on_failure("plugin trapped: out of fuel".to_owned());
        assert_eq!(
            *session.stage(),
            ExportStage::Error("plugin trapped: out of fuel".to_owned())
        );
        assert!(session.is_finished());
    }

    #[test]
    fn cancel_from_any_stage_moves_to_cancelled() {
        let mut session = ExportSession::new();
        session.cancel();
        assert_eq!(*session.stage(), ExportStage::Cancelled);

        let mut session = ExportSession::new();
        session.start();
        session.on_progress(progress("persons", 12));
        session.cancel();
        assert_eq!(*session.stage(), ExportStage::Cancelled);
        assert!(session.is_finished());
    }

    #[test]
    fn a_cancelled_run_is_not_resurrected_by_its_trailing_reports() {
        // The guest only stops at its next progress report, so a cancelled run keeps reporting and
        // then fails on the way out. Neither may overwrite the operator's decision.
        let mut session = ExportSession::new();
        session.start();
        session.cancel();

        session.on_progress(progress("families", 77));
        assert_eq!(*session.stage(), ExportStage::Cancelled);

        session.on_failure("cancelled by the frontend".to_owned());
        assert_eq!(*session.stage(), ExportStage::Cancelled);

        session.on_success(summary());
        assert_eq!(*session.stage(), ExportStage::Cancelled);
    }

    #[test]
    fn every_terminal_stage_finishes_the_session() {
        let mut summary_session = ExportSession::new();
        summary_session.on_success(summary());
        assert!(summary_session.is_finished());

        let mut error_session = ExportSession::new();
        error_session.on_failure("boom".to_owned());
        assert!(error_session.is_finished());

        let mut cancelled_session = ExportSession::new();
        cancelled_session.cancel();
        assert!(cancelled_session.is_finished());
    }

    fn default_dir() -> PathBuf {
        PathBuf::from("/ws/exports")
    }

    #[test]
    fn an_empty_destination_is_the_default_directory() {
        let destination = ExportDestination::parse("", &default_dir());
        assert_eq!(destination, ExportDestination::Directory(default_dir()));
        assert!(destination.is_directory());
        assert_eq!(destination.path(), Path::new("/ws/exports"));

        // Whitespace-only input is the same as empty.
        assert_eq!(
            ExportDestination::parse("   ", &default_dir()),
            ExportDestination::Directory(default_dir())
        );
    }

    #[test]
    fn a_trailing_separator_names_a_directory() {
        assert_eq!(
            ExportDestination::parse("backups/", &default_dir()),
            ExportDestination::Directory(PathBuf::from("/ws/exports/backups"))
        );
        assert_eq!(
            ExportDestination::parse("/srv/archive/", &default_dir()),
            ExportDestination::Directory(PathBuf::from("/srv/archive"))
        );
    }

    #[test]
    fn a_file_name_names_a_file() {
        let destination = ExportDestination::parse("family.ged", &default_dir());
        assert_eq!(
            destination,
            ExportDestination::File(PathBuf::from("/ws/exports/family.ged"))
        );
        assert!(!destination.is_directory());
    }

    #[test]
    fn a_relative_path_resolves_against_the_default_directory() {
        assert_eq!(
            ExportDestination::parse("2026/family.ged", &default_dir()),
            ExportDestination::File(PathBuf::from("/ws/exports/2026/family.ged"))
        );
    }

    #[test]
    fn an_absolute_path_is_taken_as_typed() {
        assert_eq!(
            ExportDestination::parse("/srv/archive/family.ged", &default_dir()),
            ExportDestination::File(PathBuf::from("/srv/archive/family.ged"))
        );
    }

    #[test]
    fn dot_and_dot_dot_components_resolve_lexically() {
        // `..` climbs out of the default directory — the operator typed it, so it is honoured.
        assert_eq!(
            ExportDestination::parse("../family.ged", &default_dir()),
            ExportDestination::File(PathBuf::from("/ws/family.ged"))
        );
        assert_eq!(
            ExportDestination::parse("./2026/./family.ged", &default_dir()),
            ExportDestination::File(PathBuf::from("/ws/exports/2026/family.ged"))
        );
        // A trailing `..` (or `.`) is a directory, not a file called "..".
        assert_eq!(
            ExportDestination::parse("..", &default_dir()),
            ExportDestination::Directory(PathBuf::from("/ws"))
        );
        assert_eq!(
            ExportDestination::parse(".", &default_dir()),
            ExportDestination::Directory(default_dir())
        );
        // Climbing past the root cannot escape it.
        assert_eq!(
            ExportDestination::parse("/../../family.ged", &default_dir()),
            ExportDestination::File(PathBuf::from("/family.ged"))
        );
    }

    #[test]
    fn the_root_directory_has_no_file_name_so_it_is_a_directory() {
        assert_eq!(
            ExportDestination::parse("/", &default_dir()),
            ExportDestination::Directory(PathBuf::from("/"))
        );
    }
}
