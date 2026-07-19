//! Census fixture parsing — verbatim `www.digitalarkivet.no` captures.

use genealogy_digitalarkivet::{
    Field, PageKind, classify_url, extract_urn, parse_person_page, parse_residence_page, parse_viewer_page,
};

const PERSON_HTML: &str = include_str!("fixtures/census/person.html");
const PERSON_URL: &str = "https://www.digitalarkivet.no/census/person/pf01073902000464";
const BOSTED_HTML: &str = include_str!("fixtures/census/bosted.html");
const BOSTED_URL: &str = "https://www.digitalarkivet.no/census/rural-residence/bf01073902000463";
const VIEWER_HTML: &str = include_str!("fixtures/census/viewer.html");
const VIEWER_URL: &str = "https://media.digitalarkivet.no/view/73902/1192";

#[test]
fn person_page_classifies_and_identifies() {
    assert_eq!(classify_url(PERSON_URL), PageKind::CensusPerson);
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(record.page_kind, PageKind::CensusPerson);
    assert_eq!(record.record_url, PERSON_URL);
    assert_eq!(record.external_id.authority, "digitalarkivet");
    assert_eq!(record.external_id.value, "pf01073902000464");
}

#[test]
fn person_page_extracts_focal_fields() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(record.name, "Asbjørn Andreassen Bergstøl");
    assert_eq!(record.birth.as_deref(), Some("1886-07-08"));
    assert_eq!(record.birthplace.as_deref(), Some("Greipstad"));
    assert_eq!(record.role.as_deref(), Some("hp"));
    assert_eq!(record.marital_status.as_deref(), Some("g"));
    assert_eq!(record.occupation.as_deref(), Some("Gårdbruker S."));
    // No `Bosted` field on a census person page (only `Bostatus`, which differs).
    assert_eq!(record.residence, None);
}

#[test]
fn person_page_keeps_all_rows_generically() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert!(record.fields.contains(&Field {
        key: "H.nr".into(),
        value: "01".into()
    }));
    assert!(record.fields.contains(&Field {
        key: "Yrke".into(),
        value: "Gårdbruker S.".into()
    }));
    // The bare `-` placeholder is kept in the generic row list.
    assert!(record.fields.contains(&Field {
        key: "Bostatus".into(),
        value: "-".into()
    }));
}

#[test]
fn person_page_resolves_scan_viewer_and_household() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(
        record.scan_viewer_url.as_deref(),
        Some("https://media.digitalarkivet.no/fs10771822220997")
    );
    assert_eq!(record.household.len(), 4);
    assert!(record.household.iter().all(|u| u.contains("/census/person/")));
    assert!(record.household.contains(&PERSON_URL.to_owned()));
}

#[test]
fn person_page_source_metadata() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(
        record.source.title.as_deref(),
        Some("Folketelling 1920 for 1017 Greipstad herred")
    );
    assert_eq!(record.source.year.as_deref(), Some("1920"));
    assert_eq!(record.source.repository, "Digitalarkivet (Arkivverket)");
    let heading = record.source.headings.iter().find(|f| f.key == "Tellingskrets");
    assert_eq!(
        heading.map(|f| f.value.as_str()),
        Some("002 Nodeland, Monen og Kulibygden")
    );
}

#[test]
fn residence_page_lists_household() {
    assert_eq!(classify_url(BOSTED_URL), PageKind::CensusResidence);
    let record = parse_residence_page(BOSTED_HTML, BOSTED_URL).expect("parse residence");
    assert_eq!(record.external_id.value, "bf01073902000463");
    assert_eq!(record.person_links.len(), 4);
    assert!(record.person_links.iter().all(|u| u.contains("/census/person/")));
    assert_eq!(record.source.year.as_deref(), Some("1920"));
}

#[test]
fn viewer_page_yields_permanent_image() {
    let image = parse_viewer_page(VIEWER_HTML, VIEWER_URL).expect("parse viewer");
    assert_eq!(
        image,
        "https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10771822220997.jpg"
    );
    assert_eq!(
        extract_urn(&image).as_deref(),
        Some("URN:NBN:no-a1450-fs10771822220997")
    );
}
