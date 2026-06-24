//! View-models: framework-neutral, render-ready shapes derived from `genealogy-app` DTOs.
//!
//! A view-model carries already-localized display strings (built via the [`Localizer`]) so a
//! renderer stays dumb — it lays out fields, it does not format or localize. The structured parts a
//! renderer might branch on (e.g. `private`) stay typed. A list row is the generic [`RowVm`]; the
//! detail tab strip is [`DetailTab`]s.

use std::collections::HashMap;

use genealogy_app::{
    AssociationSummary, ChangeLogEntry, CitationSummary, EvidenceAnalysis, EvidenceLevel, FactSummary, FactType,
    FamilyForPerson, NameSummary, OperatorKind, PersonFamilyRole, PersonName, PersonSummary, TagRef, WorkspaceCounts,
};

use crate::detail::DetailTab;
use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{Category, RecordRef};
use crate::presentation::{ConfidenceLevel, EvidenceAxis, RestrictionKind};

/// Builds a generic list row from a [`PersonSummary`], localizing the name and sex via `loc`.
///
/// The subtitle is the localized sex label for now; later slices enrich it with vital dates and a
/// primary place. The avatar is the person's initials, or `?` when no name is known.
#[must_use]
pub fn person_row(summary: &PersonSummary, loc: &Localizer) -> RowVm {
    RowVm {
        id: summary.human_id.clone(),
        title: loc.display_name(summary.display_name.as_deref()),
        subtitle: Some(loc.sex_label(summary.sex.as_ref())),
        avatar: Some(initials(summary)),
    }
}

/// The person's initials from the structured given/surname, or `?` when neither is known.
fn initials(summary: &PersonSummary) -> String {
    let mut initials = String::new();
    for part in [summary.given.as_deref(), summary.surname.as_deref()] {
        if let Some(first) = part.and_then(|name| name.chars().next()) {
            initials.push(first.to_ascii_uppercase());
        }
    }
    if initials.is_empty() {
        initials.push('?');
    }
    initials
}

/// One asserted name variant, for the Names tab — carrying its evidence cues (surety + source count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameVm {
    /// The localized name-type label.
    pub type_label: String,
    /// The rendered `given surname(s)` display string.
    pub display: String,
    /// The given name, if any.
    pub given: Option<String>,
    /// The primary surname, if any.
    pub surname: Option<String>,
    /// The nickname, if any.
    pub nickname: Option<String>,
    /// The localized date this name was in use, if known.
    pub date: Option<String>,
    /// The BCP-47 language tag of this name, if known.
    pub language: Option<String>,
    /// The name's confidence, as a presentation level (drives the badge colour token).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this name (its source count).
    pub source_count: usize,
}

impl NameVm {
    /// Whether the name has at least one backing source (drives the no-source flag).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// One asserted fact, for the Facts tab — the evidence-first row (confidence + source count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactVm {
    /// The localized fact-type label.
    pub type_label: String,
    /// The fact's free-text value, if any.
    pub value: Option<String>,
    /// The localized rendered date, if any.
    pub date: Option<String>,
    /// The fact's confidence, as a presentation level (drives the badge colour token).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this fact (its source count).
    pub source_count: usize,
}

impl FactVm {
    /// Whether the fact has at least one backing source (drives the no-source flag).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// One event participation, for the Events tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRefVm {
    /// The event's user-facing id (e.g. `E0001`).
    pub event_id: String,
    /// The localized participant-role label.
    pub role_label: String,
    /// The localized rendered event date, if known.
    pub date: Option<String>,
}

/// One change-log entry, for the History tab — who changed what, when, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntryVm {
    /// The localized timestamp (e.g. `2026-06-22 14:35`).
    pub when: String,
    /// The localized summary of what changed (e.g. `Name asserted`).
    pub what: String,
    /// The localized operator line (e.g. `magne · High` or `gedcom-import (software agent)`).
    pub who: String,
    /// The operator's rationale, if recorded.
    pub why: Option<String>,
    /// The assertion this entry recorded (the undo target).
    pub assertion_id: String,
    /// Whether this entry can be undone (drives the undo control).
    pub can_undo: bool,
}

impl HistoryEntryVm {
    /// Builds a history view-model from an app [`ChangeLogEntry`], localizing the summary + operator.
    #[must_use]
    pub fn from_entry(entry: &ChangeLogEntry, loc: &Localizer) -> Self {
        Self {
            when: friendly_timestamp(&entry.occurred_at),
            what: loc.change_summary(&entry.event_type),
            who: loc.operator_line(entry),
            why: entry.rationale.clone(),
            assertion_id: entry.assertion_id.clone(),
            can_undo: entry.can_undo,
        }
    }
}

