use super::{
    CitationRefVm, DetailTab, HistoryEntryVm, Localizer, RestrictionKind, RowVm, TagRef, citation_ref_from_ref,
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
