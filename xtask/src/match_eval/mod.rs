//! `cargo xtask match-eval` — the matching engine measured against a labelled corpus (ADR 0038 §9).
//!
//! The corpus under [`CORPUS_DIR`] holds pairs of person records labelled `same` or `distinct`:
//! invented ones, and transcriptions of public census and church records over 100 years old. Each
//! pair is run through what the app does with it: the blocking keys decide whether the pair is a
//! candidate at all, and `assess_persons` scores it. The report gives, per band, how many true and
//! false matches land there, the band's precision and the recall at or above it.
//!
//! The gate is on recall, not precision: every pair marked as a hard true match (a spelling variant,
//! a surname changed after a move, a census age off by a few years, a baptism standing in for a
//! birth) must be a candidate and score at least `Possible`, so a change to a comparator, weight or
//! pack that loses one fails. Each hard case must have at least one pair, or the gate would be
//! vacuous.

mod corpus;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use vitni_core::matching::{BlockingKeys, ENGINE_VERSION, MatchBand, MatchData, MatchSettings, Probe, assess_persons};

use crate::match_eval::corpus::{HardCase, Label, Pair};

/// The corpus directory, relative to the repository root.
pub const CORPUS_DIR: &str = "crates/vitni-core/matching/corpus";

/// The bands, best first, as the report lists them.
const BANDS: [MatchBand; 4] = [
    MatchBand::Deterministic,
    MatchBand::Probable,
    MatchBand::Possible,
    MatchBand::Unlikely,
];

/// Runs the evaluation from the repository root, failing if a hard case is lost.
pub fn run() -> Result<()> {
    let pairs = load(Path::new(CORPUS_DIR))?;
    let data = MatchData::embedded().context("loading the embedded matching data")?;
    let settings = MatchSettings::default();
    let mut results = Vec::new();
    for pair in &pairs {
        results.push(score(pair, &data, &settings));
    }
    print!("{}", Report::of(&results).render());
    gate(&results)?;
    println!("match-eval: ok, every hard case surfaces");
    Ok(())
}

/// Loads every `*.toml` corpus file under `dir` except its `PROVENANCE.toml` (ADR 0042), in name order,
/// rejecting an id used twice.
pub fn load(dir: &Path) -> Result<Vec<Pair>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading the corpus directory {}", dir.display()))? {
        let path = entry?.path();
        let declaration = path.file_name().is_some_and(|name| name == "PROVENANCE.toml");
        if path.extension().is_some_and(|extension| extension == "toml") && !declaration {
            files.push(path);
        }
    }
    files.sort();
    ensure!(!files.is_empty(), "no corpus files (*.toml) in {}", dir.display());
    let mut pairs = Vec::new();
    let mut ids = BTreeSet::new();
    for file in files {
        let text = fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        for pair in corpus::parse(&file.display().to_string(), &text)? {
            ensure!(
                ids.insert(pair.id.clone()),
                "pair id {:?} is used twice (again in {})",
                pair.id,
                file.display()
            );
            pairs.push(pair);
        }
    }
    Ok(pairs)
}

/// One pair's outcome: its band and score, and whether blocking kept it from being scored at all.
#[derive(Debug, Clone)]
pub struct Scored {
    /// The pair's id.
    pub id: String,
    /// The pair's label.
    pub label: Label,
    /// The hard case the pair exercises, if any.
    pub hard: Option<HardCase>,
    /// The band the engine put the pair in.
    pub band: MatchBand,
    /// The engine's score.
    pub score: f64,
    /// Whether the two records share no blocking key, so the app never scores the pair.
    pub blocked: bool,
}

impl Scored {
    /// Whether the app would show the pair: a candidate, at `Possible` or better.
    fn surfaces(&self) -> bool {
        !self.blocked && self.band >= MatchBand::Possible
    }
}

/// Blocks and scores one pair as the app would.
pub fn score(pair: &Pair, data: &MatchData, settings: &MatchSettings) -> Scored {
    let keys = BlockingKeys::new(data);
    let (left, right) = (keys.person(&pair.left), keys.person(&pair.right));
    let probe = Probe::of(&left);
    let assessment = assess_persons(&pair.left, &pair.right, data, settings);
    Scored {
        id: pair.id.clone(),
        label: pair.label,
        hard: pair.hard,
        band: assessment.band,
        score: assessment.score,
        blocked: !right.iter().any(|key| probe.meets(key)),
    }
}

/// The true and false matches landing in one band.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BandRow {
    /// Pairs labelled `same`.
    pub same: u32,
    /// Pairs labelled `distinct`.
    pub distinct: u32,
}

impl BandRow {
    /// The share of the band's pairs that are true matches; `None` for an empty band.
    pub fn precision(self) -> Option<f64> {
        ratio(self.same, self.same + self.distinct)
    }
}

/// The per-band tallies of a run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    rows: [BandRow; 4],
    /// True matches blocking kept from being scored.
    pub blocked_same: u32,
    /// False matches blocking kept from being scored.
    pub blocked_distinct: u32,
    /// Per hard case: the pairs that surface, and all its pairs.
    hard: [(u32, u32); 4],
    /// The outcome of every pair on the wrong side of `Possible`: a false match surfacing, or a
    /// true match not surfacing.
    pub misjudged: Vec<String>,
}

