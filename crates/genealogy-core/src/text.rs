//! Text, media-reference, and external-identifier value objects (data-model §7).

use serde::{Deserialize, Serialize};

use crate::ids::MediaId;
use crate::name::LanguageTag;
use crate::provenance::EvidenceRef;

/// The media type of a [`RichText`] body (data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MediaType {
    /// CommonMark Markdown (the default).
    #[default]
    Markdown,
    /// Plain text.
    Plain,
    /// HTML.
    Html,
}

/// Note / free-text content: Markdown by default, language-tagged (data-model §7, §14).
///
/// Replaces Gramps' offset-range styled text: Markdown-as-text is more expressive, diffs cleanly,
/// and needs no fragile span bookkeeping. Typed links to aggregates use the documented
/// `x-genealogy:` URI scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichText {
    /// The text content.
    pub text: String,
    /// How `text` is interpreted.
    pub media_type: MediaType,
    /// The language of the content.
    pub language: Option<LanguageTag>,
    /// Who produced this text, when it is a translation (the parent's `translations`). `None` for an
    /// original. Defaults to empty so notes stored before this field existed still decode (ADR 0004 §4).
    #[serde(default)]
    pub translator: Option<String>,
    /// Translations of this same content into other languages (GEDCOM `NOTE`.`TRAN`; mirrors
    /// [`PersonName`](crate::name::PersonName) transliterations). Defaults to empty so notes
    /// stored before this field existed still decode (ADR 0004 §4).
    #[serde(default)]
    pub translations: Vec<RichText>,
}

/// A typed external URL (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Url {
    /// The kind of URL (e.g. `home page`, `email`).
    pub url_type: Option<String>,
    /// The URL itself.
    pub href: String,
    /// A human-readable description.
    pub description: Option<String>,
}

/// A typed key/value attribute (data-model §7).
///
/// Citations backing the enclosing claim live on the assertion envelope
/// (`EventContext.citations`), the sole evidence channel (ADR 0020) — not on the attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    /// The attribute name/type.
    pub attribute_type: String,
    /// The attribute value.
    pub value: String,
}

/// A rectangular crop region within a media file (a face in a group photo, say).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge, as a percentage 0–100 of width.
    pub left: u8,
    /// Top edge, as a percentage 0–100 of height.
    pub top: u8,
    /// Width, as a percentage 0–100.
    pub width: u8,
    /// Height, as a percentage 0–100.
    pub height: u8,
}

/// The *use* of a shared `Media` aggregate at one attachment point (data-model §7).
///
/// Many objects reference the same `Media`; each `MediaRef` adds context for *this* use — an
/// optional crop, a caption, and citations — keeping per-use detail off the shared file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRef {
    /// The referenced `Media` aggregate.
    pub media_id: MediaId,
    /// An optional crop/region of interest.
    pub crop: Option<Rect>,
    /// A caption specific to this use.
    pub caption: Option<String>,
    /// Evidence for using the media here (in practice citations — ADR 0020 §3, ADR 0023).
    pub citations: Vec<EvidenceRef>,
}

/// A stable identifier in an external system (data-model §7, §11).
///
/// Held on any aggregate sourced or matched externally; it is what makes re-import idempotent
/// and enables sync, deduplication, and a provenance back-link. Maps to GEDCOM 7 `EXID`/`UID`
/// and GEDCOM X `identifiers`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalId {
    /// The external authority (e.g. `FamilySearch`, `Digitalarkivet`).
    pub authority: String,
    /// The identifier value within that authority.
    pub value: String,
    /// An optional kind/qualifier of the identifier.
    pub kind: Option<String>,
    /// An optional URL to the origin record.
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{MediaType, RichText};
    use crate::name::LanguageTag;

    #[test]
    fn markdown_is_the_default_media_type() {
        assert_eq!(MediaType::default(), MediaType::Markdown);
    }

    #[test]
    fn rich_text_round_trips_through_json() {
        let text = RichText {
            text: "Born in **Bergen**.".to_owned(),
            media_type: MediaType::Markdown,
            language: Some(LanguageTag::new("en")),
            translator: None,
            translations: Vec::new(),
        };
        let json = serde_json::to_string(&text).unwrap();
        let back: RichText = serde_json::from_str(&json).unwrap();
        assert_eq!(text, back);
    }

    #[test]
    fn rich_text_with_translations_round_trips() {
        let text = RichText {
            text: "Født i Bergen.".to_owned(),
            media_type: MediaType::Markdown,
            language: Some(LanguageTag::new("nb")),
            translator: None,
            translations: vec![RichText {
                text: "Born in Bergen.".to_owned(),
                media_type: MediaType::Markdown,
                language: Some(LanguageTag::new("en")),
                translator: Some("magne".to_owned()),
                translations: Vec::new(),
            }],
        };
        let json = serde_json::to_string(&text).unwrap();
        let back: RichText = serde_json::from_str(&json).unwrap();
        assert_eq!(text, back);
    }

    #[test]
    fn historical_rich_text_without_translations_decodes() {
        // A note stored before `translations` existed has no such key (ADR 0004 §4 additive rule).
        let json = r#"{ "text": "Born in Bergen.", "media_type": "Markdown", "language": null }"#;
        let text: RichText = serde_json::from_str(json).unwrap();
        assert!(text.translations.is_empty());
    }
}
