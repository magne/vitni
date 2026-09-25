//! API keys named by an environment variable (#296): `[map.providers.*]`'s `api-key-env` (ADR 0033)
//! and `[ai.providers.*]`'s `api_key_env` (ADR 0017 §4). Config names the variable, never the secret;
//! this module resolves the name at use time from the process environment, then
//! `<workspace>/.env`, then `~/.config/vitni/.env` — so a GUI started from a desktop launcher, which
//! inherits no shell profile, can still reach a keyed provider (ADR 0036).
//!
//! The files are parsed, never loaded into the process environment: `std::env::set_var` is unsound
//! once other threads run, and a workspace switch would leave the previous workspace's keys behind.
//! No value read here is ever logged or echoed in an error.

use std::path::{Path, PathBuf};

use crate::config::global_env_path;
use crate::error::AppError;

/// The file name looked up in the workspace directory and the global config directory.
pub const ENV_FILE_NAME: &str = ".env";

/// Resolves the secret held by the environment variable `name`: the process environment first, then
/// `<workspace_dir>/.env`, then the global `~/.config/vitni/.env`. An empty value counts as unset.
///
/// # Errors
///
/// [`AppError::Config`] naming `name` and both files if no source defines it; naming only the file
/// if a `.env` file that is consulted is malformed or unreadable (never its content); or if no home
/// directory can be determined for the global file.
pub fn require_secret_env(name: &str, workspace_dir: &Path) -> Result<String, AppError> {
    resolve(name, std::env::var(name).ok(), || {
        Ok(vec![workspace_dir.join(ENV_FILE_NAME), global_env_path()?])
    })
}

/// [`require_secret_env`] over an explicit environment value and file list — the pure core, so the
/// precedence is testable without mutating the process environment. `files` is only called once the
/// environment has no value, so a set variable never depends on finding a home directory.
fn resolve(
    name: &str,
    env_value: Option<String>,
    files: impl FnOnce() -> Result<Vec<PathBuf>, AppError>,
) -> Result<String, AppError> {
    if let Some(value) = env_value.filter(|value| !value.is_empty()) {
        return Ok(value);
    }
    let files = files()?;
    for file in &files {
        if let Some(value) = read_from_file(name, file)? {
            return Ok(value);
        }
    }
    let checked: Vec<String> = files.iter().map(|file| file.display().to_string()).collect();
    Err(AppError::Config(format!(
        "the {name:?} variable is set neither in the environment nor in {}",
        checked.join(" or ")
    )))
}

