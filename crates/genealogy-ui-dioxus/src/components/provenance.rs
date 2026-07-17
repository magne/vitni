//! The provenance block shown above Save on every edit form (`record-editing.html` §5b): the "why"
//! (rationale), the backing citations, the Evidence Explained axes, and the confidence — captured
//! once per save and applied to every assertion the form emits (operator + timestamp come from the
//! session, never typed). Controlled: the whole block binds to a [`ProvenanceDraft`] signal owned by
//! the form, so the form reads `draft()` when it dispatches the save.

use dioxus::prelude::*;
use genealogy_ui::{
    Category, CitationChangeSetRequest, CitationSourceRequest, ConfidenceLevel, EVIDENCE_KINDS, EvidenceAxis,
    INFORMATION_KINDS, Localizer, NewSourceFields, PickerSelection, PickerState, ProvenanceDraft, RecordLink,
    SOURCE_QUALITIES,
};

use crate::app::AppCtx;
use crate::components::record_picker::{
    PickerCallbacks, PickerConfig, RecordPicker, draft_card, picker_options, record_picker,
};
use crate::components::{Chip, SelectChoice, SelectInput, TextInput};
use crate::services::{Services, commit_citation_change_set, load_picker_rows};

/// One evidence-analysis axis select in the block: its accessible name and its options (the first of
/// which is the unset "—"), tagged with the axis it drives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceAxis {
    /// Which analysis axis this select drives.
    pub axis: EvidenceAxis,
    /// The already-localized accessible name (e.g. "Source quality").
    pub aria_label: String,
    /// The options, unset ("—", value "") first, then one per axis value (value = its index).
    pub options: Vec<SelectChoice>,
}

/// The provenance block: rationale · citations · evidence axes · confidence, all bound to `draft`.
///
/// Every visible string arrives already localized; the enum ↔ index mapping for the selects lives
/// here (the option values are indices into [`SOURCE_QUALITIES`] / [`INFORMATION_KINDS`] /
/// [`EVIDENCE_KINDS`] / [`ConfidenceLevel::all`]).
#[component]
pub fn ProvenanceBlock(
    /// The draft this block edits (owned by the form; read back at save time).
    draft: Signal<ProvenanceDraft>,
    /// The block's accessible group name ("Provenance").
    heading: String,
    /// The rationale field label ("Reason for this change").
    reason_label: String,
    /// The rationale field hint ("optional · shown in History").
    reason_hint: String,
    /// The confidence select label / accessible name ("Confidence").
    confidence_label: String,
    /// The evidence-row label ("Evidence").
    evidence_label: String,
    /// The five confidence options, index-valued and aligned to [`ConfidenceLevel::all`].
    confidence_options: Vec<SelectChoice>,
    /// The three evidence-analysis axis selects, in display order.
    axes: Vec<ProvenanceAxis>,
    /// Whether to offer the "cite a DNA match" evidence picker (person/family relationship
    /// inferences — data-model §12, ADR 0023). Off for aggregates a DNA match cannot back.
    #[props(default)]
    allow_dna_evidence: bool,
) -> Element {
    let mut draft = draft;
    let confidence_index = draft()
        .confidence
        .and_then(|level| ConfidenceLevel::all().iter().position(|l| *l == level))
        .map(|index| index.to_string())
        .unwrap_or_default();
    // A `.card` per `record-editing.html` §5b — the block reads as one bounded unit wherever it
    // renders (tab body, create pane, side panel), not a bare run of fields.
    rsx! {
        div { class: "card", role: "group", aria_label: "{heading}",
            div { class: "stack",
            div { class: "field", style: "margin-bottom:0",
                label { r#for: "prov-reason",
                    "{reason_label} "
                    span { class: "faint", "({reason_hint})" }
                }
                TextInput {
                    id: "prov-reason",
                    name: "prov-reason",
                    oninput: move |event: FormEvent| draft.write().rationale = event.value(),
                }
            }
            ProvenanceCitations { draft }
            if allow_dna_evidence {
                ProvenanceDnaMatches { draft }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "{evidence_label}" }
                span { class: "grow wrap",
                    for axis in axes.iter() {
                        {axis_select(draft, axis)}
                    }
                }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "{confidence_label}" }
                SelectInput {
                    style: "width:auto",
                    aria_label: "{confidence_label}",
                    selected: confidence_index,
                    options: confidence_options,
                    onchange: move |event: FormEvent| {
                        let index = event.value().parse::<usize>().ok();
                        draft.write().confidence = index.and_then(|i| ConfidenceLevel::all().get(i).copied());
                    },
                }
            }
            }
        }
    }
}

