use super::{
    DetailTab, DnaTestChangeSetRequest, DnaTestEdit, HistoryEntryVm, Localizer, RecordDraft, RestrictionKind, RowVm,
    TagRef, UsingRecordVm, nav_ref, non_blank,
};

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
    /// The raw testing provider, if set (seeds the whole-record editor's Provider select).
    pub provider_kind: Option<genealogy_app::DnaProvider>,
    /// The test type's localized label, if set.
    pub test_type: Option<String>,
    /// The raw test type, if set (seeds the whole-record editor's Type select).
    pub test_type_kind: Option<genealogy_app::DnaTestType>,
    /// The provider's kit id, if set.
    pub kit_id: Option<String>,
    /// The genome build's localized label, if set.
    pub genome_build: Option<String>,
    /// The raw genome build, if set (seeds the whole-record editor's Genome-build select).
    pub genome_build_kind: Option<genealogy_app::DnaGenomeBuild>,
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
            provider_kind: summary.provider.clone(),
            test_type: summary.test_type.map(|t| loc.dna_test_type_label(t)),
            test_type_kind: summary.test_type,
            kit_id: summary.kit_id.clone(),
            genome_build: summary.genome_build.map(|b| loc.dna_genome_build_label(b)),
            genome_build_kind: summary.genome_build,
            person,
            person_name: summary.person_name.clone(),
            haplogroups: summary.haplogroups.iter().map(|h| h.value.clone()).collect(),
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

/// The create form's in-memory draft for a new DNA test (`record-editing.html` §6): a required
/// person plus an optional provider, test type, genome build, and kit id. Create-only; nothing is
/// written until Save commits a [`DnaTestChangeSetRequest`]. The person is required (§7): Save is
/// blocked and the field is flagged while it is blank.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DnaTestDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit mode only — create auto-allocates).
    pub human_id: String,
    /// The anchoring person's `human_id` (required in create; locked in edit).
    pub person: String,
    /// The testing provider, if chosen.
    pub provider: Option<genealogy_app::DnaProvider>,
    /// The test type, if chosen.
    pub test_type: Option<genealogy_app::DnaTestType>,
    /// The reference genome build, if chosen.
    pub genome_build: Option<genealogy_app::DnaGenomeBuild>,
    /// The kit id.
    pub kit_id: String,
}

impl DnaTestDraft {
    /// A fresh empty draft for creating a new DNA test.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing test for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (supersedes) rather than creates; seeds the provider/type/build/
    /// kit and the (locked) anchoring person.
    #[must_use]
    pub fn from_detail(detail: &DnaTestDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            person: detail
                .person
                .as_ref()
                .map(|person| person.human_id.clone())
                .unwrap_or_default(),
            provider: detail.provider_kind.clone(),
            test_type: detail.test_type_kind,
            genome_build: detail.genome_build_kind,
            kit_id: detail.kit_id.clone().unwrap_or_default(),
        }
    }

    /// Whether the required person field is invalid (blank) — drives `aria-invalid` + its field error.
    #[must_use]
    pub fn person_invalid(&self) -> bool {
        non_blank(&self.person).is_none()
    }

    /// Whether the draft is valid: on create the required person is present; on edit (person is
    /// locked) it is always valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.existing_human_id.is_some() || !self.person_invalid()
    }

    /// Builds the [`DnaTestChangeSetRequest`] the app commits on Save, or `None` when the required
    /// person is blank (so Save is a no-op).
    #[must_use]
    pub fn to_request(&self) -> Option<DnaTestChangeSetRequest> {
        let person = non_blank(&self.person)?;
        Some(DnaTestChangeSetRequest {
            person,
            provider: self.provider.clone(),
            test_type: self.test_type,
            genome_build: self.genome_build,
            kit_id: non_blank(&self.kit_id),
        })
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): one `Set*` per changed scalar (a cleared select emits nothing — there is no clear
    /// command), with `SetHumanId` last so the record is only re-keyed after every other field has
    /// committed (a blank id regenerates).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<DnaTestEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.provider != seed.provider
            && let Some(provider) = self.provider.clone()
        {
            edits.push(DnaTestEdit::SetProvider {
                human_id: human_id.clone(),
                provider,
            });
        }
        if self.test_type != seed.test_type
            && let Some(test_type) = self.test_type
        {
            edits.push(DnaTestEdit::SetType {
                human_id: human_id.clone(),
                test_type,
            });
        }
        if self.genome_build != seed.genome_build
            && let Some(genome_build) = self.genome_build
        {
            edits.push(DnaTestEdit::SetGenomeBuild {
                human_id: human_id.clone(),
                genome_build,
            });
        }
        if self.kit_id != seed.kit_id {
            edits.push(DnaTestEdit::SetKitId {
                human_id: human_id.clone(),
                kit_id: self.kit_id.clone(),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(DnaTestEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }
}

impl RecordDraft for DnaTestDraft {
    type Detail = DnaTestDetail;

    fn from_detail(detail: &DnaTestDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        Self::is_valid(self)
    }
}

#[cfg(test)]
mod dna_test_draft_tests {
    use super::{DnaTestDraft, RecordDraft};
    use crate::navigation::DnaTestEdit;
    use genealogy_app::{DnaProvider, DnaTestType};

    #[test]
    fn a_fresh_draft_is_invalid_and_not_dirty() {
        let draft = DnaTestDraft::new();
        assert!(!draft.is_valid());
        assert!(draft.person_invalid());
        assert!(!draft.is_dirty_against(&DnaTestDraft::new()));
        assert!(draft.to_request().is_none(), "no request without a person");
    }

    #[test]
    fn a_person_makes_it_valid_and_dirty() {
        let draft = DnaTestDraft {
            person: "I0001".to_owned(),
            provider: Some(DnaProvider::AncestryDna),
            ..DnaTestDraft::new()
        };
        assert!(draft.is_valid());
        assert!(draft.is_dirty_against(&DnaTestDraft::new()));
        let request = draft.to_request().expect("valid");
        assert_eq!(request.person, "I0001");
        assert_eq!(request.provider, Some(DnaProvider::AncestryDna));
    }

    fn edit_seed() -> DnaTestDraft {
        DnaTestDraft {
            existing_human_id: Some("D0001".to_owned()),
            human_id: "D0001".to_owned(),
            person: "I0001".to_owned(),
            provider: Some(DnaProvider::AncestryDna),
            test_type: Some(DnaTestType::Autosomal),
            genome_build: None,
            kit_id: "AB-12".to_owned(),
        }
    }

    #[test]
    fn an_edit_draft_is_valid_even_though_person_is_not_edited() {
        assert!(edit_seed().is_valid());
    }

    #[test]
    fn an_unchanged_test_yields_no_edits() {
        assert!(edit_seed().edits_against(&edit_seed()).is_empty());
    }

    #[test]
    fn each_changed_scalar_yields_one_edit() {
        let draft = DnaTestDraft {
            kit_id: "XZ-99".to_owned(),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], DnaTestEdit::SetKitId { kit_id, .. } if kit_id == "XZ-99"));
    }

    #[test]
    fn a_blank_id_regenerates_and_is_emitted_last() {
        let draft = DnaTestDraft {
            provider: Some(DnaProvider::MyHeritage),
            human_id: String::new(),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 2);
        assert!(matches!(&edits[0], DnaTestEdit::SetProvider { .. }));
        assert!(matches!(&edits[1], DnaTestEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }
}
