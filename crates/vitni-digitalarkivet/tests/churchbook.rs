//! Church-book fixture parsing — verbatim captures.
//!
//! The person page is an event record (`Fødte og døpte`) whose participants share
//! the census `data-item` structure but a different URL scheme (`/view/<n>/pd…`)
//! and scan host (`goto.digitalarkivet.no`). The viewer is the new
//! `nye.digitalarkivet.no` IIIF SPA — no legacy permanent image.

use vitni_digitalarkivet::{PageContext, PageKind, ParseError, classify_url, parse_person_page, parse_viewer_page};

const PERSON_HTML: &str = include_str!("fixtures/churchbook/person.html");
const PERSON_URL: &str = "https://www.digitalarkivet.no/view/255/pd00000020636420";
const VIEWER_HTML: &str = include_str!("fixtures/churchbook/viewer.html");
const VIEWER_URL: &str = "https://goto.digitalarkivet.no/kb20051205050405";

#[test]
fn person_page_classifies_as_churchbook_record() {
    assert_eq!(classify_url(PERSON_URL), PageKind::ChurchbookRecord);
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse churchbook record");
    assert_eq!(record.page_kind, PageKind::ChurchbookRecord);
    assert_eq!(record.external_id.value, "pd00000020636420");
}

#[test]
fn person_page_extracts_named_focal_participant() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse churchbook record");
    // Name comes from the `Navn` field, not the `Løpenr` ordinal in the heading.
    assert_eq!(record.name, "Asbjørn Andreassen Bergstad");
    assert_eq!(record.birth.as_deref(), Some("1886"));
    assert_eq!(record.role.as_deref(), Some("far"));
    // `Stilling/stand` is `-` here, so the typed field is absent.
    assert_eq!(record.occupation, None);
}

#[test]
fn person_page_resolves_goto_scan_and_participants() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse churchbook record");
    assert_eq!(
        record.scan_viewer_url.as_deref(),
        Some("https://goto.digitalarkivet.no/kb20051205050405")
    );
    assert_eq!(record.household.len(), 3);
    assert!(record.household.iter().all(|u| u.contains("/view/")));
}

#[test]
fn person_page_event_heading_and_source() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse churchbook record");
    assert_eq!(
        record.source.title.as_deref(),
        Some("Klokkerbok for Søgne prestegjeld, Greipstad sokn 1904-1936")
    );
    assert_eq!(record.source.year.as_deref(), Some("1904"));
    let event = record.source.headings.iter().find(|f| f.key == "Fødte og døpte");
    assert_eq!(event.map(|f| f.value.as_str()), Some("1924-11-30"));
}

#[test]
fn iiif_viewer_has_no_legacy_permanent_image() {
    // The new IIIF viewer serves tiles through a manifest, not a permanent `.jpg`;
    // the legacy chain honestly reports no image rather than guessing.
    let error = parse_viewer_page(VIEWER_HTML, VIEWER_URL).expect_err("IIIF viewer has no image");
    assert_eq!(
        error,
        ParseError::ImageUrlNotFound {
            page: PageContext::Viewer
        }
    );
}
