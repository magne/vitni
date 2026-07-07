use super::{
    ChildParentRelationship, CitationRefVm, ConfidenceLevel, DetailTab, EventType, FamilyChangeSetRequest, FamilyEdit,
    FamilyForPerson, FamilySummary, HistoryEntryVm, Localizer, PartnerRequest, PersonFamilyRole, RecordDraft,
    RestrictionKind, RowVm, TagRef, citation_ref_from_ref, non_blank,
};

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

/// The buffered whole-record draft of a family (create + edit, one mechanism, `record-editing.html`
/// §2/§6). A family's only scalar is its user-facing id (everything else is collections — §8): create
/// buffers the partner `human_id`s (0..=2), edit buffers the editable id. `existing_human_id` is `None`
/// in create mode and `Some` in edit mode. Nothing is written until Save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FamilyDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit mode only — create auto-allocates).
    pub human_id: String,
    /// The partner person `human_id`s the operator has added (capped at two; create-only).
    pub partners: Vec<String>,
}

impl FamilyDraft {
    /// A fresh empty draft for creating a new family.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing family for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (renames) rather than creates; partners stay empty (on an existing
    /// family they are the collection edited per-row, not this scalar draft).
    #[must_use]
    pub fn from_detail(detail: &FamilyDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            partners: Vec::new(),
        }
    }

    /// Adds a partner by person `human_id`, ignoring a blank or duplicate and capping at two.
    pub fn add_partner(&mut self, human_id: &str) {
        let human_id = human_id.trim();
        if human_id.is_empty() || self.partners.len() >= 2 || self.partners.iter().any(|p| p == human_id) {
            return;
        }
        self.partners.push(human_id.to_owned());
    }

    /// Removes a partner by person `human_id`.
    pub fn remove_partner(&mut self, human_id: &str) {
        self.partners.retain(|p| p != human_id);
    }

    /// Builds the [`FamilyChangeSetRequest`] the app commits on Save (create mode). Every buffered
    /// partner is an existing person (the inline "+ New person" path lands with the picker in a later
    /// slice); the editable id is carried through, blank ⇒ auto-allocate.
    #[must_use]
    pub fn to_request(&self) -> FamilyChangeSetRequest {
        FamilyChangeSetRequest {
            human_id: non_blank(&self.human_id),
            partners: self
                .partners
                .iter()
                .map(|human_id| PartnerRequest::Existing(human_id.clone()))
                .collect(),
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): a family's only editable scalar is its id, so at most one [`FamilyEdit::SetHumanId`]
    /// (a blank id regenerates on save).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<FamilyEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        if self.human_id.trim() == seed.human_id {
            return Vec::new();
        }
        vec![FamilyEdit::SetHumanId {
            human_id,
            new_human_id: non_blank(&self.human_id),
        }]
    }
}

impl RecordDraft for FamilyDraft {
    type Detail = FamilyDetail;

    fn from_detail(detail: &FamilyDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod family_draft_tests {
    use super::{FamilyDraft, RecordDraft};
    use crate::navigation::FamilyEdit;

    #[test]
    fn a_fresh_draft_is_not_dirty() {
        assert!(!FamilyDraft::new().is_dirty_against(&FamilyDraft::new()));
    }

    #[test]
    fn add_partner_caps_at_two_and_ignores_duplicates_and_blanks() {
        let mut draft = FamilyDraft::new();
        draft.add_partner("I0001");
        draft.add_partner("  I0001  ");
        draft.add_partner("");
        draft.add_partner("I0002");
        draft.add_partner("I0003");
        assert_eq!(draft.partners, vec!["I0001".to_owned(), "I0002".to_owned()]);
        assert!(draft.is_dirty_against(&FamilyDraft::new()));
    }

    #[test]
    fn remove_partner_drops_the_id() {
        let mut draft = FamilyDraft::new();
        draft.add_partner("I0001");
        draft.remove_partner("I0001");
        assert!(draft.partners.is_empty());
    }

    fn seed() -> FamilyDraft {
        FamilyDraft {
            existing_human_id: Some("F0001".to_owned()),
            human_id: "F0001".to_owned(),
            partners: Vec::new(),
        }
    }

    #[test]
    fn an_unchanged_family_yields_no_edits() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn a_changed_id_yields_one_set_human_id() {
        let draft = FamilyDraft {
            human_id: "F0042".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(
            matches!(&edits[0], FamilyEdit::SetHumanId { new_human_id, .. } if new_human_id.as_deref() == Some("F0042"))
        );
    }

    #[test]
    fn a_blank_id_regenerates() {
        let draft = FamilyDraft {
            human_id: String::new(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], FamilyEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }
}
