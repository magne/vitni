use super::{
    AssociationSummary, AssociationVm, AttachedRefVm, CitationRefVm, ConfidenceLevel, DetailTab, DraftCitationRef,
    DraftNewCitation, DraftNewSource, DraftSourceRef, EventRefVm, EvidenceLevel, FactSummary, FactVm, FamilyVm,
    HistoryEntryVm, Localizer, NameSummary, NameType, NameVm, NewCitationFields, PersonChangeSetRequest, PersonName,
    PersonNameParts, PersonRow, PersonSummary, RecordDraft, RecordLink, RestrictionKind, RowVm, Sex, TagRef,
    citation_ref_from_ref,
};

/// Builds a generic list row from a [`PersonSummary`], localizing the name and sex via `loc`.
///
/// The subtitle is the localized sex label for now; later slices enrich it with vital dates and a
/// primary place. The avatar is the person's initials, or `?` when no name is known.
#[must_use]
pub fn person_row(summary: &PersonSummary, loc: &Localizer) -> RowVm {
    person_list_row_fields(
        &summary.human_id,
        summary.display_name.as_deref(),
        summary.sex.as_ref(),
        summary.given.as_deref(),
        summary.surname.as_deref(),
        loc,
    )
}

/// Builds a generic list row from a lightweight [`PersonRow`] (the list view's per-row DTO), which
/// carries only name + sex — the same rendering as [`person_row`] without loading a full summary.
#[must_use]
pub fn person_list_row(row: &PersonRow, loc: &Localizer) -> RowVm {
    person_list_row_fields(
        &row.human_id,
        row.display_name.as_deref(),
        row.sex.as_ref(),
        row.given.as_deref(),
        row.surname.as_deref(),
        loc,
    )
}

/// The shared [`RowVm`] builder behind [`person_row`] and [`person_list_row`]: the same title/subtitle/
/// avatar from the same name + sex fields, whatever DTO they were read from.
fn person_list_row_fields(
    human_id: &str,
    display_name: Option<&str>,
    sex: Option<&Sex>,
    given: Option<&str>,
    surname: Option<&str>,
    loc: &Localizer,
) -> RowVm {
    RowVm {
        id: human_id.to_owned(),
        title: loc.display_name(display_name),
        subtitle: Some(loc.sex_label(sex)),
        avatar: Some(initials(given, surname)),
        ..RowVm::default()
    }
}

/// The person's initials from the structured given/surname, or `?` when neither is known.
fn initials(given: Option<&str>, surname: Option<&str>) -> String {
    let mut initials = String::new();
    for part in [given, surname] {
        if let Some(first) = part.and_then(|name| name.chars().next()) {
            initials.push(first.to_ascii_uppercase());
        }
    }
    if initials.is_empty() {
        initials.push('?');
    }
    initials
}

/// Builds a [`NameVm`] from an asserted [`NameSummary`], localizing the type label and confidence.
fn name_vm(summary: &NameSummary, loc: &Localizer) -> NameVm {
    let name = &summary.name;
    let primary_surname = name.surnames.first();
    let surname = primary_surname.map(|element| element.surname.clone());
    let confidence = summary.confidence.map(ConfidenceLevel::from);
    NameVm {
        type_label: loc.name_type_label(&name.name_type),
        display: render_person_name(name),
        given: name.given.clone(),
        surname,
        nickname: name.nickname.clone(),
        date: name.date.as_ref().map(|date| loc.date(date)),
        language: name.language.as_ref().map(|language| language.as_str().to_owned()),
        confidence,
        confidence_label: loc.confidence_label_opt(confidence),
        source_count: summary.source_count,
        surname_prefix: primary_surname.and_then(|element| element.prefix.clone()),
        name_prefix: name.title.clone(),
        suffix: name.suffix.clone(),
        name_type: name.name_type.clone(),
        assertion_id: summary.assertion_id.clone(),
    }
}