/// Builds the History-tab rows, collapsing consecutive same-software-agent runs (e.g. an import) into
/// one `"N records imported"` entry — the same grouping as the dashboard activity feed. A collapsed
/// run is not individually undoable (it stands for many assertions), so it carries no undo control.
#[must_use]
pub fn collapse_history(entries: &[ChangeLogEntry], loc: &Localizer) -> Vec<HistoryEntryVm> {
    let mut rows = Vec::new();
    let mut index = 0;
    while index < entries.len() {
        let entry = &entries[index];
        let run = software_run_len(entries, index);
        if run >= 2 {
            rows.push(HistoryEntryVm {
                when: friendly_timestamp(&entry.occurred_at),
                what: loc.activity_import_batch(run),
                who: loc.operator_line(entry),
                why: None,
                assertion_id: String::new(),
                can_undo: false,
            });
            index += run;
        } else {
            rows.push(HistoryEntryVm::from_entry(entry, loc));
            index += 1;
        }
    }
    rows
}

/// Shortens an RFC 3339 timestamp to `YYYY-MM-DD HH:MM` for display, or returns it unchanged when it
/// is not in the expected shape.
fn friendly_timestamp(rfc3339: &str) -> String {
    match (rfc3339.len() >= 16, rfc3339.get(..16)) {
        (true, Some(head)) => head.replacen('T', " ", 1),
        _ => rfc3339.to_owned(),
    }
}

/// One row in the dashboard's recent-activity feed (a workspace-wide change-log entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityVm {
    /// The localized timestamp.
    pub when: String,
    /// The localized summary of what changed.
    pub what: String,
    /// The localized operator line.
    pub who: String,
    /// The affected record, when it resolves to a navigable detail (only People this milestone).
    pub record: Option<RecordRef>,
}

impl ActivityVm {
    /// Builds an activity row from an app [`ChangeLogEntry`], linking person records by display name.
    #[must_use]
    fn from_entry(entry: &ChangeLogEntry, loc: &Localizer, names: &HashMap<String, String>) -> Self {
        Self {
            when: friendly_timestamp(&entry.occurred_at),
            what: loc.change_summary(&entry.event_type),
            who: loc.operator_line(entry),
            record: record_for(entry, names),
        }
    }
}

/// The navigable record an entry affected (only People this milestone), labelled by display name.
fn record_for(entry: &ChangeLogEntry, names: &HashMap<String, String>) -> Option<RecordRef> {
    match (entry.aggregate_kind.as_str(), &entry.aggregate_human_id) {
        ("person", Some(human_id)) => Some(RecordRef {
            category: Category::People,
            label: names.get(human_id).cloned().unwrap_or_else(|| human_id.clone()),
            human_id: human_id.clone(),
        }),
        _ => None,
    }
}

/// Collapses runs of consecutive events by the same software agent (e.g. an import) into one row, so
/// a bulk import reads as a single "N records imported" line rather than N near-identical entries.
fn collapse_activity(activity: &[ChangeLogEntry], loc: &Localizer, names: &HashMap<String, String>) -> Vec<ActivityVm> {
    let mut rows = Vec::new();
    let mut index = 0;
    while index < activity.len() {
        let entry = &activity[index];
        let run = software_run_len(activity, index);
        if run >= 2 {
            rows.push(ActivityVm {
                when: friendly_timestamp(&entry.occurred_at),
                what: loc.activity_import_batch(run),
                who: loc.operator_line(entry),
                record: None,
            });
            index += run;
        } else {
            rows.push(ActivityVm::from_entry(entry, loc, names));
            index += 1;
        }
    }
    rows
}

/// The length of the run of consecutive software-agent events starting at `start` that share the
/// same operator; `1` (or `0` past the end) for a non-software or lone entry.
fn software_run_len(activity: &[ChangeLogEntry], start: usize) -> usize {
    let Some(first) = activity.get(start) else {
        return 0;
    };
    if first.operator_kind != OperatorKind::Software {
        return 1;
    }
    let mut end = start + 1;
    while activity.get(end).is_some_and(|next| {
        next.operator_kind == OperatorKind::Software && next.operator_display == first.operator_display
    }) {
        end += 1;
    }
    end - start
}

/// A quick entry point on the dashboard ("Jump back in") — a recently touched record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpVm {
    /// The record to open.
    pub record: RecordRef,
}

/// The dashboard's headline counts and evidence-health gauge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardStats {
    /// Persons in the workspace.
    pub people: u64,
    /// Families in the workspace.
    pub families: u64,
    /// Events in the workspace.
    pub events: u64,
    /// Percent of facts backed by at least one source (0–100).
    pub evidence_health_pct: u8,
    /// How many facts lack any source (the no-source flag count).
    pub facts_without_source: usize,
    /// Total facts considered (the evidence-health denominator).
    pub facts_total: usize,
}