impl Report {
    /// Tallies `results`.
    pub fn of(results: &[Scored]) -> Self {
        let mut report = Self::default();
        for result in results {
            if let Some(hard) = result.hard {
                let tally = &mut report.hard[hard_index(hard)];
                tally.0 += u32::from(result.surfaces());
                tally.1 += 1;
            }
            if result.surfaces() != (result.label == Label::Same) {
                report.misjudged.push(outcome(result));
            }
            if result.blocked {
                match result.label {
                    Label::Same => report.blocked_same += 1,
                    Label::Distinct => report.blocked_distinct += 1,
                }
                continue;
            }
            let row = &mut report.rows[band_index(result.band)];
            match result.label {
                Label::Same => row.same += 1,
                Label::Distinct => row.distinct += 1,
            }
        }
        report
    }

    /// The pairs in `band`.
    pub fn row(&self, band: MatchBand) -> BandRow {
        self.rows[band_index(band)]
    }

    /// The share of all true matches scored at `band` or better; `None` with no true matches.
    pub fn recall_at(&self, band: MatchBand) -> Option<f64> {
        let mut found = 0;
        let mut total = self.blocked_same;
        for candidate in BANDS {
            let row = self.row(candidate);
            total += row.same;
            if candidate >= band {
                found += row.same;
            }
        }
        ratio(found, total)
    }

    /// The report as printed.
    pub fn render(&self) -> String {
        let (mut same, mut distinct) = (self.blocked_same, self.blocked_distinct);
        for band in BANDS {
            same += self.row(band).same;
            distinct += self.row(band).distinct;
        }
        let mut lines = vec![
            format!(
                "match-eval: {} pairs ({same} same, {distinct} distinct), engine v{}",
                same + distinct,
                ENGINE_VERSION.0
            ),
            "  band            same  distinct  precision  recall at or above".to_owned(),
        ];
        for band in BANDS {
            let row = self.row(band);
            lines.push(format!(
                "  {:<14} {:>5} {:>9} {:>10} {:>19}",
                band_name(band),
                row.same,
                row.distinct,
                percent(row.precision()),
                percent(self.recall_at(band)),
            ));
        }
        lines.push(format!(
            "  {:<14} {:>5} {:>9}",
            "blocked out", self.blocked_same, self.blocked_distinct
        ));
        let mut hard = Vec::new();
        for case in HardCase::ALL {
            let (surfacing, total) = self.hard[hard_index(case)];
            hard.push(format!("{} {surfacing}/{total}", case.as_str()));
        }
        lines.push(format!("  hard cases surfacing: {}", hard.join(", ")));
        for misjudged in &self.misjudged {
            lines.push(format!("  misjudged: {misjudged}"));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

/// Fails listing every hard case that does not surface, and every hard case with no pair.
pub fn gate(results: &[Scored]) -> Result<()> {
    let mut problems = Vec::new();
    for hard in HardCase::ALL {
        if !results.iter().any(|result| result.hard == Some(hard)) {
            problems.push(format!("the corpus has no {} pair", hard.as_str()));
        }
    }
    for result in results {
        let Some(hard) = result.hard else {
            continue;
        };
        if !result.surfaces() {
            problems.push(format!("{} ({})", outcome(result), hard.as_str()));
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  error: {problem}");
    }
    bail!(
        "match-eval: {} hard-case problem(s): {}",
        problems.len(),
        problems.join("; ")
    )
}

/// One pair's outcome in words: its id, label, and band and score or its loss to blocking.
fn outcome(result: &Scored) -> String {
    let label = match result.label {
        Label::Same => "same",
        Label::Distinct => "distinct",
    };
    if result.blocked {
        return format!("{} [{label}]: lost to blocking, the records share no key", result.id);
    }
    format!(
        "{} [{label}]: {} at {:.3}",
        result.id,
        band_name(result.band),
        result.score
    )
}

fn ratio(part: u32, whole: u32) -> Option<f64> {
    (whole > 0).then(|| f64::from(part) / f64::from(whole))
}

fn percent(rate: Option<f64>) -> String {
    rate.map_or_else(|| "-".to_owned(), |rate| format!("{:.1}%", rate * 100.0))
}

fn band_index(band: MatchBand) -> usize {
    match band {
        MatchBand::Deterministic => 0,
        MatchBand::Probable => 1,
        MatchBand::Possible => 2,
        MatchBand::Unlikely => 3,
    }
}

fn band_name(band: MatchBand) -> &'static str {
    match band {
        MatchBand::Deterministic => "deterministic",
        MatchBand::Probable => "probable",
        MatchBand::Possible => "possible",
        MatchBand::Unlikely => "unlikely",
    }
}

fn hard_index(hard: HardCase) -> usize {
    match hard {
        HardCase::SpellingVariant => 0,
        HardCase::SurnameAfterMove => 1,
        HardCase::CensusAge => 2,
        HardCase::BaptismForBirth => 3,
    }
}

#[cfg(test)]
mod tests;
