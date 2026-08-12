use super::{ConfidenceLevel, EvidenceAnalysis, EvidenceAxis, Localizer, friendly_timestamp};

/// Trims a form field and maps a blank value to `None` — the "not reported" convention every create
/// draft applies to an optional field so an empty box writes nothing (`record-editing.html` §6).
#[must_use]
pub(crate) fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// How long a label derived from free text may get before it is cut short. Long enough for a real
/// note heading, short enough that a record tab or a confirm's at-stake list stays readable — the tab
/// strip does not ellipsize, so the length is bounded here at the source.
pub(crate) const LABEL_MAX_CHARS: usize = 60;

/// A one-line label for a block of free text: its first non-empty line, with any Markdown heading
/// `#` markers stripped, capped at [`LABEL_MAX_CHARS`] characters. `None` when the text has no
/// non-empty line, or nothing but `#`s.
///
/// The cap counts **characters**, not bytes, so it can never split a multi-byte character.
///
/// The one rule for turning prose into a label: a note's list title, a research note's fallback title,
/// and every create draft's `display_label` all read through it, so a record is named the same way
/// wherever it is shown.
pub(crate) fn line_label(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let line = line.trim_start_matches('#').trim();
    let label: String = line.chars().take(LABEL_MAX_CHARS).collect();
    (!label.is_empty()).then_some(label)
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
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this name (its source count).
    pub source_count: usize,
    /// The primary surname's prefix (GEDCOM `SPFX`), for edit prefill.
    pub surname_prefix: Option<String>,
    /// The name's title / prefix (GEDCOM `NPFX`), for edit prefill.
    pub name_prefix: Option<String>,
    /// The name's suffix (GEDCOM `NSFX`), for edit prefill.
    pub suffix: Option<String>,
    /// The name's type, for edit prefill (kept alongside `type_label`, the display string).
    pub name_type: genealogy_app::NameType,
    /// The `AssertionId` (a UUID string) that introduced this name — a per-row Edit's supersede
    /// target and a Retract's target (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this fact (its source count).
    pub source_count: usize,
    /// The fact's citations, for the provenance popover (source · page · surety · evidence axes).
    pub citations: Vec<CitationRefVm>,
    /// The fact's type, for edit prefill (kept alongside `type_label`, the display string).
    pub fact_type: genealogy_app::FactType,
    /// The `AssertionId` (a UUID string) that introduced this fact — a per-row Edit's supersede
    /// target and a Retract's target (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    /// The event's place display name, if the event links a place (joined in the app layer).
    pub place: Option<String>,
    /// The participant's role, for edit prefill (kept alongside `role_label`, the display string).
    pub role: genealogy_app::ParticipantRole,
    /// The localized age label (e.g. `over 42y`), if an age is recorded (ADR 0019).
    pub age_label: Option<String>,
    /// The participant's age, for edit prefill (kept alongside `age_label`, the display string).
    pub age: Option<genealogy_app::Age>,
    /// The participant-scoped typed attributes (ADR 0019), for display and edit prefill.
    pub attributes: Vec<genealogy_app::Attribute>,
    /// The `human_id`s of notes about this participation (ADR 0019), for display and edit prefill.
    pub notes: Vec<String>,
    /// The participation's confidence, as a presentation level (drives the badge colour token).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (the surety denormalized from the envelope — ADR 0020).
    pub confidence_label: String,
    /// How many citations back this participation (its source count).
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this participation — a per-row Edit's
    /// supersede target and a Retract's target (ADR 0004 §2). Never rendered. Always the person-side
    /// (canonical) assertion.
    pub assertion_id: String,
}