impl DashboardStats {
    /// Builds the stats from the per-aggregate counts and the persons' facts.
    ///
    /// Evidence health is the share of facts carrying at least one citation; with no facts it is
    /// reported as 100% (nothing is unsourced).
    #[must_use]
    pub fn build(counts: WorkspaceCounts, persons: &[PersonSummary]) -> Self {
        let mut facts_total = 0usize;
        let mut facts_with_source = 0usize;
        for person in persons {
            for fact in &person.facts {
                facts_total += 1;
                if !fact.fact.citations.is_empty() {
                    facts_with_source += 1;
                }
            }
        }
        // With no facts, nothing is unsourced — report full health (checked_div yields None at 0).
        let evidence_health_pct = (facts_with_source * 100)
            .checked_div(facts_total)
            .and_then(|pct| u8::try_from(pct).ok())
            .unwrap_or(100);
        Self {
            people: counts.person,
            families: counts.family,
            events: counts.event,
            evidence_health_pct,
            facts_without_source: facts_total - facts_with_source,
            facts_total,
        }
    }
}

/// The dashboard view-model: stats, the recent-activity feed, and quick entry points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardVm {
    /// The headline counts and evidence-health gauge.
    pub stats: DashboardStats,
    /// The most recent workspace-wide changes, newest first.
    pub recent: Vec<ActivityVm>,
    /// Quick entry points to recently touched records.
    pub jump_back: Vec<JumpVm>,
}

impl DashboardVm {
    /// Assembles the dashboard from counts, the persons (for evidence health), and recent activity.
    ///
    /// "Jump back in" is the distinct navigable records drawn from the most recent activity, capped
    /// at `jump_limit`.
    #[must_use]
    pub fn build(
        counts: WorkspaceCounts,
        persons: &[PersonSummary],
        activity: &[ChangeLogEntry],
        loc: &Localizer,
        jump_limit: usize,
    ) -> Self {
        let names: HashMap<String, String> = persons
            .iter()
            .filter_map(|person| person.display_name.clone().map(|name| (person.human_id.clone(), name)))
            .collect();
        let recent = collapse_activity(activity, loc, &names);
        let mut jump_back = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for row in &recent {
            let Some(record) = &row.record else { continue };
            if seen.insert(record.human_id.clone()) {
                jump_back.push(JumpVm { record: record.clone() });
                if jump_back.len() >= jump_limit {
                    break;
                }
            }
        }
        Self {
            stats: DashboardStats::build(counts, persons),
            recent,
            jump_back,
        }
    }
}

/// One person-to-person association, for the Associations tab — with its evidence cues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationVm {
    /// The other person's user-facing id.
    pub other_id: String,
    /// The localized association-role label.
    pub role_label: String,
    /// The association's confidence, as a presentation level (drives the badge colour token).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this association (its source count).
    pub source_count: usize,
}

impl AssociationVm {
    /// Whether the association has at least one backing source (drives the no-source flag).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// One citation backing a person, for the Citations tab — its source, surety, and evidence axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRefVm {
    /// The citation's user-facing id (e.g. `C0001`).
    pub human_id: String,
    /// The cited source's `human_id`, if resolved.
    pub source: Option<String>,
    /// The citation's confidence, if set (drives the badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label, if set.
    pub confidence_label: Option<String>,
    /// The Evidence Explained axis chips (empty when the citation records no analysis).
    pub evidence_axes: Vec<EvidenceAxisVm>,
}

/// One family the person belongs to, for the Families tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyVm {
    /// The family's user-facing id (e.g. `F0001`).
    pub family_id: String,
    /// The localized role label (spouse/partner, or the child relationship).
    pub role_label: String,
    /// The partners' user-facing ids.
    pub partners: Vec<String>,
    /// The children: each child's id and localized relationship label.
    pub children: Vec<(String, String)>,
}

impl FamilyVm {
    /// Builds a family view-model from the app's [`FamilyForPerson`], localizing role labels.
    #[must_use]
    pub fn from_app(family: &FamilyForPerson, loc: &Localizer) -> Self {
        let role_label = match &family.role {
            PersonFamilyRole::Partner => loc.role("spouse"),
            PersonFamilyRole::Child(relationship) => loc.relationship_label(relationship),
        };
        Self {
            family_id: family.family_human_id.clone(),
            role_label,
            partners: family.partners.clone(),
            children: family
                .children
                .iter()
                .map(|(id, relationship)| (id.clone(), loc.relationship_label(relationship)))
                .collect(),
        }
    }
}

/// Builds a [`NameVm`] from an asserted [`NameSummary`], localizing the type label and confidence.
fn name_vm(summary: &NameSummary, loc: &Localizer) -> NameVm {
    let name = &summary.name;
    let surname = name.surnames.first().map(|element| element.surname.clone());
    let confidence = ConfidenceLevel::from(summary.confidence);
    NameVm {
        type_label: loc.name_type_label(&name.name_type),
        display: render_person_name(name),
        given: name.given.clone(),
        surname,
        nickname: name.nickname.clone(),
        date: name.date.as_ref().map(|date| loc.date(date)),
        language: name.language.as_ref().map(|language| language.as_str().to_owned()),
        confidence,
        confidence_label: loc.confidence_label(confidence),
        source_count: summary.source_count,
    }
}