/// Renders one evidence-analysis axis select, mapping the chosen option index back to the axis's
/// enum value (or `None` for the unset "—" option).
fn axis_select(mut draft: Signal<ProvenanceDraft>, axis: &ProvenanceAxis) -> Element {
    let current = match axis.axis {
        EvidenceAxis::Source => draft()
            .source
            .and_then(|value| SOURCE_QUALITIES.iter().position(|q| *q == value)),
        EvidenceAxis::Information => draft()
            .information
            .and_then(|value| INFORMATION_KINDS.iter().position(|k| *k == value)),
        EvidenceAxis::Evidence => draft()
            .evidence
            .and_then(|value| EVIDENCE_KINDS.iter().position(|k| *k == value)),
    };
    let current = current.map(|index| index.to_string()).unwrap_or_default();
    let which = axis.axis;
    let options = axis.options.clone();
    let aria_label = axis.aria_label.clone();
    rsx! {
        SelectInput {
            style: "width:auto",
            aria_label: "{aria_label}",
            selected: current,
            options,
            onchange: move |event: FormEvent| {
                let index = event.value().parse::<usize>().ok();
                match which {
                    EvidenceAxis::Source => {
                        draft.write().source = index.and_then(|i| SOURCE_QUALITIES.get(i).copied());
                    }
                    EvidenceAxis::Information => {
                        draft.write().information = index.and_then(|i| INFORMATION_KINDS.get(i).copied());
                    }
                    EvidenceAxis::Evidence => {
                        draft.write().evidence = index.and_then(|i| EVIDENCE_KINDS.get(i).copied());
                    }
                }
            },
        }
    }
}

