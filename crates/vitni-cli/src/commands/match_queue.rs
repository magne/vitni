//! Match subcommands (ADR 0039 §3): the possible-matches review queue — every undecided pair the
//! matching engine proposes — and the two decisions that take a pair off it.

use clap::{Subcommand, ValueEnum};
use serde::Serialize;
use uuid::Uuid;
use vitni_app::{
    AppError, DecidableKind, IdentityDecision, ImportRunId, MatchBand, MatchQueueFilter, MatchVerdict, Provenance,
    QueuedMatch, Session, Workspace, assess, decide_match, match_pair_decision, match_queue,
};

use crate::args::ConfidenceArg;
use crate::i18n::Localizer;

/// CLI mirror of [`DecidableKind`].
#[derive(Clone, Copy, ValueEnum)]
pub enum KindArg {
    /// Persons.
    Person,
    /// Families.
    Family,
    /// Events.
    Event,
    /// Places.
    Place,
    /// Sources.
    Source,
    /// Repositories.
    Repository,
    /// Citations.
    Citation,
    /// Media objects.
    Media,
    /// Notes.
    Note,
}

impl From<KindArg> for DecidableKind {
    fn from(value: KindArg) -> Self {
        match value {
            KindArg::Person => Self::Person,
            KindArg::Family => Self::Family,
            KindArg::Event => Self::Event,
            KindArg::Place => Self::Place,
            KindArg::Source => Self::Source,
            KindArg::Repository => Self::Repository,
            KindArg::Citation => Self::Citation,
            KindArg::Media => Self::Media,
            KindArg::Note => Self::Note,
        }
    }
}

/// The least similar band a listed pair may have.
#[derive(Clone, Copy, ValueEnum)]
pub enum BandArg {
    /// Possible matches and better: the whole queue.
    Possible,
    /// Probable matches and better.
    Probable,
    /// Only matches established by a shared origin or external id.
    Deterministic,
}

impl From<BandArg> for MatchBand {
    fn from(value: BandArg) -> Self {
        match value {
            BandArg::Possible => Self::Possible,
            BandArg::Probable => Self::Probable,
            BandArg::Deterministic => Self::Deterministic,
        }
    }
}

/// Match subcommands.
#[derive(Subcommand)]
pub enum MatchCmd {
    /// List the undecided possible matches, the most similar first.
    List {
        /// Only pairs with a record this import run created (an id from `import-run list`).
        #[arg(long, value_name = "RUN_ID")]
        run: Option<Uuid>,
        /// Only pairs of this kind.
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
        /// Only pairs at least this similar.
        #[arg(long, value_enum, default_value = "possible")]
        band: BandArg,
        /// Print the queue as one JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Compare two records term by term, as the engine scores them.
    Show {
        /// The kind of both records.
        #[arg(value_enum)]
        kind: KindArg,
        /// The first record's id.
        first: String,
        /// The second record's id.
        second: String,
    },
    /// Decide the two records are one: the second is merged into the first.
    Same(Decision),
    /// Decide the two records are different, so the pair is never proposed again.
    Distinct(Decision),
}

/// The pair a decision is about, and the operator's judgment.
#[derive(clap::Args)]
pub struct Decision {
    /// The kind of both records.
    #[arg(value_enum)]
    kind: KindArg,
    /// The first record's id; it survives a `same`.
    first: String,
    /// The second record's id.
    second: String,
    /// How sure you are.
    #[arg(long, value_enum)]
    confidence: Option<ConfidenceArg>,
    /// Why you decided so.
    #[arg(long)]
    rationale: Option<String>,
}

/// One queued pair as `match list --json` prints it.
#[derive(Serialize)]
struct QueuedJson<'a> {
    kind: &'a str,
    a: &'a str,
    b: &'a str,
    band: &'a str,
    score: u8,
}

/// Runs a match subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: MatchCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        MatchCmd::List { run, kind, band, json } => {
            let filter = MatchQueueFilter {
                run: run.map(ImportRunId::from_uuid),
                kind: kind.map(Into::into),
                min_band: band.into(),
            };
            let queue = match_queue(workspace, &filter).await?;
            if json {
                println!("{}", queue_json(&queue));
                return Ok(());
            }
            if queue.is_empty() {
                println!("{}", localizer.match_list_empty());
            }
            for queued in &queue {
                println!("{}", localizer.match_line(queued));
            }
            Ok(())
        }
        MatchCmd::Show { kind, first, second } => {
            let kind = DecidableKind::from(kind);
            let assessment = assess(workspace, kind.matchable(), &first, &second).await?;
            let earlier = match_pair_decision(workspace, kind, &first, &second).await?;
            for line in localizer.match_show(kind, &first, &second, &assessment, earlier) {
                println!("{line}");
            }
            Ok(())
        }
        MatchCmd::Same(decision) => decide(workspace, session, decision, MatchVerdict::Same, localizer).await,
        MatchCmd::Distinct(decision) => decide(workspace, session, decision, MatchVerdict::Distinct, localizer).await,
    }
}

/// Records `verdict` on the pair, with the engine's current assessment as its evidence (ADR 0039 §2).
async fn decide(
    workspace: &Workspace,
    session: &Session,
    decision: Decision,
    verdict: MatchVerdict,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let Decision {
        kind,
        first,
        second,
        confidence,
        rationale,
    } = decision;
    let kind = DecidableKind::from(kind);
    let assessment = assess(workspace, kind.matchable(), &first, &second).await?;
    let decision = IdentityDecision {
        provenance: Provenance {
            confidence: confidence.map(Into::into),
            rationale,
            evidence_analysis: None,
            origin: None,
        },
        assessment: Some(assessment.evidence()),
    };
    decide_match(workspace, session, kind, &first, &second, verdict, decision).await?;
    println!("{}", localizer.match_decided(verdict, &first, &second));
    Ok(())
}

/// The queue as one JSON array, with stable untranslated keys.
fn queue_json(queue: &[QueuedMatch]) -> String {
    let mut rows = Vec::with_capacity(queue.len());
    for queued in queue {
        rows.push(QueuedJson {
            kind: queued.kind.as_str(),
            a: &queued.a.human_id,
            b: &queued.b.human_id,
            band: band_key(queued.assessment.band),
            score: queued.assessment.evidence().percent(),
        });
    }
    serde_json::to_string(&rows).unwrap_or_default()
}

/// The band's stable key.
fn band_key(band: MatchBand) -> &'static str {
    match band {
        MatchBand::Unlikely => "unlikely",
        MatchBand::Possible => "possible",
        MatchBand::Probable => "probable",
        MatchBand::Deterministic => "deterministic",
    }
}
