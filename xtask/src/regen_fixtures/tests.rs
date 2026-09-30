use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use vitni_digitalarkivet::{parse_person_page, parse_residence_page, parse_viewer_page};

use crate::regen_fixtures::manifest::{Manifest, Page, load_manifest};
use crate::regen_fixtures::page::{Pruner, Unmapped};
use crate::regen_fixtures::{fixture_path, surviving_values};

const PERSON_URL: &str = "https://www.digitalarkivet.no/census/person/pf01099901000101";

/// An invented census person page with the chrome a live page carries around the parsed elements.
const PERSON_PAGE: &str = r#"<!DOCTYPE html>
<html lang="no">
<head>
  <meta charset="utf-8">
  <title>001 Ola Nordmann - Folketelling 1920 for 9901 Eksempelvik herred - Digitalarkivet</title>
  <meta property="og:url" content="https://www.digitalarkivet.no/census/person/pf01099901000101">
  <meta name="csrf-token" content="secret-token">
  <style>.logo { color: red; }</style>
  <script>var person = "Kari Nordmann";</script>
</head>
<body class="page census">
  <header><img src="https://www.digitalarkivet.no/logo.png" alt="Arkivverket"><a href="/login">Logg inn</a></header>
  <article>
    <div class="parent-post wide">
      <h4>Tellingskrets: <a href="https://www.digitalarkivet.no/census/district/tf1" title="Nordbygda">001&nbsp;Nordbygda</a></h4>
    </div>
    <a class="scans btn" id="scannedImageLink" href="https://media.digitalarkivet.no/fs1" data-scans="[&quot;fs1&quot;]">Skannet</a>
    <!-- a comment naming Kari Nordmann -->
    <div data-role="container" class="list">
      <div class="data-item current" style="color: blue">
        <h4><a href="https://www.digitalarkivet.no/census/person/pf01099901000101"><span class="de-emphasized">001</span>
          Ola Nordmann</a></h4>
        <div class="row">
          <div class="col-xs-12 col-md-6">Alder/født:</div>
          <div class="col-xs-12 col-md-6 ssp-semibold">1886-07-08</div>
          <div class="col-xs-12 col-md-6">Familiestilling:</div>
          <div class="col-xs-12 col-md-6 ssp-semibold">hp &amp; far</div>
        </div>
      </div>
      <div class="data-item">
        <h4><a href="https://www.digitalarkivet.no/census/person/pf01099901000102"><span class="de-emphasized">002</span> Kari Nordmann</a></h4>
        <div class="row"><div class="col-md-6">Familiestilling:</div><div class="ssp-semibold">hm</div></div>
      </div>
    </div>
  </article>
  <footer><a href="https://www.arkivverket.no/">Arkivverket</a></footer>
</body>
</html>
"#;

const RESIDENCE_URL: &str = "https://www.digitalarkivet.no/census/rural-residence/bf01099901000100";

const RESIDENCE_PAGE: &str = r#"<html><head><title>0012 Fjellstue - Folketelling 1920 for 9901 Eksempelvik herred - Digitalarkivet</title></head>
<body><nav><a href="/search">Søk</a></nav>
<div class="data-item"><h4><a href="/census/person/pf1"><span class="de-emphasized">001</span> Ola</a></h4></div>
<aside><a href="https://www.digitalarkivet.no/census/person/pf2">Kari</a></aside>
</body></html>"#;

const VIEWER_URL: &str = "https://media.digitalarkivet.no/view/99901/42";

const VIEWER_PAGE: &str = r#"<html><head><title>Skanna arkiver</title>
<meta property="og:image" content="https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs1.jpg"></head>
<body><img src="https://media.digitalarkivet.no/assets/img/logo.svg" alt="logo">
<input id="permanent_image_link" name="permanent_image_link" value="https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs1.jpg">
<img class="viewer-img-zoomable" src="https://media.digitalarkivet.no/image/00000000-0000-4000-8000-000000000001">
</body></html>"#;

fn page(substitute: &[(&str, &str)]) -> Page {
    Page {
        id: "census-person".to_owned(),
        fixture: "census/person.html".to_owned(),
        substitute: substitute
            .iter()
            .map(|(real, invented)| ((*real).to_owned(), (*invented).to_owned()))
            .collect(),
    }
}

fn regenerate(page: &Page, html: &str, vocabulary: &BTreeSet<String>) -> Result<String, Unmapped> {
    Pruner::new().unwrap().regenerate(page, html, vocabulary)
}

/// Every value `html` holds after pruning, found by regenerating with nothing mapped.
fn unmapped(html: &str) -> BTreeSet<String> {
    let error = regenerate(&page(&[]), html, &BTreeSet::new()).unwrap_err();
    error.values
}

