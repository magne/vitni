//! Family subcommands.

use clap::Subcommand;
use vitni_app::{
    AppError, ChildParentRelationship, ListedKind, MatchableKind, MutationMeta, Provenance, Session, Workspace,
    add_child, add_partner, create_family, list_families_page, remove_child, remove_partner, set_family_restrictions,
    show_family,
};

use crate::args::{ListArgs, RelationshipArg, RestrictionsArgs};
use crate::commands::similar::hint_similar;
use crate::i18n::Localizer;

/// Family subcommands.
#[derive(Subcommand)]
pub enum FamilyCmd {
    /// Create a new family (auto-assigns a human id).
    Create,
    /// Add a partner (by person human id) to a family.
    AddPartner {
        /// The family's human id (e.g. `F0001`).
        family_id: String,
        /// The partner's person human id (e.g. `I0001`).
        person_id: String,
    },
    /// Remove a partner (by person human id) from a family.
    RemovePartner {
        /// The family's human id (e.g. `F0001`).
        family_id: String,
        /// The partner's person human id (e.g. `I0001`).
        person_id: String,
    },
    /// Add a child (by person human id) to a family.
    AddChild {
        /// The family's human id (e.g. `F0001`).
        family_id: String,
        /// The child's person human id (e.g. `I0002`).
        person_id: String,
        /// How the child relates to the family's parents.
        #[arg(long, value_enum, default_value_t = RelationshipArg::Birth)]
        relationship: RelationshipArg,
    },
    /// Remove a child (by person human id) from a family.
    RemoveChild {
        /// The family's human id (e.g. `F0001`).
        family_id: String,
        /// The child's person human id (e.g. `I0002`).
        person_id: String,
    },
    /// Set (or clear) a family's privacy restrictions, replacing its current set.
    SetRestrictions {
        /// The family's human id (e.g. `F0001`).
        human_id: String,
        #[command(flatten)]
        restrictions: RestrictionsArgs,
    },
    /// Show one family.
    Show {
        /// The family's human id (e.g. `F0001`).
        human_id: String,
    },
    /// List the families, narrowed by `--search` and `--filter`, ordered by `--sort` or else by human id;
    /// all of them or one window.
    List(ListArgs),
}

/// Runs a family subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: FamilyCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        FamilyCmd::Create => {
            let human_id = create_family(workspace, session, Provenance::default(), &[]).await?;
            println!("{}", localizer.created(&human_id));
            hint_similar(workspace, MatchableKind::Family, &human_id, localizer).await;
            Ok(())
        }
        FamilyCmd::AddPartner { family_id, person_id } => {
            add_partner(workspace, session, &family_id, &person_id, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::RemovePartner { family_id, person_id } => {
            remove_partner(workspace, session, &family_id, &person_id, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::AddChild {
            family_id,
            person_id,
            relationship,
        } => {
            // The relationship is recorded per family partner; apply the chosen relationship to each
            // current partner of the family (data-model §6 — GEDCOM `_FREL`/`_MREL`).
            let relationship: ChildParentRelationship = relationship.into();
            let relationships = match show_family(workspace, &family_id).await? {
                Some(summary) => summary
                    .partners
                    .into_iter()
                    .map(|partner| (partner.human_id, relationship.clone()))
                    .collect(),
                None => return Err(AppError::FamilyNotFound(family_id)),
            };
            add_child(
                workspace,
                session,
                &family_id,
                &person_id,
                relationships,
                MutationMeta::default(),
            )
            .await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::RemoveChild { family_id, person_id } => {
            remove_child(workspace, session, &family_id, &person_id, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::SetRestrictions { human_id, restrictions } => {
            restrict(workspace, session, &human_id, restrictions, localizer).await
        }
        FamilyCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        FamilyCmd::List(window) => list(workspace, &window, localizer).await,
    }
}

/// Replaces a family's privacy restrictions and reports the update.
async fn restrict(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: RestrictionsArgs,
    localizer: &Localizer,
) -> Result<(), AppError> {
    set_family_restrictions(
        workspace,
        session,
        human_id,
        restrictions.into_set(),
        MutationMeta::default(),
    )
    .await?;
    println!("{}", localizer.updated(human_id));
    Ok(())
}

/// Renders one family, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_family(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.family_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::FamilyNotFound(human_id.to_owned())),
    }
}

/// Renders the `page` of family records, ordered by human id.
async fn list(workspace: &Workspace, window: &ListArgs, localizer: &Localizer) -> Result<(), AppError> {
    let query = window.query(workspace, ListedKind::Family).await?;
    let page = window.page();
    let families = list_families_page(workspace, &query, page).await?.items;
    if families.is_empty() {
        println!(
            "{}",
            localizer.empty_page(page, window.narrows(), localizer.family_list_empty())
        );
        return Ok(());
    }
    for summary in &families {
        println!("{}", localizer.family_summary_line(summary));
    }
    Ok(())
}
