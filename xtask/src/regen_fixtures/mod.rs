//! `cargo xtask regen-fixtures` — rebuild the bundled Digitalarkivet fixtures from the fetched pages
//! (ADR 0042 §4).
//!
//! `fetch-fixtures` saves the real pages the external manifest lists under
//! `target/external-fixtures/digitalarkivet/`. A real page names more people than the one a test is
//! about, so this command fails closed, in three steps per page:
//!
//! 1. **Prune.** Keep only the elements `vitni-digitalarkivet`'s selectors reach
//!    ([`vitni_digitalarkivet::html::PAGE_ELEMENTS`]), with their subtrees and ancestors. Scripts,
//!    styles, comments, the logo and the site chrome fall away, and so does every attribute the parser
//!    does not read; a `class`, `id` or `property` keeps only the names a selector uses.
//! 2. **Substitute.** Replace every remaining text and `href`/`src`/`content`/`value` through the
//!    page's `[page.substitute]` table (real → invented), or keep it when the manifest's `vocabulary`
//!    lists it as naming no one. A value in neither is an error naming it.
//! 3. **Check.** Fail if any real value from any page's table appears anywhere in any output.
//!
//! Every page is regenerated before any file is written, so a failure leaves the tree unchanged. The
//! output directory's `PROVENANCE.toml` records the files as `origin = "generated"`.

mod manifest;
mod page;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::fetch_fixtures::{MANIFEST, OUT_DIR, output_path};
use crate::regen_fixtures::manifest::{Manifest, load_manifest};
use crate::regen_fixtures::page::{Pruner, escape_attribute, escape_text};

/// The bundled fixture tree, relative to the repository root.
const FIXTURE_DIR: &str = "crates/vitni-digitalarkivet/tests/fixtures";

/// Regenerates every manifest page's fixture from the repository root.
pub fn run() -> Result<()> {
    let text = fs::read_to_string(MANIFEST).with_context(|| format!("reading {MANIFEST}"))?;
    let manifest = load_manifest(&text).with_context(|| format!("parsing {MANIFEST}"))?;
    let pruner = Pruner::new()?;
    let mut outputs = Vec::new();
    let mut problems = Vec::new();
    for page in &manifest.page {
        let fetched = output_path(Path::new(OUT_DIR), &page.id);
        let Ok(html) = fs::read_to_string(&fetched) else {
            problems.push(format!(
                "{}: {} is missing; run `cargo xtask fetch-fixtures` first",
                page.id,
                fetched.display()
            ));
            continue;
        };
        match pruner.regenerate(page, &html, &manifest.vocabulary) {
            Ok(output) => outputs.push((page.id.clone(), output)),
            Err(unmapped) => problems.push(unmapped.to_string()),
        }
    }
    for (id, real) in surviving_values(&manifest, &outputs) {
        problems.push(format!("{id}: the real value {real:?} survives into the output"));
    }
    if !problems.is_empty() {
        bail!("regen-fixtures wrote nothing:\n{}", problems.join("\n"));
    }
    for (page, (_, output)) in manifest.page.iter().zip(&outputs) {
        let path = fixture_path(&page.fixture)?;
        fs::write(&path, output).with_context(|| format!("writing {}", path.display()))?;
        println!("regen-fixtures: {} ← {}", path.display(), page.id);
    }
    println!("regen-fixtures: {} fixture(s) regenerated", outputs.len());
    Ok(())
}

/// The bundled fixture at `fixture`, a `/`-separated `.html` path inside [`FIXTURE_DIR`].
///
/// # Errors
/// Returns an error when `fixture` is absolute, has an empty, `.` or `..` segment, or is not `.html`.
pub fn fixture_path(fixture: &str) -> Result<PathBuf> {
    let inside = !fixture.starts_with('/')
        && fixture
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    let is_html = Path::new(fixture)
        .extension()
        .is_some_and(|extension| extension == "html");
    if !inside || !is_html {
        bail!("fixture {fixture:?} must be a relative `.html` path inside {FIXTURE_DIR}");
    }
    Ok(Path::new(FIXTURE_DIR).join(fixture))
}

/// Every `(output id, real value)` where a real value from any page's substitutions appears in that
/// output, whether as written or as HTML-escaped.
pub fn surviving_values(manifest: &Manifest, outputs: &[(String, String)]) -> Vec<(String, String)> {
    let mut survivors = Vec::new();
    for (id, output) in outputs {
        for page in &manifest.page {
            for real in page.substitute.keys() {
                let found = output.contains(real.as_str())
                    || output.contains(&escape_text(real))
                    || output.contains(&escape_attribute(real));
                if found {
                    survivors.push((id.clone(), real.clone()));
                }
            }
        }
    }
    survivors
}

#[cfg(test)]
mod tests;