/// `html` regenerated with every value it holds kept verbatim, so only the pruning is under test.
fn pruned(html: &str) -> String {
    regenerate(&page(&[]), html, &unmapped(html)).unwrap()
}

#[test]
fn pruning_drops_scripts_styles_comments_chrome_and_unread_attributes() {
    let output = pruned(PERSON_PAGE);
    for gone in [
        "<script",
        "<style",
        "csrf-token",
        "secret-token",
        "logo.png",
        "Logg inn",
        "arkivverket.no",
        "comment naming",
        "data-scans",
        "style=",
        "title=",
        "lang=",
        "data-role",
        "col-md-6",
        "btn",
        "wide",
        "census\"",
    ] {
        assert!(!output.contains(gone), "{gone:?} survived:\n{output}");
    }
}

#[test]
fn pruning_keeps_the_classes_and_ids_the_selectors_name() {
    let output = pruned(PERSON_PAGE);
    for kept in [
        r#"class="parent-post""#,
        r#"class="data-item current""#,
        r#"class="ssp-semibold""#,
        r#"class="de-emphasized""#,
        r#"id="scannedImageLink""#,
        r#"property="og:url""#,
    ] {
        assert!(output.contains(kept), "{kept:?} missing:\n{output}");
    }
}

#[test]
fn a_pruned_person_page_parses_exactly_as_the_whole_page() {
    let whole = parse_person_page(PERSON_PAGE, PERSON_URL).unwrap();
    let output = pruned(PERSON_PAGE);
    assert_eq!(parse_person_page(&output, PERSON_URL).unwrap(), whole, "{output}");
    assert_eq!(whole.household.len(), 2);
    assert_eq!(whole.role.as_deref(), Some("hp & far"));
}

#[test]
fn a_pruned_residence_page_parses_exactly_as_the_whole_page() {
    let whole = parse_residence_page(RESIDENCE_PAGE, RESIDENCE_URL).unwrap();
    let output = pruned(RESIDENCE_PAGE);
    assert_eq!(parse_residence_page(&output, RESIDENCE_URL).unwrap(), whole, "{output}");
    assert_eq!(whole.person_links.len(), 2);
    assert!(!output.contains("/search"), "{output}");
}

#[test]
fn a_pruned_viewer_page_parses_exactly_as_the_whole_page_and_loses_the_logo() {
    let whole = parse_viewer_page(VIEWER_PAGE, VIEWER_URL).unwrap();
    let output = pruned(VIEWER_PAGE);
    assert_eq!(parse_viewer_page(&output, VIEWER_URL).unwrap(), whole, "{output}");
    assert!(!output.contains("logo.svg"), "{output}");
    assert!(output.contains("media.digitalarkivet.no/image/"), "{output}");
}

#[test]
fn every_text_and_value_the_pruned_page_holds_must_be_mapped() {
    let values = unmapped(PERSON_PAGE);
    for value in [
        "Ola Nordmann",
        "Kari Nordmann",
        "001 Nordbygda",
        "hp & far",
        "Alder/født:",
        "https://media.digitalarkivet.no/fs1",
        "https://www.digitalarkivet.no/census/person/pf01099901000102",
    ] {
        assert!(values.contains(value), "{value:?} not reported: {values:?}");
    }
    assert!(!values.contains("secret-token"), "{values:?}");
    assert!(!values.iter().any(|v| v.trim().is_empty()), "{values:?}");
}

#[test]
fn an_unmapped_value_fails_the_page_and_the_error_names_it() {
    let mut vocabulary = unmapped(PERSON_PAGE);
    vocabulary.remove("Kari Nordmann");
    let error = regenerate(&page(&[]), PERSON_PAGE, &vocabulary).unwrap_err();
    assert_eq!(error.values, BTreeSet::from(["Kari Nordmann".to_owned()]));
    let message = error.to_string();
    assert!(
        message.contains("census-person") && message.contains("\"Kari Nordmann\""),
        "{message}"
    );
}

#[test]
fn substitution_replaces_text_and_attribute_values_whole() {
    let mut vocabulary = unmapped(PERSON_PAGE);
    let real_link = "https://www.digitalarkivet.no/census/person/pf01099901000102";
    for real in ["Kari Nordmann", real_link] {
        vocabulary.remove(real);
    }
    let substituted = page(&[
        ("Kari Nordmann", "Marit Eksempel"),
        (real_link, "https://www.digitalarkivet.no/census/person/pf09"),
    ]);
    let output = regenerate(&substituted, PERSON_PAGE, &vocabulary).unwrap();
    assert!(
        !output.contains("Kari Nordmann") && !output.contains("pf01099901000102"),
        "{output}"
    );
    let record = parse_person_page(&output, PERSON_URL).unwrap();
    assert!(
        record
            .household
            .contains(&"https://www.digitalarkivet.no/census/person/pf09".to_owned())
    );
    assert!(output.contains("Marit Eksempel"), "{output}");
}

