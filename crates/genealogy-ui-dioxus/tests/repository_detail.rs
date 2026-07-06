//! SSR assertions for the Repository detail (Phase 5 PR9): render the overview (type/name + the
//! primary-contact card), the addresses cards, the URLs table, the sources-held table (call number ·
//! medium · citation count), and the tags panel. Asserts the held-source rows, the no-source flag on
//! an uncited source, and that a tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::{Address, TagRef, Url};
use genealogy_ui::{Localizer, RepositoryDetail, RestrictionKind, SourceHeldVm};
use genealogy_ui_dioxus::screens::{
    RepositoryEditForm, repository_addresses_cards, repository_overview, repository_sources_table,
    repository_tags_panel, repository_urls_table,
};

/// A representative repository detail: an archive with one address, two URLs, two held sources (one
/// cited, one not), and one tag.
fn sample() -> RepositoryDetail {
    RepositoryDetail {
        human_id: "R0004".to_owned(),
        id: "0190-repo-id".to_owned(),
        title: "National Archives".to_owned(),
        type_label: Some("Archive".to_owned()),
        addresses: vec![Address {
            lines: vec!["700 Pennsylvania Avenue NW".to_owned()],
            locality: Some("Washington".to_owned()),
            region: Some("District of Columbia".to_owned()),
            postal_code: Some("20408".to_owned()),
            country: Some("United States".to_owned()),
            phone: Some("+1 866-272-6272".to_owned()),
            email: Some("inquire@nara.gov".to_owned()),
            fax: None,
            www: None,
            original_text: None,
        }],
        urls: vec![Url {
            url_type: Some("website".to_owned()),
            href: "https://www.archives.gov".to_owned(),
            description: Some("Main catalog".to_owned()),
        }],
        sources: vec![
            SourceHeldVm {
                human_id: "S0003".to_owned(),
                id: "0190-source-3".to_owned(),
                title: "1850 U.S. Federal Census, New York".to_owned(),
                call_number: Some("M432, roll 552".to_owned()),
                media_type_label: "Film".to_owned(),
                citation_count: 18,
            },
            SourceHeldVm {
                human_id: "S0008".to_owned(),
                id: "0190-source-8".to_owned(),
                title: "Unindexed loose papers".to_owned(),
                call_number: None,
                media_type_label: "Manuscript".to_owned(),
                citation_count: 0,
            },
        ],
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Primary archive".to_owned(),
            color: Some("#6cb6ff".to_owned()),
            priority: Some(1),
        }],
        restrictions: vec![RestrictionKind::Locked],
        history: Vec::new(),
    }
}

/// Renders the overview, addresses, URLs, sources, and tags tabs together.
fn repository_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<RepositoryEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::RepositoryEdit, genealogy_ui::ProvenanceDraft)| {});
    let detail = sample();
    rsx! {
        {repository_overview(&loc, &detail, editing)}
        {repository_addresses_cards(&loc, &detail)}
        {repository_urls_table(&loc, &detail)}
        {repository_sources_table(&loc, &detail)}
        {repository_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_and_addresses_show_type_name_and_contact() {
    let mut vdom = VirtualDom::new(repository_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "National Archives",          // the name
        "Archive",                    // the type chip
        "700 Pennsylvania Avenue NW", // the street
        "Washington",                 // the locality
        "inquire@nara.gov",           // the email
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn urls_and_held_sources_carry_links_and_citation_counts() {
    let mut vdom = VirtualDom::new(repository_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "https://www.archives.gov",           // a URL
        "Main catalog",                       // its description
        "1850 U.S. Federal Census, New York", // a held source
        "M432, roll 552",                     // its call number
        "Film",                               // its medium
        "no-source",                          // the uncited source's no-source flag
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(repository_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Primary archive"), "tag name shown:\n{html}");
    assert!(html.contains("#6cb6ff"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
