//! The part of the external-fixture manifest the regeneration reads.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde::Deserialize;

use crate::regen_fixtures::fixture_path;

/// The manifest's shared vocabulary and its pages. The rights, URLs and expected facts are the
/// fetch's and the external tests' concern, so they are ignored here.
#[derive(Deserialize, Debug)]
pub struct Manifest {
    /// Values kept verbatim on every page: site labels and codes that name no one.
    #[serde(default)]
    pub vocabulary: BTreeSet<String>,
    pub page: Vec<Page>,
}

/// One external page and the bundled fixture generated from it.
#[derive(Deserialize, Debug)]
pub struct Page {
    pub id: String,
    /// The fixture's path under `crates/vitni-digitalarkivet/tests/fixtures/`.
    pub fixture: String,
    /// Real value → invented value, matched against whole whitespace-normalized texts and values.
    #[serde(default)]
    pub substitute: BTreeMap<String, String>,
}

/// The manifest, with every fixture path usable and every substitution a real change.
///
/// # Errors
/// Returns an error when the manifest does not parse, a fixture path leaves the fixture tree, or a
/// substitution is blank, keeps its real value, or substitutes a value the vocabulary keeps.
pub fn load_manifest(text: &str) -> Result<Manifest> {
    let manifest: Manifest = toml::from_str(text)?;
    for page in &manifest.page {
        fixture_path(&page.fixture)?;
        for (real, invented) in &page.substitute {
            if real.trim().is_empty() {
                bail!("{}: a substitution has a blank real value", page.id);
            }
            if real == invented {
                bail!(
                    "{}: the substitution for {real:?} keeps the real value; a value that names no one \
                     belongs in `vocabulary` instead",
                    page.id
                );
            }
            if manifest.vocabulary.contains(real) {
                bail!(
                    "{real:?} is substituted on page {} and kept verbatim by `vocabulary`; it cannot be both",
                    page.id
                );
            }
        }
    }
    Ok(manifest)
}