#[test]
fn markup_in_a_value_is_escaped() {
    let mut vocabulary = unmapped(PERSON_PAGE);
    vocabulary.remove("Kari Nordmann");
    let substituted = page(&[("Kari Nordmann", "<b>\"Marit\" & co</b>")]);
    let output = regenerate(&substituted, PERSON_PAGE, &vocabulary).unwrap();
    assert!(output.contains("&lt;b&gt;\"Marit\" &amp; co&lt;/b&gt;"), "{output}");
    assert!(output.contains("hp &amp; far"), "{output}");
}

#[test]
fn regeneration_is_deterministic_and_says_it_is_generated() {
    let output = pruned(PERSON_PAGE);
    assert_eq!(output, pruned(PERSON_PAGE));
    assert!(output.starts_with("<!DOCTYPE html>\n<!--"), "{output}");
    assert!(
        output.contains("cargo xtask regen-fixtures") && output.contains("census-person"),
        "{output}"
    );
}

#[test]
fn a_real_value_left_anywhere_in_an_output_is_reported() {
    let manifest = Manifest {
        vocabulary: BTreeSet::new(),
        page: vec![page(&[("Kari Nordmann", "Marit"), ("Nordbygda", "Sørbygda")])],
    };
    let outputs = vec![
        ("census-person".to_owned(), "<title>Marit Sørbygda</title>".to_owned()),
        ("other".to_owned(), "<a href=\"x\">Kari Nordmann</a>".to_owned()),
    ];
    assert_eq!(
        surviving_values(&manifest, &outputs),
        vec![("other".to_owned(), "Kari Nordmann".to_owned())]
    );
}

const MANIFEST: &str = r#"
vocabulary = ["Skannet", "hp"]

[[page]]
id = "census-person"
kind = "census-person"
url = "https://www.digitalarkivet.no/census/person/pf1"
fixture = "census/person.html"
[page.rights]
holder = "Nasjonalarkivet"
[page.substitute]
"Asbjørn" = "Ola"
"#;

#[test]
fn the_manifest_gives_each_page_a_fixture_and_its_substitutions() {
    let manifest = load_manifest(MANIFEST).unwrap();
    assert_eq!(
        manifest.vocabulary,
        BTreeSet::from(["Skannet".to_owned(), "hp".to_owned()])
    );
    assert_eq!(manifest.page.len(), 1);
    assert_eq!(manifest.page[0].fixture, "census/person.html");
    assert_eq!(
        manifest.page[0].substitute,
        BTreeMap::from([("Asbjørn".to_owned(), "Ola".to_owned())])
    );
}

#[test]
fn a_substitution_that_keeps_the_real_value_is_rejected() {
    let text = MANIFEST.replace("\"Asbjørn\" = \"Ola\"", "\"Asbjørn\" = \"Asbjørn\"");
    let error = load_manifest(&text).unwrap_err().to_string();
    assert!(
        error.contains("census-person") && error.contains("Asbjørn") && error.contains("vocabulary"),
        "{error}"
    );
}

#[test]
fn a_real_value_listed_as_vocabulary_is_rejected() {
    let text = MANIFEST.replace("\"Skannet\", \"hp\"", "\"Skannet\", \"Asbjørn\"");
    let error = load_manifest(&text).unwrap_err().to_string();
    assert!(error.contains("Asbjørn") && error.contains("vocabulary"), "{error}");
}

#[test]
fn a_page_without_a_fixture_is_rejected() {
    let text = MANIFEST.replace("fixture = \"census/person.html\"\n", "");
    assert!(load_manifest(&text).is_err());
}

#[test]
fn a_fixture_path_must_stay_inside_the_fixture_tree_and_be_html() {
    for bad in ["../escape.html", "/abs.html", "census/person.txt", "census//x.html", ""] {
        let error = fixture_path(bad).unwrap_err().to_string();
        assert!(error.contains(bad), "{bad:?}: {error}");
    }
    assert_eq!(
        fixture_path("census/person.html").unwrap(),
        Path::new("crates/vitni-digitalarkivet/tests/fixtures/census/person.html")
    );
}

#[test]
fn the_real_manifest_loads_with_a_distinct_fixture_per_page() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate::fetch_fixtures::MANIFEST);
    let manifest = load_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
    let fixtures: BTreeSet<&str> = manifest.page.iter().map(|p| p.fixture.as_str()).collect();
    assert_eq!(fixtures.len(), manifest.page.len());
    for page in &manifest.page {
        fixture_path(&page.fixture).unwrap();
    }
}
