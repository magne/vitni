//! Each rung of the parser's fallback chains, isolated on inline HTML.
//!
//! A full page carries its first rung (`og:url`, `a#scannedImageLink`, `input#permanent_image_link`),
//! so the later rungs are unreachable from the page fixtures. Every test here offers exactly one rung,
//! or two that disagree, so removing or reordering a rung fails a test.

use vitni_digitalarkivet::{ParseError, parse_person_page, parse_viewer_page};

const PERSON_URL: &str = "https://www.digitalarkivet.no/census/person/pf01099901000101";
const VIEWER_URL: &str = "https://media.digitalarkivet.no/view/99901/42";

/// A person page whose focal block is minimal, with `extra` spliced into the body.
fn person_page(head: &str, extra: &str) -> String {
    format!(
        r#"<!DOCTYPE html><html><head>{head}</head><body>{extra}
        <div class="data-item current"><h4><a href="{PERSON_URL}">Ola Eksempelsen Fjellstue</a></h4></div>
        </body></html>"#
    )
}

fn scan_viewer_url(extra: &str) -> Result<Option<String>, ParseError> {
    parse_person_page(&person_page("", extra), PERSON_URL).map(|record| record.scan_viewer_url)
}

#[test]
fn og_url_is_the_record_url_even_when_fetched_elsewhere() {
    let head = format!(r#"<meta property="og:url" content="{PERSON_URL}">"#);
    let fetched = "https://www.digitalarkivet.no/nn/census/person/pf01099901000101";
    let record = parse_person_page(&person_page(&head, ""), fetched).expect("parse person page");
    assert_eq!(record.record_url, PERSON_URL);
}

#[test]
fn scanned_image_link_wins_over_other_scan_links() {
    let extra = r#"
        <a data-scans="[]" href="https://goto.digitalarkivet.no/kb2">Vis</a>
        <a id="scannedImageLink" href="https://goto.digitalarkivet.no/kb1">Vis</a>"#;
    assert_eq!(
        scan_viewer_url(extra).expect("parse person page").as_deref(),
        Some("https://goto.digitalarkivet.no/kb1")
    );
}

#[test]
fn a_data_scans_anchor_is_the_scan_link_without_an_id() {
    let extra = r#"
        <a href="https://www.digitalarkivet.no/source/99901">Kilde</a>
        <a data-scans="[]" href="https://goto.digitalarkivet.no/kb1">Vis</a>"#;
    assert_eq!(
        scan_viewer_url(extra).expect("parse person page").as_deref(),
        Some("https://goto.digitalarkivet.no/kb1")
    );
}

#[test]
fn an_anchor_reading_skannet_is_the_scan_link() {
    let extra = r#"<a href="https://goto.digitalarkivet.no/kb1">Se skannet versjon</a>"#;
    assert_eq!(
        scan_viewer_url(extra).expect("parse person page").as_deref(),
        Some("https://goto.digitalarkivet.no/kb1")
    );
}

#[test]
fn an_anchor_reading_scanned_is_the_scan_link() {
    let extra = r#"<a href="https://goto.digitalarkivet.no/kb1">View scanned version</a>"#;
    assert_eq!(
        scan_viewer_url(extra).expect("parse person page").as_deref(),
        Some("https://goto.digitalarkivet.no/kb1")
    );
}

#[test]
fn a_media_host_anchor_is_the_last_resort_scan_link() {
    let extra = r#"
        <a href="https://www.digitalarkivet.no/source/99901">Kilde</a>
        <a href="https://media.digitalarkivet.no/fs10000099901042">Vis</a>"#;
    assert_eq!(
        scan_viewer_url(extra).expect("parse person page").as_deref(),
        Some("https://media.digitalarkivet.no/fs10000099901042")
    );
}

#[test]
fn a_page_with_no_scan_link_has_none() {
    let extra = r#"<a href="https://www.digitalarkivet.no/source/99901">Kilde</a>"#;
    assert_eq!(scan_viewer_url(extra).expect("parse person page"), None);
}

#[test]
fn an_og_image_that_is_the_scan_wins_over_the_displayed_image() {
    let html = r#"<html><head>
        <meta property="og:image" content="https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10000099901042.jpg">
        </head><body>
        <img src="https://media.digitalarkivet.no/image/00000000-0000-4000-8000-000000099901">
        </body></html>"#;
    assert_eq!(
        parse_viewer_page(html, VIEWER_URL).expect("parse viewer"),
        "https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10000099901042.jpg"
    );
}

#[test]
fn a_permanent_jpg_anchor_is_the_last_resort_scan() {
    let html = r#"<html><body>
        <a href="https://www.digitalarkivet.no/source/99901">Kilde</a>
        <a href="https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10000099901042.jpg">Last ned</a>
        </body></html>"#;
    assert_eq!(
        parse_viewer_page(html, VIEWER_URL).expect("parse viewer"),
        "https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10000099901042.jpg"
    );
}
