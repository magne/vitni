use super::prelude::*;
use vitni_app::{ImportRunId, RunResume};
use vitni_ui::{HistoryEntryVm, RunRecordVm, RunRecordsVm};

use crate::screens::tabs::history_entries;
use crate::services::{load_run_record_changes, load_run_records};
use crate::shell::nav_state::NavState;

/// The workspace dashboard (ADR 0008 §5; `app-shell.html`): stat cards, a workspace-wide recent
/// activity feed, quick entry points, and the computable data-quality checks. Refetches whenever a
/// mutation bumps `data_version`.
#[component]
pub fn DashboardScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let quality_services = state.services().clone();
    let loading = state.chrome().loading();
    let nav = use_context::<NavState>();
    let data = use_resource(move || {
        let services = services.clone();
        // Subscribe to `data_version` so a create/edit/undo refreshes the counts and activity.
        let _ = nav.data_version.read();
        async move { load_screen(services, Intent::ShowDashboard).await }
    });
    // The slower, whole-workspace data-quality pass fills the data-quality card on its own; it reads
    // from a `data_version`-keyed cache, so an unchanged workspace returns instantly.
    let quality = use_resource(move || {
        let services = quality_services.clone();
        let version = *nav.data_version.read();
        async move { load_data_quality(services, version).await }
    });
    // The dashboard is the workspace overview, rendered at the root of the work area (not inside a
    // record-tab body), matching `app-shell.html`.
    match detail_state(data.read_unchecked().as_ref(), |outcome| {
        if let IntentOutcome::Dashboard(found) = outcome {
            Some(found.as_ref())
        } else {
            None
        }
    }) {
        DetailState::Loading => rsx! { p { class: "loading", "{loading}" } },
        DetailState::Error(message) => rsx! { p { class: "empty", "{message}" } },
        DetailState::Ready(dashboard) => {
            let recent = nav.recent.read().clone();
            let quality = quality.read_unchecked();
            let data_quality = match &*quality {
                Some(ScreenData::Loaded(IntentOutcome::DataQuality(vm))) => Some(vm.as_ref()),
                _ => None,
            };
            dashboard_view(state.data_loc(), &recent, dashboard, data_quality)
        }
        DetailState::NotFound(_) | DetailState::Other => rsx! {},
    }
}

/// Renders a loaded dashboard: the "at a glance" stat cards, then the activity feed beside the quick
/// entry points and data-quality checks.
///
/// `data_quality` is `None` while the slower check pass is still loading (the data-quality card then
/// shows its own loading state), and `Some` once it resolves.
pub fn dashboard_view(
    loc: &Localizer,
    recent: &[RecentItem],
    dashboard: &DashboardVm,
    data_quality: Option<&DataQualityVm>,
) -> Element {
    let stats = &dashboard.stats;
    let (deaths, duplicates) = data_quality.map_or((0, 0), |quality| {
        (quality.death_before_birth.len(), quality.match_total)
    });
    rsx! {
        div { style: "padding:var(--sp-6);overflow:auto;height:100%",
            h1 { style: "border:0;margin:0 0 12px", "{loc.dashboard_label(\"title\")}" }
            div { class: "grid-3", style: "margin-bottom:8px",
                Card { title: loc.dashboard_label("stat-people"),
                    div { style: "font-size:28px;font-weight:700", "{stats.people}" }
                    div { class: "muted", "{loc.dashboard_people_caption(stats.families, stats.events)}" }
                }
                Card { title: loc.dashboard_label("stat-evidence"),
                    div { style: "font-size:28px;font-weight:700", "{stats.evidence_health_pct}%" }
                    div { class: "muted", "{loc.dashboard_label(\"stat-evidence-caption\")}" }
                }
                Card { title: loc.dashboard_label("stat-attention"),
                    div { style: "font-size:28px;font-weight:700;color:var(--warn)", "{stats.facts_without_source}" }
                    div { class: "muted",
                        "{loc.dashboard_attention_caption(stats.facts_without_source, deaths, duplicates)}"
                    }
                }
            }
            div { class: "grid-2",
                Card { title: loc.dashboard_label("recent-activity"),
                    {activity_feed(loc, &dashboard.recent)}
                }
                div { class: "stack",
                    Card { title: loc.dashboard_label("jump-back"),
                        {jump_back(recent, &dashboard.jump_back)}
                    }
                    Card { title: loc.dashboard_label("possible-matches"),
                        {possible_matches_card(loc, data_quality)}
                    }
                    Card { title: loc.dashboard_label("data-quality"),
                        {data_quality_card(loc, stats.facts_without_source, data_quality)}
                    }
                }
            }
        }
    }
}

