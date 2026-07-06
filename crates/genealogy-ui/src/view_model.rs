//! View-models: framework-neutral, render-ready shapes derived from `genealogy-app` DTOs.
//!
//! A view-model carries already-localized display strings (built via the [`Localizer`]) so a
//! renderer stays dumb — it lays out fields, it does not format or localize. The structured parts a
//! renderer might branch on (e.g. `private`) stay typed. A list row is the generic [`RowVm`]; the
//! detail tab strip is [`DetailTab`]s.

use std::collections::HashMap;

use genealogy_app::{
    AssociationSummary, ChangeLogEntry, ChildParentRelationship, CitationSummary, EventType, EvidenceAnalysis,
    EvidenceKind, EvidenceLevel, FactSummary, FactType, FamilyForPerson, FamilySummary, InformationKind, MutationMeta,
    NameSummary, NameType, OperatorKind, PersonFamilyRole, PersonName, PersonNameParts, PersonSummary, Provenance, Sex,
    SourceQuality, TagRef, WorkspaceCounts,
};

use crate::detail::DetailTab;
use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{
    Category, DraftCitationRef, DraftNewCitation, DraftNewSource, DraftSourceRef, PersonChangeSetRequest, RecordRef,
    TagChangeSetRequest,
};
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
        ..RowVm::default()
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
    /// The fact's citations, for the provenance popover (source · page · surety · evidence axes).
    pub citations: Vec<CitationRefVm>,
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
            what: loc.change_summary(entry),
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

/// The length of the run of consecutive software-agent events starting at `start` that share the
/// same operator; `1` (or `0` past the end) for a non-software or lone entry.
fn software_run_len(entries: &[ChangeLogEntry], start: usize) -> usize {
    let Some(first) = entries.get(start) else {
        return 0;
    };
    if first.operator_kind != OperatorKind::Software {
        return 1;
    }
    let mut end = start + 1;
    while entries.get(end).is_some_and(|next| {
        next.operator_kind == OperatorKind::Software && next.operator_display == first.operator_display
    }) {
        end += 1;
    }
    end - start
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
    /// The affected record, when it resolves to a navigable detail (any aggregate with a `human_id`).
    pub record: Option<RecordRef>,
}

impl ActivityVm {
    /// Builds an activity row from an app [`ChangeLogEntry`], linking the affected record by name/id.
    #[must_use]
    fn from_entry(entry: &ChangeLogEntry, loc: &Localizer, names: &HashMap<String, String>) -> Self {
        Self {
            when: friendly_timestamp(&entry.occurred_at),
            what: loc.change_summary(entry),
            who: loc.operator_line(entry),
            record: record_for(entry, names),
        }
    }
}

/// The navigable record an entry affected, across every aggregate. People are labelled by display
/// name (from `names`); other aggregates fall back to their `human_id`. A synthetic collapsed-import
/// row (no kind, no id) and a record without a resolved `human_id` are not navigable.
fn record_for(entry: &ChangeLogEntry, names: &HashMap<String, String>) -> Option<RecordRef> {
    let human_id = entry.aggregate_human_id.as_ref()?;
    let category = Category::from_aggregate_kind(&entry.aggregate_kind)?;
    Some(RecordRef {
        category,
        label: names.get(human_id).cloned().unwrap_or_else(|| human_id.clone()),
        human_id: human_id.clone(),
    })
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
        let recent: Vec<ActivityVm> = activity
            .iter()
            .map(|entry| ActivityVm::from_entry(entry, loc, &names))
            .collect();
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

/// One citation backing a record, for the Citations tab — its source, page, surety, and evidence axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRefVm {
    /// The citation's user-facing id (e.g. `C0001`).
    pub human_id: String,
    /// The cited source's display label (its title, or `human_id`), if resolved.
    pub source: Option<String>,
    /// The cited source's user-facing id (e.g. `S0001`), for navigating to it.
    pub source_id: Option<String>,
    /// The page / locator within the cited source, if set.
    pub page: Option<String>,
    /// The citation's confidence, if set (drives the badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label, if set.
    pub confidence_label: Option<String>,
    /// The Evidence Explained axis chips (empty when the citation records no analysis).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// The localized "asserted by {who} · {when}" provenance line, if the creation operator is known.
    pub asserted_by: Option<String>,
}

/// Builds a [`CitationRefVm`] from an app [`CitationRef`](genealogy_app::CitationRef) — the joined
/// citation row used by the Event/Place Citations tabs (source label, page, surety, evidence axes).
#[must_use]
pub fn citation_ref_from_ref(reference: &genealogy_app::CitationRef, loc: &Localizer) -> CitationRefVm {
    let confidence = reference.confidence.map(ConfidenceLevel::from);
    let source = reference
        .source_title
        .clone()
        .or_else(|| reference.source.as_ref().map(|s| s.human_id.clone()));
    CitationRefVm {
        human_id: reference.human_id.clone(),
        source,
        source_id: reference.source.as_ref().map(|s| s.human_id.clone()),
        page: reference.page.clone(),
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        evidence_axes: evidence_axes(reference.analysis.as_ref(), loc),
        asserted_by: reference.asserted_by.as_ref().map(|who| {
            let when = reference.asserted_at.as_deref().map(friendly_timestamp);
            loc.provenance_asserted_by(who, when.as_deref())
        }),
    }
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
            PersonFamilyRole::Child(relationships) => relationships_label(relationships, loc),
        };
        Self {
            family_id: family.family_human_id.clone(),
            role_label,
            partners: family.partners.clone(),
            children: family
                .children
                .iter()
                .map(|(id, relationships)| (id.clone(), relationships_label(relationships, loc)))
                .collect(),
        }
    }
}

/// Joins a child's per-partner relationship labels into one display string (e.g. `Birth / Step`),
/// keeping each distinct label once in order. Empty when no per-partner relationship is recorded.
fn relationships_label(relationships: &[(String, ChildParentRelationship)], loc: &Localizer) -> String {
    let mut labels: Vec<String> = Vec::new();
    for (_, relationship) in relationships {
        let label = loc.relationship_label(relationship);
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join(" / ")
}

/// A family partner row (Overview "Partners" card): name, lifespan, and source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerVm {
    /// The partner's user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The partner's display name (falls back to the `human_id`).
    pub name: String,
    /// The "born – died" lifespan, if known.
    pub vitals: Option<String>,
    /// How many citations back the partnership (drives the source count / no-source flag).
    pub source_count: usize,
    /// The partnership's citations, for the provenance popover.
    pub citations: Vec<CitationRefVm>,
}

/// A family child row (Children tab): name, birth year, per-partner relationship, surety + source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyChildVm {
    /// The child's user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The child's display name (falls back to the `human_id`).
    pub name: String,
    /// The child's birth year, if known.
    pub born: Option<String>,
    /// The relationship label to each family partner, by partner `human_id`.
    pub relationships: Vec<(String, String)>,
    /// The operator's surety in the child assertion (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the child assertion.
    pub source_count: usize,
}

/// A family event row (Overview "Marriage" card + Events tab): kind, date, place, surety + source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyEventVm {
    /// The event's user-facing id (e.g. `E0001`).
    pub human_id: String,
    /// The localized event-type label.
    pub type_label: String,
    /// The localized date, if known.
    pub date: Option<String>,
    /// The linked place's `human_id`, if any.
    pub place: Option<String>,
    /// The operator's surety in the family-event link (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label.
    pub confidence_label: String,
    /// How many citations back the event.
    pub source_count: usize,
    /// The event's citations, for the provenance popover.
    pub citations: Vec<CitationRefVm>,
}

/// A media object attached to the family (Media gallery): its id and caption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyMediaVm {
    /// The media object's user-facing id (e.g. `O0001`).
    pub human_id: String,
    /// The per-use caption, if set.
    pub caption: Option<String>,
}

/// A family's detail view — partners, the marriage/events, children with per-partner relationships,
/// attachments, and the audit history. The Family slice's copy of the evidence-first record layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyDetail {
    /// The user-facing id (e.g. `F0001`).
    pub human_id: String,
    /// The stable `FamilyId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the partners' names joined (e.g. `Mary Doe & John Smith`).
    pub title: String,
    /// The partners (neutral roles).
    pub partners: Vec<PartnerVm>,
    /// The headline marriage event for the Overview card, if one is linked.
    pub marriage: Option<FamilyEventVm>,
    /// The children, with per-partner relationships.
    pub children: Vec<FamilyChildVm>,
    /// All linked family events.
    pub events: Vec<FamilyEventVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The family's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The family's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl FamilyDetail {
    /// Builds a detail view from a [`FamilySummary`], localizing labels, dates, and confidence.
    ///
    /// The History tab starts empty and is filled by the dispatcher
    /// ([`dispatch`](crate::intent::dispatch)), which has the change-log data.
    #[must_use]
    pub fn from_summary(summary: &FamilySummary, loc: &Localizer) -> Self {
        let partners = summary
            .partners
            .iter()
            .map(|partner| PartnerVm {
                human_id: partner.human_id.clone(),
                name: partner.name.clone().unwrap_or_else(|| partner.human_id.clone()),
                vitals: partner.vitals.clone(),
                source_count: partner.source_count,
                citations: partner
                    .citations
                    .iter()
                    .map(|c| citation_ref_from_ref(c, loc))
                    .collect(),
            })
            .collect();
        let children = summary
            .children
            .iter()
            .map(|child| family_child_vm(child, loc))
            .collect();
        let events: Vec<FamilyEventVm> = summary.events.iter().map(|event| family_event_vm(event, loc)).collect();
        let marriage = summary
            .events
            .iter()
            .find(|event| event.event_type == Some(EventType::Marriage))
            .or_else(|| summary.events.first())
            .map(|event| family_event_vm(event, loc));
        let media = summary
            .media
            .iter()
            .map(|media| FamilyMediaVm {
                human_id: media.human_id.clone(),
                caption: media.caption.clone(),
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: family_title(summary),
            partners,
            marriage,
            children,
            events,
            media,
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// The partners' names joined for the header (e.g. `Mary Doe & John Smith`), or a fallback.
fn family_title(summary: &FamilySummary) -> String {
    let names: Vec<String> = summary
        .partners
        .iter()
        .map(|partner| partner.name.clone().unwrap_or_else(|| partner.human_id.clone()))
        .collect();
    if names.is_empty() {
        summary.human_id.clone()
    } else {
        names.join(" & ")
    }
}

/// Builds a [`FamilyChildVm`] from an app `ChildRef`, localizing relationships + confidence.
fn family_child_vm(child: &genealogy_app::ChildRef, loc: &Localizer) -> FamilyChildVm {
    let confidence = ConfidenceLevel::from(child.confidence);
    FamilyChildVm {
        human_id: child.human_id.clone(),
        name: child.name.clone().unwrap_or_else(|| child.human_id.clone()),
        born: child.born.clone(),
        relationships: child
            .relationships
            .iter()
            .map(|(partner, relationship)| (partner.clone(), loc.relationship_label(relationship)))
            .collect(),
        confidence,
        confidence_label: loc.confidence_label(confidence),
        source_count: child.source_count,
    }
}

/// Builds a [`FamilyEventVm`] from an app `FamilyEventRef`, localizing the type, date, and confidence.
fn family_event_vm(event: &genealogy_app::FamilyEventRef, loc: &Localizer) -> FamilyEventVm {
    let confidence = ConfidenceLevel::from(event.confidence);
    let type_label = event
        .event_type
        .as_ref()
        .map_or_else(|| event.human_id.clone(), |event_type| loc.event_type_label(event_type));
    FamilyEventVm {
        human_id: event.human_id.clone(),
        type_label,
        date: event.date.as_ref().map(|date| loc.date(date)),
        place: event.place.clone(),
        confidence,
        confidence_label: loc.confidence_label(confidence),
        source_count: event.source_count,
        citations: event.citations.iter().map(|c| citation_ref_from_ref(c, loc)).collect(),
    }
}

/// Builds a generic list row from a [`FamilySummary`]: the partners' names, a marriage/children
/// subtitle, and a couple avatar.
#[must_use]
pub fn family_row(summary: &FamilySummary, loc: &Localizer) -> RowVm {
    let title = family_title(summary);
    let marriage_year = summary
        .events
        .iter()
        .find(|event| event.event_type == Some(EventType::Marriage))
        .and_then(|event| event.date.as_ref())
        .map(|date| loc.date(date));
    let children = loc.family_children_count(summary.children.len());
    let subtitle = match marriage_year {
        Some(year) => Some(format!("{year} · {children}")),
        None => Some(children),
    };
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("👪".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a family's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn family_tabs(detail: &FamilyDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("children", Some(detail.children.len())),
        tab("events", Some(detail.events.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
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
        other_id: summary.other.human_id.clone(),
        role_label: loc.association_role_label(&summary.role),
        confidence,
        confidence_label: loc.confidence_label(confidence),
        source_count: summary.source_count,
    }
}

/// Builds an [`EventRefVm`] from a person's [`ParticipationRef`](genealogy_app::ParticipationRef),
/// localizing the role label and the event's date (both joined in the app layer).
fn participation_vm(participation: &genealogy_app::ParticipationRef, loc: &Localizer) -> EventRefVm {
    EventRefVm {
        event_id: participation.event.human_id.clone(),
        role_label: loc.participant_role_label(&participation.role),
        date: participation.date.as_ref().map(|date| loc.date(date)),
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
        citations: summary
            .citations
            .iter()
            .map(|c| citation_ref_from_ref(c, loc))
            .collect(),
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
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The person's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
    /// A draft pre-populated from this person, for the deferred edit dialog (structured name parts,
    /// gender, tags — the parts the localized display fields above do not carry structurally).
    pub edit_seed: PersonDraft,
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
            events: summary
                .participations
                .iter()
                .map(|p| participation_vm(p, loc))
                .collect(),
            associations: summary
                .associations
                .iter()
                .map(|assoc| association_vm(assoc, loc))
                .collect(),
            families: Vec::new(),
            citations: summary
                .citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            media: summary.media.iter().map(|m| m.human_id.clone()).collect(),
            notes: summary.notes.iter().map(|n| n.human_id.clone()).collect(),
            tags: summary.tag_refs.clone(),
            history: Vec::new(),
            edit_seed: PersonDraft::from_summary(summary),
        }
    }
}

/// The operator-intent a save collects once and applies to every assertion the edit form emits
/// (`record-editing.html` §5b): the free-text rationale ("why"), the confidence, the backing citation
/// `human_id`s, and the optional Evidence Explained analysis (three axes). Bound to the edit panel's
/// provenance block; reset whenever the panel opens so a prior save's rationale never leaks into the
/// next. [`Self::meta`] borrows the citations into a [`MutationMeta`] a `dispatch_*_edit` passes to
/// the use-case; operator and timestamp come from the [`Session`](genealogy_app::Session), never
/// typed. Never supersedes — corrections are PR27 territory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProvenanceDraft {
    /// Why the change is being made (free text; blank ⇒ no rationale recorded).
    pub rationale: String,
    /// The operator's surety in the asserted claim(s).
    pub confidence: ConfidenceLevel,
    /// Citation `human_id`s backing the assertion(s).
    pub citations: Vec<String>,
    /// The source-quality axis, if chosen.
    pub source: Option<SourceQuality>,
    /// The information-kind axis, if chosen.
    pub information: Option<InformationKind>,
    /// The evidence-kind axis, if chosen.
    pub evidence: Option<EvidenceKind>,
}

impl ProvenanceDraft {
    /// Builds the [`Provenance`] this draft describes: a blank/whitespace rationale becomes `None`
    /// (trimmed otherwise), and an evidence analysis is produced only when all three axes are chosen.
    #[must_use]
    pub fn provenance(&self) -> Provenance {
        let rationale = {
            let trimmed = self.rationale.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        };
        let evidence_analysis = match (self.source, self.information, self.evidence) {
            (Some(source), Some(information), Some(evidence)) => Some(EvidenceAnalysis {
                source,
                information,
                evidence,
            }),
            _ => None,
        };
        Provenance {
            confidence: self.confidence.into(),
            rationale,
            evidence_analysis,
        }
    }

    /// Bundles this draft into the [`MutationMeta`] a non-create mutation use-case takes, borrowing
    /// the citation ids. Never supersedes (PR27).
    #[must_use]
    pub fn meta(&self) -> MutationMeta<'_> {
        MutationMeta {
            provenance: self.provenance(),
            citations: &self.citations,
            supersedes: None,
        }
    }
}

/// A pending citation the operator created inside the person dialog but has not saved. It cites a
/// source (an existing one by `human_id`, or a pending one created in the same dialog) and is
/// referenced by the name via a local placeholder key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftCitation {
    /// The local placeholder key the name references this citation by (unique within the draft).
    pub placeholder: String,
    /// An existing source's `human_id`, or empty to use `new_source_title` as a pending source.
    pub source_human_id: String,
    /// The title of a pending source to create when `source_human_id` is empty.
    pub new_source_title: String,
    /// The page / locator, if given.
    pub page: String,
}

/// Which citation the preferred name cites in a [`PersonDraft`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftNameCitation {
    /// No citation attached to the name.
    None,
    /// An existing citation, by its `human_id`.
    Existing(String),
    /// The citation being created inside the dialog (its placeholder is [`PersonDraft::PENDING_KEY`]).
    New,
}

/// The buffered, editable state of the person create/edit dialog (ADR 0008 view-model). The dialog
/// binds its inputs to these fields; nothing is persisted until OK, when [`Self::to_request`] turns
/// the buffer into a [`PersonChangeSetRequest`] dispatched to the app's change-set. Cancel drops it.
///
/// One value serves both modes: [`Self::new`] is empty (create), [`Self::from_summary`] is
/// pre-populated (edit) and records the person's `human_id` in `existing_human_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonDraft {
    /// `Some` in edit mode (the person being edited); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// A `human_id` override for a new person (create mode only); empty ⇒ auto-allocate.
    pub human_id_override: String,
    /// The preferred name's type.
    pub name_type: NameType,
    /// The title / name prefix (GEDCOM `NPFX`).
    pub prefix: String,
    /// The given name (GEDCOM `GIVN`).
    pub given: String,
    /// The nickname (GEDCOM `NICK`).
    pub nickname: String,
    /// The call name — reserved for a later field; unused in this slice.
    pub call_name: String,
    /// The surname prefix (GEDCOM `SPFX`, e.g. `van`).
    pub surname_prefix: String,
    /// The primary surname (GEDCOM `SURN`).
    pub surname: String,
    /// The name suffix (GEDCOM `NSFX`, e.g. `Jr`).
    pub suffix: String,
    /// The person's sex.
    pub sex: Sex,
    /// The tags applied to the person, by aggregate id (a UUID string; never shown to the user).
    pub tags: Vec<String>,
    /// Which citation backs the preferred name.
    pub name_citation: DraftNameCitation,
    /// The pending citation being created inside the dialog, if the operator chose "+ New".
    pub pending_citation: Option<DraftCitation>,
}

