//! `DnaTest` subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, MutationMeta, NewDnaTest, Provenance, Session, Workspace, assert_dna_test_haplogroup,
    attach_dna_test_note, create_dna_test, list_dna_tests, set_dna_test_genome_build, set_dna_test_kit_id,
    set_dna_test_provider, set_dna_test_type, show_dna_test, tag_dna_test,
};
use genealogy_core::ids::NoteId;
use uuid::Uuid;

use crate::args::{DnaGenomeBuildArg, DnaProviderArg, DnaTestTypeArg};
use crate::i18n::Localizer;

/// `DnaTest` subcommands.
#[derive(Subcommand)]
pub enum DnaTestCmd {
    /// Create a new DNA test anchored to a person (auto-assigns a human id unless `--id` is given).
    Create {
        /// The anchoring person's human id (e.g. `I0001`).
        #[arg(long, value_name = "PERSON_ID")]
        person: String,
        /// A specific human id (e.g. `D0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
    },
    /// Set (or change) a test's provider.
    SetProvider {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The provider.
        #[arg(long)]
        provider: DnaProviderArg,
    },
    /// Set (or change) a test's kit id.
    SetKit {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The kit id.
        kit_id: String,
    },
    /// Set (or change) a test's type.
    SetType {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The test type.
        #[arg(long = "type")]
        test_type: DnaTestTypeArg,
    },
    /// Set (or change) a test's genome build.
    SetBuild {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The genome build.
        #[arg(long)]
        build: DnaGenomeBuildArg,
    },
    /// Assert a haplogroup on a test.
    AddHaplogroup {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The haplogroup (e.g. `R-M269`).
        haplogroup: String,
    },
    /// Attach a note to a test.
    AttachNote {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The note aggregate id (UUID).
        #[arg(long)]
        note: Uuid,
    },
    /// Apply a tag to a test.
    Tag {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from a test.
    Untag {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Show one DNA test.
    Show {
        /// The test's human id (e.g. `D0001`).
        human_id: String,
    },
    /// List all DNA tests.
    List,
}

/// Runs a `DnaTest` subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: DnaTestCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        DnaTestCmd::Create { person, id } => {
            let human_id = create_dna_test(
                workspace,
                session,
                NewDnaTest { human_id: id, person },
                Provenance::default(),
                &[],
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        DnaTestCmd::SetProvider { human_id, provider } => {
            set_dna_test_provider(workspace, session, &human_id, provider.into(), MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::SetKit { human_id, kit_id } => {
            set_dna_test_kit_id(workspace, session, &human_id, kit_id, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::SetType { human_id, test_type } => {
            set_dna_test_type(workspace, session, &human_id, test_type.into(), MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::SetBuild { human_id, build } => {
            set_dna_test_genome_build(workspace, session, &human_id, build.into(), MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::AddHaplogroup { human_id, haplogroup } => {
            assert_dna_test_haplogroup(workspace, session, &human_id, haplogroup, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::AttachNote { human_id, note } => {
            attach_dna_test_note(
                workspace,
                session,
                &human_id,
                NoteId::from_uuid(note),
                MutationMeta::default(),
            )
            .await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::Tag { human_id, tag } => {
            tag_dna_test(
                workspace,
                session,
                &human_id,
                &tag.to_string(),
                false,
                MutationMeta::default(),
            )
            .await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::Untag { human_id, tag } => {
            tag_dna_test(
                workspace,
                session,
                &human_id,
                &tag.to_string(),
                true,
                MutationMeta::default(),
            )
            .await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        DnaTestCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        DnaTestCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one DNA test, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_dna_test(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.dna_test_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::DnaTestNotFound(human_id.to_owned())),
    }
}

/// Renders every DNA test, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let tests = list_dna_tests(workspace).await?;
    if tests.is_empty() {
        println!("{}", localizer.dna_test_list_empty());
        return Ok(());
    }
    for summary in &tests {
        println!("{}", localizer.dna_test_summary_line(summary));
    }
    Ok(())
}
