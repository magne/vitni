//! DNA evidence value objects and enumerated sets (data-model §7, §12).
//!
//! A DNA match is *observed data* (high data-surety); the relationship it implies is a separate,
//! lower-confidence assertion that cites the match (data-model §12), not a field here. The decimal
//! quantities (shared cM, shared percent, per-segment cM) are scaled integers — see [`crate::fixed`]
//! — so the value objects keep `Eq` and a byte-stable serialization.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::fixed::{ParseFixedError, fixed_decimal_display, parse_decimal};
use crate::ids::PersonId;

/// Shared DNA in centimorgans, stored as hundredths of a cM (0.01 cM resolution — data-model §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Centimorgans(i64);

impl Centimorgans {
    /// Hundredths of a centimorgan.
    const SCALE: u32 = 2;

    /// Wraps a count of hundredths of a centimorgan.
    #[must_use]
    pub const fn from_hundredths(hundredths: i64) -> Self {
        Self(hundredths)
    }

    /// Returns the value in hundredths of a centimorgan.
    #[must_use]
    pub const fn as_hundredths(self) -> i64 {
        self.0
    }
}

fixed_decimal_display!(Centimorgans, Centimorgans::SCALE);

impl FromStr for Centimorgans {
    type Err = ParseFixedError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_decimal(value, Self::SCALE).map(Self)
    }
}

/// A shared-DNA percentage, stored as ten-thousandths of a percent (data-model §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PercentShared(i64);

impl PercentShared {
    /// Ten-thousandths of a percent.
    const SCALE: u32 = 4;

    /// Wraps a count of ten-thousandths of a percent.
    #[must_use]
    pub const fn from_ten_thousandths(ten_thousandths: i64) -> Self {
        Self(ten_thousandths)
    }

    /// Returns the value in ten-thousandths of a percent.
    #[must_use]
    pub const fn as_ten_thousandths(self) -> i64 {
        self.0
    }
}

fixed_decimal_display!(PercentShared, PercentShared::SCALE);

impl FromStr for PercentShared {
    type Err = ParseFixedError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_decimal(value, Self::SCALE).map(Self)
    }
}

/// Which parental side a DNA segment is assigned to (data-model §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChromosomeSide {
    /// The maternal side.
    Maternal,
    /// The paternal side.
    Paternal,
    /// Unassigned / unknown.
    Unknown,
}

/// The kind of DNA test (data-model §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnaTestType {
    /// Autosomal (atDNA).
    Autosomal,
    /// Y-chromosome (paternal line).
    YDna,
    /// Mitochondrial (maternal line).
    MtDna,
    /// X-chromosome.
    XDna,
}

/// The reference human genome build a test was called against (data-model §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnaGenomeBuild {
    /// `GRCh37` / hg19.
    GRCh37,
    /// `GRCh38` / hg38.
    GRCh38,
}

/// The DNA testing provider (closed set plus a custom escape — data-model §7, §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum DnaProvider {
    /// `AncestryDNA`.
    AncestryDna,
    /// 23andMe.
    TwentyThreeAndMe,
    /// `MyHeritage` DNA.
    MyHeritage,
    /// `FamilyTreeDNA`.
    FamilyTreeDna,
    /// `GEDmatch`.
    GedMatch,
    /// Living DNA.
    LivingDna,
    /// An application-defined provider.
    Custom(String),
}

/// One shared DNA segment between two tests (data-model §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnaSegment {
    /// The chromosome (`1`..=`22` or `X`).
    pub chromosome: String,
    /// The start position (base pairs).
    pub start: u64,
    /// The end position (base pairs).
    pub end: u64,
    /// The segment length in centimorgans.
    pub centimorgans: Centimorgans,
    /// The number of matching SNPs, if known.
    pub snps: Option<u32>,
    /// The parental side, if phased.
    pub side: ChromosomeSide,
}

/// A reference to an inferred common ancestor of a DNA match (data-model §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedAncestor {
    /// The inferred common-ancestor person, if identified in this workspace.
    pub ancestor_person_id: Option<PersonId>,
    /// A free-text note describing the shared ancestry.
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{Centimorgans, ChromosomeSide, DnaProvider, DnaSegment, PercentShared};
    use std::str::FromStr;

    #[test]
    fn centimorgans_parse_and_render_exactly() {
        let value = Centimorgans::from_str("850.5").unwrap();
        assert_eq!(value.as_hundredths(), 85050);
        assert_eq!(value.to_string(), "850.5");
    }

    #[test]
    fn percent_shared_keeps_four_decimal_places() {
        let value = PercentShared::from_str("12.3456").unwrap();
        assert_eq!(value.as_ten_thousandths(), 123_456);
        assert_eq!(value.to_string(), "12.3456");
    }

    #[test]
    fn segment_round_trips_through_json() {
        let segment = DnaSegment {
            chromosome: "7".to_owned(),
            start: 1_000_000,
            end: 5_000_000,
            centimorgans: Centimorgans::from_hundredths(1234),
            snps: Some(2500),
            side: ChromosomeSide::Maternal,
        };
        let json = serde_json::to_string(&segment).unwrap();
        let back: DnaSegment = serde_json::from_str(&json).unwrap();
        assert_eq!(segment, back);
    }

    #[test]
    fn provider_custom_is_tagged() {
        let json = serde_json::to_value(DnaProvider::Custom("LocalLab".to_owned())).unwrap();
        assert_eq!(json["type"], "Custom");
        assert_eq!(json["value"], "LocalLab");
    }
}