impl PersonDraft {
    /// The placeholder key the dialog's single pending citation is created under.
    pub const PENDING_KEY: &'static str = "name-citation";

    /// An empty draft for creating a new person (name blank, sex `Unknown`, no tags).
    #[must_use]
    pub fn new() -> Self {
        Self {
            existing_human_id: None,
            human_id_override: String::new(),
            name_type: NameType::BirthName,
            prefix: String::new(),
            given: String::new(),
            nickname: String::new(),
            call_name: String::new(),
            surname_prefix: String::new(),
            surname: String::new(),
            suffix: String::new(),
            sex: Sex::Unknown,
            tags: Vec::new(),
            name_citation: DraftNameCitation::None,
            pending_citation: None,
        }
    }

    /// A draft pre-populated from an existing person for editing. Records the `human_id` so the
    /// commit edits (diffs) rather than creates.
    #[must_use]
    pub fn from_summary(summary: &PersonSummary) -> Self {
        Self {
            existing_human_id: Some(summary.human_id.clone()),
            human_id_override: String::new(),
            name_type: summary.name_type.clone().unwrap_or(NameType::BirthName),
            prefix: summary.name_prefix.clone().unwrap_or_default(),
            given: summary.given.clone().unwrap_or_default(),
            nickname: summary.nickname.clone().unwrap_or_default(),
            call_name: String::new(),
            surname_prefix: summary.surname_prefix.clone().unwrap_or_default(),
            surname: summary.surname.clone().unwrap_or_default(),
            suffix: summary.name_suffix.clone().unwrap_or_default(),
            sex: summary.sex.clone().unwrap_or(Sex::Unknown),
            tags: summary.tags.clone(),
            name_citation: DraftNameCitation::None,
            pending_citation: None,
        }
    }

    /// The structured name parts the draft describes, or `None` when every part is blank.
    #[must_use]
    pub fn name_parts(&self) -> Option<PersonNameParts> {
        let parts = PersonNameParts {
            name_type: self.name_type.clone(),
            given: non_blank(&self.given),
            surname_prefix: non_blank(&self.surname_prefix),
            surname: non_blank(&self.surname),
            nickname: non_blank(&self.nickname),
            prefix: non_blank(&self.prefix),
            suffix: non_blank(&self.suffix),
        };
        if parts.is_empty() { None } else { Some(parts) }
    }

    /// Builds the [`PersonChangeSetRequest`] the app commits on OK, resolving the name-citation
    /// selection (existing / pending / none) and emitting the pending source + citation entries when
    /// the operator created one inside the dialog.
    #[must_use]
    pub fn to_request(&self) -> PersonChangeSetRequest {
        let mut new_sources = Vec::new();
        let mut new_citations = Vec::new();
        let name_citation = match &self.name_citation {
            DraftNameCitation::None => None,
            DraftNameCitation::Existing(human_id) => Some(DraftCitationRef::Existing(human_id.clone())),
            DraftNameCitation::New => self.pending_citation.as_ref().map(|pending| {
                let source = if pending.source_human_id.trim().is_empty() {
                    let placeholder = format!("{}-source", pending.placeholder);
                    new_sources.push(DraftNewSource {
                        placeholder: placeholder.clone(),
                        title: non_blank(&pending.new_source_title),
                    });
                    DraftSourceRef::Pending(placeholder)
                } else {
                    DraftSourceRef::Existing(pending.source_human_id.trim().to_owned())
                };
                new_citations.push(DraftNewCitation {
                    placeholder: pending.placeholder.clone(),
                    source,
                    page: non_blank(&pending.page),
                });
                DraftCitationRef::Pending(pending.placeholder.clone())
            }),
        };
        PersonChangeSetRequest {
            existing_human_id: self.existing_human_id.clone(),
            human_id_override: non_blank(&self.human_id_override),
            name: self.name_parts(),
            name_citation,
            sex: Some(self.sex.clone()),
            tags: self.tags.clone(),
            new_sources,
            new_citations,
        }
    }
}

impl Default for PersonDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// Trims a field and returns `None` when it is blank, else the owned trimmed value.
fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
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
        title: summary
            .source
            .as_ref()
            .map_or_else(|| summary.human_id.clone(), |s| s.human_id.clone()),
        subtitle: summary.page.clone(),
        avatar: Some("❝".to_owned()),
        ..RowVm::default()
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
            source: summary.source.as_ref().map(|s| s.human_id.clone()),
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

/// One event participant (Participants tab): the person, their role, surety, and source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantVm {
    /// The participant's user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The participant's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The participant's display name (falls back to the `human_id`).
    pub name: String,
    /// The localized participant-role label.
    pub role_label: String,
    /// The operator's surety in the participation (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the participation.
    pub source_count: usize,
}

/// The place an event occurred (Overview link): its name and the navigation ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceLinkVm {
    /// The place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The place's display name (falls back to the `human_id`).
    pub name: String,
}

