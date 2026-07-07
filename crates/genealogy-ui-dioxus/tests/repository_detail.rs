//! SSR assertions for the Repository detail (Phase 5 PR27): the read-first Overview record (id ·
//! type · name), its edit mode swapping in inputs plus the sticky-header Cancel/Save, the addresses
//! cards, the URLs table, the sources-held table, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use genealogy_app::{Address, RepositoryType, TagRef, Url};
use genealogy_ui::{Localizer, ProvenanceDraft, RepositoryDetail, RepositoryDraft, RestrictionKind, SourceHeldVm};
use genealogy_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, record_head_actions, repository_addresses_cards, repository_overview,
    repository_sources_table, repository_tags_panel, repository_urls_table,
};

/// A representative repository detail: an archive with one address, two URLs, two held sources (one
/// cited, one not), and one tag.
fn sample() -> RepositoryDetail {
    RepositoryDetail {
        human_id: "R0004".to_owned(),
        id: "0190-repo-id".to_owned(),
        title: "National Archives".to_owned(),
        name: Some("National Archives".to_owned()),
        repository_type: Some(RepositoryType::Archive),
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

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// A record edit state seeded from the sample, in view or edit mode.
fn state(editing: bool) -> RecordEditState<RepositoryDraft> {
    let seed = RepositoryDraft::from_detail(&sample());
    RecordEditState {
        editing: use_signal(move || editing),
        seed: use_signal({
            let seed = seed.clone();
            move || seed
        }),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    }
}

fn overview_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (RepositoryDraft, ProvenanceDraft)| {}))}
        {repository_overview(&loc, &detail, record)}
        {repository_addresses_cards(&loc, &detail)}
        {repository_urls_table(&loc, &detail)}
        {repository_sources_table(&loc, &detail)}
        {repository_tags_panel(&loc, &detail, use_signal(|| None), use_callback(|_| {}), &detail.human_id)}
    }
}

fn overview_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (RepositoryDraft, ProvenanceDraft)| {}))}
        {repository_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(overview_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit in the header:\n{html}");
    assert!(
        !html.contains("<input"),
        "view mode shows read boxes, not inputs:\n{html}"
    );
    assert!(html.contains("National Archives"), "the name is shown:\n{html}");
    assert!(html.contains("Archive"), "the type is shown:\n{html}");
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let html = render(overview_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="repository-id""#),
        "the editable human id is present:\n{html}"
    );
    assert!(
        html.contains(r#"value="National Archives""#),
        "the name input is seeded:\n{html}"
    );
}

#[test]
fn urls_and_held_sources_carry_links_and_citation_counts() {
    let html = render(overview_view);
    for needle in [
        "https://www.archives.gov",
        "Main catalog",
        "1850 U.S. Federal Census, New York",
        "M432, roll 552",
        "Film",
        "no-source",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(overview_view);
    assert!(html.contains("Primary archive"), "tag name shown:\n{html}");
    assert!(html.contains("#6cb6ff"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
