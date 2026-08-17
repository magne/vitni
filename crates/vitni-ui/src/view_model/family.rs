use super::{
    ActionLabel, AttachedRefVm, ChildParentRelationship, CitationRefVm, ConfidenceLevel, DetailTab, EventType,
    FamilyChangeSetRequest, FamilyEdit, FamilyForPerson, FamilyRow, FamilySummary, GenealogicalDate, HistoryEntryVm,
    Localizer, MediaRefVm, NewPersonFields, PartnerRequest, PersonFamilyRole, RecordDraft, RestrictionKind, RowVm,
    TagRef, citation_ref_from_ref, line_label, non_blank,
};
use crate::picker::PickerSelection;

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
    /// The `AssertionId` (a UUID string) that introduced this partner — the Remove retract target
    /// (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// One child-to-partner relationship on a child row (GEDCOM `_FREL`/`_MREL`, ADR 0021): its own
/// assertion, so the child edit form supersedes or clears it per link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRelationshipVm {
    /// The family partner the relationship is to, by `human_id`.
    pub partner_human_id: String,
    /// The localized relationship label (e.g. `Birth`), for the children-table cell.
    pub label: String,
    /// The raw relationship kind, for the Edit prefill (the localized [`label`](Self::label) can't be
    /// reversed to a kind).
    pub kind: ChildParentRelationship,
    /// The `AssertionId` (a UUID string) that introduced this link — the per-link Edit supersede /
    /// clear retract target (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
    /// How many citations back this link's assertion.
    pub source_count: usize,
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
    /// The relationship to each family partner, one per-partner assertion (ADR 0021).
    pub relationships: Vec<ChildRelationshipVm>,
    /// The operator's surety in the child's membership assertion (drives the confidence badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: String,
    /// How many citations back the child's membership assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) of the child's membership assertion — the Retract target
    /// (the row's *correct a mistake* action), cascading its relationships (ADR 0004 §2, ADR 0021).
    /// The row's Remove action ends the membership by `human_id` instead. Never rendered.
    pub assertion_id: String,
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
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label.
    pub confidence_label: String,
    /// How many citations back the event.
    pub source_count: usize,
    /// The event's citations, for the provenance popover.
    pub citations: Vec<CitationRefVm>,
    /// The `AssertionId` (a UUID string) that introduced this family-event link — the Unlink retract
    /// target (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    /// The citations backing the family's claims (Citations tab), each with its Detach target.
    pub citations: Vec<CitationRefVm>,
    /// The attached media objects.
    pub media: Vec<MediaRefVm>,
    /// The attached notes, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The family's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The research notes arguing about this record (Research notes tab, ADR 0028 §5) — the reverse
    /// index over the `ResearchNote` projection; filled by the dispatcher.
    pub research_notes: Vec<RowVm>,
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
                assertion_id: partner.assertion_id.clone(),
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
        let media = summary.media.iter().map(MediaRefVm::from_ref).collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: family_title(summary),
            partners,
            marriage,
            children,
            events,
            citations: summary
                .citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            media,
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            research_notes: Vec::new(),
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

/// Builds a [`FamilyChildVm`] from an app `ChildRef`, localizing each per-partner link + confidence.
fn family_child_vm(child: &vitni_app::ChildRef, loc: &Localizer) -> FamilyChildVm {
    let confidence = child.confidence.map(ConfidenceLevel::from);
    FamilyChildVm {
        human_id: child.human_id.clone(),
        name: child.name.clone().unwrap_or_else(|| child.human_id.clone()),
        born: child.born.clone(),
        relationships: child
            .relationships
            .iter()
            .map(|link| ChildRelationshipVm {
                partner_human_id: link.partner_human_id.clone(),
                label: loc.relationship_label(&link.relationship),
                kind: link.relationship.clone(),
                assertion_id: link.assertion_id.clone(),
                source_count: link.source_count,
            })
            .collect(),
        confidence,
        confidence_label: loc.confidence_label_opt(confidence),
        source_count: child.source_count,
        assertion_id: child.assertion_id.clone(),
    }
}

