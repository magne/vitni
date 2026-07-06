use genealogy_ui::{EVIDENCE_KINDS, EvidenceAxis, INFORMATION_KINDS, ProvenanceDraft, SOURCE_QUALITIES, tab_label};

use super::prelude::*;
use crate::components::{ProvenanceAxis, ProvenanceBlock};
use crate::services::resolve_record_name;
use crate::shell::{CachedName, NameCache, NameState};

/// A clickable link to another record's detail screen: opens it as a tab and navigates to its
/// category (resolving `NavState` from context, so any screen can drop it in). Shared by the
/// dashboard feed/jump-back and every detail tab that references a record.
///
/// The link shows the record's **current** name, resolved live through the shared [`NameCache`] and
/// falling back to the human id when the record has no name (the [`tab_label`] rule). The supplied
/// `label` is only the placeholder shown until resolution lands (and the sole text under bare SSR,
/// where no cache/[`AppCtx`] is present). `icon` prefixes the entity emoji (off for table cells);
/// `button` renders the button-chip style (the jump-back pills) instead of the inline link style.
#[component]
pub fn RecordLink(
    category: Category,
    human_id: String,
    label: String,
    #[props(default)] icon: bool,
    #[props(default)] button: bool,
) -> Element {
    let mut nav = use_context::<NavState>();
    let version = *nav.data_version.read();
    let cache = try_consume_context::<NameCache>();
    let services = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        _ => None,
    };
    let key = (category.id().to_owned(), human_id.clone());

    // The resolved name (id fallback via `tab_label`) when the cache holds an entry for this data
    // version; otherwise the supplied label, or the id when even that is blank.
    let resolved = cache
        .and_then(|cache| cache.0.read().get(&key).cloned())
        .filter(|entry| entry.version == version)
        .map(|entry| entry.state);
    let display = match resolved {
        Some(NameState::Ready(name)) => tab_label(name.as_deref(), &human_id),
        _ if !label.is_empty() => label.clone(),
        _ => human_id.clone(),
    };

    // Resolve on a miss and re-resolve after a data change, off the render via an effect. A no-op
    // when there is no cache or no ready workspace (bare SSR): the link then keeps its placeholder.
    let key_for_effect = key.clone();
    let human_for_effect = human_id.clone();
    use_effect(move || {
        let version = *nav.data_version.read();
        let (Some(mut cache), Some(services)) = (cache, services.clone()) else {
            return;
        };
        if cache
            .0
            .peek()
            .get(&key_for_effect)
            .is_some_and(|entry| entry.version == version)
        {
            return;
        }
        cache.0.write().insert(
            key_for_effect.clone(),
            CachedName {
                version,
                state: NameState::Loading,
            },
        );
        let services = services.clone();
        let key = key_for_effect.clone();
        let human_id = human_for_effect.clone();
        spawn(async move {
            let name = resolve_record_name(services, category, human_id).await;
            cache.0.write().insert(
                key,
                CachedName {
                    version,
                    state: NameState::Ready(name),
                },
            );
        });
    });

    let record = RecordRef {
        category,
        human_id,
        label: display.clone(),
    };
    let class = if button { "btn" } else { "src-link" };
    rsx! {
        button {
            class,
            r#type: "button",
            onclick: move |_| {
                nav.go_to(Destination::Category(record.category));
                nav.open_record(record.clone());
            },
            if icon {
                span { aria_hidden: "true", "{category.icon()} " }
            }
            "{display}"
        }
    }
}

/// A "Jump back in" pill for a persisted recent item: a record link (the shared [`RecordLink`]) or a
/// tool button that navigates to the tool. An unknown kind/tool (e.g. after a vocabulary change) and
/// a missing shell context render nothing.
#[component]
pub fn JumpButton(item: RecentItem) -> Element {
    match item {
        RecentItem::Record { kind, human_id, label } => match Category::from_aggregate_kind(&kind) {
            Some(category) => rsx! {
                RecordLink { category, human_id, label, icon: true, button: true }
            },
            None => rsx! {},
        },
        RecentItem::Tool { tool } => {
            let (Some(mut nav), Some(tool)) = (try_consume_context::<NavState>(), Tool::from_id(&tool)) else {
                return rsx! {};
            };
            let label = try_consume_context::<ChromeCtx>()
                .map_or_else(|| tool.id().to_owned(), |chrome| chrome.0.rail_label(tool.label_id()));
            let icon = tool.icon();
            rsx! {
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: move |_| nav.go_to(Destination::Tool(tool)),
                    span { aria_hidden: "true", "{icon} " }
                    "{label}"
                }
            }
        }
    }
}