/// The first non-empty value `file` gives `name`, or `None` if the file is absent or never defines it.
/// The whole file is parsed even after a match, so a malformed line anywhere fails it.
///
/// # Errors
///
/// [`AppError::Config`] naming `file` if it cannot be read or is not valid `.env` syntax. The parser's
/// own error carries the offending line, which may be a secret, so it is dropped rather than wrapped.
fn read_from_file(name: &str, file: &Path) -> Result<Option<String>, AppError> {
    let entries = match dotenvy::from_path_iter(file) {
        Ok(entries) => entries,
        Err(error) if error.not_found() => return Ok(None),
        Err(_) => return Err(AppError::Config(format!("{} cannot be read", file.display()))),
    };
    let mut found = None;
    for entry in entries {
        let Ok((key, value)) = entry else {
            return Err(AppError::Config(format!(
                "{} is not valid .env syntax (KEY=value lines)",
                file.display()
            )));
        };
        if found.is_none() && key == name && !value.is_empty() {
            found = Some(value);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = "VITNI_TEST_SECRET";

    fn write(dir: &Path, file: &str, content: &str) -> PathBuf {
        let path = dir.join(file);
        std::fs::write(&path, content).expect("writing a test .env file");
        path
    }

    fn resolved(env_value: Option<&str>, files: &[PathBuf]) -> String {
        resolve(NAME, env_value.map(str::to_owned), || Ok(files.to_vec())).expect("the secret resolves")
    }

    fn resolve_error(files: &[PathBuf]) -> AppError {
        resolve(NAME, None, || Ok(files.to_vec())).expect_err("the secret does not resolve")
    }

    #[test]
    fn a_set_environment_value_needs_no_fallback_paths() {
        let no_home = || Err(AppError::Config("no valid home directory".to_owned()));
        let value = resolve(NAME, Some("from-shell".to_owned()), no_home).expect("the environment suffices");
        assert_eq!(value, "from-shell");
    }

    #[test]
    fn the_environment_wins_over_both_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(dir.path(), "ws.env", "VITNI_TEST_SECRET=from-workspace\n");
        let global = write(dir.path(), "global.env", "VITNI_TEST_SECRET=from-global\n");
        assert_eq!(resolved(Some("from-shell"), &[workspace, global]), "from-shell");
    }

    #[test]
    fn the_workspace_file_wins_over_the_global_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(dir.path(), "ws.env", "VITNI_TEST_SECRET=from-workspace\n");
        let global = write(dir.path(), "global.env", "VITNI_TEST_SECRET=from-global\n");
        assert_eq!(resolved(None, &[workspace, global]), "from-workspace");
    }

    #[test]
    fn a_missing_workspace_file_falls_through_to_the_global_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let global = write(dir.path(), "global.env", "VITNI_TEST_SECRET=from-global\n");
        assert_eq!(resolved(None, &[dir.path().join("absent.env"), global]), "from-global");
    }

    #[test]
    fn a_file_without_the_name_falls_through_to_the_next() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(dir.path(), "ws.env", "OTHER_KEY=unrelated\n");
        let global = write(dir.path(), "global.env", "VITNI_TEST_SECRET=from-global\n");
        assert_eq!(resolved(None, &[workspace, global]), "from-global");
    }

    #[test]
    fn an_empty_value_counts_as_unset() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(dir.path(), "ws.env", "VITNI_TEST_SECRET=\n");
        let global = write(dir.path(), "global.env", "VITNI_TEST_SECRET=from-global\n");
        assert_eq!(resolved(Some(""), &[workspace, global]), "from-global");
    }

    #[test]
    fn quoting_comments_and_export_follow_dotenv_syntax() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(
            dir.path(),
            "ws.env",
            "# the map key\nexport VITNI_TEST_SECRET=\"a b#c\"\n",
        );
        assert_eq!(resolved(None, &[workspace]), "a b#c");
    }

    #[test]
    fn unset_everywhere_names_the_variable_and_every_file_checked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = [dir.path().join("ws.env"), dir.path().join("global.env")];
        let error = resolve_error(&files);
        let message = error.to_string();
        assert!(message.contains(NAME), "{message}");
        for file in &files {
            assert!(message.contains(&file.display().to_string()), "{message}");
        }
    }

    #[test]
    fn a_malformed_file_names_its_path_but_never_its_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(dir.path(), "ws.env", "VITNI_TEST_SECRET='hunter2-unterminated\n");
        let global = write(dir.path(), "global.env", "VITNI_TEST_SECRET=from-global\n");
        let error = resolve_error(&[workspace.clone(), global]);
        let message = error.to_string();
        assert!(message.contains(&workspace.display().to_string()), "{message}");
        assert!(
            !message.contains("hunter2"),
            "the error must not echo file content: {message}"
        );
    }

    #[test]
    fn a_malformed_line_after_the_match_still_fails_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(
            dir.path(),
            "ws.env",
            "VITNI_TEST_SECRET=from-workspace\nOTHER='unterminated\n",
        );
        let error = resolve_error(std::slice::from_ref(&workspace));
        assert!(error.to_string().contains(&workspace.display().to_string()), "{error}");
    }

    #[test]
    fn a_set_environment_value_never_reads_a_malformed_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = write(dir.path(), "ws.env", "VITNI_TEST_SECRET='unterminated\n");
        assert_eq!(resolved(Some("from-shell"), &[workspace]), "from-shell");
    }
}
