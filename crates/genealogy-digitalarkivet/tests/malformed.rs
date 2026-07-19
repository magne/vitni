//! Malformed and adversarial input: parsers return typed errors (or honest empty
//! results), never a panic.

use genealogy_digitalarkivet::{PageContext, ParseError, parse_person_page, parse_residence_page, parse_viewer_page};

const PERSON_HTML: &str = include_str!("fixtures/census/person.html");
const PERSON_URL: &str = "https://www.digitalarkivet.no/census/person/pf01073902000464";
const BOSTED_HTML: &str = include_str!("fixtures/census/bosted.html");
const BOSTED_URL: &str = "https://www.digitalarkivet.no/census/rural-residence/bf01073902000463";

const MISSING_FOCAL: ParseError = ParseError::MissingElement {
    page: PageContext::CensusPerson,
    what: "focal person (.data-item.current)",
};

#[test]
fn empty_input_to_person_parser_errors() {
    let error = parse_person_page("", PERSON_URL).expect_err("empty input has no focal person");
    assert_eq!(error, MISSING_FOCAL);
}

#[test]
fn truncated_html_to_person_parser_errors() {
    let error =
        parse_person_page("<html><body><div class=\"data", PERSON_URL).expect_err("truncated html has no focal person");
    assert_eq!(error, MISSING_FOCAL);
}

#[test]
fn residence_fed_to_person_parser_errors() {
    // A residence page has no focal `.data-item.current`.
    let error = parse_person_page(BOSTED_HTML, BOSTED_URL).expect_err("residence has no focal person");
    assert_eq!(error, MISSING_FOCAL);
}

#[test]
fn person_fed_to_residence_parser_is_honest() {
    // Not an error: a person page also carries `/census/person/` links, so the
    // residence parser returns them rather than pretending to fail.
    let record = parse_residence_page(PERSON_HTML, PERSON_URL).expect("residence parse is lenient");
    assert!(!record.person_links.is_empty());
    assert!(record.person_links.iter().all(|u| u.contains("/census/person/")));
}

#[test]
fn empty_input_to_viewer_parser_reports_no_image() {
    let error =
        parse_viewer_page("", "https://media.digitalarkivet.no/view/1/1").expect_err("empty viewer has no image");
    assert_eq!(
        error,
        ParseError::ImageUrlNotFound {
            page: PageContext::Viewer
        }
    );
}

#[test]
fn residence_parser_on_empty_input_yields_no_links() {
    let record = parse_residence_page("", BOSTED_URL).expect("empty residence parses to no links");
    assert!(record.person_links.is_empty());
}

#[test]
fn person_parser_survives_garbage_without_panicking() {
    // Adversarial bytes must not panic; an error result is acceptable either way.
    let _ = parse_person_page("<<<>>>&&&\u{0}\u{fffd}not html", "not a url");
    let _ = parse_viewer_page("\u{0}\u{fffd}", "");
}