/// Builds a [`FamilyEventVm`] from an app `FamilyEventRef`, localizing the type, date, and confidence.
fn family_event_vm(event: &vitni_app::FamilyEventRef, loc: &Localizer) -> FamilyEventVm {
    let confidence = event.confidence.map(ConfidenceLevel::from);
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
        confidence_label: loc.confidence_label_opt(confidence),
        source_count: event.source_count,
        citations: event.citations.iter().map(|c| citation_ref_from_ref(c, loc)).collect(),
        assertion_id: event.assertion_id.clone(),
    }
}

/// Builds a generic list row from a [`FamilySummary`]: the partners' names, a marriage/children
/// subtitle, and a couple avatar.
#[must_use]
pub fn family_row(summary: &FamilySummary, loc: &Localizer) -> RowVm {
    let labels: Vec<String> = summary
        .partners
        .iter()
        .map(|partner| partner.name.clone().unwrap_or_else(|| partner.human_id.clone()))
        .collect();
    let marriage_date = summary
        .events
        .iter()
        .find(|event| event.event_type == Some(EventType::Marriage))
        .and_then(|event| event.date.clone());
    family_row_fields(
        &summary.human_id,
        &labels,
        marriage_date.as_ref(),
        summary.children.len(),
        loc,
    )
}

/// Builds a generic list row from a lightweight [`FamilyRow`] (the list view's per-row DTO), the same
/// rendering as [`family_row`] without loading a full summary.
#[must_use]
pub fn family_list_row(row: &FamilyRow, loc: &Localizer) -> RowVm {
    let labels: Vec<String> = row
        .partners
        .iter()
        .map(|partner| partner.name.clone().unwrap_or_else(|| partner.human_id.clone()))
        .collect();
    family_row_fields(&row.human_id, &labels, row.marriage_date.as_ref(), row.child_count, loc)
}

/// The shared [`RowVm`] builder behind [`family_row`] and [`family_list_row`]: the partners joined
/// with ` & ` as the title (or the `human_id` when partnerless), a `marriage-year · children`
/// subtitle, and the couple avatar. `partner_labels` are already resolved to name-or-`human_id`.
fn family_row_fields(
    human_id: &str,
    partner_labels: &[String],
    marriage_date: Option<&GenealogicalDate>,
    child_count: usize,
    loc: &Localizer,
) -> RowVm {
    let title = if partner_labels.is_empty() {
        human_id.to_owned()
    } else {
        partner_labels.join(" & ")
    };
    let marriage_year = marriage_date.map(|date| loc.date(date));
    let children = loc.family_children_count(child_count);
    let subtitle = match marriage_year {
        Some(year) => Some(format!("{year} · {children}")),
        None => Some(children),
    };
    RowVm {
        id: human_id.to_owned(),
        title,
        subtitle,
        avatar: Some("👪".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a family's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn family_tabs(detail: &FamilyDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>, action: Option<ActionLabel>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
        action,
    };
    vec![
        tab("overview", None, None),
        tab("children", Some(detail.children.len()), Some(ActionLabel::AddChild)),
        tab("events", Some(detail.events.len()), Some(ActionLabel::LinkEvent)),
        tab(
            "citations",
            Some(detail.citations.len()),
            Some(ActionLabel::AttachCitation),
        ),
        tab("media", Some(detail.media.len()), Some(ActionLabel::AttachMedia)),
        tab("notes", Some(detail.notes.len()), Some(ActionLabel::AttachNote)),
        tab(
            "research-notes",
            Some(detail.research_notes.len()),
            Some(ActionLabel::NewResearchNote),
        ),
        tab("tags", Some(detail.tags.len()), Some(ActionLabel::AddTag)),
        tab("history", None, None),
    ]
}

/// A partner buffered on a family create draft: an existing person (picked via the record picker) or
/// one created inline from the picker's "+ New person" (`family.html`). The view-model twin of the
/// app's `PartnerInput` — an existing partner keeps its [`PickerSelection`] (id + display title) so a
/// chip can show the name, and a new partner keeps its [`NewPersonFields`]. Maps to navigation's
/// [`PartnerRequest`] on [`FamilyDraft::to_request`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartnerInput {
    /// An existing person, by their picker selection (its `human_id` + display title).
    Existing(PickerSelection),
    /// A person created inline, by their name parts.
    New(NewPersonFields),
}

