//! `cargo xtask fetch-fixtures` — download the external Digitalarkivet fixtures (ADR 0042 §3).
//!
//! The manifest lists real pages the project may not redistribute. This command fetches each into the
//! gitignored `target/external-fixtures/digitalarkivet/<id>.html`, where the opt-in tests in
//! `crates/vitni-digitalarkivet/tests/external.rs` read them. It runs `curl` with an honest,
//! non-crawler user agent and waits five seconds between requests, as the archive's `robots.txt`
//! asks (`Crawl-delay: 5`). It needs the network, so it is not part of `cargo xtask check`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// The manifest, relative to the repository root.
const MANIFEST: &str = "crates/vitni-digitalarkivet/tests/external/manifest.toml";

/// Where the fetched pages land, relative to the repository root.
const OUT_DIR: &str = "target/external-fixtures/digitalarkivet";

/// The archive's `robots.txt` `Crawl-delay`.
const CRAWL_DELAY: Duration = Duration::from_secs(5);

/// Names this tool honestly, as the Digitalarkivet research doc asks of any client.
const USER_AGENT: &str = "vitni-fixture-fetch (+https://github.com/magne/vitni)";

/// The part of a manifest entry the fetch needs. The rights and expected facts are the tests' concern.
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct PageRef {
    pub id: String,
    pub url: String,
}

#[derive(Deserialize)]
struct Manifest {
    page: Vec<PageRef>,
}

/// Fetches every manifest page from the repository root.
pub fn run() -> Result<()> {
    let text = fs::read_to_string(MANIFEST).with_context(|| format!("reading {MANIFEST}"))?;
    let pages = load_pages(&text).with_context(|| format!("parsing {MANIFEST}"))?;
    fs::create_dir_all(OUT_DIR).with_context(|| format!("creating {OUT_DIR}"))?;
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            thread::sleep(CRAWL_DELAY);
        }
        let out = output_path(Path::new(OUT_DIR), &page.id);
        // A page left over from an earlier run must not pass for this run's when this fetch fails.
        if out.exists() {
            fs::remove_file(&out).with_context(|| format!("removing the stale {}", out.display()))?;
        }
        println!("fetch-fixtures: {} ← {}", out.display(), page.url);
        let status = Command::new("curl")
            .args(curl_args(&page.url, &out))
            .status()
            .context("running curl (is it installed?)")?;
        if !status.success() {
            bail!("fetching {} ({}) failed: curl {status}", page.id, page.url);
        }
    }
    println!(
        "fetch-fixtures: {} page(s) in {OUT_DIR}; now run `cargo nextest run -p vitni-digitalarkivet --run-ignored only`",
        pages.len()
    );
    Ok(())
}

/// The manifest's pages, each with an id safe to use as a file name and an https URL.
///
/// # Errors
/// Returns an error when the manifest does not parse, or an entry's id or URL is unusable.
pub fn load_pages(text: &str) -> Result<Vec<PageRef>> {
    let manifest: Manifest = toml::from_str(text)?;
    for page in &manifest.page {
        let id_is_safe = !page.id.is_empty()
            && page
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_is_safe {
            bail!(
                "id {:?} must match [a-z0-9-]+: it names a file under {OUT_DIR}",
                page.id
            );
        }
        if !page.url.starts_with("https://") {
            bail!("{}: {:?} is not an https URL", page.id, page.url);
        }
    }
    Ok(manifest.page)
}

/// The file the page with `id` is saved to.
pub fn output_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.html"))
}

/// `curl`'s arguments for fetching `url` into `out`: fail on an HTTP error, leave no partial file
/// behind on any error, follow redirects (the church-book viewer redirects to its new host), and give
/// up after 30 seconds.
pub fn curl_args(url: &str, out: &Path) -> Vec<String> {
    vec![
        "--fail".to_owned(),
        "--silent".to_owned(),
        "--show-error".to_owned(),
        "--remove-on-error".to_owned(),
        "--location".to_owned(),
        "--max-time".to_owned(),
        "30".to_owned(),
        "--user-agent".to_owned(),
        USER_AGENT.to_owned(),
        "--output".to_owned(),
        out.display().to_string(),
        url.to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{PageRef, curl_args, load_pages, output_path};

    const TWO_PAGES: &str = r#"
        [[page]]
        id = "census-person"
        kind = "census-person"
        url = "https://www.digitalarkivet.no/census/person/pf1"
        [page.rights]
        holder = "Nasjonalarkivet"
        [page.expected]
        name = "Ola"

        [[page]]
        id = "census-viewer"
        kind = "viewer"
        url = "https://media.digitalarkivet.no/fs1"
    "#;

    #[test]
    fn pages_keep_their_ids_and_urls_in_order_and_ignore_the_rest() {
        let pages = load_pages(TWO_PAGES).unwrap();
        assert_eq!(
            pages,
            vec![
                PageRef {
                    id: "census-person".to_owned(),
                    url: "https://www.digitalarkivet.no/census/person/pf1".to_owned(),
                },
                PageRef {
                    id: "census-viewer".to_owned(),
                    url: "https://media.digitalarkivet.no/fs1".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn an_id_that_could_escape_the_output_directory_is_rejected() {
        for id in ["../escape", "a/b", "Upper", "", "with space"] {
            let text = format!("[[page]]\nid = {id:?}\nurl = \"https://www.digitalarkivet.no/x\"\n");
            let error = load_pages(&text).unwrap_err();
            assert!(error.to_string().contains("[a-z0-9-]+"), "{id:?}: {error}");
        }
    }

    #[test]
    fn a_plain_http_url_is_rejected() {
        let text = "[[page]]\nid = \"p\"\nurl = \"http://www.digitalarkivet.no/x\"\n";
        let error = load_pages(text).unwrap_err();
        assert!(error.to_string().contains("not an https URL"), "{error}");
    }

    #[test]
    fn the_real_manifest_loads() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(super::MANIFEST);
        let pages = load_pages(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(!pages.is_empty());
    }

    #[test]
    fn a_page_is_saved_as_its_id_under_the_output_directory() {
        assert_eq!(
            output_path(Path::new("target/x"), "census-person"),
            Path::new("target/x/census-person.html")
        );
    }

    #[test]
    fn curl_fails_on_http_errors_leaves_no_partial_file_follows_redirects_and_names_itself() {
        let args = curl_args("https://goto.digitalarkivet.no/kb1", Path::new("out.html"));
        for flag in ["--fail", "--remove-on-error", "--location", "--max-time"] {
            assert!(args.iter().any(|a| a == flag), "missing {flag}: {args:?}");
        }
        let agent = args.iter().position(|a| a == "--user-agent").unwrap();
        assert!(args[agent + 1].starts_with("vitni-fixture-fetch"), "{args:?}");
        let output = args.iter().position(|a| a == "--output").unwrap();
        assert_eq!(args[output + 1], "out.html");
        assert_eq!(
            args.last().map(String::as_str),
            Some("https://goto.digitalarkivet.no/kb1")
        );
    }
}