/// A minimal list of related-item ids, or an empty-state when there are none.
pub fn id_list(loc: &Localizer, ids: &[String]) -> Element {
    if ids.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        ul { class: "id-list",
            for id in ids.iter() {
                li { "{id}" }
            }
        }
    }
}

/// The Media tab: a thumbnail gallery, one placeholder card per attached media id.
pub fn media_gallery(loc: &Localizer, media: &[String]) -> Element {
    if media.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-3",
            for id in media.iter() {
                div { class: "card", style: "text-align:center",
                    div {
                        class: "faint",
                        style: "height:120px;background:var(--panel-2);border-radius:var(--r-md);display:grid;place-items:center",
                        "🖼"
                    }
                    div { style: "margin-top:8px", "{id}" }
                }
            }
        }
    }
}

/// The Tags tab: each applied tag as a name + colour-dot chip (never its UUID — data-model §9).
pub fn tags_panel(loc: &Localizer, tags: &[TagRef]) -> Element {
    if tags.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "wrap",
            for tag in tags.iter() {
                Chip { label: tag.name.clone(), dot_color: tag.color.clone() }
            }
        }
    }
}

/// Returns `None` for a blank field (so an absent field is not asserted), else the value as typed.
#[must_use]
pub fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() { None } else { Some(value) }
}

/// The evidence-first source cue: a source-count link, or a no-source flag when unsourced.
pub fn source_cue(loc: &Localizer, source_count: usize) -> Element {
    if source_count > 0 {
        rsx! { SourceLink { label: loc.source_count(source_count), onclick: move |_| {} } }
    } else {
        rsx! { NoSourceFlag { label: loc.no_source() } }
    }
}

/// The Media tab: a thumbnail gallery, one card per attached media object (caption or id).
pub fn family_media_gallery(loc: &Localizer, media: &[FamilyMediaVm]) -> Element {
    if media.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-3",
            for item in media.iter() {
                div { class: "card", style: "text-align:center",
                    div {
                        class: "faint",
                        style: "height:120px;background:var(--panel-2);border-radius:var(--r-md);display:grid;place-items:center",
                        "🖼"
                    }
                    div { style: "margin-top:8px", {item.caption.clone().unwrap_or_else(|| item.human_id.clone())} }
                }
            }
        }
    }
}

/// A shared citations table (Event/Place Citations tab): source · page · surety · evidence axes.
pub fn citation_table(loc: &Localizer, citations: &[CitationRefVm]) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("source"),
                loc.field_label("page"),
                loc.field_label("surety"),
                loc.field_label("evidence"),
            ],
            for citation in citations.iter() {
                tr {
                    td {
                        if let Some(source_id) = &citation.source_id {
                            RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())}
                        }
                    }
                    td { class: "muted", {citation.page.clone().unwrap_or_else(|| "—".to_owned())} }
                    td {
                        if let (Some(level), Some(label)) = (citation.confidence, citation.confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { class: "wrap",
                        for chip in citation.evidence_axes.iter() {
                            EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                        }
                    }
                }
            }
        }
    }
}

/// The evidence-first source cue with a "Why we believe" popover: a source-count link that, on
/// activation, floats the claim's citations beside it; or a no-source flag when the claim is
/// unsourced. The Overview-pane counterpart of [`source_cue`] (which stays plain for the tabs).
pub fn provenance_cue(loc: &Localizer, title: String, citations: &[CitationRefVm]) -> Element {
    if citations.is_empty() {
        rsx! { NoSourceFlag { label: loc.no_source() } }
    } else {
        rsx! {
            ProvenanceTrigger {
                label: loc.source_count(citations.len()),
                title,
                dismiss_label: loc.action_label("dismiss"),
                citations: citations.to_vec(),
            }
        }
    }
}

/// A source-count link that toggles an anchored "Why we believe" popover listing the claim's
/// citations. Self-contained: owns its open state, dismissed by Esc or by clicking the backdrop.
#[component]
pub fn ProvenanceTrigger(
    /// The already-localized source-count text (e.g. "2 sources").
    label: String,
    /// The already-localized popover heading (e.g. "Why we believe: Birth").
    title: String,
    /// The already-localized accessible name for the dismiss backdrop.
    dismiss_label: String,
    /// The claim's citations, rendered as provenance rows.
    citations: Vec<CitationRefVm>,
) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        span { class: "prov-anchor",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    open.set(false);
                }
            },
            button {
                class: "src-link",
                r#type: "button",
                style: "border:0;background:none;font:inherit;cursor:pointer",
                aria_haspopup: "dialog",
                aria_expanded: if open() { "true" } else { "false" },
                onclick: move |_| {
                    let now = open();
                    open.set(!now);
                },
                "❝ {label}"
            }
            if open() {
                button {
                    class: "prov-backdrop",
                    r#type: "button",
                    aria_label: dismiss_label,
                    onclick: move |_| open.set(false),
                }
                ProvenancePopover { title,
                    for citation in citations.iter() {
                        {provenance_claim_row(citation)}
                    }
                }
            }
        }
    }
}

