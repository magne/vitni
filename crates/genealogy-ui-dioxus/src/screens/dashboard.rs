use super::prelude::*;

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
    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::Dashboard(dashboard))) => {
            let recent = nav.recent.read().clone();
            let quality = quality.read_unchecked();
            let data_quality = match &*quality {
                Some(ScreenData::Loaded(IntentOutcome::DataQuality(vm))) => Some(vm.as_ref()),
                _ => None,
            };
            dashboard_view(state.data_loc(), &recent, dashboard, data_quality)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::DataQuality(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_),
        )) => rsx! {},
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
        (quality.death_before_birth.len(), quality.duplicate_count)
    });
    rsx! {
        div { style: "padding:var(--sp-6);overflow:auto;height:100%",
            h2 { style: "border:0;margin:0 0 12px", "{loc.dashboard_label(\"title\")}" }
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
            for row in recent.iter() {
                div { class: "tl-item",
                    div { class: "tl-when", "{row.when}" }
                    div { class: "tl-what",
                        "{row.what}"
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
                }
            }
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
/// keeps its computed count; possible-duplicates offers a Compare button into the merge wizard.
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
                tr {
                    td { "⇄ {loc.dashboard_label(\"possible-duplicates\")}" }
                    td { class: "muted", "{data_quality.duplicate_count}" }
                    td { class: "row-actions",
                        CompareButton { label: loc.dashboard_label("compare") }
                    }
                }
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

/// The possible-duplicates row's Compare action: navigates to the merge wizard (the merge tool loads
/// the same candidate pairs). A component so it can resolve `NavState` from context — like
/// [`RecordLink`]/[`JumpButton`] — keeping `data_quality` a plain render helper.
#[component]
fn CompareButton(label: String) -> Element {
    let mut nav = use_context::<NavState>();
    rsx! {
        button {
            class: "btn sm ghost",
            r#type: "button",
            onclick: move |_| nav.go_to(Destination::Tool(Tool::Merge)),
            "{label}"
        }
    }
}
