use super::{
    CitationRefVm, DetailTab, HistoryEntryVm, Localizer, MediaChangeSetRequest, MediaEdit, RecordDraft,
    RestrictionKind, RowVm, TagRef, citation_ref_from_ref, non_blank,
};

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
pub(crate) fn using_record_vm(reference: &genealogy_app::UsingRecordRef, loc: &Localizer) -> UsingRecordVm {
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
            file_path: summary.file_path.clone(),
            web_path: summary.web_path.clone(),
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

/// The buffered whole-record draft of a media object (create + edit, one mechanism,
/// `record-editing.html` §2/§6): the editable user-facing id, a file path, a web path, and a MIME
/// type. Checksum and date are locked (§3) — read-only in the editor, not represented here. Date
/// editing is PR29. `existing_human_id` is `None` in create mode and `Some` in edit mode.
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
    /// The localized date, shown read-only in the editor (locked, §3): seeded from the record, never
    /// edited (structured date editing is PR29).
    pub date: String,
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
            date: detail.date.clone().unwrap_or_default(),
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
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): one `Set*` per changed scalar, with `SetHumanId` emitted last so the record is only
    /// re-keyed after every other field has committed against its current id (a blank id regenerates).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<MediaEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
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
        true
    }
}

#[cfg(test)]
mod media_draft_tests {
    use super::MediaDraft;
    use crate::navigation::MediaEdit;

    fn seed() -> MediaDraft {
        MediaDraft {
            existing_human_id: Some("O0001".to_owned()),
            human_id: "O0001".to_owned(),
            file_path: "photos/ada.jpg".to_owned(),
            web_path: String::new(),
            mime: "image/jpeg".to_owned(),
            checksum: "abc123".to_owned(),
            date: "1998".to_owned(),
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
}
