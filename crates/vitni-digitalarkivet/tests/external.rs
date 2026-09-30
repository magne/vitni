//! External Digitalarkivet fixtures (ADR 0042 §3): the live pages the parser is checked against.
//!
//! `tests/external/manifest.toml` lists real pages the project may not redistribute, with the rights
//! their source states and the facts each must parse to. `cargo xtask fetch-fixtures` downloads them
//! into the gitignored `target/external-fixtures/digitalarkivet/`, and the one `#[ignore]` test here
//! checks every page against its expected facts:
//!
//! ```text
//! cargo xtask fetch-fixtures
//! cargo nextest run -p vitni-digitalarkivet --run-ignored only
//! ```
//!
//! It is a manual drift check before a release, not a CI job. The other tests run everywhere and check
//! the manifest and the checking logic itself, over the bundled invented pages.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use vitni_digitalarkivet::{PageKind, classify_url, parse_person_page, parse_residence_page, parse_viewer_page};

const MANIFEST: &str = include_str!("external/manifest.toml");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    page: Vec<Page>,
}

/// One external page: where it lives, what it is, who holds it, and what it must parse to.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    id: String,
    kind: Kind,
    url: String,
    rights: Rights,
    #[serde(default)]
    expected: Expected,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    CensusPerson,
    CensusResidence,
    ChurchbookPerson,
    Viewer,
}

/// The rights as the source page states them. `redistributable` is always `false`: a page the
/// project could redistribute would be a bundled `licensed` fixture instead.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rights {
    source: String,
    holder: String,
    transcriber: String,
    licence: String,
    redistributable: bool,
}

/// The facts a page must parse to. Only the ones given are checked.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Expected {
    name: Option<String>,
    birth: Option<String>,
    role: Option<String>,
    occupation: Option<String>,
    scan_viewer_url: Option<String>,
    household: Option<usize>,
    source_title: Option<String>,
    person_links: Option<usize>,
    image: Option<String>,
    no_image: Option<bool>,
}

/// Every expected fact `html` does not parse to, one message each. Empty when the page holds up.
fn check(page: &Page, html: &str) -> Vec<String> {
    let mut mismatches = Vec::new();
    let id = page.id.as_str();
    let expected = &page.expected;
    match page.kind {
        Kind::CensusPerson | Kind::ChurchbookPerson => match parse_person_page(html, &page.url) {
            Ok(record) => {
                compare(
                    &mut mismatches,
                    id,
                    "name",
                    expected.name.as_deref(),
                    Some(record.name.as_str()),
                );
                compare(
                    &mut mismatches,
                    id,
                    "birth",
                    expected.birth.as_deref(),
                    record.birth.as_deref(),
                );
                compare(
                    &mut mismatches,
                    id,
                    "role",
                    expected.role.as_deref(),
                    record.role.as_deref(),
                );
                compare(
                    &mut mismatches,
                    id,
                    "occupation",
                    expected.occupation.as_deref(),
                    record.occupation.as_deref(),
                );
                compare(
                    &mut mismatches,
                    id,
                    "scan viewer",
                    expected.scan_viewer_url.as_deref(),
                    record.scan_viewer_url.as_deref(),
                );
                compare(
                    &mut mismatches,
                    id,
                    "household",
                    expected.household.as_ref(),
                    Some(&record.household.len()),
                );
                compare(
                    &mut mismatches,
                    id,
                    "source title",
                    expected.source_title.as_deref(),
                    record.source.title.as_deref(),
                );
            }
            Err(error) => mismatches.push(format!("{id}: does not parse: {error}")),
        },
        Kind::CensusResidence => match parse_residence_page(html, &page.url) {
            Ok(record) => {
                compare(
                    &mut mismatches,
                    id,
                    "person links",
                    expected.person_links.as_ref(),
                    Some(&record.person_links.len()),
                );
                compare(
                    &mut mismatches,
                    id,
                    "source title",
                    expected.source_title.as_deref(),
                    record.source.title.as_deref(),
                );
            }
            Err(error) => mismatches.push(format!("{id}: does not parse: {error}")),
        },
        Kind::Viewer => check_viewer(&mut mismatches, page, html),
    }
    mismatches
}

