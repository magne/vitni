//! Citation subcommands.

use clap::Subcommand;
use vitni_app::{
    AppError, DateParts, EvidenceAnalysis, MediaRefInput, MutationMeta, NewCitation, Provenance, Session, Workspace,
    add_citation_attribute, assert_citation_date, attach_citation_media, attach_citation_note, create_citation,
    list_citations, set_citation_confidence, set_citation_evidence_analysis, set_page, show_citation, tag_citation,
};

use crate::args::{ConfidenceArg, EvidenceKindArg, InformationKindArg, SourceQualityArg};
use crate::i18n::Localizer;

/// Citation subcommands.
#[derive(Subcommand)]
pub enum CitationCmd {
    /// Create a new citation against a source (auto-assigns a human id unless `--id` is given).
    Create {
        /// The cited source's human id (e.g. `S0001`).
        #[arg(long, value_name = "SOURCE_ID")]
        source: String,
        /// A specific human id (e.g. `C0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// An initial page / locator within the source.
        #[arg(long)]
        page: Option<String>,
    },
    /// Set (or change) an existing citation's page / locator.
    SetPage {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The page / locator text.
        page: String,
    },
    /// Assert the date of the cited record (Gregorian; year required, month/day optional).
    AssertDate {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The year (negative for BCE).
        #[arg(long)]
        year: i32,
        /// The month, 1–12.
        #[arg(long)]
        month: Option<u8>,
        /// The day, 1–31.
        #[arg(long)]
        day: Option<u8>,
    },
    /// Set (or change) the operator's confidence in the citation.
    SetConfidence {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The confidence level.
        #[arg(long, value_enum)]
        confidence: ConfidenceArg,
    },
    /// Set (or change) the citation's evidence analysis (the *Evidence Explained* axes).
    SetEvidenceAnalysis {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The source axis (original vs derivative).
        #[arg(long)]
        source: SourceQualityArg,
        /// The information axis (primary vs secondary).
        #[arg(long)]
        information: InformationKindArg,
        /// The evidence axis (direct / indirect / negative).
        #[arg(long)]
        evidence: EvidenceKindArg,
    },
    /// Add a typed attribute to the citation.
    AddAttribute {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The attribute name / type.
        attribute_type: String,
        /// The attribute value.
        value: String,
    },
    /// Attach a media object to the citation.
    AttachMedia {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The media object's human id (e.g. `O0001`).
        #[arg(long)]
        media: String,
        /// A caption specific to this use.
        #[arg(long)]
        caption: Option<String>,
    },
    /// Attach a note to the citation.
    AttachNote {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The note's human id (e.g. `N0001`).
        #[arg(long)]
        note: String,
    },
    /// Apply a tag to the citation, by tag name.
    Tag {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The tag's name.
        #[arg(long)]
        tag: String,
    },
    /// Remove a tag from the citation, by tag name.
    Untag {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The tag's name.
        #[arg(long)]
        tag: String,
    },
    /// Show one citation.
    Show {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
    },
    /// List all citations.
    List,
}

/// Runs a citation subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: CitationCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let meta = MutationMeta::default();
    match command {
        CitationCmd::Create { source, id, page } => {
            let human_id = create_citation(
                workspace,
                session,
                NewCitation {
                    human_id: id,
                    source,
                    page,
                },
                Provenance::default(),
                &[],
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        CitationCmd::SetPage { human_id, page } => {
            set_page(workspace, session, &human_id, page, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::AssertDate {
            human_id,
            year,
            month,
            day,
        } => {
            assert_citation_date(workspace, session, &human_id, DateParts { year, month, day }, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::SetConfidence { human_id, confidence } => {
            set_citation_confidence(workspace, session, &human_id, confidence.into(), meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::SetEvidenceAnalysis {
            human_id,
            source,
            information,
            evidence,
        } => {
            let analysis = EvidenceAnalysis {
                source: source.into(),
                information: information.into(),
                evidence: evidence.into(),
            };
            set_citation_evidence_analysis(workspace, session, &human_id, analysis, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => {
            add_citation_attribute(workspace, session, &human_id, attribute_type, value, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::AttachMedia {
            human_id,
            media,
            caption,
        } => {
            let input = MediaRefInput { crop: None, caption };
            attach_citation_media(workspace, session, &human_id, &media, input, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::AttachNote { human_id, note } => {
            attach_citation_note(workspace, session, &human_id, &note, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::Tag { human_id, tag } => {
            tag_citation(workspace, session, &human_id, &tag, false, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::Untag { human_id, tag } => {
            tag_citation(workspace, session, &human_id, &tag, true, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        CitationCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one citation, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_citation(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.citation_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::CitationNotFound(human_id.to_owned())),
    }
}

/// Renders every citation, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let citations = list_citations(workspace).await?;
    if citations.is_empty() {
        println!("{}", localizer.citation_list_empty());
        return Ok(());
    }
    for summary in &citations {
        println!("{}", localizer.citation_summary_line(summary));
    }
    Ok(())
}
