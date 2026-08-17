use super::{
    ActionLabel, AttachedRefVm, CitationRefVm, DateDraft, DetailTab, HistoryEntryVm, Localizer, MediaChangeSetRequest,
    MediaEdit, RecordDraft, RestrictionKind, RowVm, TagRef, citation_ref_from_ref, line_label, media_asset_src,
    media_is_image, non_blank,
};

/// A record that references a media object or note (Media "Used by" / Note "References"): its kind
/// drives the route, plus the display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsingRecordVm {
    /// The referencing aggregate's kind (drives the navigation route and the kind chip).
    pub kind: vitni_app::UsingKind,
    /// The referencing record's user-facing id.
    pub human_id: String,
    /// The referencing record's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The referencing record's display label (a name/title, or the `human_id` fallback).
    pub label: String,
    /// The localized kind label (the chip text — colour/route is never the only signal).
    pub kind_label: String,
}

/// Builds a [`UsingRecordVm`] from an app [`UsingRecordRef`](vitni_app::UsingRecordRef).
pub(crate) fn using_record_vm(reference: &vitni_app::UsingRecordRef, loc: &Localizer) -> UsingRecordVm {
    UsingRecordVm {
        kind: reference.kind,
        human_id: reference.human_id.clone(),
        id: reference.id.clone(),
        label: reference.label.clone().unwrap_or_else(|| reference.human_id.clone()),
        kind_label: loc.using_kind_label(reference.kind),
    }
}

/// One typed attribute on a media object (Media File card): a typed `(type, value)` pair plus the
/// `AssertionId` that introduced it — the target a per-row Edit supersedes and a Retract retracts
/// (ADR 0004 §2). The assertion id is never rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAttributeVm {
    /// The attribute's type / key.
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
    /// The `AssertionId` (a UUID string) that introduced this attribute. Never rendered.
    pub assertion_id: String,
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
    /// The media's raw filesystem path, if its location is a local file (seeds the editor's File-path
    /// field; mutually exclusive with [`Self::web_path`]).
    pub file_path: Option<String>,
    /// The media's raw web reference, if its location is a URL (seeds the editor's Web-path field;
    /// mutually exclusive with [`Self::file_path`]).
    pub web_path: Option<String>,
    /// The media's MIME type (e.g. `image/jpeg`), if set.
    pub mime: Option<String>,
    /// The media's checksum, if set.
    pub checksum: Option<String>,
    /// The media's localized date, if asserted.
    pub date: Option<String>,
    /// The media's structured date, if asserted (seeds the whole-record editor).
    pub date_value: Option<vitni_app::GenealogicalDate>,
    /// The recorded attributes (File card metadata).
    pub attributes: Vec<MediaAttributeVm>,
    /// The citations backing the media's claims.
    pub citations: Vec<CitationRefVm>,
    /// The attached notes, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
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
    /// Builds a detail view from a [`MediaSummary`](vitni_app::MediaSummary), localizing the date
    /// and confidence. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &vitni_app::MediaSummary, loc: &Localizer) -> Self {
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
            file_path: summary.file_path.clone(),
            web_path: summary.web_path.clone(),
            mime: summary.mime.clone(),
            checksum: summary.checksum.clone(),
            date: summary.date.as_ref().map(|date| loc.date(date)),
            date_value: summary.date.clone(),
            attributes: summary
                .attributes
                .iter()
                .map(|a| MediaAttributeVm {
                    attribute_type: a.attribute_type.clone(),
                    value: a.value.clone(),
                    assertion_id: a.assertion_id.clone(),
                })
                .collect(),
            citations: summary
                .citations
                .iter()
                .map(|c| citation_ref_from_ref(c, loc))
                .collect(),
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
            tags: summary.tags.clone(),
            used_by: summary.used_by.iter().map(|u| using_record_vm(u, loc)).collect(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }

    /// Whether the object is an image ([`media_is_image`]) — the Overview shows a real preview for an
    /// image and a glyph placeholder otherwise.
    #[must_use]
    pub fn is_image(&self) -> bool {
        media_is_image(self.mime.as_deref(), self.location())
    }

    /// The source the Overview preview loads: a web reference verbatim, else the local file served by
    /// the desktop asset handler ([`media_asset_src`]). `None` when the object has no location, or one
    /// the media root cannot serve.
    #[must_use]
    pub fn preview_src(&self) -> Option<String> {
        if let Some(web) = &self.web_path {
            return Some(web.clone());
        }
        media_asset_src(self.file_path.as_deref())
    }

    /// The object's location, whichever kind it has — the path a MIME is inferred from when the record
    /// carries none.
    fn location(&self) -> Option<&str> {
        self.file_path.as_deref().or(self.web_path.as_deref())
    }
}