/// One person-to-person association, for the Associations tab — with its evidence cues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationVm {
    /// The other person's user-facing id.
    pub other_id: String,
    /// The localized association-role label.
    pub role_label: String,
    /// The association's confidence, as a presentation level (drives the badge colour token).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this association (its source count).
    pub source_count: usize,
    /// The association's role, for edit prefill (kept alongside `role_label`, the display string).
    pub role: genealogy_app::AssociationRole,
    /// The `AssertionId` (a UUID string) that introduced this association — a per-row Edit's
    /// supersede target and a Retract's target (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
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
    /// How many records this citation backs (the Citations tab's "Backs" column). `0` unless the app
    /// lookup filled it (only the tabs that render Backs — Person, Family — pay to compute it).
    pub backs_count: usize,
    /// The citation's confidence, if set (drives the badge).
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label, if set.
    pub confidence_label: Option<String>,
    /// The Evidence Explained axis chips (empty when the citation records no analysis).
    pub evidence_axes: Vec<EvidenceAxisVm>,
    /// The localized "asserted by {who} · {when}" provenance line, if the creation operator is known.
    pub asserted_by: Option<String>,
    /// The `AssertionId` (a UUID string) of the attach assertion when this row is an owner's own
    /// attached citation — the Detach target (ADR 0004 §2). `None` when the citation is shown as
    /// evidence (a fact's backing citations), not as a detachable attachment. Never rendered.
    pub assertion_id: Option<String>,
}

/// One postal address recorded on an aggregate (Repository or Event › Addresses tab): the postal
/// address plus the `AssertionId` that introduced it — the target a per-card Edit supersedes and a
/// Retract retracts (ADR 0004 §2). The assertion id is never rendered. Shared by every aggregate that
/// carries a `Vec<Attributed<Address>>` (data-model §7, §17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressVm {
    /// The postal address (street · locality · region · …), read directly by the card.
    pub address: genealogy_app::Address,
    /// The `AssertionId` (a UUID string) that introduced this address. Never rendered.
    pub assertion_id: String,
}

/// A record attached to an aggregate at the record level (a note, a media object), for a detail VM —
/// its display `human_id` plus the attach `AssertionId` a Detach retracts (ADR 0004 §2). Replaces the
/// bare `Vec<String>` of `human_id`s so a row can carry a Detach affordance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedRefVm {
    /// The attached record's user-facing id (e.g. `N0001`), for display and navigation.
    pub human_id: String,
    /// The `AssertionId` (a UUID string) of the attach assertion — the Detach target. Never rendered.
    pub assertion_id: String,
}

impl AttachedRefVm {
    /// Builds an [`AttachedRefVm`] from an app [`AttachedRef`](genealogy_app::AttachedRef).
    #[must_use]
    pub fn from_ref(reference: &genealogy_app::AttachedRef) -> Self {
        Self {
            human_id: reference.human_id.clone(),
            assertion_id: reference.assertion_id.clone(),
        }
    }
}

/// A media object attached to an aggregate (a Media gallery card), carrying everything a gallery or
/// the media viewer renders: the object's display id, the per-use caption + crop, the object's file
/// path/URL + MIME (joined from the Media projection), and the attach `AssertionId` a Detach retracts
/// or a region edit supersedes (ADR 0004 §2; ADR 0017 §9). Replaces the id-only `AttachedRefVm` for
/// media so a card can show a real thumbnail with its crop outline. The assertion id is never rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRefVm {
    /// The media object's user-facing id (e.g. `O0001`), for display and navigation.
    pub human_id: String,
    /// The `AssertionId` (a UUID string) of the attach assertion — the Detach / region-edit target.
    pub assertion_id: String,
    /// The per-use caption, if set.
    pub caption: Option<String>,
    /// The per-use crop/region of interest within the media, if set (data-model §7).
    pub crop: Option<genealogy_app::Rect>,
    /// The media object's file path or web URL (from the Media projection), for rendering the image.
    pub path: Option<String>,
    /// The media object's MIME type (from the Media projection), for choosing how to render it.
    pub mime: Option<String>,
}

impl MediaRefVm {
    /// Builds a [`MediaRefVm`] from an app [`MediaRefSummary`](genealogy_app::MediaRefSummary).
    #[must_use]
    pub fn from_ref(reference: &genealogy_app::MediaRefSummary) -> Self {
        Self {
            human_id: reference.human_id.clone(),
            assertion_id: reference.assertion_id.clone(),
            caption: reference.caption.clone(),
            crop: reference.crop,
            path: reference.path.clone(),
            mime: reference.mime.clone(),
        }
    }