/// The citations row of the provenance block: the attached-citation chips plus a find-or-create
/// citation picker (`record-editing.html` §6b). Picking an existing citation appends its `human_id`
/// to the draft (never a blind free-text id); "+ New citation" opens an inline
/// [`ProvenanceNewCitation`] card that commits a new citation and appends it. Split out of
/// [`ProvenanceBlock`] so the picker's hooks (options resource, picker state) stay isolated and the
/// parent stays within the length cap.
///
/// In the app this renders inside an `AppCtx::Ready` context for its services and localizer; the SSR
/// tests render it without an `AppCtx`, falling back to a baseline localizer and empty options so the
/// markup (labels, placeholder) is still exercised.
#[component]
fn ProvenanceCitations(draft: Signal<ProvenanceDraft>) -> Element {
    let mut draft = draft;
    let mut picker_state = use_signal(PickerState::default);
    let mut new_open = use_signal(|| false);
    let ctx = try_consume_context::<AppCtx>();
    let services = ctx_services(ctx.as_ref());
    let row_services = services.clone();
    let rows = use_resource(move || {
        let services = row_services.clone();
        async move {
            match services {
                Some(services) => load_picker_rows(services, Category::Citations).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let onpick = use_callback(move |selection: PickerSelection| {
        draft.write().citations.push(selection.human_id);
        picker_state.write().clear();
    });
    let onclear = use_callback(move |()| {});
    let onnew = use_callback(move |_query: String| new_open.set(true));
    let fallback;
    let loc: &Localizer = match &ctx {
        Some(AppCtx::Ready(state)) => state.data_loc(),
        Some(AppCtx::Failed(_)) | None => {
            fallback = Localizer::with_languages(None, &[]);
            &fallback
        }
    };
    let citations = draft().citations;
    let citations_label = loc.field_label("citations");
    let detach_label = loc.action_label("detach-citation");
    let picker = RecordPicker {
        config: PickerConfig {
            label: loc.provenance_attach_citation(),
            name: "prov-citation".to_owned(),
            entity_label: loc.picker_entity(Category::Citations),
            allow_new: true,
        },
        state: picker_state,
        options: picker_options(rows.read_unchecked().as_ref()),
        exclude: citations.clone(),
        callbacks: PickerCallbacks { onpick, onclear, onnew },
    };
    rsx! {
        div { class: "fact-row",
            span { class: "field-label", style: "width:96px;margin:0", "{citations_label}" }
            span { class: "grow",
                span { class: "wrap",
                    for (index , cid) in citations.iter().enumerate() {
                        Chip {
                            key: "{index}",
                            label: cid.clone(),
                            icon: "❝".to_owned(),
                            delete_label: detach_label.clone(),
                            ondelete: move |()| {
                                draft.write().citations.remove(index);
                            },
                        }
                    }
                }
                {record_picker(loc, &picker)}
                if new_open() {
                    ProvenanceNewCitation { draft, onclose: move |()| new_open.set(false) }
                }
            }
        }
    }
}

/// The DNA-match evidence row of the provenance block (data-model §12, ADR 0023): the cited-match
/// chips plus a picker of existing DNA matches. Picking a match appends its `human_id` to the draft's
/// `dna_matches`, recording a DNA-backed relationship inference on the person/family assertion. There
/// is no "+ New" here — a match is observed on the DNA screens, never minted inline as evidence.
/// Rendered only when the form opts in (`allow_dna_evidence`), so it appears on relationship-bearing
/// forms and not on aggregates a match cannot back.
#[component]
fn ProvenanceDnaMatches(draft: Signal<ProvenanceDraft>) -> Element {
    let mut draft = draft;
    let mut picker_state = use_signal(PickerState::default);
    let ctx = try_consume_context::<AppCtx>();
    let services = ctx_services(ctx.as_ref());
    let row_services = services.clone();
    let rows = use_resource(move || {
        let services = row_services.clone();
        async move {
            match services {
                Some(services) => load_picker_rows(services, Category::DnaMatches).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let onpick = use_callback(move |selection: PickerSelection| {
        draft.write().dna_matches.push(selection.human_id);
        picker_state.write().clear();
    });
    let onclear = use_callback(move |()| {});
    let onnew = use_callback(move |_query: String| {});
    let fallback;
    let loc: &Localizer = match &ctx {
        Some(AppCtx::Ready(state)) => state.data_loc(),
        Some(AppCtx::Failed(_)) | None => {
            fallback = Localizer::with_languages(None, &[]);
            &fallback
        }
    };
    let dna_matches = draft().dna_matches;
    let label = loc.field_label("dna-evidence");
    let detach_label = loc.action_label("detach-dna-match");
    let picker = RecordPicker {
        config: PickerConfig {
            label: loc.provenance_attach_dna_match(),
            name: "prov-dna-match".to_owned(),
            entity_label: loc.picker_entity(Category::DnaMatches),
            allow_new: false,
        },
        state: picker_state,
        options: picker_options(rows.read_unchecked().as_ref()),
        exclude: dna_matches.clone(),
        callbacks: PickerCallbacks { onpick, onclear, onnew },
    };
    rsx! {
        div { class: "fact-row",
            span { class: "field-label", style: "width:96px;margin:0", "{label}" }
            span { class: "grow",
                span { class: "wrap",
                    for (index , mid) in dna_matches.iter().enumerate() {
                        Chip {
                            key: "{index}",
                            label: mid.clone(),
                            icon: "🔗".to_owned(),
                            delete_label: detach_label.clone(),
                            ondelete: move |()| {
                                draft.write().dna_matches.remove(index);
                            },
                        }
                    }
                }
                {record_picker(loc, &picker)}
            }
        }
    }
}

/// The inline "new citation" draft card mounted from the citations picker's "+ New citation" row
/// (`record-editing.html` §6b): a required source find-or-create picker (its own "+ New" creates a
/// source inline by title) plus a page input. Add commits the citation via
/// [`commit_citation_change_set`] and appends the returned `human_id` to the draft — commit-on-add,
/// so the record persists even if the outer form is later cancelled. A commit failure is shown in
/// place, not swallowed.
#[component]
fn ProvenanceNewCitation(draft: Signal<ProvenanceDraft>, onclose: EventHandler<()>) -> Element {
    let mut draft = draft;
    let source_state = use_signal(PickerState::default);
    let mut source_link = use_signal(RecordLink::<NewSourceFields>::default);
    let page = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let ctx = try_consume_context::<AppCtx>();
    let services = ctx_services(ctx.as_ref());
    let source_services = services.clone();
    let source_rows = use_resource(move || {
        let services = source_services.clone();
        async move {
            match services {
                Some(services) => load_picker_rows(services, Category::Sources).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let source_onpick =
        use_callback(move |selection: PickerSelection| source_link.set(RecordLink::Existing(selection)));
    let source_onclear = use_callback(move |()| source_link.set(RecordLink::Empty));
    let source_onnew =
        use_callback(move |query: String| source_link.set(RecordLink::New(NewSourceFields { title: query })));
    let add_services = services.clone();
    let on_add = use_callback(move |()| {
        let Some(services) = add_services.clone() else {
            return;
        };
        let source = source_link.read().clone();
        let page_value = page.read().clone();
        let Some(request) = build_citation_request(&source, &page_value) else {
            return;
        };
        spawn(async move {
            match commit_citation_change_set(services, request, ProvenanceDraft::default()).await {
                Ok(id) => {
                    draft.write().citations.push(id);
                    onclose.call(());
                }
                Err(message) => error.set(Some(message)),
            }
        });
    });
    let can_add = source_link.read().is_set() && services.is_some();
    let fallback;
    let loc: &Localizer = match &ctx {
        Some(AppCtx::Ready(state)) => state.data_loc(),
        Some(AppCtx::Failed(_)) | None => {
            fallback = Localizer::with_languages(None, &[]);
            &fallback
        }
    };
    let title = loc.citation_new_title();
    let source_picker = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("source"),
            name: "prov-new-source".to_owned(),
            entity_label: loc.picker_entity(Category::Sources),
            allow_new: true,
        },
        state: source_state,
        options: picker_options(source_rows.read_unchecked().as_ref()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: source_onpick,
            onclear: source_onclear,
            onnew: source_onnew,
        },
    };
    let body = new_citation_body(loc, &source_picker, page, error, can_add, on_add);
    draft_card(
        &title,
        &loc.draft_card_badge(),
        loc.draft_card_discard(&title),
        Callback::new(move |()| onclose.call(())),
        body,
    )
}

/// The body of the inline new-citation card: the source picker, the page input, an in-place commit
/// error (when present), and the Add button (disabled until a source is chosen). Factored out of
/// [`ProvenanceNewCitation`] to keep it within the length cap.
fn new_citation_body(
    loc: &Localizer,
    source_picker: &RecordPicker,
    mut page: Signal<String>,
    error: Signal<Option<String>>,
    can_add: bool,
    on_add: Callback<()>,
) -> Element {
    let page_label = loc.field_label("page");
    let add_label = loc.provenance_new_citation_add();
    rsx! {
        div { class: "stack",
            {record_picker(loc, source_picker)}
            div { class: "field",
                label { r#for: "prov-new-page", "{page_label}" }
                TextInput {
                    id: "prov-new-page",
                    name: "prov-new-page",
                    value: "{page}",
                    oninput: move |event: FormEvent| page.set(event.value()),
                }
            }
            if let Some(message) = error() {
                span { class: "field-error", role: "alert", "{message}" }
            }
            button {
                class: "btn sm primary",
                r#type: "button",
                disabled: !can_add,
                onclick: move |_| on_add.call(()),
                "{add_label}"
            }
        }
    }
}

/// Renders the inline new-citation card in isolation, for SSR tests. The card is otherwise reachable
/// only by clicking the citations picker's "+ New citation" row, which SSR cannot drive.
pub fn provenance_new_citation_card(draft: Signal<ProvenanceDraft>) -> Element {
    rsx! {
        ProvenanceNewCitation { draft, onclose: move |()| {} }
    }
}

/// The services handle from an [`AppCtx`], or `None` when startup failed or no context is present
/// (an SSR render — the picker then loads no options and Add is disabled).
fn ctx_services(ctx: Option<&AppCtx>) -> Option<Services> {
    match ctx {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        Some(AppCtx::Failed(_)) | None => None,
    }
}

/// Builds the [`CitationChangeSetRequest`] for the inline new-citation card, or `None` when the
/// required source is unset. The inline citation records no confidence, evidence analysis, or
/// cited-record date of its own.
fn build_citation_request(source: &RecordLink<NewSourceFields>, page: &str) -> Option<CitationChangeSetRequest> {
    let source = match source {
        RecordLink::Existing(selection) => CitationSourceRequest::Existing(selection.human_id.clone()),
        RecordLink::New(fields) => CitationSourceRequest::New {
            title: non_blank(&fields.title),
        },
        RecordLink::Empty => return None,
    };
    Some(CitationChangeSetRequest {
        source,
        page: non_blank(page),
        confidence: None,
        evidence: None,
        date: None,
    })
}

/// The trimmed value, or `None` when it is blank.
fn non_blank(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value.to_owned()) }
}