/// The basename of a file path (the segment after the last `/` or `\`), for the media title.
fn file_basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

/// Builds a generic list row from a [`MediaSummary`](vitni_app::MediaSummary): the filename, a
/// `mime · date` subtitle, and a 📷 avatar.
#[must_use]
pub fn media_row(summary: &vitni_app::MediaSummary, loc: &Localizer) -> RowVm {
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
    let tab = |id: &'static str, count: Option<usize>, action: Option<ActionLabel>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
        action,
    };
    vec![
        tab("overview", None, None),
        tab(
            "attributes",
            Some(detail.attributes.len()),
            Some(ActionLabel::AddAttribute),
        ),
        tab(
            "citations",
            Some(detail.citations.len()),
            Some(ActionLabel::AttachCitation),
        ),
        tab("notes", Some(detail.notes.len()), Some(ActionLabel::AttachNote)),
        tab("tags", Some(detail.tags.len()), Some(ActionLabel::AddTag)),
        tab("history", None, None),
    ]
}

/// The buffered whole-record draft of a media object (create + edit, one mechanism,
/// `record-editing.html` §2/§6): the editable user-facing id, a file path, a web path, a MIME type, and
/// the structured date. Checksum is locked (§3) — read-only in the editor.
/// `existing_human_id` is `None` in create mode and `Some` in edit mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit) / auto-allocated (create).
    pub human_id: String,
    /// A local file path.
    pub file_path: String,
    /// A web reference.
    pub web_path: String,
    /// The MIME type.
    pub mime: String,
    /// The checksum, shown read-only in the editor (locked, §3): seeded from the record, never edited.
    pub checksum: String,
    /// The structured date (`event.html` control cluster). On create it is carried in the change-set
    /// request and asserted after the commit; on edit a change emits a `SetDate` on Save. A blank
    /// draft emits nothing.
    pub date: DateDraft,
    /// The media object's privacy restrictions (GEDCOM `RESN`); empty is unrestricted. Edit-only — the
    /// change-set request carries none, so a create form does not offer the field.
    pub restrictions: Vec<RestrictionKind>,
}