/// Builds an [`AssociationVm`] from an app [`AssociationSummary`], localizing the role + confidence.
fn association_vm(summary: &AssociationSummary, loc: &Localizer) -> AssociationVm {
    let confidence = summary.confidence.map(ConfidenceLevel::from);
    AssociationVm {
        other_id: summary.other.human_id.clone(),
        role_label: loc.association_role_label(&summary.role),
        confidence,
        confidence_label: loc.confidence_label_opt(confidence),
        source_count: summary.source_count,
        role: summary.role.clone(),
        assertion_id: summary.assertion_id.clone(),
    }
}

/// Orders a person's participations chronologically for the Events tab (design-system.html §191):
/// by the joined event date's sort key, undated participations last. Assertion order (the app's
/// projection order) is stable within an equal date. Returns borrows in the display order.
fn sorted_participations(participations: &[genealogy_app::ParticipationRef]) -> Vec<&genealogy_app::ParticipationRef> {
    let mut ordered: Vec<&genealogy_app::ParticipationRef> = participations.iter().collect();
    ordered.sort_by_key(|participation| participation.date.as_ref().map_or(i64::MAX, |date| date.sort_value));
    ordered
}

/// Builds an [`EventRefVm`] from a person's [`ParticipationRef`](genealogy_app::ParticipationRef),
/// localizing the role label and the event's date (both joined in the app layer).
fn participation_vm(participation: &genealogy_app::ParticipationRef, loc: &Localizer) -> EventRefVm {
    EventRefVm {
        event_id: participation.event.human_id.clone(),
        role_label: loc.participant_role_label(&participation.role),
        date: participation.date.as_ref().map(|date| loc.date(date)),
        place: participation.place.clone(),
        role: participation.role.clone(),
        age_label: participation.age.as_ref().map(|age| loc.age_label(age)),
        age: participation.age.clone(),
        attributes: participation.attributes.clone(),
        notes: participation.notes.iter().map(|note| note.human_id.clone()).collect(),
        confidence: participation.confidence.map(ConfidenceLevel::from),
        confidence_label: loc.confidence_label_opt(participation.confidence.map(ConfidenceLevel::from)),
        source_count: participation.source_count,
        assertion_id: participation.assertion_id.clone(),
    }
}

/// Builds a [`FactVm`] from an app [`FactSummary`], localizing labels and the date.
fn fact_vm(summary: &FactSummary, loc: &Localizer) -> FactVm {
    let confidence = summary.confidence.map(ConfidenceLevel::from);
    FactVm {
        type_label: loc.fact_type_label(&summary.fact.fact_type),
        value: summary.fact.value.clone(),
        date: summary.fact.date.as_ref().map(|date| loc.date(date)),
        confidence,
        confidence_label: loc.confidence_label_opt(confidence),
        source_count: summary.citations.len(),
        citations: summary
            .citations
            .iter()
            .map(|c| citation_ref_from_ref(c, loc))
            .collect(),
        fact_type: summary.fact.fact_type.clone(),
        assertion_id: summary.assertion_id.clone(),
    }
}