/// The workspace-wide recent-activity timeline; each row that resolves to a record links to it.
fn activity_feed(loc: &Localizer, recent: &[ActivityVm]) -> Element {
    if recent.is_empty() {
        return rsx! { span { class: "muted", "{loc.dashboard_label(\"activity-empty\")}" } };
    }
    rsx! {
        div { class: "timeline", style: "margin-top:8px",
            for (index, row) in recent.iter().enumerate() {
                div {
                    key: "{activity_row_key(index, row)}",
                    class: "tl-item",
                    div { class: "tl-when", "{row.when}" }
                    div { class: "tl-what",
                        if let Some(value) = &row.value {
                            "{row.what}: "
                            b { "{value}" }
                        } else {
                            "{row.what}"
                        }
                        if let Some(count) = &row.count {
                            span { class: "muted tl-count", "{count}" }
                        }
                        if let Some(resume) = &row.resume {
                            ResumeButton {
                                label: loc.import_run_resume(),
                                aria_label: resume.label.clone(),
                                resume: resume.run.clone(),
                            }
                        }
                        if let Some(record) = &row.record {
                            " — "
                            RecordLink {
                                category: record.category,
                                human_id: record.human_id.clone(),
                                label: record.label.clone(),
                                icon: true,
                            }
                        }
                    }
                    div { class: "tl-who", "{row.who}" }
                    if let Some(run) = &row.run {
                        RunDisclosure {
                            run: run.id,
                            label: loc.history_run_changes_toggle(),
                            aria_label: run.label.clone(),
                        }
                    }
                }
            }
        }
    }
}

/// The key of an activity row: a run row by its run, so an open disclosure stays with its run when
/// newer activity shifts the feed; any other row by its position.
fn activity_row_key(index: usize, row: &ActivityVm) -> String {
    match &row.run {
        Some(run) => format!("run-{}", run.id),
        None => format!("row-{index}"),
    }
}

/// An import-run row's disclosure: closed, it reads nothing; open, it lists the records the run
/// changed, read by run so a run of any size lists them all (#588).
#[component]
fn RunDisclosure(run: ImportRunId, label: String, aria_label: String) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "row-actions", style: "margin-top:4px",
            button {
                class: "btn sm ghost",
                r#type: "button",
                aria_expanded: if open() { "true" } else { "false" },
                "aria-label": "{aria_label}",
                onclick: move |_| open.toggle(),
                if open() { "▾ {label}" } else { "▸ {label}" }
            }
        }
        if open() {
            RunRecordsPanel { run }
        }
    }
}

/// The records an open import-run row lists, a page at a time; *Show more* reads one more page.
#[component]
fn RunRecordsPanel(run: ImportRunId) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let nav = use_context::<NavState>();
    let mut pages = use_signal(|| 1_u32);
    let records = use_resource(move || {
        let services = services.clone();
        let _ = nav.data_version.read();
        let pages = pages();
        async move { load_run_records(services, run, pages).await }
    });
    let loc = state.data_loc();
    match &*records.read_unchecked() {
        None => rsx! { p { class: "loading", "{state.chrome().loading()}" } },
        Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(Ok(page)) => run_records_list(loc, run, page, move |()| pages += 1),
    }
}

/// The records an open import-run row lists under its count line, grouped by the source item they came
/// from, each behind its own disclosure of what the run changed in it, then *Show more* when the run
/// changed more than are loaded (#660).
pub fn run_records_list(
    loc: &Localizer,
    run: ImportRunId,
    page: &RunRecordsVm,
    on_more: impl FnMut(()) + 'static,
) -> Element {
    if page.groups.is_empty() {
        return rsx! { p { class: "muted", "{loc.run_records_empty()}" } };
    }
    let on_more = EventHandler::new(on_more);
    let label = loc.history_run_changes_toggle();
    rsx! {
        p { class: "muted", style: "margin-top:4px", "data-hook": "run-counts", "{page.counts}" }
        div { class: "timeline tl-children", "data-hook": "run-records",
            for group in page.groups.iter().cloned() {
                RunRecordItem {
                    key: "{group.lead.kind}-{group.lead.aggregate_id}",
                    run,
                    record: group.lead,
                    label: label.clone(),
                    note: group.summary,
                    members: group.members,
                }
            }
        }
        if page.more {
            button {
                class: "btn sm ghost",
                r#type: "button",
                style: "margin-top:4px",
                onclick: move |_| on_more.call(()),
                "{loc.run_records_more()}"
            }
        }
    }
}