/// An event's detail view — type/date/place facts, participants, citations, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDetail {
    /// The user-facing id (e.g. `E0001`).
    pub human_id: String,
    /// The stable `EventId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the localized event-type label (falls back to the `human_id`).
    pub title: String,
    /// The localized event-type label.
    pub type_label: String,
    /// The localized date, if known.
    pub date: Option<String>,
    /// The operator's surety in the date (drives the confidence badge), if asserted.
    pub date_confidence: Option<ConfidenceLevel>,
    /// The localized date confidence label, if asserted.
    pub date_confidence_label: Option<String>,
    /// How many citations back the date assertion.
    pub date_source_count: usize,
    /// The date assertion's citations, for the provenance popover.
    pub date_citations: Vec<CitationRefVm>,
    /// The linked place, if any.
    pub place: Option<PlaceLinkVm>,
    /// The operator's surety in the place link, if linked.
    pub place_confidence: Option<ConfidenceLevel>,
    /// The localized place confidence label, if linked.
    pub place_confidence_label: Option<String>,
    /// The event's free-text description, if set.
    pub description: Option<String>,
    /// The participants, joined to the person projection.
    pub participants: Vec<ParticipantVm>,
    /// The citations backing the event, with source · page · surety · evidence axes.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The event's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The event's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl EventDetail {
    /// Builds a detail view from an [`EventSummary`](genealogy_app::EventSummary), localizing labels,
    /// dates, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::EventSummary, loc: &Localizer) -> Self {
        let type_label = summary.event_type.as_ref().map_or_else(
            || summary.human_id.clone(),
            |event_type| loc.event_type_label(event_type),
        );
        let participants = summary
            .participants
            .iter()
            .map(|participant| {
                let confidence = ConfidenceLevel::from(participant.confidence);
                ParticipantVm {
                    human_id: participant.human_id.clone(),
                    id: participant.id.clone(),
                    name: participant.name.clone().unwrap_or_else(|| participant.human_id.clone()),
                    role_label: loc.participant_role_label(&participant.role),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                    source_count: participant.source_count,
                }
            })
            .collect();
        let date_confidence = summary.date_confidence.map(ConfidenceLevel::from);
        let place_confidence = summary.place_confidence.map(ConfidenceLevel::from);
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: type_label.clone(),
            type_label,
            date: summary.date.as_ref().map(|date| loc.date(date)),
            date_confidence,
            date_confidence_label: date_confidence.map(|level| loc.confidence_label(level)),
            date_source_count: summary.date_source_count,
            date_citations: summary
                .date_citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            place: summary.place.as_ref().map(|place| PlaceLinkVm {
                human_id: place.human_id.clone(),
                id: place.id.clone(),
                name: place.name.clone().unwrap_or_else(|| place.human_id.clone()),
            }),
            place_confidence,
            place_confidence_label: place_confidence.map(|level| loc.confidence_label(level)),
            description: summary.description.clone(),
            participants,
            citations: summary
                .citations
                .iter()
                .map(|citation| citation_ref_from_ref(citation, loc))
                .collect(),
            media: summary
                .media
                .iter()
                .map(|media| FamilyMediaVm {
                    human_id: media.human_id.clone(),
                    caption: media.caption.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a generic list row from an [`EventSummary`](genealogy_app::EventSummary): the type label,
/// a `date · place` subtitle, and a per-type avatar.
#[must_use]
pub fn event_row(summary: &genealogy_app::EventSummary, loc: &Localizer) -> RowVm {
    let title = summary.event_type.as_ref().map_or_else(
        || summary.human_id.clone(),
        |event_type| loc.event_type_label(event_type),
    );
    let date = summary.date.as_ref().map(|date| loc.date(date));
    let place = summary
        .place
        .as_ref()
        .map(|p| p.name.clone().unwrap_or_else(|| p.human_id.clone()));
    let subtitle = match (date, place) {
        (Some(date), Some(place)) => Some(format!("{date} · {place}")),
        (Some(date), None) => Some(date),
        (None, Some(place)) => Some(place),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some(event_avatar(summary.event_type.as_ref())),
        ..RowVm::default()
    }
}

/// The decorative avatar glyph for an event row, by type (a generic calendar otherwise).
fn event_avatar(event_type: Option<&EventType>) -> String {
    match event_type {
        Some(EventType::Marriage) => "💍",
        Some(EventType::Birth) => "👶",
        Some(EventType::Census) => "📋",
        Some(EventType::Burial | EventType::Cremation) => "⚰",
        Some(EventType::Baptism | EventType::Christening) => "✝",
        _ => "📅",
    }
    .to_owned()
}

/// The tab strip for an event's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn event_tabs(detail: &EventDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("participants", Some(detail.participants.len())),
        tab("citations", Some(detail.citations.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// One asserted place name (Names tab): text, language, date, surety, and source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceNameVm {
    /// The name text.
    pub text: String,
    /// The BCP-47 language tag, if recorded.
    pub language: Option<String>,
    /// The localized date the name was in use, if known.
    pub date: Option<String>,
    /// The operator's surety in the name assertion (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the name assertion.
    pub source_count: usize,
}

/// One enclosing place (Hierarchy tab): the place, its type, the dated link, and surety.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceHierarchyVm {
    /// The enclosing place's user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The enclosing place's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The enclosing place's display name (falls back to the `human_id`).
    pub name: String,
    /// The enclosing place's localized type label, if resolved.
    pub type_label: Option<String>,
    /// The localized dated link (when the enclosing relationship was valid), if dated.
    pub date: Option<String>,
    /// The operator's surety in the enclosing-by assertion (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
}

/// A place's detail view — type/coordinates/code facts, name history, the jurisdiction chain,
/// citations, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceDetail {
    /// The user-facing id (e.g. `P0001`).
    pub human_id: String,
    /// The stable `PlaceId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the place's primary name (falls back to the `human_id`).
    pub title: String,
    /// The localized place-type label, if set.
    pub type_label: Option<String>,
    /// The place's coordinates rendered as `lat, long`, if asserted.
    pub coordinates: Option<String>,
    /// The operator's surety in the coordinates, if asserted.
    pub coordinates_confidence: Option<ConfidenceLevel>,
    /// The localized coordinates confidence label, if asserted.
    pub coordinates_confidence_label: Option<String>,
    /// The coordinate assertion's citations, for the provenance popover.
    pub coordinate_citations: Vec<CitationRefVm>,
    /// The place's code, if set.
    pub code: Option<String>,
    /// The asserted names, with language/date + surety.
    pub names: Vec<PlaceNameVm>,
    /// The jurisdiction chain (enclosing places), nearest first.
    pub hierarchy: Vec<PlaceHierarchyVm>,
    /// The citations backing the place, with source · page · surety · evidence axes.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The place's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The place's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl PlaceDetail {
    /// Builds a detail view from a [`PlaceSummary`](genealogy_app::PlaceSummary), localizing labels,
    /// dates, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::PlaceSummary, loc: &Localizer) -> Self {
        let coordinates_confidence = summary.coordinates_confidence.map(ConfidenceLevel::from);
        let names = summary
            .names
            .iter()
            .map(|name| {
                let confidence = ConfidenceLevel::from(name.confidence);
                PlaceNameVm {
                    text: name.text.clone(),
                    language: name.language.clone(),
                    date: name.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                    source_count: name.source_count,
                }
            })
            .collect();
        let hierarchy = summary
            .enclosing
            .iter()
            .map(|enclosing| {
                let confidence = ConfidenceLevel::from(enclosing.confidence);
                PlaceHierarchyVm {
                    human_id: enclosing.human_id.clone(),
                    id: enclosing.id.clone(),
                    name: enclosing.name.clone().unwrap_or_else(|| enclosing.human_id.clone()),
                    type_label: enclosing.place_type.as_ref().map(|t| loc.place_type_label(t)),
                    date: enclosing.date.as_ref().map(|date| loc.date(date)),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                }
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: place_title(summary),
            type_label: summary.place_type.as_ref().map(|t| loc.place_type_label(t)),
            coordinates: summary.coordinates.clone(),
            coordinates_confidence,
            coordinates_confidence_label: coordinates_confidence.map(|level| loc.confidence_label(level)),
            coordinate_citations: summary
                .coordinate_citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            code: summary.code.clone(),
            names,
            hierarchy,
            citations: summary
                .citations
                .iter()
                .map(|citation| citation_ref_from_ref(citation, loc))
                .collect(),
            media: summary
                .media
                .iter()
                .map(|media| FamilyMediaVm {
                    human_id: media.human_id.clone(),
                    caption: media.caption.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// The place's primary name for the header (its first asserted name), or the `human_id` fallback.
fn place_title(summary: &genealogy_app::PlaceSummary) -> String {
    summary
        .names
        .first()
        .map_or_else(|| summary.human_id.clone(), |name| name.text.clone())
}

/// Builds a generic list row from a [`PlaceSummary`](genealogy_app::PlaceSummary): the primary name,
/// a `type · enclosing` subtitle, and a per-type avatar.
#[must_use]
pub fn place_row(summary: &genealogy_app::PlaceSummary, loc: &Localizer) -> RowVm {
    let type_label = summary.place_type.as_ref().map(|t| loc.place_type_label(t));
    let enclosing = summary
        .enclosing
        .first()
        .map(|e| e.name.clone().unwrap_or_else(|| e.human_id.clone()));
    let subtitle = match (type_label, enclosing) {
        (Some(type_label), Some(enclosing)) => Some(format!("{type_label} · {enclosing}")),
        (Some(type_label), None) => Some(type_label),
        (None, Some(enclosing)) => Some(enclosing),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title: place_title(summary),
        subtitle,
        avatar: Some(place_avatar(summary.place_type.as_ref())),
        ..RowVm::default()
    }
}

/// The decorative avatar glyph for a place row, by type (a generic pin otherwise).
fn place_avatar(place_type: Option<&genealogy_app::PlaceType>) -> String {
    use genealogy_app::PlaceType;
    match place_type {
        Some(PlaceType::Parish) => "⛪",
        _ => "📍",
    }
    .to_owned()
}

/// The tab strip for a place's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn place_tabs(detail: &PlaceDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("names", Some(detail.names.len())),
        tab("hierarchy", Some(detail.hierarchy.len())),
        tab("citations", Some(detail.citations.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// One repository a source is held in (Source › Repositories tab): the repo, call number, medium,
/// and the link's surety + source count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryLinkVm {
    /// The repository's user-facing id (e.g. `R0001`), if still projected.
    pub human_id: Option<String>,
    /// The repository's stable id (a UUID string) — the navigation key, if still projected.
    pub id: Option<String>,
    /// The repository's display name (falls back to the `human_id`).
    pub name: String,
    /// The source's call number / shelf mark in this repository, if recorded.
    pub call_number: Option<String>,
    /// The localized medium label (book, film, electronic, …).
    pub media_type_label: String,
    /// The operator's surety in the link (drives the confidence badge).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the link assertion.
    pub source_count: usize,
}

/// A record that uses a citation (Source › Citations "Backs record" cell): its kind drives the
/// route, plus the display label and the localized sub-context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitingRecordVm {
    /// The citing aggregate's kind (drives the navigation route and avatar).
    pub kind: genealogy_app::CitingKind,
    /// The citing record's user-facing id.
    pub human_id: String,
    /// The citing record's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The citing record's display label (a name/title, or the `human_id` fallback).
    pub label: String,
    /// The localized sub-context (e.g. "Birth", a participant role), empty for a row-level cite.
    pub context_label: String,
}

/// One citation that uses a source (Source › Citations tab): the citation row + the records it backs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCitationVm {
    /// The citation (source · page · surety · evidence axes).
    pub citation: CitationRefVm,
    /// The records that use this citation.
    pub backers: Vec<CitingRecordVm>,
}

/// One source attribute (Source › Attributes tab): key, value, and how many citations back it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAttributeVm {
    /// The attribute's type / key (verbatim — a free-text key).
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
    /// How many citations back the attribute.
    pub source_count: usize,
}

impl SourceAttributeVm {
    /// Whether a source backs this attribute (drives the no-source flag — colour-not-alone).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// The reliability synthesis for a source (Source › Overview "Reliability" card).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceReliabilityVm {
    /// The modal surety across the source's citations (drives the badge), if any.
    pub confidence: Option<ConfidenceLevel>,
    /// The localized modal-surety label, if any.
    pub confidence_label: Option<String>,
    /// The modal Evidence Explained axis chips (empty when no citation is analysed).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// How many citations cite this source.
    pub citation_count: usize,
    /// How many distinct records use those citations.
    pub record_count: usize,
}

/// A source's detail view — bibliographic facts, repository links, the citations that use it,
/// attributes, attachments, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDetail {
    /// The user-facing id (e.g. `S0001`).
    pub human_id: String,
    /// The stable `SourceId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the source's title (falls back to the `human_id`).
    pub title: String,
    /// The bibliographic author, if set.
    pub author: Option<String>,
    /// The publication info, if set.
    pub pub_info: Option<String>,
    /// The abbreviation, if set.
    pub abbrev: Option<String>,
    /// The repositories that hold this source.
    pub repositories: Vec<RepositoryLinkVm>,
    /// The citations that use this source, with their backing records.
    pub citations: Vec<SourceCitationVm>,
    /// The source's attributes.
    pub attributes: Vec<SourceAttributeVm>,
    /// The attached media objects.
    pub media: Vec<FamilyMediaVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The reliability synthesis derived from the source's citation set.
    pub reliability: SourceReliabilityVm,
    /// The source's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The source's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl SourceDetail {
    /// Builds a detail view from a [`SourceSummary`](genealogy_app::SourceSummary), localizing labels,
    /// media-type labels, and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::SourceSummary, loc: &Localizer) -> Self {
        let repositories = summary
            .repositories
            .iter()
            .map(|link| {
                let confidence = ConfidenceLevel::from(link.confidence);
                RepositoryLinkVm {
                    human_id: link.repository.as_ref().map(|r| r.human_id.clone()),
                    id: link.repository.as_ref().map(|r| r.id.clone()),
                    name: link.name.clone().unwrap_or_else(|| {
                        link.repository
                            .as_ref()
                            .map_or_else(String::new, |r| r.human_id.clone())
                    }),
                    call_number: link.call_number.clone(),
                    media_type_label: loc.source_media_type_label(&link.media_type),
                    confidence,
                    confidence_label: loc.confidence_label(confidence),
                    source_count: link.source_count,
                }
            })
            .collect();
        let citations = summary
            .citations
            .iter()
            .map(|row| SourceCitationVm {
                citation: citation_ref_from_ref(&row.citation, loc),
                backers: row.backers.iter().map(|b| citing_record_vm(b, loc)).collect(),
            })
            .collect();
        let attributes = summary
            .attributes
            .iter()
            .map(|a| SourceAttributeVm {
                attribute_type: a.attribute_type.clone(),
                value: a.value.clone(),
                source_count: a.source_count,
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: summary.title.clone().unwrap_or_else(|| summary.human_id.clone()),
            author: summary.author.clone(),
            pub_info: summary.pub_info.clone(),
            abbrev: summary.abbrev.clone(),
            repositories,
            citations,
            attributes,
            media: summary
                .media
                .iter()
                .map(|media| FamilyMediaVm {
                    human_id: media.human_id.clone(),
                    caption: media.caption.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            reliability: reliability_vm(&summary.reliability, loc),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a [`CitingRecordVm`] from an app [`CitingRecordRef`](genealogy_app::CitingRecordRef),
/// localizing its sub-context and falling back to the `human_id` for the label.
fn citing_record_vm(reference: &genealogy_app::CitingRecordRef, loc: &Localizer) -> CitingRecordVm {
    CitingRecordVm {
        kind: reference.kind,
        human_id: reference.human_id.clone(),
        id: reference.id.clone(),
        label: reference.label.clone().unwrap_or_else(|| reference.human_id.clone()),
        context_label: loc.citing_context_label(&reference.context),
    }
}

/// Builds the reliability view-model from the app [`SourceReliability`](genealogy_app::SourceReliability).
fn reliability_vm(reliability: &genealogy_app::SourceReliability, loc: &Localizer) -> SourceReliabilityVm {
    let confidence = reliability.typical_surety.map(ConfidenceLevel::from);
    SourceReliabilityVm {
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        evidence_axes: evidence_axes(reliability.evidence.as_ref(), loc),
        citation_count: reliability.citation_count,
        record_count: reliability.record_count,
    }
}

/// Builds a generic list row from a [`SourceSummary`](genealogy_app::SourceSummary): the title, an
/// `author · N citations` subtitle, and a 📚 avatar.
#[must_use]
pub fn source_row(summary: &genealogy_app::SourceSummary, loc: &Localizer) -> RowVm {
    let title = summary.title.clone().unwrap_or_else(|| summary.human_id.clone());
    let citations = loc.source_count(summary.reliability.citation_count);
    let subtitle = match &summary.author {
        Some(author) => Some(format!("{author} · {citations}")),
        None => Some(citations),
    };
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("📚".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a source's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn source_tabs(detail: &SourceDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("repositories", Some(detail.repositories.len())),
        tab("citations", Some(detail.citations.len())),
        tab("attributes", Some(detail.attributes.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// One source held by a repository (Repository › Sources tab): the source, call number, medium, and
/// how many citations cite it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHeldVm {
    /// The source's user-facing id (e.g. `S0001`).
    pub human_id: String,
    /// The source's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The source's display title (falls back to the `human_id`).
    pub title: String,
    /// The source's call number / shelf mark in this repository, if recorded.
    pub call_number: Option<String>,
    /// The localized medium label (book, film, electronic, …).
    pub media_type_label: String,
    /// How many citations cite the source.
    pub citation_count: usize,
}

/// A repository's detail view — type/name facts, addresses, URLs, the sources it holds, and the
/// audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryDetail {
    /// The user-facing id (e.g. `R0001`).
    pub human_id: String,
    /// The stable `RepositoryId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the repository's name (falls back to the `human_id`).
    pub title: String,
    /// The localized repository-type label, if set.
    pub type_label: Option<String>,
    /// The recorded postal addresses.
    pub addresses: Vec<genealogy_app::Address>,
    /// The recorded URLs.
    pub urls: Vec<genealogy_app::Url>,
    /// The sources held by this repository.
    pub sources: Vec<SourceHeldVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The repository's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The repository's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl RepositoryDetail {
    /// Builds a detail view from a [`RepositorySummary`](genealogy_app::RepositorySummary), localizing
    /// the type label and medium labels. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::RepositorySummary, loc: &Localizer) -> Self {
        let sources = summary
            .sources
            .iter()
            .map(|held| SourceHeldVm {
                human_id: held.source.human_id.clone(),
                id: held.source.id.clone(),
                title: held.title.clone().unwrap_or_else(|| held.source.human_id.clone()),
                call_number: held.call_number.clone(),
                media_type_label: loc.source_media_type_label(&held.media_type),
                citation_count: held.citation_count,
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: summary.name.clone().unwrap_or_else(|| summary.human_id.clone()),
            type_label: summary.repository_type.as_ref().map(|t| loc.repository_type_label(t)),
            addresses: summary.addresses.clone(),
            urls: summary.urls.clone(),
            sources,
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a generic list row from a [`RepositorySummary`](genealogy_app::RepositorySummary): the
/// name, a `type · locality` subtitle, and a per-type avatar.
#[must_use]
pub fn repository_row(summary: &genealogy_app::RepositorySummary, loc: &Localizer) -> RowVm {
    let type_label = summary.repository_type.as_ref().map(|t| loc.repository_type_label(t));
    let locality = summary.addresses.first().and_then(|a| a.locality.clone());
    let subtitle = match (type_label, locality) {
        (Some(type_label), Some(locality)) => Some(format!("{type_label} · {locality}")),
        (Some(type_label), None) => Some(type_label),
        (None, Some(locality)) => Some(locality),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title: summary.name.clone().unwrap_or_else(|| summary.human_id.clone()),
        subtitle,
        avatar: Some(repository_avatar(summary.repository_type.as_ref())),
        ..RowVm::default()
    }
}

/// The decorative avatar glyph for a repository row, by type (a generic building otherwise).
fn repository_avatar(repository_type: Option<&genealogy_app::RepositoryType>) -> String {
    use genealogy_app::RepositoryType;
    match repository_type {
        Some(RepositoryType::Church) => "⛪",
        Some(RepositoryType::Cemetery) => "🪦",
        Some(RepositoryType::Library) => "📚",
        Some(RepositoryType::Website) => "🌐",
        _ => "🏛",
    }
    .to_owned()
}

/// The tab strip for a repository's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn repository_tabs(detail: &RepositoryDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("addresses", Some(detail.addresses.len())),
        tab("urls", Some(detail.urls.len())),
        tab("sources", Some(detail.sources.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// A record that references a media object or note (Media "Used by" / Note "References"): its kind
/// drives the route, plus the display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsingRecordVm {
    /// The referencing aggregate's kind (drives the navigation route and the kind chip).
    pub kind: genealogy_app::UsingKind,
    /// The referencing record's user-facing id.
    pub human_id: String,
    /// The referencing record's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The referencing record's display label (a name/title, or the `human_id` fallback).
    pub label: String,
    /// The localized kind label (the chip text — colour/route is never the only signal).
    pub kind_label: String,
}

/// Builds a [`UsingRecordVm`] from an app [`UsingRecordRef`](genealogy_app::UsingRecordRef).
fn using_record_vm(reference: &genealogy_app::UsingRecordRef, loc: &Localizer) -> UsingRecordVm {
    UsingRecordVm {
        kind: reference.kind,
        human_id: reference.human_id.clone(),
        id: reference.id.clone(),
        label: reference.label.clone().unwrap_or_else(|| reference.human_id.clone()),
        kind_label: loc.using_kind_label(reference.kind),
    }
}

/// One typed attribute on a media object (Media File card): key and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAttributeVm {
    /// The attribute's type / key.
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
}

/// A media object's detail view — file metadata, the citations backing it, attached notes, tags, the
/// records that use it, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDetail {
    /// The user-facing id (e.g. `O0001`).
    pub human_id: String,
    /// The stable `MediaId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the file's basename (falls back to the `human_id`).
    pub title: String,
    /// The media's location rendered for display, if set.
    pub path: Option<String>,
    /// The media's MIME type (e.g. `image/jpeg`), if set.
    pub mime: Option<String>,
    /// The media's checksum, if set.
    pub checksum: Option<String>,
    /// The media's localized date, if asserted.
    pub date: Option<String>,
    /// The recorded attributes (File card metadata).
    pub attributes: Vec<MediaAttributeVm>,
    /// The citations backing the media's claims.
    pub citations: Vec<CitationRefVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The records that reference this media (the "Used by" card).
    pub used_by: Vec<UsingRecordVm>,
    /// The media's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The media's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl MediaDetail {
    /// Builds a detail view from a [`MediaSummary`](genealogy_app::MediaSummary), localizing the date
    /// and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::MediaSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: summary
                .path
                .as_deref()
                .map(file_basename)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| summary.human_id.clone()),
            path: summary.path.clone(),
            mime: summary.mime.clone(),
            checksum: summary.checksum.clone(),
            date: summary.date.as_ref().map(|date| loc.date(date)),
            attributes: summary
                .attributes
                .iter()
                .map(|a| MediaAttributeVm {
                    attribute_type: a.attribute_type.clone(),
                    value: a.value.clone(),
                })
                .collect(),
            citations: summary
                .citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            used_by: summary.used_by.iter().map(|u| using_record_vm(u, loc)).collect(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// The basename of a file path (the segment after the last `/` or `\`), for the media title.
fn file_basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

/// Builds a generic list row from a [`MediaSummary`](genealogy_app::MediaSummary): the filename, a
/// `mime · date` subtitle, and a 📷 avatar.
#[must_use]
pub fn media_row(summary: &genealogy_app::MediaSummary, loc: &Localizer) -> RowVm {
    let title = summary
        .path
        .as_deref()
        .map(file_basename)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| summary.human_id.clone());
    let date = summary.date.as_ref().map(|date| loc.date(date));
    let subtitle = match (summary.mime.clone(), date) {
        (Some(mime), Some(date)) => Some(format!("{mime} · {date}")),
        (Some(mime), None) => Some(mime),
        (None, Some(date)) => Some(date),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("📷".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a media object's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn media_tabs(detail: &MediaDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("citations", Some(detail.citations.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// One translation of a note's content (Note Language tab): language, text, and translator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationVm {
    /// The translation's language tag (e.g. `nb-NO`), if recorded.
    pub language: Option<String>,
    /// The translated text.
    pub text: String,
    /// Who produced the translation, if recorded.
    pub translator: Option<String>,
}

/// A note's detail view — its type, rich-text content, language + translations, the records that
/// reference it, tags, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDetail {
    /// The user-facing id (e.g. `N0001`).
    pub human_id: String,
    /// The stable `NoteId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the first line of the note's text (falls back to the `human_id`).
    pub title: String,
    /// The note's type, if set (carried for the edit form).
    pub note_type: Option<genealogy_app::NoteType>,
    /// The localized note-type label, if set.
    pub note_type_label: Option<String>,
    /// The note's primary text content, if set.
    pub text: Option<String>,
    /// The primary content's language tag, if recorded.
    pub language: Option<String>,
    /// Translations of the primary content into other languages.
    pub translations: Vec<TranslationVm>,
    /// The records that reference this note.
    pub references: Vec<UsingRecordVm>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The note's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The note's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl NoteDetail {
    /// Builds a detail view from a [`NoteSummary`](genealogy_app::NoteSummary), localizing the type
    /// label. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::NoteSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: note_title(summary.text.as_deref()).unwrap_or_else(|| summary.human_id.clone()),
            note_type: summary.note_type.clone(),
            note_type_label: summary.note_type.as_ref().map(|t| loc.note_type_label(t)),
            text: summary.text.clone(),
            language: summary.language.clone(),
            translations: summary
                .translations
                .iter()
                .map(|t| TranslationVm {
                    language: t.language.clone(),
                    text: t.text.clone(),
                    translator: t.translator.clone(),
                })
                .collect(),
            references: summary.references.iter().map(|u| using_record_vm(u, loc)).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// A note's title: the first non-empty line of its text, with a leading Markdown heading marker
/// stripped, truncated for the list/header. `None` when the note has no text.
fn note_title(text: Option<&str>) -> Option<String> {
    let line = text?.lines().map(str::trim).find(|line| !line.is_empty())?;
    let line = line.trim_start_matches('#').trim();
    let title: String = line.chars().take(60).collect();
    (!title.is_empty()).then_some(title)
}

/// Builds a generic list row from a [`NoteSummary`](genealogy_app::NoteSummary): a title from the
/// text, a `type · language · N references` subtitle, and a 🗒 avatar.
#[must_use]
pub fn note_row(summary: &genealogy_app::NoteSummary, loc: &Localizer) -> RowVm {
    let title = note_title(summary.text.as_deref()).unwrap_or_else(|| summary.human_id.clone());
    let mut parts: Vec<String> = Vec::new();
    if let Some(note_type) = &summary.note_type {
        parts.push(loc.note_type_label(note_type));
    }
    if let Some(language) = &summary.language {
        parts.push(language.clone());
    }
    if !summary.references.is_empty() {
        parts.push(loc.reference_count(summary.references.len()));
    }
    let subtitle = (!parts.is_empty()).then(|| parts.join(" · "));
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("🗒".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a note's detail: the content, then the related-item tabs with counts.
#[must_use]
pub fn note_tabs(detail: &NoteDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("content", None),
        tab("language", Some(detail.translations.len() + 1)),
        tab("references", Some(detail.references.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// Builds a navigable reference vm (kind + ids + localized kind label) for a related record.
fn nav_ref(kind: genealogy_app::UsingKind, human_id: &str, id: &str, label: String, loc: &Localizer) -> UsingRecordVm {
    UsingRecordVm {
        kind,
        human_id: human_id.to_owned(),
        id: id.to_owned(),
        label,
        kind_label: loc.using_kind_label(kind),
    }
}

/// One object-type group on the Tag Usage tab: the localized kind, the count, and a few examples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagUsageGroupVm {
    /// The localized object-type label (the row's first cell).
    pub kind_label: String,
    /// How many records of this kind carry the tag.
    pub count: usize,
    /// The first few carrying records, navigable.
    pub examples: Vec<UsingRecordVm>,
}

/// A tag's detail view — its name, colour, priority, the records that carry it grouped by type, and
/// the audit history. The tag's UUID is the join key but is never rendered (data-model §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDetail {
    /// The stable `TagId` (a UUID string) — the navigation/join key, never rendered.
    pub id: String,
    /// The header title: the tag name (falls back to a placeholder).
    pub title: String,
    /// The tag's name, if set (carried for the edit form).
    pub name: Option<String>,
    /// The tag's colour (a CSS hex string), if set.
    pub color: Option<String>,
    /// The tag's sort priority, if set.
    pub priority: Option<i32>,
    /// How many records carry this tag in total (the header subtitle).
    pub total: usize,
    /// The records carrying this tag, grouped by object type (the Usage tab).
    pub usage: Vec<TagUsageGroupVm>,
    /// The tag's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl TagDetail {
    /// Builds a detail view from a [`TagSummary`](genealogy_app::TagSummary).
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::TagSummary, loc: &Localizer) -> Self {
        let total = summary.usage_count;
        let usage = summary
            .usage
            .iter()
            .map(|group| TagUsageGroupVm {
                kind_label: loc.using_kind_label(group.kind),
                count: group.count,
                examples: group.examples.iter().map(|u| using_record_vm(u, loc)).collect(),
            })
            .collect();
        Self {
            id: summary.id.clone(),
            title: summary.name.clone().unwrap_or_else(|| loc.display_name(None)),
            name: summary.name.clone(),
            color: summary.color.clone(),
            priority: summary.priority,
            total,
            usage,
            history: Vec::new(),
        }
    }
}

/// The default sort priority a fresh tag draft seeds (data-model §9; the mockup's `priority 1`).
pub const DEFAULT_TAG_PRIORITY: i32 = 1;

/// The default colour a fresh tag draft seeds (a CSS hex string — the mockup's neutral swatch).
pub const DEFAULT_TAG_COLOR: &str = "#1A2129";

/// The buffered state of the directly-editable tag record (create + edit, one mechanism). The Dioxus
/// tag Overview binds its Name / Priority / Colour inputs to these fields; nothing is persisted until
/// Save, when [`Self::to_request`] turns the buffer into a [`TagChangeSetRequest`]. Cancel drops it.
///
/// One value serves both modes: [`Self::new`] seeds the create defaults (empty name, priority 1,
/// colour `#1A2129`), [`Self::from_detail`] is pre-populated (edit) and records the tag's id in
/// `existing_id`. Priority is held as the raw text the number spinner emits, so an in-progress empty
/// or non-numeric entry is representable (and flagged invalid) rather than silently coerced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDraft {
    /// `Some` in edit mode (the tag being edited); `None` in create mode.
    pub existing_id: Option<String>,
    /// The tag's name (required, non-empty).
    pub name: String,
    /// The tag's sort priority, as the raw spinner text (required; must parse to an `i32`).
    pub priority: String,
    /// The tag's colour, a CSS hex string (required, non-empty).
    pub color: String,
}

impl TagDraft {
    /// A fresh draft for creating a new tag: empty name, the default priority, and the default colour.
    #[must_use]
    pub fn new() -> Self {
        Self {
            existing_id: None,
            name: String::new(),
            priority: DEFAULT_TAG_PRIORITY.to_string(),
            color: DEFAULT_TAG_COLOR.to_owned(),
        }
    }

    /// A draft pre-populated from an existing tag for editing. Records the id so the commit edits
    /// (diffs) rather than creates; seeds each unset field with its create default.
    #[must_use]
    pub fn from_detail(detail: &TagDetail) -> Self {
        Self {
            existing_id: Some(detail.id.clone()),
            name: detail.name.clone().unwrap_or_default(),
            priority: detail
                .priority
                .map_or_else(|| DEFAULT_TAG_PRIORITY.to_string(), |p| p.to_string()),
            color: detail.color.clone().unwrap_or_else(|| DEFAULT_TAG_COLOR.to_owned()),
        }
    }

    /// The priority parsed from the spinner text, or `None` when empty / non-numeric (invalid).
    #[must_use]
    pub fn parsed_priority(&self) -> Option<i32> {
        self.priority.trim().parse::<i32>().ok()
    }

    /// Whether every field is present and valid (name non-empty, priority a number, colour non-empty)
    /// — the Save gate.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && self.parsed_priority().is_some() && !self.color.trim().is_empty()
    }

    /// Builds the [`TagChangeSetRequest`] the app commits on Save, or `None` when the draft is
    /// invalid (so Save is a no-op rather than committing a partial tag).
    #[must_use]
    pub fn to_request(&self) -> Option<TagChangeSetRequest> {
        let priority = self.parsed_priority()?;
        if self.name.trim().is_empty() || self.color.trim().is_empty() {
            return None;
        }
        Some(TagChangeSetRequest {
            existing_id: self.existing_id.clone(),
            name: self.name.trim().to_owned(),
            priority,
            color: self.color.trim().to_owned(),
        })
    }
}

impl Default for TagDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a list row from a [`TagSummary`](genealogy_app::TagSummary): the name, a `priority N · X
/// objects` subtitle, a colour-dot avatar, and the `#hex` colour as the trailing id label. The
/// navigation key is the tag's stable id (a UUID, never rendered — data-model §9).
#[must_use]
pub fn tag_row(summary: &genealogy_app::TagSummary, loc: &Localizer) -> RowVm {
    let priority = summary.priority.unwrap_or(DEFAULT_TAG_PRIORITY);
    RowVm {
        id: summary.id.clone(),
        title: summary.name.clone().unwrap_or_else(|| loc.display_name(None)),
        subtitle: Some(loc.tag_row_subtitle(priority, summary.usage_count)),
        avatar: None,
        // A tag with no colour still gets a (neutral) dot so the avatar column is consistent, and an
        // empty id label — never the UUID, which `display_id` would otherwise fall back to (§9).
        dot_color: Some(summary.color.clone().unwrap_or_else(|| DEFAULT_TAG_COLOR.to_owned())),
        id_label: Some(summary.color.clone().unwrap_or_default()),
    }
}

/// The tab strip for a tag's detail: overview, usage (with the total count), and history.
#[must_use]
pub fn tag_tabs(detail: &TagDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("usage", Some(detail.total)),
        tab("history", None),
    ]
}

/// A match this kit produced — one row on the DNA test › Matches tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestMatchVm {
    /// The match record, navigable (to the DNA-match detail).
    pub match_ref: UsingRecordVm,
    /// The other test compared against this kit, navigable, if still projected.
    pub compared_test: Option<UsingRecordVm>,
    /// Total shared centimorgans, rendered for display.
    pub shared_cm: Option<String>,
    /// Shared percentage, rendered for display.
    pub percent_shared: Option<String>,
    /// The provider's predicted relationship, if any.
    pub predicted: Option<String>,
}

/// A DNA test's detail view — kit metadata, the anchoring person, haplogroups, the matches it
/// produced, attached notes, tags, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestDetail {
    /// The user-facing id (e.g. `D0001`).
    pub human_id: String,
    /// The stable `DnaTestId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: `provider — person` (falls back to the `human_id`).
    pub title: String,
    /// The testing provider's localized label, if set.
    pub provider: Option<String>,
    /// The test type's localized label, if set.
    pub test_type: Option<String>,
    /// The provider's kit id, if set.
    pub kit_id: Option<String>,
    /// The genome build's localized label, if set.
    pub genome_build: Option<String>,
    /// The anchoring person, navigable, if still projected.
    pub person: Option<UsingRecordVm>,
    /// The anchoring person's display name, if resolvable.
    pub person_name: Option<String>,
    /// The recorded haplogroups (the Haplogroups tab).
    pub haplogroups: Vec<String>,
    /// The matches this kit produced (the Matches tab).
    pub matches: Vec<DnaTestMatchVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The test's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The test's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl DnaTestDetail {
    /// Builds a detail view from a [`DnaTestSummary`](genealogy_app::DnaTestSummary).
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::DnaTestSummary, loc: &Localizer) -> Self {
        let provider = summary.provider.as_ref().map(|p| loc.dna_provider_label(p));
        let title = match (provider.clone(), summary.person_name.clone()) {
            (Some(provider), Some(person)) => format!("{provider} — {person}"),
            (Some(provider), None) => provider,
            (None, Some(person)) => person,
            (None, None) => summary.human_id.clone(),
        };
        let person = summary.person.as_ref().map(|p| {
            let label = summary.person_name.clone().unwrap_or_else(|| p.human_id.clone());
            nav_ref(genealogy_app::UsingKind::Person, &p.human_id, &p.id, label, loc)
        });
        let matches = summary
            .matches
            .iter()
            .map(|m| DnaTestMatchVm {
                match_ref: nav_ref(
                    genealogy_app::UsingKind::DnaMatch,
                    &m.dna_match.human_id,
                    &m.dna_match.id,
                    m.dna_match.human_id.clone(),
                    loc,
                ),
                compared_test: m.compared_test.as_ref().map(|t| {
                    nav_ref(
                        genealogy_app::UsingKind::DnaTest,
                        &t.human_id,
                        &t.id,
                        t.human_id.clone(),
                        loc,
                    )
                }),
                shared_cm: m.shared_cm.clone(),
                percent_shared: m.percent_shared.clone(),
                predicted: m.predicted_relationship.clone(),
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title,
            provider,
            test_type: summary.test_type.map(|t| loc.dna_test_type_label(t)),
            kit_id: summary.kit_id.clone(),
            genome_build: summary.genome_build.map(|b| loc.dna_genome_build_label(b)),
            person,
            person_name: summary.person_name.clone(),
            haplogroups: summary.haplogroups.clone(),
            matches,
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a list row from a [`DnaTestSummary`](genealogy_app::DnaTestSummary): the person name (or id),
/// a `provider · type` subtitle, and a 🧬 avatar.
#[must_use]
pub fn dna_test_row(summary: &genealogy_app::DnaTestSummary, loc: &Localizer) -> RowVm {
    let mut parts: Vec<String> = Vec::new();
    if let Some(provider) = &summary.provider {
        parts.push(loc.dna_provider_label(provider));
    }
    if let Some(test_type) = summary.test_type {
        parts.push(loc.dna_test_type_label(test_type));
    }
    let subtitle = (!parts.is_empty()).then(|| parts.join(" · "));
    RowVm {
        id: summary.human_id.clone(),
        title: summary.person_name.clone().unwrap_or_else(|| summary.human_id.clone()),
        subtitle,
        avatar: Some("🧬".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a DNA test's detail: overview, then haplogroups/matches/notes/tags with counts.
#[must_use]
pub fn dna_test_tabs(detail: &DnaTestDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("haplogroups", Some(detail.haplogroups.len())),
        tab("matches", Some(detail.matches.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// One matching segment on the DNA match › Segments tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaSegmentVm {
    /// The chromosome (`1`..=`22` or `X`).
    pub chromosome: String,
    /// The start position (base pairs), rendered.
    pub start: String,
    /// The end position (base pairs), rendered.
    pub end: String,
    /// The segment length in centimorgans, rendered.
    pub centimorgans: String,
    /// The matching-SNP count, rendered, if known.
    pub snps: Option<String>,
    /// The localized parental-side (phasing) label.
    pub side: String,
}

/// One inferred shared ancestor on the DNA match › Shared ancestors tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedAncestorVm {
    /// The inferred ancestor, navigable to the Person record, if identified.
    pub person: Option<UsingRecordVm>,
    /// The free-text note describing the shared ancestry, if any.
    pub note: Option<String>,
}

/// A DNA match's detail view — the two compared tests, the observed shared-DNA totals, the inferred
/// relationship, segments, shared ancestors, notes, tags, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaMatchDetail {
    /// The user-facing id (e.g. `X0001`).
    pub human_id: String,
    /// The stable `DnaMatchId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: `person A ⟷ person B` (falls back to the test ids or the `human_id`).
    pub title: String,
    /// One side's test, navigable, if still projected.
    pub test_a: Option<UsingRecordVm>,
    /// The other side's test, navigable, if still projected.
    pub test_b: Option<UsingRecordVm>,
    /// The provider the match was observed at, localized, if set.
    pub provider: Option<String>,
    /// Total shared centimorgans, rendered.
    pub shared_cm: Option<String>,
    /// Shared percentage, rendered.
    pub percent_shared: Option<String>,
    /// The largest shared segment's length, rendered.
    pub largest_segment_cm: Option<String>,
    /// The provider's predicted relationship, if any (the inferred-relationship conclusion).
    pub predicted_relationship: Option<String>,
    /// The localized confirmation-status label.
    pub status: String,
    /// The recorded shared segments (the Segments tab).
    pub segments: Vec<DnaSegmentVm>,
    /// The inferred shared ancestors (the Shared ancestors tab).
    pub shared_ancestors: Vec<SharedAncestorVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The match's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The match's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl DnaMatchDetail {
    /// Builds a detail view from a [`DnaMatchSummary`](genealogy_app::DnaMatchSummary).
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::DnaMatchSummary, loc: &Localizer) -> Self {
        let test_ref = |test: &Option<genealogy_app::AggRef>, person: &Option<String>| {
            test.as_ref().map(|t| {
                let label = person.clone().unwrap_or_else(|| t.human_id.clone());
                nav_ref(genealogy_app::UsingKind::DnaTest, &t.human_id, &t.id, label, loc)
            })
        };
        let test_a = test_ref(&summary.test_a, &summary.test_a_person);
        let test_b = test_ref(&summary.test_b, &summary.test_b_person);
        let title = match (summary.test_a_person.clone(), summary.test_b_person.clone()) {
            (Some(a), Some(b)) => format!("{a} ⟷ {b}"),
            _ => summary.human_id.clone(),
        };
        let segments = summary
            .segments
            .iter()
            .map(|s| DnaSegmentVm {
                chromosome: s.chromosome.clone(),
                start: s.start.to_string(),
                end: s.end.to_string(),
                centimorgans: s.centimorgans.to_string(),
                snps: s.snps.map(|n| n.to_string()),
                side: loc.chromosome_side_label(s.side),
            })
            .collect();
        let shared_ancestors = summary
            .shared_ancestors
            .iter()
            .map(|a| SharedAncestorVm {
                person: a.person.as_ref().map(|p| {
                    let label = a.person_name.clone().unwrap_or_else(|| p.human_id.clone());
                    nav_ref(genealogy_app::UsingKind::Person, &p.human_id, &p.id, label, loc)
                }),
                note: a.note.clone(),
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title,
            test_a,
            test_b,
            provider: summary.provider.as_ref().map(|p| loc.dna_provider_label(p)),
            shared_cm: summary.shared_cm.clone(),
            percent_shared: summary.percent_shared.clone(),
            largest_segment_cm: summary.largest_segment_cm.clone(),
            predicted_relationship: summary.predicted_relationship.clone(),
            status: loc.match_status_label(summary.status),
            segments,
            shared_ancestors,
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a list row from a [`DnaMatchSummary`](genealogy_app::DnaMatchSummary): `A ⟷ B`, a
/// `shared cM · predicted` subtitle, and a 🔗 avatar.
#[must_use]
pub fn dna_match_row(summary: &genealogy_app::DnaMatchSummary, loc: &Localizer) -> RowVm {
    let title = match (summary.test_a_person.clone(), summary.test_b_person.clone()) {
        (Some(a), Some(b)) => format!("{a} ⟷ {b}"),
        _ => summary.human_id.clone(),
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(shared) = &summary.shared_cm {
        parts.push(format!("{shared} {}", loc.field_label("centimorgans")));
    }
    if let Some(predicted) = &summary.predicted_relationship {
        parts.push(predicted.clone());
    }
    let subtitle = (!parts.is_empty()).then(|| parts.join(" · "));
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("🔗".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a DNA match's detail: overview, then segments/ancestors/notes/tags with counts.
#[must_use]
pub fn dna_match_tabs(detail: &DnaMatchDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("segments", Some(detail.segments.len())),
        tab("ancestors", Some(detail.shared_ancestors.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

// ---------------------------------------------------------------------------------------------------
// Pedigree tool (PR 18): ancestor/descendant charts + the kinship calculator
// ---------------------------------------------------------------------------------------------------

/// One person referenced from a pedigree chart: display name + lifespan, evidence cues, and stable
/// id for navigation. `confidence`/`source_count` describe the parent-child assertion linking this
/// node to the adjacent one; the focus person carries none (it is not itself an assertion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedigreeNodeVm {
    /// The person's user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The person's display name (falls back to the `human_id`).
    pub name: String,
    /// The "born – died" lifespan, if known.
    pub vitals: Option<String>,
    /// The surety of the parent-child link to the adjacent node, absent for the focus person.
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: Option<String>,
    /// How many citations back that link.
    pub source_count: usize,
    /// The person's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// Whether this node has at least one further generation beyond the flattened chart (the
    /// `aria-expanded` cue on its `role="treeitem"` — the chart itself never collapses/expands, so
    /// this only says whether the fan would continue). Unused (`false`) on the focus person and the
    /// relationship calculator's two people, which are not chart nodes.
    pub has_more: bool,
}

/// One slot in an ancestor-chart generation: a known ancestor, or a placeholder naming which parent
/// (of whom) is still unresearched — never rendered as a dead end (the evidence-first differentiator
/// carried into the pedigree chart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedigreeSlotVm {
    /// A known ancestor.
    Known(PedigreeNodeVm),
    /// No ancestor is shown at this slot; `hint` names whose parent it is, or a generic
    /// "unresearched" hint once the branch above is itself unknown.
    Unknown {
        /// The already-localized placeholder hint.
        hint: String,
    },
}

/// The Pedigree tool's view-model (PR 18): the focus person, the ancestor chart's generations (each
/// a complete row of `2^generation` slots, padded with [`PedigreeSlotVm::Unknown`] so the fan stays
/// rectangular), and the descendant chart's generations (variable width — a childless branch simply
/// ends, it is not a research gap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedigreeVm {
    /// The person both charts are centered on.
    pub focus: PedigreeNodeVm,
    /// Ancestor generations, nearest first (index 0 = parents).
    pub ancestor_generations: Vec<Vec<PedigreeSlotVm>>,
    /// Descendant generations, nearest first (index 0 = children).
    pub descendant_generations: Vec<Vec<PedigreeNodeVm>>,
}

impl PedigreeVm {
    /// Builds the view-model from the app's ancestor and descendant charts, localizing confidence
    /// labels and the unresearched-slot hints. `depth` is the number of generations shown on each
    /// side (as requested of the app use-cases) and bounds how many rows are flattened.
    #[must_use]
    pub fn build(
        ancestors: &genealogy_app::PedigreeChart,
        descendants: &genealogy_app::DescendantChart,
        depth: usize,
        loc: &Localizer,
    ) -> Self {
        Self {
            focus: pedigree_node_vm(&ancestors.focus, None, 0, false, loc),
            ancestor_generations: flatten_ancestors(ancestors, depth, loc),
            descendant_generations: flatten_descendants(descendants, depth, loc),
        }
    }
}

/// Builds a [`PedigreeNodeVm`] from an app [`PedigreePersonRef`](genealogy_app::PedigreePersonRef).
fn pedigree_node_vm(
    person: &genealogy_app::PedigreePersonRef,
    confidence: Option<genealogy_app::Confidence>,
    source_count: usize,
    has_more: bool,
    loc: &Localizer,
) -> PedigreeNodeVm {
    let confidence = confidence.map(ConfidenceLevel::from);
    PedigreeNodeVm {
        human_id: person.human_id.clone(),
        name: person.name.clone().unwrap_or_else(|| person.human_id.clone()),
        vitals: person.vitals.clone(),
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        source_count,
        restrictions: person.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
        has_more,
    }
}

/// Which parent slot a placeholder/ancestor is — carried through the flatten walk to build the
/// "father of {name}" / "mother of {name}" hint (or the generic "unresearched" form).
#[derive(Debug, Clone, Copy)]
enum ParentSlot {
    Father,
    Mother,
}

/// The context a slot in the flatten walk carries: which parent role it is, and the known
/// descendant's name it is a parent of (absent once the branch itself has gone unresearched).
struct SlotContext {
    of_name: Option<String>,
    role: ParentSlot,
}

/// Flattens the ancestor tree into complete generation rows (padding unresearched branches so every
/// row has exactly `2^generation` slots), up to `depth` generations.
fn flatten_ancestors(chart: &genealogy_app::PedigreeChart, depth: usize, loc: &Localizer) -> Vec<Vec<PedigreeSlotVm>> {
    let focus_name = chart.focus.name.clone().unwrap_or_else(|| chart.focus.human_id.clone());
    let mut frontier: Vec<(Option<&genealogy_app::AncestorSlot>, SlotContext)> = vec![
        (
            Some(&chart.father),
            SlotContext {
                of_name: Some(focus_name.clone()),
                role: ParentSlot::Father,
            },
        ),
        (
            Some(&chart.mother),
            SlotContext {
                of_name: Some(focus_name),
                role: ParentSlot::Mother,
            },
        ),
    ];
    let mut generations = Vec::with_capacity(depth);
    for _ in 0..depth {
        if frontier.is_empty() {
            break;
        }
        let mut row = Vec::with_capacity(frontier.len());
        let mut next = Vec::with_capacity(frontier.len() * 2);
        for (slot, context) in frontier {
            match slot {
                Some(genealogy_app::AncestorSlot::Known(node)) => {
                    let name = node.person.name.clone().unwrap_or_else(|| node.person.human_id.clone());
                    let has_more = matches!(node.father, genealogy_app::AncestorSlot::Known(_))
                        || matches!(node.mother, genealogy_app::AncestorSlot::Known(_));
                    row.push(PedigreeSlotVm::Known(pedigree_node_vm(
                        &node.person,
                        Some(node.confidence),
                        node.source_count,
                        has_more,
                        loc,
                    )));
                    next.push((
                        Some(&node.father),
                        SlotContext {
                            of_name: Some(name.clone()),
                            role: ParentSlot::Father,
                        },
                    ));
                    next.push((
                        Some(&node.mother),
                        SlotContext {
                            of_name: Some(name),
                            role: ParentSlot::Mother,
                        },
                    ));
                }
                None | Some(genealogy_app::AncestorSlot::Unknown) => {
                    row.push(PedigreeSlotVm::Unknown {
                        hint: unknown_hint(loc, &context),
                    });
                    next.push((
                        None,
                        SlotContext {
                            of_name: None,
                            role: ParentSlot::Father,
                        },
                    ));
                    next.push((
                        None,
                        SlotContext {
                            of_name: None,
                            role: ParentSlot::Mother,
                        },
                    ));
                }
            }
        }
        generations.push(row);
        frontier = next;
    }
    generations
}

/// The localized hint for an unresearched ancestor slot: "father/mother of {name}" when the known
/// descendant's name is still in context, else the generic "line unresearched" form.
fn unknown_hint(loc: &Localizer, context: &SlotContext) -> String {
    match (&context.of_name, context.role) {
        (Some(name), ParentSlot::Father) => loc.pedigree_unknown_father_of(name),
        (Some(name), ParentSlot::Mother) => loc.pedigree_unknown_mother_of(name),
        (None, ParentSlot::Father) => loc.pedigree_father_unresearched(),
        (None, ParentSlot::Mother) => loc.pedigree_mother_unresearched(),
    }
}

/// Flattens the descendant tree into generation rows, up to `depth` generations. Unlike the ancestor
/// chart, rows are not padded — an empty branch means no known children, not a research gap.
fn flatten_descendants(
    tree: &genealogy_app::DescendantChart,
    depth: usize,
    loc: &Localizer,
) -> Vec<Vec<PedigreeNodeVm>> {
    let mut frontier: Vec<&genealogy_app::DescendantNode> = tree.children.iter().collect();
    let mut generations = Vec::with_capacity(depth);
    for _ in 0..depth {
        if frontier.is_empty() {
            break;
        }
        let mut row = Vec::with_capacity(frontier.len());
        let mut next = Vec::new();
        for node in frontier {
            row.push(pedigree_node_vm(
                &node.person,
                Some(node.confidence),
                node.source_count,
                !node.children.is_empty(),
                loc,
            ));
            next.extend(node.children.iter());
        }
        generations.push(row);
        frontier = next;
    }
    generations
}

/// The kinship calculator's view-model: the two people, each with their evidence-free node vm, and
/// the localized relationship summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipVm {
    /// The first person.
    pub person_a: PedigreeNodeVm,
    /// The second person.
    pub person_b: PedigreeNodeVm,
    /// The already-localized relationship description, or the "not found" message.
    pub summary: String,
}

impl RelationshipVm {
    /// Builds the view-model from the app's [`RelationshipResult`](genealogy_app::RelationshipResult),
    /// localizing the kinship into a display sentence.
    #[must_use]
    pub fn build(result: &genealogy_app::RelationshipResult, loc: &Localizer) -> Self {
        let person_a = pedigree_node_vm(&result.person_a, None, 0, false, loc);
        let person_b = pedigree_node_vm(&result.person_b, None, 0, false, loc);
        let summary = match &result.kinship {
            Some(kinship) => loc.kinship_summary(&person_a.name, &person_b.name, kinship),
            None => loc.kinship_not_found(),
        };
        Self {
            person_a,
            person_b,
            summary,
        }
    }
}

/// One flagged possible-duplicate pair (Phase 5 PR 19's Compare/merge screen): the two persons, why
/// they were flagged (already localized), and the heuristic's confidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateCandidateVm {
    /// The first person.
    pub a: PedigreeNodeVm,
    /// The second person.
    pub b: PedigreeNodeVm,
    /// The already-localized reason the pair was flagged.
    pub reason: String,
    /// The heuristic's match confidence, as a presentation level (not an operator-asserted surety —
    /// [`genealogy_app::DuplicateCandidate::score`] is a match score, mapped onto the same five-level
    /// scale the rest of the UI already renders via [`ConfidenceBadge`](crate::presentation)).
    pub confidence: ConfidenceLevel,
    /// The already-localized confidence label.
    pub confidence_label: String,
}

impl DuplicateCandidateVm {
    /// Builds the view-model from an app [`DuplicateCandidate`](genealogy_app::DuplicateCandidate),
    /// localizing the match reason and mapping its raw `0..=100` score onto a [`ConfidenceLevel`].
    #[must_use]
    pub fn build(candidate: &genealogy_app::DuplicateCandidate, loc: &Localizer) -> Self {
        let confidence = confidence_level_from_score(candidate.score);
        Self {
            a: node_ref(&candidate.a),
            b: node_ref(&candidate.b),
            reason: loc.duplicate_match_reason(&candidate.kind),
            confidence,
            confidence_label: loc.confidence_label(confidence),
        }
    }
}

/// Builds a bare [`PedigreeNodeVm`] from an [`AggRef`](genealogy_app::AggRef) — no vitals/confidence,
/// just the id + display fallback the duplicates table and merge picker need for navigation.
fn node_ref(agg: &genealogy_app::AggRef) -> PedigreeNodeVm {
    PedigreeNodeVm {
        human_id: agg.human_id.clone(),
        name: agg.human_id.clone(),
        vitals: None,
        confidence: None,
        confidence_label: None,
        source_count: 0,
        restrictions: Vec::new(),
        has_more: false,
    }
}

/// Maps a heuristic match score (`0..=100`, higher = more likely a duplicate) onto the presentation
/// [`ConfidenceLevel`] scale, so the duplicates table can reuse the existing confidence badge.
fn confidence_level_from_score(score: u8) -> ConfidenceLevel {
    match score {
        0..=20 => ConfidenceLevel::VeryLow,
        21..=40 => ConfidenceLevel::Low,
        41..=60 => ConfidenceLevel::Normal,
        61..=80 => ConfidenceLevel::High,
        _ => ConfidenceLevel::VeryHigh,
    }
}

/// One field row in the merge compare grid: the field's label and each side's current value.
///
/// Carries only display data — no per-field "chosen" mutation exists in the core (a `MergePersons`
/// command is one atomic same-as event, not a field-by-field reconciliation). The radios the screen
/// renders over these rows are read-only context for the operator's decision, not a granular-apply
/// mechanism (see [`MergeCompareVm`] doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeFieldRowVm {
    /// The field's already-localized label (e.g. "Name", "Birth").
    pub label: String,
    /// The survivor's current value for this field, or `None` if unrecorded.
    pub survivor_value: Option<String>,
    /// The merged person's current value for this field, or `None` if unrecorded.
    pub merged_value: Option<String>,
}

/// The Compare/merge wizard's view-model (Phase 5 PR 19): the two people's headline info and a
/// field-by-field grid of their current values.
///
/// The per-field rows are informational only. The core's `MergePersons` command has no concept of
/// selecting individual field values from the persona onto the survivor — merging is a single atomic
/// same-as event (data-model §9). The screen's "Merge" action always performs that one atomic call;
/// nothing here drives which fields end up where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCompareVm {
    /// The surviving person (keeps their id and `human_id`).
    pub survivor: PedigreeNodeVm,
    /// The person who would become a persona of the survivor.
    pub merged: PedigreeNodeVm,
    /// The field-by-field comparison rows.
    pub fields: Vec<MergeFieldRowVm>,
}

impl MergeCompareVm {
    /// Builds the view-model from the two persons' summaries, comparing name/birth/death/occupation
    /// (the fields the mockup shows) — only fields the summaries actually carry, no fabricated rows.
    #[must_use]
    pub fn build(
        survivor: &genealogy_app::PersonSummary,
        merged: &genealogy_app::PersonSummary,
        loc: &Localizer,
    ) -> Self {
        let fields = vec![
            MergeFieldRowVm {
                label: loc.merge_field_name(),
                survivor_value: survivor.display_name.clone(),
                merged_value: merged.display_name.clone(),
            },
            fact_row(
                loc.merge_field_birth(),
                survivor,
                merged,
                &genealogy_app::FactType::Birth,
            ),
            fact_row(
                loc.merge_field_death(),
                survivor,
                merged,
                &genealogy_app::FactType::Death,
            ),
            fact_row(
                loc.merge_field_occupation(),
                survivor,
                merged,
                &genealogy_app::FactType::Occupation,
            ),
        ];
        Self {
            survivor: summary_node_ref(survivor),
            merged: summary_node_ref(merged),
            fields,
        }
    }
}

/// Builds a [`MergeFieldRowVm`] comparing both persons' first-asserted fact of `fact_type`.
fn fact_row(
    label: String,
    survivor: &genealogy_app::PersonSummary,
    merged: &genealogy_app::PersonSummary,
    fact_type: &genealogy_app::FactType,
) -> MergeFieldRowVm {
    MergeFieldRowVm {
        label,
        survivor_value: fact_value(survivor, fact_type),
        merged_value: fact_value(merged, fact_type),
    }
}

/// The display value of a person's first-asserted fact of `fact_type`, if any.
fn fact_value(summary: &genealogy_app::PersonSummary, fact_type: &genealogy_app::FactType) -> Option<String> {
    let asserted = summary.facts.iter().find(|f| f.fact.fact_type == *fact_type)?;
    let value = asserted.fact.value.clone();
    let year = year_of_fact(summary, fact_type).map(|year| year.to_string());
    match (value, year) {
        (Some(value), Some(year)) => Some(format!("{value} ({year})")),
        (Some(value), None) => Some(value),
        (None, Some(year)) => Some(year),
        (None, None) => None,
    }
}

/// The representative year of a person's first-asserted fact of `fact_type`, mirroring
/// `genealogy_app`'s internal `year_of_fact` (not exported, so re-derived from the public
/// [`genealogy_app::PersonSummary::facts`] here).
fn year_of_fact(summary: &genealogy_app::PersonSummary, fact_type: &genealogy_app::FactType) -> Option<i32> {
    let date = summary
        .facts
        .iter()
        .find(|f| f.fact.fact_type == *fact_type)?
        .fact
        .date
        .as_ref()?;
    let year = date.sort_value / 10_000;
    (year != 0).then(|| i32::try_from(year).unwrap_or_default())
}

/// Builds a bare [`PedigreeNodeVm`] from a [`PersonSummary`](genealogy_app::PersonSummary) for the
/// merge wizard's header row (name only — the wizard's field grid carries the vitals).
fn summary_node_ref(summary: &genealogy_app::PersonSummary) -> PedigreeNodeVm {
    PedigreeNodeVm {
        human_id: summary.human_id.clone(),
        name: summary.display_name.clone().unwrap_or_else(|| summary.human_id.clone()),
        vitals: None,
        confidence: None,
        confidence_label: None,
        source_count: 0,
        restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
        has_more: false,
    }
}

/// The result of a completed merge (Phase 5 PR 19): the refreshed survivor, the merged person's id,
/// and an accurate — not fabricated — summary of what changed.
///
/// `summary` deliberately never claims relationships were "re-pointed": `PersonsMerged` only records
/// a same-as link on the survivor (data-model §9); Family/Association/Participation records that name
/// the merged person are left exactly as they were. `still_referenced` counts how many such records
/// still name the merged person's id, worded as still-linked, not re-pointed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResultVm {
    /// The survivor's `human_id` (unchanged by the merge).
    pub survivor_human_id: String,
    /// The merged person's `human_id` (their own record is untouched and still resolvable).
    pub merged_human_id: String,
    /// The already-localized outcome summary.
    pub summary: String,
}

impl MergeResultVm {
    /// Builds the view-model from the app's [`MergeResult`](genealogy_app::MergeResult).
    #[must_use]
    pub fn build(result: &genealogy_app::MergeResult, loc: &Localizer) -> Self {
        Self {
            survivor_human_id: result.survivor.human_id.clone(),
            merged_human_id: result.merged_human_id.clone(),
            summary: loc.merge_result_summary(
                &result.merged_human_id,
                &result.survivor.human_id,
                result.still_referenced,
            ),
        }
    }
}

#[cfg(test)]
mod merge_tests {
    use super::{ConfidenceLevel, DuplicateCandidateVm, MergeCompareVm, MergeResultVm};
    use crate::i18n::Localizer;
    use genealogy_app::{AggRef, Confidence, DuplicateCandidate, FactSummary, MatchKind, MergeResult, PersonSummary};
    use genealogy_app::{Fact, FactType};
    use std::collections::BTreeSet;

    fn agg(human_id: &str) -> AggRef {
        AggRef {
            human_id: human_id.to_owned(),
            id: format!("{human_id}-id"),
        }
    }

    fn bare_summary(human_id: &str, display_name: Option<&str>) -> PersonSummary {
        PersonSummary {
            human_id: human_id.to_owned(),
            evidence_level: genealogy_app::EvidenceLevel::Conclusion,
            display_name: display_name.map(ToOwned::to_owned),
            given: None,
            surname: None,
            surname_prefix: None,
            nickname: None,
            name_prefix: None,
            name_suffix: None,
            name_type: None,
            primary_name_assertion: None,
            names: Vec::new(),
            sex: None,
            facts: Vec::new(),
            associations: Vec::new(),
            participations: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            tag_refs: Vec::new(),
            restrictions: BTreeSet::new(),
            merged: Vec::new(),
        }
    }

    #[test]
    fn duplicate_candidate_maps_score_and_localizes_reason() {
        let loc = Localizer::for_test("en");
        let candidate = DuplicateCandidate {
            a: agg("I0042"),
            b: agg("I0099"),
            kind: MatchKind::NameVariant,
            score: 94,
        };
        let vm = DuplicateCandidateVm::build(&candidate, &loc);
        assert_eq!(vm.a.human_id, "I0042");
        assert_eq!(vm.b.human_id, "I0099");
        assert_eq!(vm.confidence, ConfidenceLevel::VeryHigh);
        assert!(!vm.reason.is_empty());
    }

    #[test]
    fn compare_grid_carries_only_real_fields() {
        let loc = Localizer::for_test("en");
        let mut survivor = bare_summary("I0042", Some("John Smith"));
        survivor.facts.push(FactSummary {
            fact: Fact {
                fact_type: FactType::Occupation,
                date: None,
                place_id: None,
                value: Some("Carpenter".to_owned()),
                citations: Vec::new(),
            },
            confidence: Confidence::Normal,
            citations: Vec::new(),
        });
        let merged = bare_summary("I0099", Some("John Smyth"));

        let vm = MergeCompareVm::build(&survivor, &merged, &loc);
        assert_eq!(vm.survivor.human_id, "I0042");
        assert_eq!(vm.merged.human_id, "I0099");
        let occupation = vm
            .fields
            .iter()
            .find(|row| row.survivor_value.as_deref() == Some("Carpenter"))
            .expect("occupation row present");
        assert_eq!(occupation.merged_value, None, "merged has no occupation recorded");
    }

    #[test]
    fn merge_result_summary_never_claims_repointing() {
        let loc = Localizer::for_test("en");
        let result = MergeResult {
            survivor: bare_summary("I0042", Some("John Smith")),
            merged_human_id: "I0099".to_owned(),
            still_referenced: 3,
        };
        let vm = MergeResultVm::build(&result, &loc);
        assert!(
            !vm.summary.to_lowercase().contains("re-point") && !vm.summary.to_lowercase().contains("repoint"),
            "must not claim re-pointing: {}",
            vm.summary
        );
        assert!(vm.summary.contains("I0099"));
        assert!(vm.summary.contains("I0042"));
    }
}

#[cfg(test)]
mod pedigree_tests {
    use super::{PedigreeSlotVm, PedigreeVm, RelationshipVm};
    use crate::i18n::Localizer;
    use genealogy_app::{
        AncestorNode, AncestorSlot, Confidence, DescendantChart, DescendantNode, Kinship, PedigreeChart,
        PedigreePersonRef, RelationshipResult,
    };
    use std::collections::BTreeSet;

    fn person(human_id: &str, name: &str) -> PedigreePersonRef {
        PedigreePersonRef {
            human_id: human_id.to_owned(),
            id: format!("{human_id}-id"),
            name: Some(name.to_owned()),
            vitals: Some("1850 – 1920".to_owned()),
            restrictions: BTreeSet::new(),
        }
    }

    fn no_descendants(focus: PedigreePersonRef) -> DescendantChart {
        DescendantChart {
            focus,
            children: Vec::new(),
        }
    }

    #[test]
    fn ancestor_generations_pad_unknown_slots_with_localized_hints() {
        let loc = Localizer::for_test("en");
        let focus = person("I0001", "John Smith");
        let father = AncestorNode {
            person: person("I0002", "Thomas Smith"),
            confidence: Confidence::Normal,
            source_count: 1,
            father: AncestorSlot::Unknown,
            mother: AncestorSlot::Unknown,
        };
        let chart = PedigreeChart {
            focus: focus.clone(),
            father: AncestorSlot::Known(Box::new(father)),
            mother: AncestorSlot::Unknown,
        };
        let vm = PedigreeVm::build(&chart, &no_descendants(focus), 3, &loc);

        assert_eq!(vm.focus.name, "John Smith");
        assert!(vm.focus.confidence.is_none(), "the focus is not itself an assertion");
        assert_eq!(vm.ancestor_generations.len(), 3);
        let PedigreeSlotVm::Known(dad) = &vm.ancestor_generations[0][0] else {
            panic!("father known")
        };
        assert_eq!(dad.name, "Thomas Smith");
        let PedigreeSlotVm::Unknown { hint } = &vm.ancestor_generations[0][1] else {
            panic!("mother unknown")
        };
        assert_eq!(hint, "mother of John Smith");
        // Gen 2 (grandparents): Thomas Smith's own two slots, both unresearched (named) since he has
        // no recorded parents.
        let PedigreeSlotVm::Unknown { hint: paternal_gf } = &vm.ancestor_generations[1][0] else {
            panic!("paternal grandfather unknown")
        };
        assert_eq!(paternal_gf, "father of Thomas Smith");
        // Gen 3: below an unresearched slot, the hint drops the name (a generic "unresearched" form).
        let PedigreeSlotVm::Unknown { hint: generic } = &vm.ancestor_generations[2][0] else {
            panic!("gen 3 slot unknown")
        };
        assert_eq!(generic, "father (line unresearched)");
        assert_eq!(vm.ancestor_generations[0].len(), 2, "gen 1 has 2 slots");
        assert_eq!(vm.ancestor_generations[1].len(), 4, "gen 2 has 4 slots");
        assert_eq!(
            vm.ancestor_generations[2].len(),
            8,
            "gen 3 has 8 slots — the fan stays rectangular"
        );
    }

    #[test]
    fn descendant_generations_are_not_padded() {
        let loc = Localizer::for_test("en");
        let focus = person("I0001", "Grand Parent");
        let child = DescendantNode {
            person: person("I0002", "Mid Parent"),
            confidence: Confidence::Normal,
            source_count: 0,
            children: Vec::new(),
        };
        let tree = DescendantChart {
            focus: focus.clone(),
            children: vec![child],
        };
        let chart = PedigreeChart {
            focus,
            father: AncestorSlot::Unknown,
            mother: AncestorSlot::Unknown,
        };
        let vm = PedigreeVm::build(&chart, &tree, 4, &loc);

        assert_eq!(vm.descendant_generations.len(), 1, "no grandchildren recorded");
        assert_eq!(vm.descendant_generations[0].len(), 1);
        assert_eq!(vm.descendant_generations[0][0].name, "Mid Parent");
        assert_eq!(vm.descendant_generations[0][0].source_count, 0);
    }

    #[test]
    fn relationship_vm_localizes_the_kinship_summary() {
        let loc = Localizer::for_test("en");
        let result = RelationshipResult {
            person_a: person("I0001", "Alice"),
            person_b: person("I0002", "Bob"),
            kinship: Some(Kinship::Sibling { full: true }),
        };
        let vm = RelationshipVm::build(&result, &loc);
        assert_eq!(vm.summary, "Alice and Bob are full siblings.");
    }

    #[test]
    fn relationship_vm_reports_when_no_kinship_is_found() {
        let loc = Localizer::for_test("en");
        let result = RelationshipResult {
            person_a: person("I0001", "Alice"),
            person_b: person("I0002", "Zoe"),
            kinship: None,
        };
        let vm = RelationshipVm::build(&result, &loc);
        assert_eq!(
            vm.summary,
            "No known relationship found within the searched generations."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CitationDetail, DEFAULT_TAG_COLOR, DEFAULT_TAG_PRIORITY, DashboardVm, PersonDetail, ProvenanceDraft, TagDraft,
        citation_row, citation_tabs, evidence_axes, person_row, person_tabs,
    };
    use crate::i18n::Localizer;
    use crate::presentation::ConfidenceLevel;
    use crate::presentation::EvidenceAxis;
    use genealogy_app::{
        ActivityDetail, AssociationRole, AssociationSummary, Calendar, ChangeLogEntry, CitationSummary, Confidence,
        DateModifier, DatePoint, DateQuality, EvidenceAnalysis, EvidenceKind, EvidenceLevel, Fact, FactSummary,
        FactType, GenealogicalDate, GenealogicalDateBody, InformationKind, NameSummary, NameType, OperatorKind,
        PersonName, PersonSummary, Restriction, Sex, SourceQuality, Surname, TagRef, WorkspaceCounts,
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
            citations: Vec::new(),
            evidence_analysis: None,
            detail: None,
            can_undo: false,
        }
    }

    #[test]
    fn dashboard_renders_a_collapsed_import_and_labels_records_by_name() {
        let loc = Localizer::for_test("en");
        // `summary()` is the person I0001 / "Ada Lovelace".
        let person = summary();
        // The app pre-collapses an import burst into one ImportBatch row; then a human edit on a person.
        let mut import = log_entry("", None, OperatorKind::Software, "gedcom-import");
        import.event_type = "ImportBatch".to_owned();
        import.detail = Some(ActivityDetail::ImportBatch { count: 3 });
        let activity = vec![import, log_entry("person", Some("I0001"), OperatorKind::Human, "magne")];
        let vm = DashboardVm::build(WorkspaceCounts::default(), &[person], &activity, &loc, 4);

        assert_eq!(vm.recent.len(), 2);
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
    fn dashboard_summary_names_the_fact_kind() {
        let loc = Localizer::for_test("en");
        let mut entry = log_entry("person", Some("I0001"), OperatorKind::Human, "magne");
        entry.event_type = "FactAsserted".to_owned();
        entry.detail = Some(ActivityDetail::Fact {
            fact_type: FactType::Birth,
        });
        let vm = DashboardVm::build(WorkspaceCounts::default(), &[summary()], &[entry], &loc, 4);
        assert_eq!(vm.recent[0].what, "Birth asserted", "a fact assertion names its kind");
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
            citations: Vec::new(),
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
            citations: Vec::new(),
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
            primary_name_assertion: None,
            names: vec![NameSummary {
                name: birth_name(),
                confidence: Confidence::High,
                source_count: 1,
            }],
            sex: Some(Sex::Female),
            facts: vec![occupation_fact()],
            associations: vec![AssociationSummary {
                other: genealogy_app::AggRef {
                    human_id: "I0002".to_owned(),
                    id: "11111111-1111-7111-8111-111111111111".to_owned(),
                },
                role: AssociationRole::Godparent,
                confidence: Confidence::Normal,
                source_count: 0,
            }],
            participations: Vec::new(),
            citations: vec![genealogy_app::CitationRef {
                human_id: "C0001".to_owned(),
                id: "22222222-2222-7222-8222-222222222222".to_owned(),
                source: None,
                source_title: None,
                page: None,
                confidence: None,
                analysis: None,
                asserted_by: None,
                asserted_at: None,
            }],
            media: Vec::new(),
            notes: vec![
                genealogy_app::AggRef {
                    human_id: "N0001".to_owned(),
                    id: "33333333-3333-7333-8333-333333333333".to_owned(),
                },
                genealogy_app::AggRef {
                    human_id: "N0002".to_owned(),
                    id: "44444444-4444-7444-8444-444444444444".to_owned(),
                },
            ],
            tags: Vec::new(),
            tag_refs: Vec::new(),
            restrictions: BTreeSet::new(),
            merged: Vec::new(),
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
    fn empty_draft_builds_a_create_request_with_no_name() {
        use super::PersonDraft;
        let request = PersonDraft::new().to_request();
        assert_eq!(request.existing_human_id, None, "an empty draft creates a new person");
        assert!(request.name.is_none(), "a blank name asserts nothing");
        assert_eq!(request.sex, Some(Sex::Unknown));
        assert!(request.new_sources.is_empty() && request.new_citations.is_empty());
    }

    #[test]
    fn edit_draft_seeds_from_the_summary_and_targets_the_existing_person() {
        use super::PersonDraft;
        let draft = PersonDraft::from_summary(&summary());
        assert_eq!(draft.existing_human_id.as_deref(), Some("I0001"));
        assert_eq!(draft.given, "Ada");
        assert_eq!(draft.surname, "Lovelace");
        assert_eq!(draft.sex, Sex::Female);
        let request = draft.to_request();
        assert_eq!(
            request.existing_human_id.as_deref(),
            Some("I0001"),
            "an edit targets the person"
        );
        let name = request.name.expect("a seeded name");
        assert_eq!(name.given.as_deref(), Some("Ada"));
        assert_eq!(name.surname.as_deref(), Some("Lovelace"));
    }

    #[test]
    fn a_pending_citation_with_a_new_source_emits_both_referenced_by_the_name() {
        use super::{DraftCitation, DraftNameCitation, PersonDraft};
        let mut draft = PersonDraft::new();
        draft.given = "John".to_owned();
        draft.surname = "Smith".to_owned();
        draft.name_citation = DraftNameCitation::New;
        draft.pending_citation = Some(DraftCitation {
            placeholder: PersonDraft::PENDING_KEY.to_owned(),
            source_human_id: String::new(),
            new_source_title: "Baptism register".to_owned(),
            page: "p. 14".to_owned(),
        });
        let request = draft.to_request();
        assert_eq!(request.new_sources.len(), 1, "a pending source is created once");
        assert_eq!(request.new_citations.len(), 1, "a pending citation is created once");
        // The name cites the pending citation by its placeholder; the citation cites the pending source.
        let name_ref = request.name_citation.expect("the name cites the pending citation");
        assert_eq!(
            name_ref,
            crate::navigation::DraftCitationRef::Pending(PersonDraft::PENDING_KEY.to_owned())
        );
        let source_placeholder = format!("{}-source", PersonDraft::PENDING_KEY);
        assert_eq!(request.new_sources[0].placeholder, source_placeholder);
        assert_eq!(
            request.new_citations[0].source,
            crate::navigation::DraftSourceRef::Pending(source_placeholder)
        );
    }

    #[test]
    fn a_pending_citation_against_an_existing_source_emits_no_new_source() {
        use super::{DraftCitation, DraftNameCitation, PersonDraft};
        let mut draft = PersonDraft::new();
        draft.given = "Mary".to_owned();
        draft.name_citation = DraftNameCitation::New;
        draft.pending_citation = Some(DraftCitation {
            placeholder: PersonDraft::PENDING_KEY.to_owned(),
            source_human_id: "S0001".to_owned(),
            new_source_title: String::new(),
            page: String::new(),
        });
        let request = draft.to_request();
        assert!(request.new_sources.is_empty(), "an existing source is not re-created");
        assert_eq!(request.new_citations.len(), 1);
        assert_eq!(
            request.new_citations[0].source,
            crate::navigation::DraftSourceRef::Existing("S0001".to_owned())
        );
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
            primary_name_assertion: None,
            names: Vec::new(),
            sex: None,
            facts: Vec::new(),
            associations: Vec::new(),
            participations: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            tag_refs: Vec::new(),
            restrictions: BTreeSet::from([Restriction::Privacy]),
            merged: Vec::new(),
        };
        let row = person_row(&summary, &loc);
        assert_eq!(row.title, "(no name)");
        assert_eq!(row.subtitle.as_deref(), Some("-"));
        assert_eq!(row.avatar.as_deref(), Some("?"));
    }

    fn citation_summary() -> CitationSummary {
        CitationSummary {
            human_id: "C0001".to_owned(),
            source: Some(genealogy_app::AggRef {
                human_id: "S0001".to_owned(),
                id: "55555555-5555-7555-8555-555555555555".to_owned(),
            }),
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

    #[test]
    fn a_fresh_tag_draft_seeds_the_create_defaults() {
        let draft = TagDraft::new();
        assert!(draft.existing_id.is_none());
        assert!(draft.name.is_empty());
        assert_eq!(draft.priority, DEFAULT_TAG_PRIORITY.to_string());
        assert_eq!(draft.color, DEFAULT_TAG_COLOR);
        // A blank name means the draft is not committable yet (Save disabled).
        assert!(!draft.is_valid());
        assert!(draft.to_request().is_none());
    }

    #[test]
    fn a_tag_draft_is_valid_only_when_all_three_fields_are_present() {
        let mut draft = TagDraft::new();
        draft.name = "Direct ancestor".to_owned();
        assert!(draft.is_valid(), "name + default priority + default colour is valid");

        draft.priority = String::new();
        assert!(!draft.is_valid(), "an empty priority is invalid");
        draft.priority = "notanumber".to_owned();
        assert!(!draft.is_valid(), "a non-numeric priority is invalid");
        draft.priority = "2".to_owned();

        draft.color = "   ".to_owned();
        assert!(!draft.is_valid(), "an empty colour is invalid");
    }

    #[test]
    fn a_valid_tag_draft_builds_a_trimmed_request() {
        let mut draft = TagDraft::new();
        draft.existing_id = Some("tag-uuid".to_owned());
        draft.name = "  Needs sources  ".to_owned();
        draft.priority = " 3 ".to_owned();
        draft.color = " #e0884a ".to_owned();
        let request = draft.to_request().expect("valid draft commits");
        assert_eq!(request.existing_id.as_deref(), Some("tag-uuid"));
        assert_eq!(request.name, "Needs sources");
        assert_eq!(request.priority, 3);
        assert_eq!(request.color, "#e0884a");
    }

    #[test]
    fn default_provenance_draft_maps_to_default_meta() {
        let draft = ProvenanceDraft::default();
        let provenance = draft.provenance();
        assert_eq!(
            provenance.confidence,
            Confidence::Normal,
            "default confidence is Normal"
        );
        assert!(provenance.rationale.is_none(), "no rationale by default");
        assert!(
            provenance.evidence_analysis.is_none(),
            "no evidence analysis by default"
        );
        let meta = draft.meta();
        assert!(meta.citations.is_empty(), "no citations by default");
        assert!(meta.supersedes.is_none(), "a draft never supersedes (PR27)");
    }

    #[test]
    fn filled_draft_maps_rationale_confidence_and_citations() {
        let mut draft = ProvenanceDraft {
            rationale: "   ".to_owned(),
            ..ProvenanceDraft::default()
        };
        assert!(
            draft.provenance().rationale.is_none(),
            "a blank/whitespace rationale drops to None"
        );
        draft.rationale = "  Baptism register gives the date  ".to_owned();
        assert_eq!(
            draft.provenance().rationale.as_deref(),
            Some("Baptism register gives the date"),
            "a real rationale is trimmed"
        );
        draft.confidence = ConfidenceLevel::High;
        assert_eq!(
            draft.provenance().confidence,
            Confidence::High,
            "confidence maps through"
        );
        draft.citations = vec!["C0001".to_owned(), "C0002".to_owned()];
        let meta = draft.meta();
        assert_eq!(
            meta.citations,
            &["C0001".to_owned(), "C0002".to_owned()],
            "citations pass through by borrow"
        );
    }

    #[test]
    fn evidence_analysis_requires_all_three_axes() {
        let mut draft = ProvenanceDraft {
            source: Some(SourceQuality::Original),
            information: Some(InformationKind::Primary),
            ..ProvenanceDraft::default()
        };
        assert!(
            draft.provenance().evidence_analysis.is_none(),
            "a missing evidence axis yields no analysis"
        );
        draft.evidence = Some(EvidenceKind::Direct);
        let analysis = draft
            .provenance()
            .evidence_analysis
            .expect("all three axes chosen yields an analysis");
        assert_eq!(
            analysis,
            EvidenceAnalysis {
                source: SourceQuality::Original,
                information: InformationKind::Primary,
                evidence: EvidenceKind::Direct,
            }
        );
    }
}