/// Builds an [`AssociationVm`] from an app [`AssociationSummary`], localizing the role + confidence.
fn association_vm(summary: &AssociationSummary, loc: &Localizer) -> AssociationVm {
    let confidence = ConfidenceLevel::from(summary.confidence);
    AssociationVm {
        other_id: summary.other_id.clone(),
        role_label: loc.association_role_label(&summary.role),
        confidence,
        confidence_label: loc.confidence_label(confidence),
        source_count: summary.source_count,
    }
}

/// Builds a [`CitationRefVm`] from a backing [`CitationSummary`], localizing the confidence + axes.
#[must_use]
pub fn citation_ref_vm(summary: &CitationSummary, loc: &Localizer) -> CitationRefVm {
    let confidence = summary.confidence.map(ConfidenceLevel::from);
    CitationRefVm {
        human_id: summary.human_id.clone(),
        source: summary.source.clone(),
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        evidence_axes: evidence_axes(summary.evidence_analysis.as_ref(), loc),
    }
}

/// Builds a [`FactVm`] from an app [`FactSummary`], localizing labels and the date.
fn fact_vm(summary: &FactSummary, loc: &Localizer) -> FactVm {
    let confidence = ConfidenceLevel::from(summary.confidence);
    FactVm {
        type_label: loc.fact_type_label(&summary.fact.fact_type),
        value: summary.fact.value.clone(),
        date: summary.fact.date.as_ref().map(|date| loc.date(date)),
        confidence,
        confidence_label: loc.confidence_label(confidence),
        source_count: summary.fact.citations.len(),
    }
}

/// Builds the localized vital summary (`b. <date> · d. <date>`) from a person's birth/death facts.
///
/// Only dated births/deaths contribute; place names need place resolution and are left to a later
/// slice. Returns `None` when neither birth nor death is dated.
fn vital_summary(summary: &PersonSummary, loc: &Localizer) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for fact in &summary.facts {
        let Some(date) = fact.fact.date.as_ref() else {
            continue;
        };
        match fact.fact.fact_type {
            FactType::Birth => parts.push(loc.vital_born(&loc.date(date))),
            FactType::Death => parts.push(loc.vital_died(&loc.date(date))),
            _ => {}
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

/// Renders a [`PersonName`] as `given surname(s)` for display.
fn render_person_name(name: &PersonName) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(given) = name.given.as_deref() {
        parts.push(given);
    }
    for surname in &name.surnames {
        parts.push(&surname.surname);
    }
    parts.join(" ")
}

/// A person's detail view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonDetail {
    /// The user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// Whether this person is a persona (single-source extract) rather than a synthesized conclusion.
    pub is_persona: bool,
    /// The localized evidence-level label ("Persona" / "Conclusion") — the personas badge.
    pub evidence_level_label: String,
    /// The localized display name, or the localized "no name" placeholder.
    pub name: String,
    /// The structured given name, if asserted.
    pub given: Option<String>,
    /// The structured primary surname, if asserted.
    pub surname: Option<String>,
    /// The localized sex label, or the localized "no value" placeholder.
    pub sex: String,
    /// A localized vital summary (`b. <date> · d. <date>`) derived from the birth/death facts, or
    /// `None` when neither is dated. The detail header appends the sex to this.
    pub vitals: Option<String>,
    /// The person's privacy restrictions (GEDCOM `RESN`), as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// Every asserted name variant (Names tab).
    pub names: Vec<NameVm>,
    /// Every asserted fact, with confidence + source count (Facts tab).
    pub facts: Vec<FactVm>,
    /// Event participations (Events tab); dates are joined by the dispatcher.
    pub events: Vec<EventRefVm>,
    /// Person-to-person associations (Associations tab).
    pub associations: Vec<AssociationVm>,
    /// Families this person belongs to (Families tab); filled by the dispatcher.
    pub families: Vec<FamilyVm>,
    /// The citations backing this person, with source + surety + evidence axes (Citations tab);
    /// filled by the dispatcher, which joins each citation id to its summary.
    pub citations: Vec<CitationRefVm>,
    /// The human ids of the media attached to this person.
    pub media: Vec<String>,
    /// The human ids of the notes attached to this person.
    pub notes: Vec<String>,
    /// The ids of the tags applied to this person.
    pub tags: Vec<String>,
    /// The person's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl PersonDetail {
    /// Builds a detail view from a [`PersonSummary`], localizing labels via `loc`.
    ///
    /// The summary-derived tabs (names, facts, associations) are built here; the cross-aggregate
    /// tabs (events, families) start empty and are filled by the dispatcher
    /// ([`dispatch`](crate::intent::dispatch)), which has the joined event/family data.
    #[must_use]
    pub fn from_summary(summary: &PersonSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            is_persona: summary.evidence_level == EvidenceLevel::Persona,
            evidence_level_label: loc.evidence_level_label(summary.evidence_level),
            name: loc.display_name(summary.display_name.as_deref()),
            given: summary.given.clone(),
            surname: summary.surname.clone(),
            sex: loc.sex_label(summary.sex.as_ref()),
            vitals: vital_summary(summary, loc),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            names: summary.names.iter().map(|name| name_vm(name, loc)).collect(),
            facts: summary.facts.iter().map(|fact| fact_vm(fact, loc)).collect(),
            events: Vec::new(),
            associations: summary
                .associations
                .iter()
                .map(|assoc| association_vm(assoc, loc))
                .collect(),
            families: Vec::new(),
            citations: Vec::new(),
            media: summary.media.clone(),
            notes: summary.notes.clone(),
            tags: summary.tags.clone(),
            history: Vec::new(),
        }
    }
}