/// One record an open import-run row lists: a link to it, a muted `note` after it, and a disclosure of
/// what the run changed in it, named after the record. A source item's own entity carries the kinds of
/// its records as its note, and those records (`members`) nested under it.
#[component]
fn RunRecordItem(
    run: ImportRunId,
    record: RunRecordVm,
    label: String,
    note: Option<String>,
    members: Vec<RunRecordVm>,
) -> Element {
    let mut open = use_signal(|| false);
    let hook = (!members.is_empty()).then_some("run-group");
    rsx! {
        div { class: "tl-item", "data-hook": hook,
            div { class: "tl-what",
                if let Some(link) = &record.record {
                    RecordLink {
                        category: link.category,
                        human_id: link.human_id.clone(),
                        label: link.label.clone(),
                        icon: true,
                    }
                } else {
                    "{record.label}"
                }
                if let Some(note) = &note {
                    span { class: "muted tl-count", "{note}" }
                }
            }
            div { class: "row-actions", style: "margin-top:4px",
                button {
                    class: "btn sm ghost",
                    r#type: "button",
                    aria_expanded: if open() { "true" } else { "false" },
                    "aria-label": "{record.toggle_label}",
                    onclick: move |_| open.toggle(),
                    if open() { "▾ {label}" } else { "▸ {label}" }
                }
            }
            if open() {
                RunRecordChanges { run, record: record.clone() }
            }
            if !members.is_empty() {
                div { class: "timeline tl-children",
                    for member in members.iter().cloned() {
                        RunRecordItem {
                            key: "{member.kind}-{member.aggregate_id}",
                            run,
                            note: None,
                            record: member,
                            label: label.clone(),
                            members: Vec::new(),
                        }
                    }
                }
            }
        }
    }
}

/// What the run changed in one record, read when the record's disclosure opens.
#[component]
fn RunRecordChanges(run: ImportRunId, record: RunRecordVm) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let nav = use_context::<NavState>();
    let changes = use_resource(move || {
        let services = services.clone();
        let record = record.clone();
        let _ = nav.data_version.read();
        async move { load_run_record_changes(services, run, record).await }
    });
    match &*changes.read_unchecked() {
        None => rsx! { p { class: "loading", "{state.chrome().loading()}" } },
        Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(Ok(entries)) => run_record_changes_list(state.data_loc(), entries),
    }
}

/// What an import run changed in one record, nested under it. Display-only: the Dashboard undoes
/// nothing, so no entry carries an undo control.
pub fn run_record_changes_list(loc: &Localizer, entries: &[HistoryEntryVm]) -> Element {
    let entries = history_entries(loc, entries);
    rsx! {
        div { "data-hook": "run-record-changes",
            HistoryTimeline { entries, onundo: move |_| {}, nested: true }
        }
    }
}

/// The "Jump back in" quick entry points: the persisted recently-opened records when present, else
/// (a fresh workspace) the records derived from recent activity.
fn jump_back(recent: &[RecentItem], fallback: &[JumpVm]) -> Element {
    rsx! {
        div { class: "wrap", style: "margin-top:8px",
            if recent.is_empty() {
                for jump in fallback.iter() {
                    RecordLink {
                        category: jump.record.category,
                        human_id: jump.record.human_id.clone(),
                        label: jump.record.label.clone(),
                        icon: true,
                        button: true,
                    }
                }
            } else {
                for item in recent.iter() {
                    JumpButton { item: item.clone() }
                }
            }
        }
    }
}

/// How many flagged records a check's row lists inline before collapsing the rest into `+N more`.
const MAX_FLAGGED_LINKS: usize = 5;

/// The data-quality card: one row per check with its real count and an action. Death-before-birth
/// lists the flagged persons as navigable links (no list-filter screen exists); facts-without-source
/// keeps its computed count.
///
/// `data_quality` is `None` while the whole-workspace check pass is still loading — the card then
/// shows a localized loading line in place of the check rows (`facts_without_source` comes from the
/// fast dashboard, so it is available immediately but is only shown once the card resolves for a
/// consistent per-card state).
fn data_quality_card(loc: &Localizer, facts_without_source: usize, data_quality: Option<&DataQualityVm>) -> Element {
    let Some(data_quality) = data_quality else {
        return rsx! {
            p { class: "loading", style: "margin-top:4px", "{loc.dashboard_label(\"data-quality-loading\")}" }
        };
    };
    rsx! {
        table { class: "tbl", style: "margin-top:4px",
            caption { class: "sr-only", "{loc.dashboard_label(\"data-quality\")}" }
            tbody {
                tr {
                    td {
                        NoSourceFlag { label: loc.dashboard_label("death-before-birth") }
                    }
                    td { class: "muted", "{data_quality.death_before_birth.len()}" }
                    td { class: "row-actions",
                        {flagged_person_links(loc, &data_quality.death_before_birth)}
                    }
                }
                tr {
                    td {
                        NoSourceFlag { label: loc.dashboard_label("no-source-facts") }
                    }
                    td { class: "muted", "{facts_without_source}" }
                    td {}
                }
            }
        }
    }
}