    /// Whether the object is an image ([`media_is_image`]) — a gallery renders an `<img>` thumbnail
    /// for an image and a glyph placeholder otherwise.
    #[must_use]
    pub fn is_image(&self) -> bool {
        media_is_image(self.mime.as_deref(), self.path.as_deref())
    }

    /// The source a renderer loads the image from ([`media_asset_src`]).
    #[must_use]
    pub fn src(&self) -> Option<String> {
        media_asset_src(self.path.as_deref())
    }

    /// The gallery card caption: the per-use caption, falling back to the object's `human_id`.
    #[must_use]
    pub fn caption_or_id(&self) -> String {
        self.caption.clone().unwrap_or_else(|| self.human_id.clone())
    }
}

/// The URL a renderer loads a media object's bytes from, or `None` when there is nothing it can load.
///
/// A web reference is used verbatim. A local file is served by the desktop asset handler at
/// `/media/<path below the workspace media root>` — the prefix is added exactly once, by
/// [`media_root_relative`](genealogy_app::media_root_relative), whether or not the stored path already
/// carries it. `None` for a location the media root cannot serve (an absolute path outside the
/// workspace, or one that traverses out of it): the caller then shows its glyph placeholder, which is
/// honest, rather than a broken `<img>`.
#[must_use]
pub fn media_asset_src(location: Option<&str>) -> Option<String> {
    let location = location?;
    if location.starts_with("http://") || location.starts_with("https://") {
        return Some(location.to_owned());
    }
    let rel = genealogy_app::media_root_relative(location)?;
    Some(format!("/{}/{rel}", genealogy_app::MEDIA_DIR))
}

/// Whether a media object should render as an image rather than a document glyph.
///
/// A recorded MIME is an operator's assertion and wins outright, so `application/pdf` on a `.jpg`
/// stays a document. Only when no MIME is recorded does the location's extension decide
/// ([`mime_for_path`](genealogy_app::mime_for_path)) — nothing writes an inferred MIME back into the
/// record, so this is a display rule, not a stored one.
#[must_use]
pub fn media_is_image(mime: Option<&str>, location: Option<&str>) -> bool {
    let mime = match mime {
        Some(mime) => mime,
        None => match location.and_then(genealogy_app::mime_for_path) {
            Some(guessed) => guessed,
            None => return false,
        },
    };
    mime.starts_with("image/")
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
        backs_count: reference.backs_count,
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        evidence_axes: evidence_axes(reference.analysis.as_ref(), loc),
        asserted_by: reference.asserted_by.as_ref().map(|who| {
            let who = match reference.asserted_by_kind {
                Some(kind) => loc.agent_name_with_kind(who, kind),
                None => who.clone(),
            };
            let when = reference.asserted_at.as_deref().map(friendly_timestamp);
            loc.provenance_asserted_by(&who, when.as_deref())
        }),
        assertion_id: reference.assertion_id.clone(),
    }
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

#[cfg(test)]
mod line_label_tests {
    use super::{LABEL_MAX_CHARS, line_label};

    #[test]
    fn the_first_non_empty_line_becomes_the_label() {
        assert_eq!(
            line_label("\n   \nAda's baptism\nmore prose\n"),
            Some("Ada's baptism".to_owned()),
            "leading blank lines are skipped and the line is trimmed"
        );
    }

    #[test]
    fn a_markdown_heading_loses_its_hashes() {
        assert_eq!(line_label("## Ada's baptism"), Some("Ada's baptism".to_owned()));
    }

    #[test]
    fn text_with_no_content_has_no_label() {
        assert_eq!(line_label(""), None, "no line at all");
        assert_eq!(line_label("\n  \n\t\n"), None, "only blank lines");
        assert_eq!(line_label("###"), None, "a heading marker with no heading");
    }

