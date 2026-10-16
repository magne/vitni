//! List summaries over the golden backup fixture (ADR 0041 §5), which holds every event variant of
//! every aggregate: each kind's list summary reads exactly as `show_*` does, except the reverse-usage
//! fields no index answers (#673) — a citation's Backs count, a source citation's backers, a media
//! object's or note's users, a DNA match's citing inferences, a DNA test's matches — which a list leaves
//! empty and only `show_*` fills. A place's successions and event pins are `show_place`'s alone, as
//! before. Pages tile the whole list.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::fmt::Debug;
use std::path::Path;

use uuid::Uuid;
use vitni_app::backup::current_format_version;
use vitni_app::{
    ListPage, OperatorConfig, RestoreRequest, Workspace, WorkspaceDefaults, list_citations, list_citations_page,
    list_dna_matches, list_dna_matches_page, list_dna_tests, list_dna_tests_page, list_events, list_events_page,
    list_families, list_families_page, list_media, list_media_page, list_notes, list_notes_page, list_persons,
    list_persons_page, list_places, list_places_page, list_repositories, list_repositories_page, list_research_notes,
    list_research_notes_page, list_sources, list_sources_page, restore_backup, show_citation, show_dna_match,
    show_dna_test, show_event, show_family, show_media, show_note, show_person, show_place, show_repository,
    show_research_note, show_source,
};
use vitni_core::ids::AgentId;

const PAGE: u32 = 3;

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: None,
        email: None,
    }
}

/// The current golden fixture, restored into a fresh workspace.
async fn fixture() -> (Workspace, tempfile::TempDir) {
    let home = tempfile::tempdir().expect("tempdir");
    let config = home.path().join("config.toml");
    let target = home.path().join("restored");
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/backup")
        .join(format!("v{}", current_format_version()))
        .join("workspace.vitni-backup");
    let request = RestoreRequest {
        config_path: &config,
        archive: &archive,
        name: "fixture",
        dir: Some(&target),
        database_url: None,
    };
    restore_backup(&request).await.expect("the fixture restores");
    let workspace = Workspace::open(&target, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open the restored fixture");
    (workspace, home)
}

/// `summary` as a string with every citation's Backs count zeroed, wherever it nests.
fn without_backs(summary: &impl Debug) -> String {
    let rendered = format!("{summary:#?}");
    let mut out = String::with_capacity(rendered.len());
    let mut rest = rendered.as_str();
    while let Some(at) = rest.find("backs_count: ") {
        let (head, tail) = rest.split_at(at + "backs_count: ".len());
        out.push_str(head);
        out.push('0');
        rest = tail.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    out.push_str(rest);
    out
}

/// Runs `clear` on `summary`.
fn apply<T>(summary: &mut T, clear: impl FnOnce(&mut T)) {
    clear(summary);
}

/// Asserts that every listed summary reads as `show_*` with `clear` applied, that `clear` leaves a listed
/// summary unchanged (its reverse-usage fields are already empty), and that pages tile the list.
macro_rules! check_kind {
    ($ws:expr, $list:ident, $page:ident, $show:ident, $clear:expr) => {{
        let ws = $ws;
        let listed = $list(ws).await.expect(stringify!($list));
        assert!(
            !listed.is_empty(),
            "the fixture holds no record for {}",
            stringify!($list)
        );
        for summary in &listed {
            let mut shown = $show(ws, &summary.human_id)
                .await
                .expect(stringify!($show))
                .expect("a listed record shows");
            let mut cleared = summary.clone();
            apply(&mut cleared, $clear);
            apply(&mut shown, $clear);
            assert_eq!(
                without_backs(summary),
                without_backs(&cleared),
                "{} leaves {}'s reverse usage empty",
                stringify!($list),
                summary.human_id
            );
            assert_eq!(
                format!("{summary:#?}"),
                without_backs(summary),
                "{} counts no Backs for {}",
                stringify!($list),
                summary.human_id
            );
            assert_eq!(
                without_backs(summary),
                without_backs(&shown),
                "{} reads {} as {} does",
                stringify!($list),
                summary.human_id,
                stringify!($show)
            );
        }
        let mut tiled = Vec::new();
        for offset in (0..).step_by(PAGE as usize) {
            let page = $page(
                ws,
                ListPage {
                    offset,
                    limit: Some(PAGE),
                },
            )
            .await
            .expect(stringify!($page));
            if page.is_empty() {
                break;
            }
            tiled.extend(page);
        }
        assert_eq!(tiled, listed, "the pages of {} tile the list", stringify!($list));
    }};
}

#[tokio::test]
async fn list_summaries_read_as_shown_without_reverse_usage() {
    let (ws, _home) = fixture().await;
    let ws = &ws;
    check_kind!(ws, list_persons, list_persons_page, show_person, |_| {});
    check_kind!(ws, list_families, list_families_page, show_family, |_| {});
    check_kind!(ws, list_events, list_events_page, show_event, |_| {});
    check_kind!(ws, list_places, list_places_page, show_place, |s| {
        for row in s.predecessors.iter().chain(&s.successors) {
            s.claim_owners.remove(&row.assertion_id);
        }
        s.predecessors.clear();
        s.successors.clear();
        s.events.clear();
    });
    check_kind!(ws, list_sources, list_sources_page, show_source, |s| {
        for citation in &mut s.citations {
            citation.backers.clear();
        }
        s.reliability.record_count = 0;
    });
    check_kind!(ws, list_citations, list_citations_page, show_citation, |_| {});
    check_kind!(ws, list_repositories, list_repositories_page, show_repository, |_| {});
    check_kind!(ws, list_media, list_media_page, show_media, |s| s.used_by.clear());
    check_kind!(ws, list_notes, list_notes_page, show_note, |s| s.references.clear());
    check_kind!(ws, list_dna_tests, list_dna_tests_page, show_dna_test, |s| s
        .matches
        .clear());
    check_kind!(ws, list_dna_matches, list_dna_matches_page, show_dna_match, |s| s
        .cited_by
        .clear());
    check_kind!(
        ws,
        list_research_notes,
        list_research_notes_page,
        show_research_note,
        |_| {}
    );
}