impl MediaDraft {
    /// A fresh empty draft for creating a new media object.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing media object for editing. Records the current `human_id`
    /// so [`Self::edits_against`] diffs (supersedes) rather than creates. The checksum and date are
    /// seeded for the locked display fields but never diffed.
    #[must_use]
    pub fn from_detail(detail: &MediaDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            file_path: detail.file_path.clone().unwrap_or_default(),
            web_path: detail.web_path.clone().unwrap_or_default(),
            mime: detail.mime.clone().unwrap_or_default(),
            checksum: detail.checksum.clone().unwrap_or_default(),
            date: detail.date_value.as_ref().map_or_else(DateDraft::default, |value| {
                DateDraft::from_value(value, detail.date.clone().unwrap_or_default())
            }),
            restrictions: detail.restrictions.clone(),
        }
    }

    /// Builds the [`MediaChangeSetRequest`] the app commits on Save (create mode).
    #[must_use]
    pub fn to_request(&self) -> MediaChangeSetRequest {
        MediaChangeSetRequest {
            human_id: non_blank(&self.human_id),
            file_path: non_blank(&self.file_path),
            web_path: non_blank(&self.web_path),
            mime: non_blank(&self.mime),
            date: self.date.to_input().ok().flatten(),
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): one `Set*` per changed scalar, with `SetHumanId` emitted last so the record is only
    /// re-keyed after every other field has committed against its current id (a blank id regenerates) —
    /// the restriction set included, so it too commits against the id the record still has.
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<MediaEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.date != seed.date
            && let Ok(Some(date)) = self.date.to_input()
        {
            edits.push(MediaEdit::SetDate {
                human_id: human_id.clone(),
                date,
            });
        }
        if self.file_path != seed.file_path {
            edits.push(MediaEdit::SetFilePath {
                human_id: human_id.clone(),
                path: self.file_path.clone(),
            });
        }
        if self.web_path != seed.web_path {
            edits.push(MediaEdit::SetWebPath {
                human_id: human_id.clone(),
                href: self.web_path.clone(),
            });
        }
        if self.mime != seed.mime {
            edits.push(MediaEdit::SetMime {
                human_id: human_id.clone(),
                mime: self.mime.clone(),
            });
        }
        if self.restrictions != seed.restrictions {
            edits.push(MediaEdit::SetRestrictions {
                human_id: human_id.clone(),
                restrictions: self.restrictions.clone(),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(MediaEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }
}

impl RecordDraft for MediaDraft {
    type Detail = MediaDetail;

    fn from_detail(detail: &MediaDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        !self.date.is_invalid()
    }

    fn display_label(&self) -> Option<String> {
        line_label(&file_basename(self.file_path.trim())).or_else(|| line_label(&self.web_path))
    }

    fn editable_restrictions(&self) -> Option<&[RestrictionKind]> {
        self.existing_human_id.is_some().then_some(self.restrictions.as_slice())
    }

    fn set_restrictions(&mut self, restrictions: Vec<RestrictionKind>) {
        self.restrictions = restrictions;
    }
}

#[cfg(test)]
mod media_draft_tests {
    use super::{DateDraft, MediaDetail, MediaDraft, RecordDraft};
    use crate::navigation::MediaEdit;
    use crate::presentation::RestrictionKind;

    fn seed() -> MediaDraft {
        MediaDraft {
            existing_human_id: Some("O0001".to_owned()),
            human_id: "O0001".to_owned(),
            file_path: "photos/ada.jpg".to_owned(),
            web_path: String::new(),
            mime: "image/jpeg".to_owned(),
            checksum: "abc123".to_owned(),
            date: typed_date("1998"),
            restrictions: vec![RestrictionKind::Confidential],
        }
    }

    fn detail() -> MediaDetail {
        MediaDetail {
            human_id: "O0009".to_owned(),
            id: "media-uuid".to_owned(),
            title: "ada.jpg".to_owned(),
            path: Some("photos/ada.jpg".to_owned()),
            file_path: Some("photos/ada.jpg".to_owned()),
            web_path: None,
            mime: Some("image/jpeg".to_owned()),
            checksum: None,
            date: None,
            date_value: None,
            attributes: Vec::new(),
            citations: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            used_by: Vec::new(),
            restrictions: vec![RestrictionKind::Locked],
            history: Vec::new(),
        }
    }

    #[test]
    fn a_changed_restriction_set_yields_one_restriction_edit() {
        let draft = MediaDraft {
            restrictions: vec![RestrictionKind::Confidential, RestrictionKind::Privacy],
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(
            &edits[0],
            MediaEdit::SetRestrictions { restrictions, .. }
                if restrictions == &[RestrictionKind::Confidential, RestrictionKind::Privacy]
        ));
    }

    #[test]
    fn an_unchanged_restriction_set_yields_no_restriction_edit() {
        let draft = MediaDraft {
            mime: "image/png".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert!(
            !edits
                .iter()
                .any(|edit| matches!(edit, MediaEdit::SetRestrictions { .. }))
        );
    }

    #[test]
    fn from_detail_seeds_the_restrictions_and_offers_the_field() {
        let draft = MediaDraft::from_detail(&detail());
        assert_eq!(draft.restrictions, vec![RestrictionKind::Locked]);
        assert_eq!(
            draft.editable_restrictions(),
            Some([RestrictionKind::Locked].as_slice()),
            "a stored record offers the restriction field"
        );
    }

    #[test]
    fn a_create_draft_offers_no_restriction_field() {
        assert_eq!(MediaDraft::new().editable_restrictions(), None);
    }

    fn typed_date(text: &str) -> DateDraft {
        DateDraft {
            start: text.to_owned(),
            ..DateDraft::default()
        }
    }

    #[test]
    fn to_request_trims_each_path_and_mime() {
        let draft = MediaDraft {
            file_path: "  photos/ada.jpg  ".to_owned(),
            web_path: String::new(),
            mime: "image/jpeg".to_owned(),
            ..MediaDraft::new()
        };
        let request = draft.to_request();
        assert_eq!(request.file_path.as_deref(), Some("photos/ada.jpg"));
        assert_eq!(request.web_path, None);
        assert_eq!(request.mime.as_deref(), Some("image/jpeg"));
    }

    #[test]
    fn an_unchanged_draft_yields_no_edits() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn changing_the_mime_yields_one_set_mime() {
        let draft = MediaDraft {
            mime: "image/png".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], MediaEdit::SetMime { mime, .. } if mime == "image/png"));
    }

    #[test]
    fn a_blank_human_id_regenerates() {
        let draft = MediaDraft {
            human_id: String::new(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], MediaEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }

    #[test]
    fn a_changed_date_makes_it_dirty_and_emits_set_date() {
        let draft = MediaDraft {
            date: typed_date("14 Jun 1876"),
            ..seed()
        };
        assert!(draft.is_dirty_against(&seed()));
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        let MediaEdit::SetDate { date, .. } = &edits[0] else {
            panic!("expected a SetDate, got {:?}", edits[0]);
        };
        assert_eq!(*date, typed_date("14 Jun 1876").to_input().unwrap().unwrap());
    }

    #[test]
    fn an_untouched_date_emits_no_set_date() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn an_invalid_date_blocks_validity() {
        let draft = MediaDraft {
            date: typed_date("gibberish"),
            ..seed()
        };
        assert!(!draft.is_valid());
    }

    #[test]
    fn a_create_request_carries_a_parsed_date() {
        let draft = MediaDraft {
            date: typed_date("14 Jun 1876"),
            ..MediaDraft::new()
        };
        assert_eq!(draft.to_request().date, typed_date("14 Jun 1876").to_input().unwrap());
    }

    #[test]
    fn a_blank_create_date_maps_to_none() {
        assert!(MediaDraft::new().to_request().date.is_none());
    }
}

#[cfg(test)]
mod media_preview_tests {
    use super::MediaDetail;

    /// A detail with only the fields the preview reads populated.
    fn detail(file_path: Option<&str>, web_path: Option<&str>, mime: Option<&str>) -> MediaDetail {
        MediaDetail {
            human_id: "O0001".to_owned(),
            id: "0190-media-id".to_owned(),
            title: "ada.jpg".to_owned(),
            path: file_path.or(web_path).map(str::to_owned),
            file_path: file_path.map(str::to_owned),
            web_path: web_path.map(str::to_owned),
            mime: mime.map(str::to_owned),
            checksum: None,
            date: None,
            date_value: None,
            attributes: Vec::new(),
            citations: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            used_by: Vec::new(),
            restrictions: Vec::new(),
            history: Vec::new(),
        }
    }

    #[test]
    fn a_stored_path_is_served_with_exactly_one_media_prefix() {
        // #301 cause 2: the stored form carries `media/`, so prepending it again asked for
        // `/media/media/…` and the asset handler resolved nothing.
        let detail = detail(Some("media/portraits/ada.jpg"), None, Some("image/jpeg"));
        assert_eq!(detail.preview_src().as_deref(), Some("/media/portraits/ada.jpg"));
    }

    #[test]
    fn a_bare_root_relative_path_is_served_from_the_same_url() {
        let detail = detail(Some("portraits/ada.jpg"), None, Some("image/jpeg"));
        assert_eq!(detail.preview_src().as_deref(), Some("/media/portraits/ada.jpg"));
    }

    #[test]
    fn a_file_outside_the_workspace_has_no_preview_source() {
        let detail = detail(Some("/home/ada/photos/ada.jpg"), None, Some("image/jpeg"));
        assert_eq!(
            detail.preview_src(),
            None,
            "the asset handler serves only the media root; the glyph is honest"
        );
    }

    #[test]
    fn a_web_reference_is_previewed_verbatim() {
        let detail = detail(None, Some("https://example.org/ada.jpg"), Some("image/jpeg"));
        assert_eq!(detail.preview_src().as_deref(), Some("https://example.org/ada.jpg"));
    }

    #[test]
    fn a_record_with_no_location_has_no_preview_source() {
        assert_eq!(detail(None, None, Some("image/jpeg")).preview_src(), None);
    }

    #[test]
    fn an_absent_mime_falls_back_to_the_extension() {
        // #301 cause 1: nothing infers a MIME and the CLI cannot set one, so an image created without
        // one rendered the glyph forever.
        assert!(detail(Some("media/portraits/ada.jpg"), None, None).is_image());
        assert!(!detail(Some("media/deeds/deed.pdf"), None, None).is_image());
        assert!(detail(None, Some("https://example.org/ada.png"), None).is_image());
    }

    #[test]
    fn a_recorded_non_image_mime_beats_the_extension() {
        assert!(!detail(Some("media/scans/deed.jpg"), None, Some("application/pdf")).is_image());
    }
}

#[cfg(test)]
mod media_display_label_tests {
    use super::{MediaDraft, RecordDraft};

    #[test]
    fn the_label_is_the_files_basename_not_its_whole_path() {
        let draft = MediaDraft {
            file_path: "/home/ada/photos/ada.jpg".to_owned(),
            ..MediaDraft::new()
        };
        assert_eq!(draft.display_label(), Some("ada.jpg".to_owned()));
    }

    #[test]
    fn a_web_reference_names_a_draft_with_no_local_file() {
        let draft = MediaDraft {
            web_path: "https://example.org/ada.jpg".to_owned(),
            ..MediaDraft::new()
        };
        assert_eq!(draft.display_label(), Some("https://example.org/ada.jpg".to_owned()));
    }

    #[test]
    fn a_draft_with_no_location_has_no_label() {
        let draft = MediaDraft {
            mime: "image/jpeg".to_owned(),
            ..MediaDraft::new()
        };
        assert_eq!(draft.display_label(), None);
    }
}
