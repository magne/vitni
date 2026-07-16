//! SSR assertions for the Repository detail (Phase 5 PR27): the read-first Overview record (id ·
//! type · name), its edit mode swapping in inputs plus the sticky-header Cancel/Save, the addresses
//! cards, the URLs table, the sources-held table, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use genealogy_app::{Address, RepositoryType, TagRef};
use genealogy_ui::{
    AddressVm, AttachedRefVm, Localizer, ProvenanceDraft, RepositoryDetail, RepositoryDraft, RepositoryUrlVm,
    RestrictionKind, SourceHeldVm,
};
use genealogy_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, RepositoryEditForm, address_cards, id_list, record_head_actions,
    repository_overview, repository_sources_table, repository_urls_table, tags_panel,
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
        addresses: vec![AddressVm {
            address: Address {
                lines: vec!["700 Pennsylvania Avenue NW".to_owned()],
                locality: Some("Washington".to_owned()),
                region: Some("District of Columbia".to_owned()),
                postal_code: Some("20408".to_owned()),
                country: Some("United States".to_owned()),
                phone: Some("+1 866-272-6272".to_owned()),
                email: Some("inquire@nara.gov".to_owned()),
                fax: Some("+1 301-837-0483".to_owned()),
                www: Some("https://www.archives.gov".to_owned()),
                original_text: None,
            },
            assertion_id: "0190-addr-assert-1".to_owned(),
        }],
        urls: vec![RepositoryUrlVm {
            url_type: Some("website".to_owned()),
            href: "https://www.archives.gov".to_owned(),
            description: Some("Main catalog".to_owned()),
            assertion_id: "0190-url-assert-1".to_owned(),
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
        notes: vec![AttachedRefVm {
            human_id: "N0004".to_owned(),
            assertion_id: "0190-note-attach-1".to_owned(),
        }],
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
    let onedit = use_callback(|_: RepositoryEditForm| {});
    let onedit_address = use_callback(|_: AddressVm| {});
    let onretract = use_callback(|_: (String, String, bool)| {});
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (RepositoryDraft, ProvenanceDraft)| {}))}
        {repository_overview(&loc, &detail, record)}
        {address_cards(&loc, &detail.addresses, onedit_address, onretract)}
        {repository_urls_table(&loc, &detail, onedit, onretract)}
        {repository_sources_table(&loc, &detail)}
        {id_list(&loc, &detail.notes, Some(onretract))}
        {tags_panel(&loc, &detail.tags, use_signal(|| None::<RepositoryEditForm>), RepositoryEditForm::Tag, use_callback(|_: String| {}))}
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

/// Renders only the reverse-index Sources table (sources held by this repository) — it carries no
/// per-row corrections, so it must render no row-action buttons.
fn sources_only() -> Element {
    let loc = loc();
    let detail = sample();
    rsx! { {repository_sources_table(&loc, &detail)} }
}

#[test]
fn url_rows_carry_edit_and_retract_with_row_scoped_labels() {
    let html = render(overview_view);
    assert!(
        html.contains(r#"aria-label="Edit https://www.archives.gov""#),
        "the URL row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract https://www.archives.gov""#),
        "the URL row Retract carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract button carries the retract-title tooltip:\n{html}"
    );
}

#[test]
fn address_cards_render_fax_www_and_carry_edit_retract() {
    let html = render(overview_view);
    assert!(
        html.contains("+1 301-837-0483"),
        "the address fax number renders:\n{html}"
    );
    assert!(
        html.contains("https://www.archives.gov"),
        "the address www URL renders:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Edit Washington""#),
        "the address card Edit carries a card-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract Washington""#),
        "the address card Retract carries a card-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract button carries the retract-title tooltip:\n{html}"
    );
}

#[test]
fn reverse_index_sources_table_has_no_row_actions() {
    let html = render(sources_only);
    assert!(!html.contains("row-actions"), "no row-actions cell:\n{html}");
    for needle in [">Edit<", ">Retract<", ">Detach<", ">Unlink<"] {
        assert!(
            !html.contains(needle),
            "the reverse-index sources table carries no {needle:?}:\n{html}"
        );
    }
}

#[test]
fn notes_carry_detach() {
    let html = render(overview_view);
    assert!(
        html.contains(r#"aria-label="Detach N0004""#),
        "the attached note carries a Detach:\n{html}"
    );
}

#[test]
fn no_assertion_id_is_ever_rendered() {
    let html = render(overview_view);
    for assertion_id in [
        "0190-url-assert-1",
        "0190-addr-assert-1",
        "0190-note-attach-1",
        "0190-secret-tag-id",
    ] {
        assert!(
            !html.contains(assertion_id),
            "assertion/tag id {assertion_id:?} must never be rendered:\n{html}"
        );
    }
}