/// The *Possible matches* card (ADR 0039 §3): how many undecided pairs each kind has, the strongest of
/// them, and *Review*, which opens the Matches tool. Loads with the data-quality check pass, which
/// reads the same pairs.
fn possible_matches_card(loc: &Localizer, data_quality: Option<&DataQualityVm>) -> Element {
    let Some(data_quality) = data_quality else {
        return rsx! {
            p { class: "loading", style: "margin-top:4px", "{loc.dashboard_label(\"data-quality-loading\")}" }
        };
    };
    if data_quality.match_total == 0 {
        return rsx! {
            p { class: "muted", "{loc.dashboard_label(\"no-matches\")}" }
        };
    }
    rsx! {
        div { class: "wrap",
            span { "{loc.dashboard_matches_count(data_quality.match_total)}" }
            span { class: "muted", "{data_quality.match_counts.join(\" · \")}" }
            ReviewButton {
                label: loc.dashboard_label("review"),
                aria_label: loc.dashboard_label("review-label"),
            }
        }
        {match_pairs(loc, &data_quality.matches, data_quality.match_total)}
    }
}

/// The strongest possible matches, with a muted `+N more` for the rest of the `total`: both records as
/// links, the engine's probability and band, and its reasons.
fn match_pairs(loc: &Localizer, pairs: &[QueuedMatchVm], total: usize) -> Element {
    let overflow = total.saturating_sub(pairs.len());
    let tooltip = loc.dashboard_label("score-tooltip");
    rsx! {
        div { class: "stack", style: "margin-top:var(--sp-2)",
            for pair in pairs {
                div {
                    div { class: "wrap",
                        RecordLink {
                            category: pair.a.category,
                            human_id: pair.a.human_id.clone(),
                            label: pair.a.label.clone(),
                            icon: true,
                        }
                        span { class: "muted", "⇄" }
                        RecordLink {
                            category: pair.b.category,
                            human_id: pair.b.human_id.clone(),
                            label: pair.b.label.clone(),
                            icon: true,
                        }
                        span { class: "badge", title: "{tooltip}", "{pair.percent}%" }
                        span { class: "muted", "{pair.band}" }
                    }
                    if !pair.reasons.is_empty() {
                        div { class: "muted match-reasons", "{pair.reasons.join(\" · \")}" }
                    }
                }
            }
            if overflow > 0 {
                span { class: "muted", "{loc.dashboard_more(overflow)}" }
            }
        }
    }
}

/// Renders the death-before-birth check's flagged persons as navigable links, capped at
/// [`MAX_FLAGGED_LINKS`] with a muted `+N more` suffix for the overflow.
fn flagged_person_links(loc: &Localizer, records: &[RecordRef]) -> Element {
    let overflow = records.len().saturating_sub(MAX_FLAGGED_LINKS);
    rsx! {
        div { class: "wrap",
            for record in records.iter().take(MAX_FLAGGED_LINKS) {
                RecordLink {
                    category: record.category,
                    human_id: record.human_id.clone(),
                    label: record.label.clone(),
                }
            }
            if overflow > 0 {
                span { class: "muted", "{loc.dashboard_more(overflow)}" }
            }
        }
    }
}

/// An interrupted import run's *Resume* (ADR 0040 §5): re-runs it in the Import tool. A component so
/// it can resolve `NavState` from context, like [`ReviewButton`].
#[component]
fn ResumeButton(label: String, aria_label: String, resume: RunResume) -> Element {
    let mut nav = use_context::<NavState>();
    rsx! {
        button {
            class: "btn sm ghost",
            style: "margin-left:var(--sp-2)",
            r#type: "button",
            aria_label: "{aria_label}",
            onclick: move |_| nav.resume_import(resume.clone()),
            "{label}"
        }
    }
}

/// The *Possible matches* card's Review action: navigates to the Matches tool (which computes the same
/// pairs). A component so it can resolve `NavState` from context — like [`RecordLink`]/[`JumpButton`] —
/// keeping the card a plain render helper.
#[component]
fn ReviewButton(label: String, aria_label: String) -> Element {
    let mut nav = use_context::<NavState>();
    rsx! {
        button {
            class: "btn sm ghost",
            r#type: "button",
            aria_label: "{aria_label}",
            onclick: move |_| nav.go_to(Destination::Tool(Tool::Matches)),
            "{label}"
        }
    }
}