/// The tab strip for a person's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn person_tabs(detail: &PersonDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("names", Some(detail.names.len())),
        tab("facts", Some(detail.facts.len())),
        tab("events", Some(detail.events.len())),
        tab("associations", Some(detail.associations.len())),
        tab("families", Some(detail.families.len())),
        tab("citations", Some(detail.citations.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// One Evidence Explained axis chip: which axis it is (drives the hue) and its localized value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceAxisVm {
    /// The axis (source / information / evidence).
    pub axis: EvidenceAxis,
    /// The already-localized axis value (e.g. "Original", "Primary", "Direct").
    pub label: String,
}

/// Builds the three Evidence Explained axis chips from a citation's [`EvidenceAnalysis`], localizing
/// each value via `loc`. Returns an empty vec when no analysis is recorded.
#[must_use]
pub fn evidence_axes(analysis: Option<&EvidenceAnalysis>, loc: &Localizer) -> Vec<EvidenceAxisVm> {
    let Some(analysis) = analysis else {
        return Vec::new();
    };
    vec![
        EvidenceAxisVm {
            axis: EvidenceAxis::Source,
            label: loc.evidence_source_label(analysis.source),
        },
        EvidenceAxisVm {
            axis: EvidenceAxis::Information,
            label: loc.evidence_information_label(analysis.information),
        },
        EvidenceAxisVm {
            axis: EvidenceAxis::Evidence,
            label: loc.evidence_kind_label(analysis.evidence),
        },
    ]
}

/// Builds a generic list row from a [`CitationSummary`]: the cited source (or the citation id) as the
/// title, the page as the subtitle, and the quote glyph as the avatar.
#[must_use]
pub fn citation_row(summary: &CitationSummary, _loc: &Localizer) -> RowVm {
    RowVm {
        id: summary.human_id.clone(),
        title: summary.source.clone().unwrap_or_else(|| summary.human_id.clone()),
        subtitle: summary.page.clone(),
        avatar: Some("❝".to_owned()),
    }
}

/// A citation's detail view — its evidence axes, confidence, source, page, date, attributes, and
/// attachments. The research-grade-citation differentiator (Evidence Explained axes) lives here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationDetail {
    /// The user-facing id (e.g. `C0001`).
    pub human_id: String,
    /// The cited source's `human_id`, if resolved.
    pub source: Option<String>,
    /// The page / locator within the source.
    pub page: Option<String>,
    /// The localized date of the cited record.
    pub date: Option<String>,
    /// The citation's confidence as a presentation level, if set (drives the badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: Option<String>,
    /// The Evidence Explained axis chips (empty when no analysis is recorded).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// The citation's privacy restrictions (GEDCOM `RESN`), as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The recorded attributes, as `(type, value)` pairs.
    pub attributes: Vec<(String, String)>,
    /// The `human_id`s of the media objects attached to this citation.
    pub media: Vec<String>,
    /// The `human_id`s of the notes attached to this citation.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The citation's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl CitationDetail {
    /// Builds a detail view from a [`CitationSummary`], localizing labels and the date via `loc`.
    ///
    /// The History tab starts empty and is filled by the dispatcher
    /// ([`dispatch`](crate::intent::dispatch)), which has the change-log data.
    #[must_use]
    pub fn from_summary(summary: &CitationSummary, loc: &Localizer) -> Self {
        let confidence = summary.confidence.map(ConfidenceLevel::from);
        Self {
            human_id: summary.human_id.clone(),
            source: summary.source.clone(),
            page: summary.page.clone(),
            date: summary.date.as_ref().map(|date| loc.date(date)),
            confidence,
            confidence_label: confidence.map(|level| loc.confidence_label(level)),
            evidence_axes: evidence_axes(summary.evidence_analysis.as_ref(), loc),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            attributes: summary.attributes.clone(),
            media: summary.media.clone(),
            notes: summary.notes.clone(),
            tags: summary.tags.clone(),
            history: Vec::new(),
        }
    }
}