/// The buffered whole-record draft of a family (create + edit, one mechanism, `record-editing.html`
/// §2/§6). A family's only scalar is its user-facing id (everything else is collections — §8): create
/// buffers the partners (0..=2, existing or inline-new), edit buffers the editable id.
/// `existing_human_id` is `None` in create mode and `Some` in edit mode. Nothing is written until Save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FamilyDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit mode only — create auto-allocates).
    pub human_id: String,
    /// The partners the operator has added (capped at two; create-only), existing or inline-new.
    pub partners: Vec<PartnerInput>,
    /// The family's privacy restrictions (GEDCOM `RESN`); empty is unrestricted. Edit-only — the
    /// change-set request carries none, so a create form does not offer the field.
    pub restrictions: Vec<RestrictionKind>,
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
            restrictions: detail.restrictions.clone(),
        }
    }

    /// Adds an existing partner by picker selection, ignoring a duplicate (by `human_id`) and capping
    /// at two.
    pub fn add_partner(&mut self, selection: PickerSelection) {
        if self.partners.len() >= 2 || self.is_partner(&selection.human_id) {
            return;
        }
        self.partners.push(PartnerInput::Existing(selection));
    }

    /// Adds a new partner from its inline name parts, capping at two and rejecting an all-blank name
    /// (a partner with neither a given name nor a surname is not created).
    pub fn add_new_partner(&mut self, fields: NewPersonFields) {
        if self.partners.len() >= 2 || (fields.given.trim().is_empty() && fields.surname.trim().is_empty()) {
            return;
        }
        self.partners.push(PartnerInput::New(fields));
    }

    /// Removes the partner at `index`, ignoring an out-of-range index.
    pub fn remove_partner(&mut self, index: usize) {
        if index < self.partners.len() {
            self.partners.remove(index);
        }
    }

    /// Whether an existing partner with `human_id` is already added.
    fn is_partner(&self, human_id: &str) -> bool {
        self.partners.iter().any(|partner| match partner {
            PartnerInput::Existing(selection) => selection.human_id == human_id,
            PartnerInput::New(_) => false,
        })
    }

    /// Builds the [`FamilyChangeSetRequest`] the app commits on Save (create mode): each partner maps
    /// to its [`PartnerRequest`] (existing by `human_id`, or new by non-blank name parts); the editable
    /// id is carried through, blank ⇒ auto-allocate.
    #[must_use]
    pub fn to_request(&self) -> FamilyChangeSetRequest {
        FamilyChangeSetRequest {
            human_id: non_blank(&self.human_id),
            partners: self.partners.iter().map(partner_request).collect(),
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): a family's only editable scalars are its restriction set and its id, with `SetHumanId`
    /// emitted last so the restrictions commit against the id the record still has (a blank id
    /// regenerates on save).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<FamilyEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.restrictions != seed.restrictions {
            edits.push(FamilyEdit::SetRestrictions {
                human_id: human_id.clone(),
                restrictions: self.restrictions.clone(),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(FamilyEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }
}

/// Maps a buffered [`PartnerInput`] to the navigation [`PartnerRequest`] the app commits: an existing
/// partner by its `human_id`, a new one by its non-blank name parts.
fn partner_request(partner: &PartnerInput) -> PartnerRequest {
    match partner {
        PartnerInput::Existing(selection) => PartnerRequest::Existing(selection.human_id.clone()),
        PartnerInput::New(fields) => PartnerRequest::New {
            given: non_blank(&fields.given),
            surname: non_blank(&fields.surname),
        },
    }
}

impl RecordDraft for FamilyDraft {
    type Detail = FamilyDetail;

    fn from_detail(detail: &FamilyDetail) -> Self {
        Self::from_detail(detail)
    }

    /// A create draft needs at least one partner (`family.html`); an edit draft (seeded with
    /// `existing_human_id`) stays valid with no partners, since partners are edited per-row there.
    fn is_valid(&self) -> bool {
        self.existing_human_id.is_some() || !self.partners.is_empty()
    }

    fn display_label(&self) -> Option<String> {
        let mut names = Vec::new();
        for partner in &self.partners {
            let name = match partner {
                PartnerInput::Existing(selection) => selection.title.trim().to_owned(),
                PartnerInput::New(fields) => [fields.given.trim(), fields.surname.trim()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            };
            if !name.is_empty() {
                names.push(name);
            }
        }
        line_label(&names.join(" & "))
    }

    fn editable_restrictions(&self) -> Option<&[RestrictionKind]> {
        self.existing_human_id.is_some().then_some(self.restrictions.as_slice())
    }

    fn set_restrictions(&mut self, restrictions: Vec<RestrictionKind>) {
        self.restrictions = restrictions;
    }
}

#[cfg(test)]
mod family_draft_tests {
    use super::{FamilyDetail, FamilyDraft, NewPersonFields, PartnerInput, RecordDraft};
    use crate::navigation::{FamilyEdit, PartnerRequest};
    use crate::picker::PickerSelection;
    use crate::presentation::RestrictionKind;

    fn existing(human_id: &str) -> PickerSelection {
        PickerSelection {
            human_id: human_id.to_owned(),
            title: format!("{human_id} name"),
        }
    }

    fn new_person(given: &str, surname: &str) -> NewPersonFields {
        NewPersonFields {
            given: given.to_owned(),
            surname: surname.to_owned(),
        }
    }

    #[test]
    fn a_fresh_draft_is_not_dirty() {
        assert!(!FamilyDraft::new().is_dirty_against(&FamilyDraft::new()));
    }

    #[test]
    fn a_create_draft_with_no_partner_is_invalid_and_one_partner_makes_it_valid() {
        let mut draft = FamilyDraft::new();
        assert!(!draft.is_valid(), "create needs at least one partner");
        draft.add_partner(existing("I0001"));
        assert!(draft.is_valid(), "one partner satisfies the create rule");
    }

    #[test]
    fn an_edit_draft_with_no_partner_stays_valid() {
        let draft = FamilyDraft {
            existing_human_id: Some("F0001".to_owned()),
            human_id: "F0001".to_owned(),
            ..FamilyDraft::new()
        };
        assert!(draft.is_valid(), "an existing family edits partners per-row, not here");
    }

    #[test]
    fn add_partner_caps_at_two_and_ignores_duplicate_ids() {
        let mut draft = FamilyDraft::new();
        draft.add_partner(existing("I0001"));
        draft.add_partner(existing("I0001"));
        draft.add_partner(existing("I0002"));
        draft.add_partner(existing("I0003"));
        assert_eq!(draft.partners.len(), 2, "duplicates ignored, capped at two");
        assert!(draft.is_dirty_against(&FamilyDraft::new()));
    }

    #[test]
    fn add_new_partner_rejects_an_all_blank_name_and_caps_at_two() {
        let mut draft = FamilyDraft::new();
        draft.add_new_partner(new_person("  ", ""));
        assert!(draft.partners.is_empty(), "an all-blank new partner is not added");
        draft.add_new_partner(new_person("Ada", ""));
        draft.add_new_partner(new_person("", "Hopper"));
        draft.add_new_partner(new_person("Grace", "Hopper"));
        assert_eq!(draft.partners.len(), 2, "named new partners are added, capped at two");
    }

    #[test]
    fn remove_partner_drops_the_entry_at_the_index() {
        let mut draft = FamilyDraft::new();
        draft.add_partner(existing("I0001"));
        draft.add_new_partner(new_person("Grace", "Hopper"));
        draft.remove_partner(0);
        assert_eq!(draft.partners.len(), 1);
        assert!(matches!(&draft.partners[0], PartnerInput::New(fields) if fields.given == "Grace"));
        draft.remove_partner(5);
        assert_eq!(draft.partners.len(), 1, "an out-of-range index is ignored");
    }

    #[test]
    fn to_request_maps_both_partner_variants_and_the_id_override() {
        let mut draft = FamilyDraft {
            human_id: "F0042".to_owned(),
            ..FamilyDraft::new()
        };
        draft.add_partner(existing("I0001"));
        draft.add_new_partner(new_person("Grace", "  "));
        let request = draft.to_request();
        assert_eq!(
            request.human_id.as_deref(),
            Some("F0042"),
            "the id override threads through"
        );
        assert_eq!(request.partners.len(), 2);
        assert_eq!(request.partners[0], PartnerRequest::Existing("I0001".to_owned()));
        assert_eq!(
            request.partners[1],
            PartnerRequest::New {
                given: Some("Grace".to_owned()),
                surname: None,
            },
            "a blank surname collapses to None",
        );
    }

    #[test]
    fn to_request_omits_a_blank_id_override() {
        let mut draft = FamilyDraft::new();
        draft.add_partner(existing("I0001"));
        assert!(draft.to_request().human_id.is_none(), "a blank id auto-allocates");
    }

    fn seed() -> FamilyDraft {
        FamilyDraft {
            existing_human_id: Some("F0001".to_owned()),
            human_id: "F0001".to_owned(),
            partners: Vec::new(),
            restrictions: vec![RestrictionKind::Confidential],
        }
    }

    fn detail() -> FamilyDetail {
        FamilyDetail {
            human_id: "F0009".to_owned(),
            id: "family-uuid".to_owned(),
            title: "Ada & Grace".to_owned(),
            partners: Vec::new(),
            marriage: None,
            children: Vec::new(),
            events: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: vec![RestrictionKind::Locked],
            research_notes: Vec::new(),
            history: Vec::new(),
        }
    }

    #[test]
    fn a_changed_restriction_set_yields_one_restriction_edit() {
        let draft = FamilyDraft {
            restrictions: vec![RestrictionKind::Confidential, RestrictionKind::Privacy],
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(
            &edits[0],
            FamilyEdit::SetRestrictions { restrictions, .. }
                if restrictions == &[RestrictionKind::Confidential, RestrictionKind::Privacy]
        ));
    }

    #[test]
    fn an_unchanged_restriction_set_yields_no_restriction_edit() {
        let draft = FamilyDraft {
            human_id: "F0042".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert!(
            !edits
                .iter()
                .any(|edit| matches!(edit, FamilyEdit::SetRestrictions { .. }))
        );
    }

    #[test]
    fn from_detail_seeds_the_restrictions_and_offers_the_field() {
        let draft = FamilyDraft::from_detail(&detail());
        assert_eq!(draft.restrictions, vec![RestrictionKind::Locked]);
        assert_eq!(
            draft.editable_restrictions(),
            Some([RestrictionKind::Locked].as_slice()),
            "a stored record offers the restriction field"
        );
    }

    #[test]
    fn a_create_draft_offers_no_restriction_field() {
        assert_eq!(FamilyDraft::new().editable_restrictions(), None);
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

#[cfg(test)]
mod family_display_label_tests {
    use super::{FamilyDraft, NewPersonFields, PartnerInput, RecordDraft};
    use crate::picker::PickerSelection;

    #[test]
    fn the_label_joins_the_partners_with_an_ampersand() {
        let mut draft = FamilyDraft::new();
        draft.add_partner(PickerSelection {
            human_id: "I0001".to_owned(),
            title: "Ada Lovelace".to_owned(),
        });
        draft.add_new_partner(NewPersonFields {
            given: "Bob".to_owned(),
            surname: "Byron".to_owned(),
        });
        assert_eq!(draft.display_label(), Some("Ada Lovelace & Bob Byron".to_owned()));
    }

    #[test]
    fn one_partner_names_the_family_alone() {
        let draft = FamilyDraft {
            partners: vec![PartnerInput::New(NewPersonFields {
                given: String::new(),
                surname: "Byron".to_owned(),
            })],
            ..FamilyDraft::new()
        };
        assert_eq!(draft.display_label(), Some("Byron".to_owned()));
    }

    #[test]
    fn a_draft_with_no_partners_has_no_label() {
        assert_eq!(FamilyDraft::new().display_label(), None);
    }
}