/// Pushes a mismatch for each expected viewer fact `html` does not hold.
fn check_viewer(mismatches: &mut Vec<String>, page: &Page, html: &str) {
    let id = page.id.as_str();
    let expected = &page.expected;
    let image = match parse_viewer_page(html, &page.url) {
        Ok(image) => Some(image),
        Err(error) if expected.image.is_some() => {
            mismatches.push(format!("{id}: does not parse: {error}"));
            return;
        }
        Err(_) => None,
    };
    compare(mismatches, id, "image", expected.image.as_deref(), image.as_deref());
    if expected.no_image == Some(true)
        && let Some(image) = image
    {
        mismatches.push(format!("{id}: image: expected none, parsed {image:?}"));
    }
}

fn manifest() -> Result<Manifest, toml::de::Error> {
    toml::from_str(MANIFEST)
}

/// Where `cargo xtask fetch-fixtures` saves the page with `id`.
fn fetched_path(id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/external-fixtures/digitalarkivet")
        .join(format!("{id}.html"))
}

/// A census person page over the bundled invented fixture, expecting what it holds.
fn invented_census_person() -> Page {
    Page {
        id: "invented-census-person".to_owned(),
        kind: Kind::CensusPerson,
        url: "https://www.digitalarkivet.no/census/person/pf01099901000101".to_owned(),
        rights: Rights {
            source: "invented".to_owned(),
            holder: "vitni".to_owned(),
            transcriber: "vitni".to_owned(),
            licence: "invented".to_owned(),
            redistributable: false,
        },
        expected: Expected {
            name: Some("Ola Eksempelsen Fjellstue".to_owned()),
            birth: Some("1886-07-08".to_owned()),
            role: Some("hp".to_owned()),
            occupation: Some("Gårdbruker S.".to_owned()),
            scan_viewer_url: Some("https://media.digitalarkivet.no/fs10000099901042".to_owned()),
            household: Some(4),
            source_title: Some("Folketelling 1920 for 9901 Eksempelvik herred".to_owned()),
            ..Expected::default()
        },
    }
}

const INVENTED_CENSUS_PERSON: &str = include_str!("fixtures/census/person.html");

#[test]
fn manifest_is_well_formed() {
    let manifest = manifest().expect("the manifest parses");
    assert!(!manifest.page.is_empty(), "the manifest lists no pages");
    let mut ids = BTreeSet::new();
    for page in &manifest.page {
        assert!(ids.insert(page.id.as_str()), "duplicate id {}", page.id);
        assert!(
            !page.id.is_empty()
                && page
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "id {:?} must match [a-z0-9-]+: it names a file under target/",
            page.id
        );
        assert!(
            page.url.starts_with("https://") && page.url.contains(".digitalarkivet.no/"),
            "{}: {} is not an https Digitalarkivet URL",
            page.id,
            page.url
        );
        let rights = &page.rights;
        for (what, value) in [
            ("source", &rights.source),
            ("holder", &rights.holder),
            ("transcriber", &rights.transcriber),
            ("licence", &rights.licence),
        ] {
            assert!(!value.trim().is_empty(), "{}: rights.{what} is empty", page.id);
        }
        assert!(
            !rights.redistributable,
            "{}: a redistributable page belongs in tests/fixtures/ as `licensed` (ADR 0042 §1)",
            page.id
        );
        assert!(
            !check_nothing(&page.expected),
            "{}: no expected facts, so the page checks nothing",
            page.id
        );
    }
}

#[test]
fn check_passes_when_every_expected_fact_holds() {
    assert_eq!(
        check(&invented_census_person(), INVENTED_CENSUS_PERSON),
        Vec::<String>::new()
    );
}

#[test]
fn check_reports_a_wrong_expected_fact() {
    let mut page = invented_census_person();
    page.expected.name = Some("Kari Eksempelsen".to_owned());
    assert_eq!(
        check(&page, INVENTED_CENSUS_PERSON),
        vec![
            r#"invented-census-person: name: expected "Kari Eksempelsen", parsed "Ola Eksempelsen Fjellstue""#
                .to_owned()
        ]
    );
}

