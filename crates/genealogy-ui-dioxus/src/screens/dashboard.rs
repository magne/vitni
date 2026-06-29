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
    let loading = state.chrome().loading();
    let nav = use_context::<NavState>();
    let data = use_resource(move || {
        let services = services.clone();
        // Subscribe to `data_version` so a create/edit/undo refreshes the counts and activity.
        let _ = nav.data_version.read();
        async move { load_screen(services, Intent::ShowDashboard).await }
    });
    // The dashboard is the workspace overview, rendered at the root of the work area (not inside a
    // record-tab body), matching `app-shell.html`.
    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::Dashboard(dashboard))) => {
            let recent = nav.recent.read().clone();
            dashboard_view(state.data_loc(), &recent, dashboard)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
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
            | IntentOutcome::DnaMatchDetail(_),
        )) => rsx! {},
    }
}

/// Renders a loaded dashboard: the "at a glance" stat cards, then the activity feed beside the quick
/// entry points and data-quality checks.
pub fn dashboard_view(loc: &Localizer, recent: &[RecentItem], dashboard: &DashboardVm) -> Element {
    let stats = &dashboard.stats;
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
                    div { class: "muted", "{loc.dashboard_label(\"no-source-facts\")}" }
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
                        {data_quality(loc, stats)}
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

/// The "Jump back in" quick entry points: the persisted recently-opened records/tools when present,
/// else (a fresh workspace) the records derived from recent activity.
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

/// The data-quality card: the computable no-source count now, with the remaining checks flagged as
/// arriving in a later milestone (no fabricated numbers).
fn data_quality(loc: &Localizer, stats: &genealogy_ui::DashboardStats) -> Element {
    rsx! {
        table { class: "tbl", style: "margin-top:4px",
            tbody {
                tr {
                    td {
                        NoSourceFlag { label: loc.dashboard_label("no-source-facts") }
                    }
                    td { class: "muted", "{stats.facts_without_source}" }
                }
                tr {
                    td { class: "muted", colspan: "2", "{loc.dashboard_label(\"later-milestone\")}" }
                }
            }
        }
    }
}
