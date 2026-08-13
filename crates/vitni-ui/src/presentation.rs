//! Presentation-only render enums shared across framework renderers (ADR 0008).
//!
//! These mirror the data-model's evidence/privacy semantics for display purposes and expose the
//! stable token a stylesheet keys on, so a renderer never hardcodes the string. They carry no
//! display text (a localized label is supplied per use, keeping colour-not-alone honest) and no
//! `vitni-app`/domain dependency yet — a renderer maps a DTO value to one of these. Conversions
//! from the core types are added when a consumer needs them.

use serde::{Deserialize, Serialize};

pub use vitni_app::{EvidenceKind, InformationKind, SourceQuality};

/// A surety level for an asserted fact, shown as a confidence badge (colour + label).
///
/// [`Self::data_level`] returns the value used in the `data-level` attribute the confidence-badge
/// CSS keys on (`.conf[data-level="…"]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfidenceLevel {
    /// Lowest surety.
    VeryLow,
    /// Low surety.
    Low,
    /// Middling surety.
    Normal,
    /// High surety.
    High,
    /// Highest surety.
    VeryHigh,
}

impl ConfidenceLevel {
    /// Every level, lowest to highest (for building a confidence picker).
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [Self::VeryLow, Self::Low, Self::Normal, Self::High, Self::VeryHigh]
    }

    /// The stable `data-level` token this level renders with.
    #[must_use]
    pub fn data_level(self) -> &'static str {
        match self {
            Self::VeryLow => "very-low",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::VeryHigh => "very-high",
        }
    }
}

/// One axis of the Evidence Explained analysis model, shown as an evidence chip.
///
/// Each axis gets a stable hue via its CSS class ([`Self::css_class`]); the chip's text is the
/// axis *value* (e.g. "original", "primary", "direct"), supplied per use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceAxis {
    /// Source axis: original vs derivative.
    Source,
    /// Information axis: primary vs secondary.
    Information,
    /// Evidence axis: direct, indirect, or negative.
    Evidence,
}

impl EvidenceAxis {
    /// The stable CSS class this axis renders with (`.ev.<class>`).
    #[must_use]
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Information => "info",
            Self::Evidence => "evidence",
        }
    }
}

/// A privacy restriction on a record (GEDCOM v7 `RESN`), shown as a multi-select toggle.
///
/// [`Self::data_kind`] returns the value used in the `data-kind` attribute the restriction-toggle
/// CSS keys on (`.resn[data-kind="…"]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestrictionKind {
    /// Confidential: hide from general view.
    Confidential,
    /// Locked: protected from edits.
    Locked,
    /// Privacy: living-person privacy.
    Privacy,
}

impl RestrictionKind {
    /// Every restriction kind (for building the multi-select toggle set).
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Confidential, Self::Locked, Self::Privacy]
    }

    /// The stable `data-kind` token this restriction renders with.
    #[must_use]
    pub fn data_kind(self) -> &'static str {
        match self {
            Self::Confidential => "confidential",
            Self::Locked => "locked",
            Self::Privacy => "privacy",
        }
    }
}

impl From<vitni_app::Restriction> for RestrictionKind {
    fn from(restriction: vitni_app::Restriction) -> Self {
        match restriction {
            vitni_app::Restriction::Confidential => Self::Confidential,
            vitni_app::Restriction::Locked => Self::Locked,
            vitni_app::Restriction::Privacy => Self::Privacy,
        }
    }
}

impl From<RestrictionKind> for vitni_app::Restriction {
    fn from(kind: RestrictionKind) -> Self {
        match kind {
            RestrictionKind::Confidential => Self::Confidential,
            RestrictionKind::Locked => Self::Locked,
            RestrictionKind::Privacy => Self::Privacy,
        }
    }
}

/// Every source-quality axis value, for building the evidence-analysis picker (mirrors
/// [`RestrictionKind::all`]).
pub const SOURCE_QUALITIES: [vitni_app::SourceQuality; 2] =
    [vitni_app::SourceQuality::Original, vitni_app::SourceQuality::Derivative];

/// Every information-kind axis value, for building the evidence-analysis picker.
pub const INFORMATION_KINDS: [vitni_app::InformationKind; 2] = [
    vitni_app::InformationKind::Primary,
    vitni_app::InformationKind::Secondary,
];

/// Every evidence-kind axis value, for building the evidence-analysis picker.
pub const EVIDENCE_KINDS: [vitni_app::EvidenceKind; 3] = [
    vitni_app::EvidenceKind::Direct,
    vitni_app::EvidenceKind::Indirect,
    vitni_app::EvidenceKind::Negative,
];

impl From<vitni_app::Confidence> for ConfidenceLevel {
    fn from(confidence: vitni_app::Confidence) -> Self {
        match confidence {
            vitni_app::Confidence::VeryLow => Self::VeryLow,
            vitni_app::Confidence::Low => Self::Low,
            vitni_app::Confidence::Normal => Self::Normal,
            vitni_app::Confidence::High => Self::High,
            vitni_app::Confidence::VeryHigh => Self::VeryHigh,
        }
    }
}

impl From<ConfidenceLevel> for vitni_app::Confidence {
    fn from(level: ConfidenceLevel) -> Self {
        match level {
            ConfidenceLevel::VeryLow => Self::VeryLow,
            ConfidenceLevel::Low => Self::Low,
            ConfidenceLevel::Normal => Self::Normal,
            ConfidenceLevel::High => Self::High,
            ConfidenceLevel::VeryHigh => Self::VeryHigh,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfidenceLevel, EvidenceAxis, RestrictionKind};

    #[test]
    fn confidence_levels_map_to_css_tokens() {
        assert_eq!(ConfidenceLevel::VeryLow.data_level(), "very-low");
        assert_eq!(ConfidenceLevel::Normal.data_level(), "normal");
        assert_eq!(ConfidenceLevel::VeryHigh.data_level(), "very-high");
    }

    #[test]
    fn evidence_axes_map_to_css_classes() {
        assert_eq!(EvidenceAxis::Source.css_class(), "source");
        assert_eq!(EvidenceAxis::Information.css_class(), "info");
        assert_eq!(EvidenceAxis::Evidence.css_class(), "evidence");
    }

    #[test]
    fn restriction_kinds_map_to_css_tokens() {
        assert_eq!(RestrictionKind::Confidential.data_kind(), "confidential");
        assert_eq!(RestrictionKind::Locked.data_kind(), "locked");
        assert_eq!(RestrictionKind::Privacy.data_kind(), "privacy");
    }

    #[test]
    fn confidence_round_trips_through_the_app_type() {
        for level in ConfidenceLevel::all() {
            let confidence: vitni_app::Confidence = level.into();
            assert_eq!(ConfidenceLevel::from(confidence), level);
        }
    }

    #[test]
    fn restriction_round_trips_through_the_app_type() {
        for kind in RestrictionKind::all() {
            let restriction: vitni_app::Restriction = kind.into();
            assert_eq!(RestrictionKind::from(restriction), kind);
        }
    }
}