/// Builds the localized vital summary (`b. <date> · d. <date>`) from a person's birth/death facts.
///
/// Only dated births/deaths contribute; place names need place resolution and are left to a later
/// slice. Returns `None` when neither birth nor death is dated.
fn vital_summary(summary: &PersonSummary, loc: &Localizer) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(date) = summary.birth_date.as_ref() {
        parts.push(loc.vital_born(&loc.date(date)));
    }
    if let Some(date) = summary.death_date.as_ref() {
        parts.push(loc.vital_died(&loc.date(date)));
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
    /// The media attached to this person, each with its attach `AssertionId` (the Detach target).
    pub media: Vec<AttachedRefVm>,
    /// The notes attached to this person, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
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
            events: sorted_participations(&summary.participations)
                .into_iter()
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
            media: summary
                .media
                .iter()
                .map(|m| AttachedRefVm {
                    human_id: m.human_id.clone(),
                    assertion_id: m.assertion_id.clone(),
                })
                .collect(),
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
            tags: summary.tag_refs.clone(),
            history: Vec::new(),
            edit_seed: PersonDraft::from_summary(summary),
        }
    }
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
    /// The editable user-facing id: on create a `human_id` override (empty ⇒ auto-allocate); on edit
    /// the person's current id, which the operator can change (empty ⇒ regenerate from the id format).
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
    /// The citation backing the preferred name: unset, an existing citation, or one created inline
    /// (which itself cites an existing or a nested new source — §6b, data-model §7).
    pub name_citation: RecordLink<NewCitationFields>,
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
            name_citation: RecordLink::Empty,
        }
    }

    /// A draft pre-populated from an existing person for editing. Records the `human_id` so the
    /// commit edits (diffs) rather than creates.
    #[must_use]
    pub fn from_summary(summary: &PersonSummary) -> Self {
        Self {
            existing_human_id: Some(summary.human_id.clone()),
            // Seed the editable human-id field with the current id (blank ⇒ regenerate on save).
            human_id_override: summary.human_id.clone(),
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
            name_citation: RecordLink::Empty,
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
            RecordLink::Empty => None,
            RecordLink::Existing(selection) => Some(DraftCitationRef::Existing(selection.human_id.clone())),
            RecordLink::New(citation) => {
                let source = match &citation.source {
                    RecordLink::Existing(selection) => DraftSourceRef::Existing(selection.human_id.clone()),
                    RecordLink::New(source) => {
                        let placeholder = format!("{}-source", Self::PENDING_KEY);
                        new_sources.push(DraftNewSource {
                            placeholder: placeholder.clone(),
                            title: non_blank(&source.title),
                        });
                        DraftSourceRef::Pending(placeholder)
                    }
                    RecordLink::Empty => {
                        let placeholder = format!("{}-source", Self::PENDING_KEY);
                        new_sources.push(DraftNewSource {
                            placeholder: placeholder.clone(),
                            title: None,
                        });
                        DraftSourceRef::Pending(placeholder)
                    }
                };
                new_citations.push(DraftNewCitation {
                    placeholder: Self::PENDING_KEY.to_owned(),
                    source,
                    page: non_blank(&citation.page),
                });
                Some(DraftCitationRef::Pending(Self::PENDING_KEY.to_owned()))
            }
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

impl RecordDraft for PersonDraft {
    type Detail = PersonDetail;

    fn from_detail(detail: &PersonDetail) -> Self {
        detail.edit_seed.clone()
    }

    /// A person has no required scalar field (an unnamed persona is a legitimate record), so an edit
    /// is committable whenever it is dirty; the Save gate reduces to dirtiness for this aggregate.
    fn is_valid(&self) -> bool {
        true
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

#[cfg(test)]
mod tests {
    use super::PersonDraft;

    fn edit_draft(human_id: &str) -> PersonDraft {
        let mut draft = PersonDraft::new();
        draft.existing_human_id = Some("I0001".to_owned());
        draft.human_id_override = human_id.to_owned();
        draft
    }

    #[test]
    fn an_unchanged_human_id_leaves_the_edit_draft_clean_against_its_seed() {
        let seed = edit_draft("I0001");
        let draft = edit_draft("I0001");
        assert_eq!(seed, draft, "seeding the field with the current id is not a change");
    }

    #[test]
    fn changing_the_human_id_makes_the_draft_dirty() {
        let seed = edit_draft("I0001");
        let draft = edit_draft("I0777");
        assert_ne!(seed, draft);
    }

    #[test]
    fn a_blank_human_id_requests_regeneration_on_save() {
        let request = edit_draft("").to_request();
        assert_eq!(
            request.existing_human_id.as_deref(),
            Some("I0001"),
            "still an edit of I0001"
        );
        assert_eq!(
            request.human_id_override, None,
            "a cleared id carries no override, so the dispatch regenerates it"
        );
    }

    #[test]
    fn a_changed_human_id_is_carried_as_the_override() {
        let request = edit_draft("I0777").to_request();
        assert_eq!(request.human_id_override.as_deref(), Some("I0777"));
    }
}
