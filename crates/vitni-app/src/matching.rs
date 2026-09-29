//! Loading the record-matching data (ADR 0038 §5): the shipped name-culture packs and region table,
//! with the shared and workspace overrides layered over them.
//!
//! The layers mirror the Fluent catalogues (ADR 0003), highest precedence first: the workspace's
//! `matching/` directory, the shared application directory ([`crate::config::shared_matching_dir`]),
//! then the packs embedded in `vitni-core`. Each layer may hold `cultures/<id>.toml` — replacing the
//! pack of that id, or adding one — and a `regions.toml` replacing the region table. An absent layer
//! contributes nothing; a file that cannot be read or parsed fails with its path.

use std::io;
use std::path::{Path, PathBuf};

use vitni_core::matching::MatchData;
use vitni_core::matching::pack::{PackError, PackSource};

/// The directory of culture packs within a matching layer.
const CULTURES_DIR: &str = "cultures";

/// The region table within a matching layer.
const REGIONS_FILE: &str = "regions.toml";

/// Why the matching data could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum MatchDataError {
    /// A layer's directory or file could not be read.
    #[error("could not read {path}: {source}")]
    Read {
        /// The directory or file.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// A pack or region file was rejected.
    #[error(transparent)]
    Pack(#[from] PackError),
}

/// The matching data with every override layered in.
///
/// `workspace_dir` is a workspace **root** (its `matching/` subdirectory is the layer); `shared_dir`
/// already points at a matching directory.
pub fn load_match_data(workspace_dir: Option<&Path>, shared_dir: Option<&Path>) -> Result<MatchData, MatchDataError> {
    let mut data = MatchData::embedded()?;
    let workspace_layer = workspace_dir.map(|dir| dir.join("matching"));
    for layer in [shared_dir.map(Path::to_path_buf), workspace_layer]
        .into_iter()
        .flatten()
    {
        if !layer.is_dir() {
            continue;
        }
        let packs = read_packs(&layer.join(CULTURES_DIR))?;
        let regions = read_optional(&layer.join(REGIONS_FILE))?;
        data = data.layered(packs, regions.as_ref())?;
    }
    Ok(data)
}

/// Every `*.toml` file in `dir`, in file-name order; none when `dir` is absent.
fn read_packs(dir: &Path) -> Result<Vec<PackSource>, MatchDataError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let read_error = |source| MatchDataError::Read {
        path: dir.to_path_buf(),
        source,
    };
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(read_error)? {
        let path = entry.map_err(read_error)?.path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "toml") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        sources.push(read_source(&path)?);
    }
    Ok(sources)
}

/// The file at `path`, or `None` when it does not exist.
fn read_optional(path: &Path) -> Result<Option<PackSource>, MatchDataError> {
    if path.is_file() {
        read_source(path).map(Some)
    } else {
        Ok(None)
    }
}

/// The file at `path` as a source named by its path.
fn read_source(path: &Path) -> Result<PackSource, MatchDataError> {
    let text = std::fs::read_to_string(path).map_err(|source| MatchDataError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(PackSource::new(path.display().to_string(), text))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use vitni_core::enums::Sex;
    use vitni_core::matching::profile::PersonProfile;
    use vitni_core::matching::{CultureId, MatchData, MatchSettings, assess_persons};
    use vitni_core::name::{LanguageTag, NameType, PersonName};

    use super::{MatchDataError, load_match_data};

    const TOY: &str = include_str!("../../vitni-core/tests/fixtures/matching/toy.toml");

    fn speaker(given: &str) -> PersonProfile {
        let name = PersonName {
            name_type: NameType::BirthName,
            given: Some(given.to_owned()),
            surnames: Vec::new(),
            suffix: None,
            title: None,
            nickname: None,
            call_name: None,
            date: None,
            language: Some(LanguageTag::new("tq")),
            transliterations: Vec::new(),
        };
        PersonProfile {
            names: vec![name],
            sex: Some(Sex::Male),
            ..PersonProfile::default()
        }
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn score(data: &MatchData) -> f64 {
        assess_persons(&speaker("Zorbo"), &speaker("Quimble"), data, &MatchSettings::default()).score
    }

    #[test]
    fn with_no_override_the_embedded_data_loads() {
        let workspace = tempfile::tempdir().unwrap();
        let data = load_match_data(Some(workspace.path()), None).unwrap();
        assert_eq!(data, MatchData::embedded().unwrap());
    }

    #[test]
    fn a_workspace_pack_changes_a_score_with_no_code_change() {
        let workspace = tempfile::tempdir().unwrap();
        let before = score(&load_match_data(Some(workspace.path()), None).unwrap());
        write(&workspace.path().join("matching/cultures/toy.toml"), TOY);
        let data = load_match_data(Some(workspace.path()), None).unwrap();
        assert!(data.packs.get(&CultureId::new("toy")).is_some());
        assert!(score(&data) > before);
    }

    #[test]
    fn the_workspace_layer_wins_over_the_shared_layer() {
        let (workspace, shared) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write(&shared.path().join("cultures/toy.toml"), TOY);
        let with_shared = load_match_data(Some(workspace.path()), Some(shared.path())).unwrap();
        assert!(with_shared.packs.get(&CultureId::new("toy")).is_some());
        write(&workspace.path().join("matching/cultures/toy.toml"), "id = \"toy\"\n");
        let overridden = load_match_data(Some(workspace.path()), Some(shared.path())).unwrap();
        assert!(
            score(&overridden) < score(&with_shared),
            "the workspace's empty toy pack replaced the shared one"
        );
    }

    #[test]
    fn a_workspace_region_table_replaces_the_shipped_one() {
        let workspace = tempfile::tempdir().unwrap();
        write(&workspace.path().join("matching/regions.toml"), "");
        let data = load_match_data(Some(workspace.path()), None).unwrap();
        assert_ne!(data.regions, MatchData::embedded().unwrap().regions);
    }

    #[test]
    fn a_malformed_pack_fails_with_its_path() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("matching/cultures/xx.toml");
        write(&path, "id = ");
        let error = load_match_data(Some(workspace.path()), None).unwrap_err();
        assert!(matches!(error, MatchDataError::Pack(_)), "{error:?}");
        assert!(error.to_string().contains(&path.display().to_string()), "{error}");
    }

    #[test]
    fn a_non_toml_file_is_ignored() {
        let workspace = tempfile::tempdir().unwrap();
        write(&workspace.path().join("matching/cultures/README.md"), "not a pack");
        assert!(load_match_data(Some(workspace.path()), None).is_ok());
    }
}