/// One provenance claim row inside the popover: surety badge, the backing source (link + page), the
/// Evidence Explained axes, and the "asserted by" line. Reuses the shared evidence components.
pub fn provenance_claim_row(citation: &CitationRefVm) -> Element {
    let label = citation
        .source
        .clone()
        .unwrap_or_else(|| citation.source_id.clone().unwrap_or_else(|| citation.human_id.clone()));
    rsx! {
        div { class: "prov-claim",
            if let (Some(level), Some(confidence_label)) = (citation.confidence, citation.confidence_label.clone()) {
                ConfidenceBadge { level, label: confidence_label }
            }
            div {
                div {
                    if let Some(source_id) = &citation.source_id {
                        RecordLink { category: Category::Sources, human_id: source_id.clone(), label }
                    } else {
                        "{label}"
                    }
                    if let Some(page) = &citation.page {
                        span { class: "muted", " — {page}" }
                    }
                }
                if !citation.evidence_axes.is_empty() {
                    div { class: "wrap", style: "margin-top:4px",
                        for chip in citation.evidence_axes.iter() {
                            EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                        }
                    }
                }
                if let Some(asserted_by) = &citation.asserted_by {
                    div { class: "tl-who", "{asserted_by}" }
                }
            }
        }
    }
}

/// The provenance block (`record-editing.html` §5b) for an edit form, bound to `draft`: resolves
/// every label via `loc` and hands the controlled block its options. Rendered directly above a form's
/// Save button; the form reads `draft()` when it dispatches the save. The confidence select and the
/// three evidence-axis selects are index-valued (into [`ConfidenceLevel::all`] and the axis consts),
/// with a leading unset "—" on each axis.
pub fn provenance_block(loc: &Localizer, draft: Signal<ProvenanceDraft>) -> Element {
    let confidence_options: Vec<SelectChoice> = ConfidenceLevel::all()
        .iter()
        .enumerate()
        .map(|(index, level)| SelectChoice {
            value: index.to_string(),
            label: loc.confidence_label(*level),
        })
        .collect();
    let unset = loc.evidence_axis_unset();
    let axes = vec![
        ProvenanceAxis {
            axis: EvidenceAxis::Source,
            aria_label: loc.evidence_axis_label(EvidenceAxis::Source),
            options: axis_options(
                &unset,
                SOURCE_QUALITIES.iter().map(|value| loc.evidence_source_label(*value)),
            ),
        },
        ProvenanceAxis {
            axis: EvidenceAxis::Information,
            aria_label: loc.evidence_axis_label(EvidenceAxis::Information),
            options: axis_options(
                &unset,
                INFORMATION_KINDS
                    .iter()
                    .map(|value| loc.evidence_information_label(*value)),
            ),
        },
        ProvenanceAxis {
            axis: EvidenceAxis::Evidence,
            aria_label: loc.evidence_axis_label(EvidenceAxis::Evidence),
            options: axis_options(
                &unset,
                EVIDENCE_KINDS.iter().map(|value| loc.evidence_kind_label(*value)),
            ),
        },
    ];
    rsx! {
        ProvenanceBlock {
            draft,
            heading: loc.provenance_heading(),
            reason_label: loc.provenance_reason_label(),
            reason_hint: loc.provenance_reason_hint(),
            citations_label: loc.field_label("citations"),
            attach_label: loc.provenance_attach_citation(),
            detach_label: loc.action_label("detach-citation"),
            confidence_label: loc.field_label("confidence"),
            evidence_label: loc.field_label("evidence"),
            confidence_options,
            axes,
        }
    }
}

/// Builds an axis select's options: the unset "—" (value "") first, then one option per axis value
/// with its index as the value.
fn axis_options(unset: &str, labels: impl Iterator<Item = String>) -> Vec<SelectChoice> {
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: unset.to_owned(),
    }];
    for (index, label) in labels.enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label,
        });
    }
    options
}

// ---------------------------------------------------------------------------------------------------
// Event slice
// ---------------------------------------------------------------------------------------------------

/// The source media types offered by the link forms (a common subset; the model has more).
#[must_use]
pub fn source_media_type_choices() -> [SourceMediaType; 6] {
    [
        SourceMediaType::Book,
        SourceMediaType::Film,
        SourceMediaType::Electronic,
        SourceMediaType::Fiche,
        SourceMediaType::Manuscript,
        SourceMediaType::Photo,
    ]
}
