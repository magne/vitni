//! Census fixture parsing — pages `cargo xtask regen-fixtures` generates from the live site, pruned to
//! the DOM the parser reads and carrying only invented values (ADR 0042 §4).

use vitni_digitalarkivet::{
    Field, HouseholdPosition, Municipality, PageContext, PageKind, ParseError, Residence, classify_url, extract_urn,
    family_position, household_number, municipality, parse_person_page, parse_residence_page, parse_viewer_page,
    residence,
};

const PERSON_HTML: &str = include_str!("fixtures/census/person.html");
const PERSON_URL: &str = "https://www.digitalarkivet.no/census/person/pf01099901000101";
const BOSTED_HTML: &str = include_str!("fixtures/census/bosted.html");
const BOSTED_URL: &str = "https://www.digitalarkivet.no/census/rural-residence/bf01099901000100";
const VIEWER_HTML: &str = include_str!("fixtures/census/viewer.html");
const VIEWER_URL: &str = "https://media.digitalarkivet.no/view/99901/42";

#[test]
fn person_page_classifies_and_identifies() {
    assert_eq!(classify_url(PERSON_URL), PageKind::CensusPerson);
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(record.page_kind, PageKind::CensusPerson);
    assert_eq!(record.record_url, PERSON_URL);
    assert_eq!(record.external_id.authority, "digitalarkivet");
    assert_eq!(record.external_id.value, "pf01099901000101");
}

#[test]
fn person_page_extracts_focal_fields() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(record.name, "Ola Eksempelsen Fjellstue");
    assert_eq!(record.birth.as_deref(), Some("1887-03-14"));
    assert_eq!(record.birthplace.as_deref(), Some("Eksempelvik"));
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
        Some("https://media.digitalarkivet.no/fs10000099901042")
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
        Some("Folketelling 1920 for 9901 Eksempelvik herred")
    );
    assert_eq!(record.source.year.as_deref(), Some("1920"));
    assert_eq!(record.source.repository, "Digitalarkivet (Arkivverket)");
    let heading = record.source.headings.iter().find(|f| f.key == "Tellingskrets");
    assert_eq!(heading.map(|f| f.value.as_str()), Some("001 Nordbygda"));
    assert_eq!(
        heading.and_then(|f| f.url.as_deref()),
        Some("https://www.digitalarkivet.no/census/district/tf01099901000001")
    );
}

#[test]
fn person_page_names_its_residence() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(
        residence(&record),
        Some(Residence {
            id: "bf01099901000100".to_owned(),
            name: "Fjellstue".to_owned(),
            rural: true,
        })
    );
}

#[test]
fn a_head_joins_the_family_of_a_household_with_others_in_it() {
    let mut record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(family_position(&record), Some(HouseholdPosition::Head));
    record.household.truncate(1);
    assert_eq!(family_position(&record), None, "a head living alone founds no family");
    record.role = Some("s".to_owned());
    assert_eq!(family_position(&record), Some(HouseholdPosition::Child));
    record.role = Some("tj".to_owned());
    assert_eq!(family_position(&record), None, "a servant joins no family");
    record.role = None;
    assert_eq!(family_position(&record), None);
}

#[test]
fn person_page_names_its_household_number() {
    let mut record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(household_number(&record).as_deref(), Some("01"));
    record.fields.retain(|field| field.key != "H.nr");
    assert_eq!(household_number(&record), None);
}

#[test]
fn person_page_names_its_municipality() {
    let record = parse_person_page(PERSON_HTML, PERSON_URL).expect("parse census person");
    assert_eq!(
        record.source.title.as_deref().and_then(municipality),
        Some(Municipality {
            code: "9901".to_owned(),
            name: "Eksempelvik".to_owned(),
        })
    );
}

#[test]
fn residence_page_lists_household() {
    assert_eq!(classify_url(BOSTED_URL), PageKind::CensusResidence);
    let record = parse_residence_page(BOSTED_HTML, BOSTED_URL).expect("parse residence");
    assert_eq!(record.external_id.value, "bf01099901000100");
    assert_eq!(record.person_links.len(), 4);
    assert!(record.person_links.iter().all(|u| u.contains("/census/person/")));
    assert_eq!(record.source.year.as_deref(), Some("1920"));
}

#[test]
fn viewer_page_yields_permanent_image() {
    let image = parse_viewer_page(VIEWER_HTML, VIEWER_URL).expect("parse viewer");
    assert_eq!(
        image,
        "https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10000099901042.jpg"
    );
    assert_eq!(
        extract_urn(&image).as_deref(),
        Some("URN:NBN:no-a1450-fs10000099901042")
    );
}

// A viewer without `permanent_image_link` whose `og:image` is the site logo: the scan must come
// from the displayed `media.digitalarkivet.no/image/<uuid>` element, never the logo (the reported
// "downloaded image is the Digitalarkivet logo" bug).
const LOGO_TRAP_VIEWER: &str = r#"<!DOCTYPE html><html><head>
    <meta property="og:image" content="https://media.digitalarkivet.no/assets/img/logo.svg" />
    </head><body>
    <img class="viewer-img-zoomable" src="https://media.digitalarkivet.no/image/2dbfa4da-7820-4bba-99b8-f5127b9c9500" alt="scan" />
    </body></html>"#;

#[test]
fn viewer_without_permanent_link_takes_the_scan_not_the_logo() {
    let image = parse_viewer_page(LOGO_TRAP_VIEWER, VIEWER_URL).expect("parse viewer");
    assert_eq!(
        image,
        "https://media.digitalarkivet.no/image/2dbfa4da-7820-4bba-99b8-f5127b9c9500"
    );
}

#[test]
fn viewer_never_returns_a_non_scan_og_image() {
    let logo_only = r#"<html><head>
        <meta property="og:image" content="https://media.digitalarkivet.no/assets/img/logo.svg" />
        </head><body></body></html>"#;
    let error = parse_viewer_page(logo_only, VIEWER_URL).expect_err("a logo og:image is not a scan");
    assert_eq!(
        error,
        ParseError::ImageUrlNotFound {
            page: PageContext::Viewer
        }
    );
}

// A residence-level row outside the focal block, which the live page's pruned markup no longer carries:
// the parser must read the focal block's own `Fødested`, never the one before it.
const ROW_OUTSIDE_FOCAL: &str = r#"<!DOCTYPE html><html><body>
    <div class="row"><div>Fødested:</div><div class="ssp-semibold">Utlandet</div></div>
    <div class="data-item current"><h4><a href="/census/person/pf01099901000101">Ola</a></h4>
      <div class="row"><div>Fødested:</div><div class="ssp-semibold">Eksempelvik</div></div>
    </div>
    </body></html>"#;

#[test]
fn person_page_reads_only_rows_inside_the_focal_block() {
    let record = parse_person_page(ROW_OUTSIDE_FOCAL, PERSON_URL).expect("parse census person");
    assert_eq!(record.birthplace.as_deref(), Some("Eksempelvik"));
    assert_eq!(record.fields.len(), 1, "{:?}", record.fields);
}