#[test]
fn check_reports_a_page_that_does_not_parse() {
    let page = invented_census_person();
    let mismatches = check(&page, "<html><body></body></html>");
    assert_eq!(mismatches.len(), 1, "{mismatches:?}");
    assert!(
        mismatches[0].starts_with("invented-census-person: does not parse:"),
        "{mismatches:?}"
    );
}

#[test]
fn check_reports_why_a_viewer_expected_to_show_a_scan_does_not_parse() {
    let page = Page {
        id: "invented-viewer".to_owned(),
        kind: Kind::Viewer,
        url: "https://media.digitalarkivet.no/view/99901/42".to_owned(),
        rights: invented_census_person().rights,
        expected: Expected {
            image: Some("https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10000099901042.jpg".to_owned()),
            ..Expected::default()
        },
    };
    let mismatches = check(&page, "<html><body></body></html>");
    assert_eq!(mismatches.len(), 1, "{mismatches:?}");
    assert!(
        mismatches[0].starts_with("invented-viewer: does not parse:"),
        "{mismatches:?}"
    );
}

#[test]
fn no_image_false_is_not_an_expected_fact() {
    assert!(check_nothing(&Expected {
        no_image: Some(false),
        ..Expected::default()
    }));
    assert!(!check_nothing(&Expected {
        no_image: Some(true),
        ..Expected::default()
    }));
}

#[test]
#[ignore = "needs `cargo xtask fetch-fixtures`; a manual drift check against the live site"]
fn live_pages_parse_to_their_expected_facts() {
    let manifest = manifest().expect("the manifest parses");
    let mut mismatches = Vec::new();
    for page in &manifest.page {
        let path = fetched_path(&page.id);
        match std::fs::read_to_string(&path) {
            Ok(html) => mismatches.extend(check(page, &html)),
            Err(error) => mismatches.push(format!(
                "{}: cannot read {} ({error}); run `cargo xtask fetch-fixtures` first",
                page.id,
                path.display()
            )),
        }
    }
    assert!(mismatches.is_empty(), "\n{}", mismatches.join("\n"));
}

/// True when `expected` names no fact at all.
fn check_nothing(expected: &Expected) -> bool {
    let Expected {
        name,
        birth,
        role,
        occupation,
        scan_viewer_url,
        household,
        source_title,
        person_links,
        image,
        no_image,
    } = expected;
    name.is_none()
        && birth.is_none()
        && role.is_none()
        && occupation.is_none()
        && scan_viewer_url.is_none()
        && household.is_none()
        && source_title.is_none()
        && person_links.is_none()
        && image.is_none()
        && *no_image != Some(true)
}

/// Pushes a mismatch when `expected` is given and `actual` differs from it.
fn compare<T: PartialEq + Debug + ?Sized>(
    mismatches: &mut Vec<String>,
    id: &str,
    field: &str,
    expected: Option<&T>,
    actual: Option<&T>,
) {
    if let Some(expected) = expected
        && Some(expected) != actual
    {
        match actual {
            Some(actual) => mismatches.push(format!("{id}: {field}: expected {expected:?}, parsed {actual:?}")),
            None => mismatches.push(format!("{id}: {field}: expected {expected:?}, parsed nothing")),
        }
    }
}

/// The page kind a manifest kind must classify as, for the person kinds.
fn person_page_kind(kind: Kind) -> Option<PageKind> {
    match kind {
        Kind::CensusPerson => Some(PageKind::CensusPerson),
        Kind::ChurchbookPerson => Some(PageKind::ChurchbookRecord),
        Kind::CensusResidence | Kind::Viewer => None,
    }
}

#[test]
fn every_person_page_url_classifies_as_its_kind() {
    let manifest = manifest().expect("the manifest parses");
    for page in &manifest.page {
        if let Some(kind) = person_page_kind(page.kind) {
            assert_eq!(classify_url(&page.url), kind, "{}: {}", page.id, page.url);
        }
    }
}
