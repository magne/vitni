//! `DnaMatch` subcommands.

use clap::Subcommand;
use uuid::Uuid;
use vitni_app::{
    AppError, Centimorgans, DnaSegment, MutationMeta, NewDnaMatch, PercentShared, Provenance, Session, SharedAncestor,
    Workspace, add_dna_match_segment, assert_dna_match_shared_ancestor, attach_dna_match_note, list_dna_matches,
    observe_dna_match, set_dna_match_status, show_dna_match, tag_dna_match,
};
use vitni_core::ids::{NoteId, PersonId};

use crate::args::{ChromosomeSideArg, DnaProviderArg};
use crate::i18n::Localizer;

/// `DnaMatch` subcommands.
#[derive(Subcommand)]
pub enum DnaMatchCmd {
    /// Observe a match between two DNA tests (auto-assigns a human id unless `--id` is given).
    Observe {
        /// One side's test human id (e.g. `D0001`).
        #[arg(long = "test-a", value_name = "DNA_TEST_ID")]
        test_a: String,
        /// The other side's test human id (e.g. `D0002`).
        #[arg(long = "test-b", value_name = "DNA_TEST_ID")]
        test_b: String,
        /// The provider the match was observed at.
        #[arg(long)]
        provider: DnaProviderArg,
        /// Total shared centimorgans (decimal, e.g. `850.5`).
        #[arg(long = "shared-cm")]
        shared_cm: Centimorgans,
        /// Shared percentage (decimal), if reported.
        #[arg(long)]
        percent: Option<PercentShared>,
        /// The number of shared segments.
        #[arg(long, default_value_t = 0)]
        segments: u32,
        /// The largest shared segment's length in centimorgans (decimal).
        #[arg(long = "largest-cm", default_value = "0")]
        largest_cm: Centimorgans,
        /// The provider's predicted relationship.
        #[arg(long)]
        predicted: Option<String>,
        /// A specific human id (e.g. `X0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
    },
    /// Add a shared segment to a match.
    AddSegment {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
        /// The chromosome (`1`..=`22` or `X`).
        #[arg(long)]
        chromosome: String,
        /// The segment start (base pairs).
        #[arg(long)]
        start: u64,
        /// The segment end (base pairs).
        #[arg(long)]
        end: u64,
        /// The segment length in centimorgans (decimal).
        #[arg(long)]
        cm: Centimorgans,
        /// The number of matching SNPs.
        #[arg(long)]
        snps: Option<u32>,
        /// The parental side, if phased.
        #[arg(long, value_enum, default_value_t = ChromosomeSideArg::Unknown)]
        side: ChromosomeSideArg,
    },
    /// Assert an inferred shared ancestor on a match.
    AddAncestor {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
        /// The inferred common-ancestor person's aggregate id (UUID), if identified.
        #[arg(long)]
        person: Option<Uuid>,
        /// A free-text note describing the shared ancestry.
        #[arg(long)]
        note: Option<String>,
    },
    /// Confirm a match.
    Confirm {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
    },
    /// Reject a match.
    Reject {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
    },
    /// Attach a note to a match.
    AttachNote {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
        /// The note aggregate id (UUID).
        #[arg(long)]
        note: Uuid,
    },
    /// Apply a tag to a match.
    Tag {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from a match.
    Untag {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Show one DNA match.
    Show {
        /// The match's human id (e.g. `X0001`).
        human_id: String,
    },
    /// List all DNA matches.
    List,
}

/// Runs a `DnaMatch` subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: DnaMatchCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let meta = MutationMeta::default();
    match command {
        DnaMatchCmd::Observe {
            test_a,
            test_b,
            provider,
            shared_cm,
            percent,
            segments,
            largest_cm,
            predicted,
            id,
        } => {
            let human_id = observe_dna_match(
                workspace,
                session,
                NewDnaMatch {
                    human_id: id,
                    test_a,
                    test_b,
                    provider: provider.into(),
                    shared_cm,
                    percent_shared: percent,
                    segment_count: segments,
                    largest_segment_cm: largest_cm,
                    predicted_relationship: predicted,
                },
                Provenance::default(),
                &[],
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        DnaMatchCmd::AddSegment {
            human_id,
            chromosome,
            start,
            end,
            cm,
            snps,
            side,
        } => {
            let segment = DnaSegment {
                chromosome,
                start,
                end,
                centimorgans: cm,
                snps,
                side: side.into(),
            };
            add_dna_match_segment(workspace, session, &human_id, segment, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaMatchCmd::AddAncestor { human_id, person, note } => {
            let ancestor = SharedAncestor {
                ancestor_person_id: person.map(PersonId::from_uuid),
                note,
            };
            assert_dna_match_shared_ancestor(workspace, session, &human_id, ancestor, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaMatchCmd::Confirm { human_id } => {
            set_dna_match_status(workspace, session, &human_id, true, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaMatchCmd::Reject { human_id } => {
            set_dna_match_status(workspace, session, &human_id, false, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaMatchCmd::AttachNote { human_id, note } => {
            attach_dna_match_note(workspace, session, &human_id, NoteId::from_uuid(note), meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaMatchCmd::Tag { human_id, tag } => {
            tag_dna_match(workspace, session, &human_id, &tag.to_string(), false, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaMatchCmd::Untag { human_id, tag } => {
            tag_dna_match(workspace, session, &human_id, &tag.to_string(), true, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaMatchCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        DnaMatchCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one DNA match, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_dna_match(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.dna_match_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::DnaMatchNotFound(human_id.to_owned())),
    }
}

/// Renders every DNA match, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let matches = list_dna_matches(workspace).await?;
    if matches.is_empty() {
        println!("{}", localizer.dna_match_list_empty());
        return Ok(());
    }
    for summary in &matches {
        println!("{}", localizer.dna_match_summary_line(summary));
    }
    Ok(())
}