    #[test]
    fn a_long_line_is_capped_without_splitting_a_character() {
        // A cap taken in bytes would panic (or produce invalid UTF-8) mid-character; taken in `chars`
        // it cannot. Each `æ` is two bytes, so the byte length exceeds the char cap.
        let long = "æ".repeat(LABEL_MAX_CHARS + 10);
        let label = line_label(&long).expect("a label");
        assert_eq!(label.chars().count(), LABEL_MAX_CHARS, "capped by characters");
        assert_eq!(label, "æ".repeat(LABEL_MAX_CHARS));
    }

    #[test]
    fn a_line_at_the_cap_is_kept_whole() {
        let exact = "a".repeat(LABEL_MAX_CHARS);
        assert_eq!(line_label(&exact), Some(exact));
    }
}

#[cfg(test)]
mod media_asset_tests {
    use super::{MediaRefVm, media_asset_src, media_is_image};

    fn media_ref(path: Option<&str>, mime: Option<&str>) -> MediaRefVm {
        MediaRefVm {
            human_id: "O0001".to_owned(),
            assertion_id: "0190-attach-id".to_owned(),
            caption: None,
            crop: None,
            path: path.map(str::to_owned),
            mime: mime.map(str::to_owned),
        }
    }

    #[test]
    fn a_stored_path_keeps_exactly_one_media_prefix() {
        // #301 cause 2: the stored form already carries `media/`, so prepending it unconditionally
        // asked the asset handler for `/media/media/…`, which resolves to nothing.
        assert_eq!(
            media_asset_src(Some("media/portraits/ada.jpg")).as_deref(),
            Some("/media/portraits/ada.jpg")
        );
    }

    #[test]
    fn a_bare_root_relative_path_gets_the_prefix() {
        assert_eq!(
            media_asset_src(Some("portraits/ada.jpg")).as_deref(),
            Some("/media/portraits/ada.jpg")
        );
    }

    #[test]
    fn a_web_reference_is_used_verbatim() {
        assert_eq!(
            media_asset_src(Some("https://example.org/img.png")).as_deref(),
            Some("https://example.org/img.png")
        );
        assert_eq!(
            media_asset_src(Some("http://example.org/img.png")).as_deref(),
            Some("http://example.org/img.png")
        );
    }

    #[test]
    fn a_location_the_media_root_cannot_serve_has_no_source() {
        assert_eq!(media_asset_src(None), None, "no location at all");
        assert_eq!(
            media_asset_src(Some("/home/ada/photos/ada.jpg")),
            None,
            "a file outside the workspace is not served — the glyph placeholder is honest"
        );
        assert_eq!(media_asset_src(Some("media/../secret.txt")), None, "traversal");
        assert_eq!(media_asset_src(Some("")), None, "an empty location");
    }

    #[test]
    fn a_recorded_image_mime_wins() {
        assert!(media_is_image(Some("image/jpeg"), Some("media/portraits/ada.jpg")));
        assert!(
            media_is_image(Some("image/png"), None),
            "no location needed to classify"
        );
    }

    #[test]
    fn a_recorded_non_image_mime_beats_the_extension() {
        assert!(
            !media_is_image(Some("application/pdf"), Some("media/scans/deed.jpg")),
            "an operator's recorded MIME is the assertion; the extension only fills a gap"
        );
    }

    #[test]
    fn an_absent_mime_falls_back_to_the_extension() {
        // #301 cause 1: nothing in the workspace infers a MIME and the CLI cannot set one, so every
        // record created without one rendered the glyph forever.
        assert!(media_is_image(None, Some("media/portraits/ada.jpg")));
        assert!(!media_is_image(None, Some("media/deeds/deed.pdf")));
        assert!(!media_is_image(None, Some("media/misc/noext")));
        assert!(!media_is_image(None, None));
    }

    #[test]
    fn a_media_ref_delegates_to_the_shared_rules() {
        let item = media_ref(Some("media/portraits/ada.jpg"), None);
        assert_eq!(item.src().as_deref(), Some("/media/portraits/ada.jpg"));
        assert!(item.is_image());
    }
}