/// The tab strip for a citation's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn citation_tabs(detail: &CitationDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("attributes", Some(detail.attributes.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        CitationDetail, DashboardVm, PersonDetail, citation_row, citation_tabs, evidence_axes, person_row, person_tabs,
    };
    use crate::i18n::Localizer;
    use crate::presentation::ConfidenceLevel;
    use crate::presentation::EvidenceAxis;
    use genealogy_app::{
        AssociationRole, AssociationSummary, Calendar, ChangeLogEntry, CitationSummary, Confidence, DateModifier,
        DatePoint, DateQuality, EvidenceAnalysis, EvidenceKind, EvidenceLevel, Fact, FactSummary, FactType,
        GenealogicalDate, GenealogicalDateBody, InformationKind, NameSummary, NameType, OperatorKind, PersonName,
        PersonSummary, Restriction, Sex, SourceQuality, Surname, TagRef, WorkspaceCounts,
    };
    use std::collections::BTreeSet;

    /// A change-log entry for the activity-feed tests.
    fn log_entry(kind: &str, human_id: Option<&str>, operator: OperatorKind, who: &str) -> ChangeLogEntry {
        ChangeLogEntry {
            aggregate_kind: kind.to_owned(),
            aggregate_human_id: human_id.map(ToOwned::to_owned),
            assertion_id: "a".to_owned(),
            sequence: 1,
            event_type: "PersonCreated".to_owned(),
            occurred_at: "2026-06-22T14:35:00Z".to_owned(),
            operator_display: Some(who.to_owned()),
            operator_kind: operator,
            confidence: Confidence::Normal,
            rationale: None,
            can_undo: false,
        }
    }

    #[test]
    fn dashboard_collapses_an_import_run_and_labels_records_by_name() {
        let loc = Localizer::for_test("en");
        // `summary()` is the person I0001 / "Ada Lovelace".
        let person = summary();
        // Three consecutive import-agent events, then a human edit on a person.
        let activity = vec![
            log_entry("person", Some("I0002"), OperatorKind::Software, "gedcom-import"),
            log_entry("family", None, OperatorKind::Software, "gedcom-import"),
            log_entry("event", None, OperatorKind::Software, "gedcom-import"),
            log_entry("person", Some("I0001"), OperatorKind::Human, "magne"),
        ];
        let vm = DashboardVm::build(WorkspaceCounts::default(), &[person], &activity, &loc, 4);

        assert_eq!(vm.recent.len(), 2, "the import run collapses into one row");
        assert_eq!(vm.recent[0].what, "3 records imported");
        assert!(vm.recent[0].record.is_none(), "a collapsed import spans many records");
        // The human edit links to the person by display name, not the human id.
        let linked = vm.recent[1].record.as_ref().expect("person record");
        assert_eq!(linked.label, "Ada Lovelace");
        assert_eq!(linked.human_id, "I0001");
        // Jump-back surfaces the same named record.
        assert_eq!(vm.jump_back.len(), 1);
        assert_eq!(vm.jump_back[0].record.label, "Ada Lovelace");
    }

    #[test]
    fn history_collapses_consecutive_import_events() {
        use super::collapse_history;
        let loc = Localizer::for_test("en");
        let entries = vec![
            log_entry("person", Some("I0001"), OperatorKind::Human, "magne"),
            log_entry("person", Some("I0001"), OperatorKind::Software, "gedcom-import"),
            log_entry("person", Some("I0001"), OperatorKind::Software, "gedcom-import"),
        ];
        let rows = collapse_history(&entries, &loc);
        assert_eq!(rows.len(), 2, "the two import events collapse into one");
        assert_eq!(rows[0].what, "Person created", "the human edit stays an individual row");
        assert_eq!(rows[1].what, "2 records imported");
        assert!(!rows[1].can_undo, "a collapsed import run is not undoable");
        assert!(
            rows[1].assertion_id.is_empty(),
            "a collapsed run has no single undo target"
        );
    }

    fn year(year: i32) -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(year),
                month: None,
                day: None,
            })),
            quality: DateQuality::Normal,
            time: None,
            new_year_begins: None,
            sort_value: 0,
            original_text: None,
        }
    }

    fn dated_fact(fact_type: FactType, year_value: i32) -> FactSummary {
        FactSummary {
            fact: Fact {
                fact_type,
                date: Some(year(year_value)),
                place_id: None,
                value: None,
                citations: Vec::new(),
            },
            confidence: Confidence::Normal,
        }
    }

    fn birth_name() -> PersonName {
        PersonName {
            name_type: NameType::BirthName,
            given: Some("Ada".to_owned()),
            surnames: vec![Surname {
                prefix: None,
                surname: "Lovelace".to_owned(),
                primary: true,
                connector: None,
            }],
            suffix: None,
            title: None,
            nickname: None,
            call_name: None,
            date: None,
            language: None,
            transliterations: Vec::new(),
        }
    }

    fn occupation_fact() -> FactSummary {
        FactSummary {
            fact: Fact {
                fact_type: FactType::Occupation,
                date: None,
                place_id: None,
                value: Some("Mathematician".to_owned()),
                citations: Vec::new(),
            },
            confidence: Confidence::High,
        }
    }

    fn summary() -> PersonSummary {
        PersonSummary {
            human_id: "I0001".to_owned(),
            evidence_level: EvidenceLevel::Conclusion,
            display_name: Some("Ada Lovelace".to_owned()),
            given: Some("Ada".to_owned()),
            surname: Some("Lovelace".to_owned()),
            surname_prefix: None,
            nickname: None,
            name_prefix: None,
            name_suffix: None,
            name_type: None,
            names: vec![NameSummary {
                name: birth_name(),
                confidence: Confidence::High,
                source_count: 1,
            }],
            sex: Some(Sex::Female),
            facts: vec![occupation_fact()],
            associations: vec![AssociationSummary {
                other_id: "I0002".to_owned(),
                role: AssociationRole::Godparent,
                confidence: Confidence::Normal,
                source_count: 0,
            }],
            participations: Vec::new(),
            citations: vec!["C0001".to_owned()],
            media: Vec::new(),
            notes: vec!["N0001".to_owned(), "N0002".to_owned()],
            tags: Vec::new(),
            restrictions: BTreeSet::new(),
        }
    }

    #[test]
    fn row_localizes_name_sex_and_initials() {
        let loc = Localizer::for_test("en");
        let row = person_row(&summary(), &loc);
        assert_eq!(row.id, "I0001");
        assert_eq!(row.title, "Ada Lovelace");
        assert_eq!(row.subtitle.as_deref(), Some("female"));
        assert_eq!(row.avatar.as_deref(), Some("AL"));
    }

    #[test]
    fn detail_keeps_structured_parts_and_localizes_in_norwegian() {
        let loc = Localizer::for_test("no");
        let detail = PersonDetail::from_summary(&summary(), &loc);
        assert_eq!(detail.given.as_deref(), Some("Ada"));
        assert_eq!(detail.surname.as_deref(), Some("Lovelace"));
        assert_eq!(detail.sex, "kvinne");
    }

    #[test]
    fn detail_builds_name_fact_and_association_view_models() {
        let loc = Localizer::for_test("en");
        let detail = PersonDetail::from_summary(&summary(), &loc);

        // The personas badge surfaces the evidence level.
        assert!(!detail.is_persona);
        assert_eq!(detail.evidence_level_label, "Conclusion");

        assert_eq!(detail.names.len(), 1);
        assert_eq!(detail.names[0].type_label, "Birth name");
        assert_eq!(detail.names[0].display, "Ada Lovelace");
        // The name carries its surety + source count (the evidence-first cue).
        assert_eq!(detail.names[0].confidence, ConfidenceLevel::High);
        assert_eq!(detail.names[0].source_count, 1);
        assert!(detail.names[0].has_source());

        assert_eq!(detail.facts.len(), 1);
        let fact = &detail.facts[0];
        assert_eq!(fact.type_label, "Occupation");
        assert_eq!(fact.value.as_deref(), Some("Mathematician"));
        assert_eq!(fact.confidence, ConfidenceLevel::High);
        assert_eq!(fact.confidence_label, "High");
        assert_eq!(fact.source_count, 0);
        assert!(!fact.has_source(), "no citations means no source");

        assert_eq!(detail.associations.len(), 1);
        assert_eq!(detail.associations[0].other_id, "I0002");
        assert_eq!(detail.associations[0].role_label, "Godparent");
        // The association carries its surety; the default fixture has no backing source.
        assert_eq!(detail.associations[0].confidence, ConfidenceLevel::Normal);
        assert!(!detail.associations[0].has_source());
    }

    #[test]
    fn persona_evidence_level_surfaces_on_the_badge() {
        let loc = Localizer::for_test("en");
        let mut summary = summary();
        summary.evidence_level = EvidenceLevel::Persona;
        let detail = PersonDetail::from_summary(&summary, &loc);
        assert!(detail.is_persona);
        assert_eq!(detail.evidence_level_label, "Persona");
    }

    #[test]
    fn tabs_carry_localized_labels_and_related_counts() {
        let loc = Localizer::for_test("en");
        let detail = PersonDetail::from_summary(&summary(), &loc);
        let tabs = person_tabs(&detail, &loc);
        assert_eq!(tabs[0].id, "overview");
        assert_eq!(tabs[0].label, "Overview");
        assert_eq!(tabs[0].count, None);
        assert_eq!(tabs[1].id, "names");
        assert_eq!(tabs[1].count, Some(1));
        let facts = tabs.iter().find(|tab| tab.id == "facts").expect("facts tab");
        assert_eq!(facts.count, Some(1));
        let notes = tabs.iter().find(|tab| tab.id == "notes").expect("notes tab");
        assert_eq!(notes.count, Some(2));
        let history = tabs.iter().find(|tab| tab.id == "history").expect("history tab");
        assert_eq!(history.count, None, "history count is unknown until PR5");
    }

    #[test]
    fn vitals_summarize_dated_birth_and_death() {
        let loc = Localizer::for_test("en");
        let mut summary = summary();
        summary.facts = vec![dated_fact(FactType::Birth, 1850), dated_fact(FactType::Death, 1920)];
        let detail = PersonDetail::from_summary(&summary, &loc);
        assert_eq!(detail.vitals.as_deref(), Some("b. 1850 · d. 1920"));
    }

    #[test]
    fn vitals_absent_without_dated_vital_facts() {
        let loc = Localizer::for_test("en");
        // The default summary's only fact is an undated occupation.
        let detail = PersonDetail::from_summary(&summary(), &loc);
        assert_eq!(detail.vitals, None);
    }

    #[test]
    fn missing_name_and_sex_use_placeholders() {
        let loc = Localizer::for_test("en");
        let summary = PersonSummary {
            human_id: "I0002".to_owned(),
            evidence_level: EvidenceLevel::Conclusion,
            display_name: None,
            given: None,
            surname: None,
            surname_prefix: None,
            nickname: None,
            name_prefix: None,
            name_suffix: None,
            name_type: None,
            names: Vec::new(),
            sex: None,
            facts: Vec::new(),
            associations: Vec::new(),
            participations: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: BTreeSet::from([Restriction::Privacy]),
        };
        let row = person_row(&summary, &loc);
        assert_eq!(row.title, "(no name)");
        assert_eq!(row.subtitle.as_deref(), Some("-"));
        assert_eq!(row.avatar.as_deref(), Some("?"));
    }

    fn citation_summary() -> CitationSummary {
        CitationSummary {
            human_id: "C0001".to_owned(),
            source: Some("S0001".to_owned()),
            page: Some("p. 42".to_owned()),
            date: Some(year(1880)),
            confidence: Some(Confidence::High),
            evidence_analysis: Some(EvidenceAnalysis {
                source: SourceQuality::Original,
                information: InformationKind::Primary,
                evidence: EvidenceKind::Direct,
            }),
            attributes: vec![("quality".to_owned(), "good".to_owned())],
            media: vec!["O0001".to_owned()],
            notes: vec!["N0001".to_owned()],
            tags: vec![TagRef {
                id: "0190-tag".to_owned(),
                name: "Direct ancestor".to_owned(),
                color: Some("#e5534b".to_owned()),
                priority: Some(1),
            }],
            restrictions: BTreeSet::new(),
        }
    }

    #[test]
    fn citation_detail_maps_axes_confidence_and_attachments() {
        let loc = Localizer::for_test("en");
        let detail = CitationDetail::from_summary(&citation_summary(), &loc);
        assert_eq!(detail.source.as_deref(), Some("S0001"));
        assert_eq!(detail.page.as_deref(), Some("p. 42"));
        assert_eq!(detail.confidence, Some(ConfidenceLevel::High));
        assert_eq!(detail.confidence_label.as_deref(), Some("High"));
        assert_eq!(detail.evidence_axes.len(), 3);
        assert_eq!(detail.evidence_axes[0].axis, EvidenceAxis::Source);
        assert_eq!(detail.evidence_axes[0].label, "Original");
        assert_eq!(detail.evidence_axes[1].label, "Primary");
        assert_eq!(detail.evidence_axes[2].label, "Direct");
        assert_eq!(detail.attributes.len(), 1);
        assert_eq!(detail.media, vec!["O0001".to_owned()]);
        assert_eq!(detail.notes, vec!["N0001".to_owned()]);
        // Tags surface name/colour/priority — never the id.
        assert_eq!(detail.tags[0].name, "Direct ancestor");
        assert_eq!(detail.tags[0].color.as_deref(), Some("#e5534b"));
        assert_eq!(detail.tags[0].priority, Some(1));
    }

    #[test]
    fn evidence_axes_are_empty_without_analysis() {
        let loc = Localizer::for_test("en");
        assert!(evidence_axes(None, &loc).is_empty());
    }

    #[test]
    fn citation_row_titles_by_source_and_subtitles_by_page() {
        let loc = Localizer::for_test("en");
        let row = citation_row(&citation_summary(), &loc);
        assert_eq!(row.id, "C0001");
        assert_eq!(row.title, "S0001");
        assert_eq!(row.subtitle.as_deref(), Some("p. 42"));
    }

    #[test]
    fn citation_tabs_carry_attachment_counts() {
        let loc = Localizer::for_test("en");
        let detail = CitationDetail::from_summary(&citation_summary(), &loc);
        let tabs = citation_tabs(&detail, &loc);
        assert_eq!(tabs[0].id, "overview");
        let attributes = tabs.iter().find(|tab| tab.id == "attributes").expect("attributes tab");
        assert_eq!(attributes.count, Some(1));
        let tags = tabs.iter().find(|tab| tab.id == "tags").expect("tags tab");
        assert_eq!(tags.count, Some(1));
    }
}
