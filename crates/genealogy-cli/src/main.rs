//! The `genealogy` binary — a thin terminal frontend over `genealogy-app` (ADR 0006).
//!
//! This crate is I/O only: it parses arguments, resolves the workspace, calls a use-case, and
//! renders the result. All coordination — config, the operator identity, id/clock generation,
//! command execution — lives in `genealogy-app`. stdout/stderr are the interface, so the print
//! lints are relaxed for this crate only.

mod i18n;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use genealogy_app::config::{self, load, load_or_bootstrap};
use genealogy_app::{
    AppError, ChildParentRelationship, Config, DateParts, EventType, NewCitation, NewEvent, NewPerson, NewPlace,
    NewSource, ParticipantRole, PlaceType, Session, Workspace, add_child, add_name, add_partner, add_place_name,
    assert_event_date, assert_participation, create_citation, create_event, create_family, create_person, create_place,
    create_source, link_place, list_citations, list_events, list_families, list_persons, list_places, list_sources,
    remove_child, remove_partner, set_event_type, set_page, set_place_type, set_title, show_citation, show_event,
    show_family, show_person, show_place, show_source,
};
use genealogy_core::enums::EvidenceLevel;

use crate::i18n::Localizer;

/// Event-sourced genealogy at the command line.
#[derive(Parser)]
#[command(name = "genealogy", version, about)]
struct Cli {
    /// Workspace name (overrides the default and `GENEALOGY_WORKSPACE`).
    #[arg(long, global = true, value_name = "NAME")]
    workspace: Option<String>,
    #[command(subcommand)]
    command: Command,
}

/// Top-level commands.
#[derive(Subcommand)]
enum Command {
    /// Create and register a named workspace, bootstrapping configuration if needed.
    Init {
        /// The workspace name (e.g. `gen`).
        name: String,
        /// The workspace directory to create.
        path: PathBuf,
    },
    /// Operate on persons.
    Person {
        #[command(subcommand)]
        command: PersonCmd,
    },
    /// Operate on families.
    Family {
        #[command(subcommand)]
        command: FamilyCmd,
    },
    /// Operate on places.
    Place {
        #[command(subcommand)]
        command: PlaceCmd,
    },
    /// Operate on sources.
    Source {
        #[command(subcommand)]
        command: SourceCmd,
    },
    /// Operate on citations.
    Citation {
        #[command(subcommand)]
        command: CitationCmd,
    },
    /// Operate on events.
    Event {
        #[command(subcommand)]
        command: EventCmd,
    },
}

/// Person subcommands.
#[derive(Subcommand)]
enum PersonCmd {
    /// Create a new person (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `I0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// The given name(s).
        #[arg(long)]
        given: Option<String>,
        /// The surname.
        #[arg(long)]
        surname: Option<String>,
        /// Whether this is a persona or a conclusion.
        #[arg(long, value_enum, default_value_t = EvidenceArg::Conclusion)]
        evidence: EvidenceArg,
    },
    /// Assert an additional name on an existing person.
    AddName {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
        /// The given name(s).
        #[arg(long)]
        given: Option<String>,
        /// The surname.
        #[arg(long)]
        surname: Option<String>,
        /// A citation human id backing this name (repeatable); links the assertion's provenance to
        /// a real Citation aggregate (data-model §8).
        #[arg(long = "citation", value_name = "CITATION_ID")]
        citations: Vec<String>,
    },
    /// Assert that a person participated in an event, with a role.
    AddParticipation {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
        /// The event's human id (e.g. `E0001`).
        #[arg(long, value_name = "EVENT_ID")]
        event: String,
        /// The participant's role in the event.
        #[arg(long, value_enum, default_value_t = ParticipantRoleArg::Primary)]
        role: ParticipantRoleArg,
    },
    /// Show one person.
    Show {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
    },
    /// List all persons.
    List,
}

/// Place subcommands.
#[derive(Subcommand)]
enum PlaceCmd {
    /// Create a new place (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `P0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// The place's type.
        #[arg(long, value_enum, default_value_t = PlaceTypeArg::Parish)]
        r#type: PlaceTypeArg,
        /// An initial name for the place.
        #[arg(long)]
        name: Option<String>,
    },
    /// Set (or change) an existing place's type.
    SetType {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The new place type.
        #[arg(long, value_enum)]
        r#type: PlaceTypeArg,
    },
    /// Assert an additional name on an existing place.
    AddName {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The name to assert.
        name: String,
    },
    /// Show one place.
    Show {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
    },
    /// List all places.
    List,
}

/// Source subcommands.
#[derive(Subcommand)]
enum SourceCmd {
    /// Create a new source (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `S0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// An initial bibliographic title.
        #[arg(long)]
        title: Option<String>,
    },
    /// Set (or change) an existing source's title.
    SetTitle {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The bibliographic title.
        title: String,
    },
    /// Show one source.
    Show {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
    },
    /// List all sources.
    List,
}

/// Citation subcommands.
#[derive(Subcommand)]
enum CitationCmd {
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
    /// Show one citation.
    Show {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
    },
    /// List all citations.
    List,
}

/// Event subcommands.
#[derive(Subcommand)]
enum EventCmd {
    /// Create a new event (auto-assigns a human id unless `--id` is given).
    Create {
        /// The kind of event.
        #[arg(long, value_enum)]
        r#type: EventTypeArg,
        /// A specific human id (e.g. `E0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
    },
    /// Set (or change) an existing event's type.
    SetType {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The new event type.
        #[arg(long, value_enum)]
        r#type: EventTypeArg,
    },
    /// Assert when an event occurred (Gregorian; year required, month/day optional).
    AssertDate {
        /// The event's human id (e.g. `E0001`).
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
    /// Link an event to the place it occurred.
    LinkPlace {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The place's human id (e.g. `P0001`).
        place_id: String,
    },
    /// Show one event.
    Show {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
    },
    /// List all events.
    List,
}

/// Family subcommands.
#[derive(Subcommand)]
enum FamilyCmd {
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
    /// Show one family.
    Show {
        /// The family's human id (e.g. `F0001`).
        human_id: String,
    },
    /// List all families.
    List,
}

/// CLI mirror of [`EvidenceLevel`] (keeps clap's `ValueEnum` off the domain type).
#[derive(Clone, Copy, ValueEnum)]
enum EvidenceArg {
    /// A single-source persona.
    Persona,
    /// A researcher's conclusion.
    Conclusion,
}

impl From<EvidenceArg> for EvidenceLevel {
    fn from(value: EvidenceArg) -> Self {
        match value {
            EvidenceArg::Persona => Self::Persona,
            EvidenceArg::Conclusion => Self::Conclusion,
        }
    }
}

/// CLI mirror of [`ChildParentRelationship`] (keeps clap's `ValueEnum` off the domain type).
#[derive(Clone, Copy, ValueEnum)]
enum RelationshipArg {
    /// A biological / birth relationship.
    Birth,
    /// An adoptive relationship.
    Adopted,
    /// A foster relationship.
    Foster,
    /// A step relationship.
    Step,
    /// A sealed relationship (LDS).
    Sealed,
    /// An unknown / unrecorded relationship.
    Unknown,
}

impl From<RelationshipArg> for ChildParentRelationship {
    fn from(value: RelationshipArg) -> Self {
        match value {
            RelationshipArg::Birth => Self::Birth,
            RelationshipArg::Adopted => Self::Adopted,
            RelationshipArg::Foster => Self::Foster,
            RelationshipArg::Step => Self::Step,
            RelationshipArg::Sealed => Self::Sealed,
            RelationshipArg::Unknown => Self::Unknown,
        }
    }
}

/// CLI mirror of [`PlaceType`]'s closed variants (keeps clap's `ValueEnum` off the domain type).
/// The domain's `Custom` escape is not exposed on the CLI yet.
#[derive(Clone, Copy, ValueEnum)]
enum PlaceTypeArg {
    /// A country.
    Country,
    /// A first-level division (county, state, province).
    County,
    /// A municipality / kommune.
    Municipality,
    /// An ecclesiastical parish.
    Parish,
    /// A city.
    City,
    /// A town.
    Town,
    /// A village.
    Village,
    /// A farm / gård.
    Farm,
    /// A single building.
    Building,
}

impl From<PlaceTypeArg> for PlaceType {
    fn from(value: PlaceTypeArg) -> Self {
        match value {
            PlaceTypeArg::Country => Self::Country,
            PlaceTypeArg::County => Self::County,
            PlaceTypeArg::Municipality => Self::Municipality,
            PlaceTypeArg::Parish => Self::Parish,
            PlaceTypeArg::City => Self::City,
            PlaceTypeArg::Town => Self::Town,
            PlaceTypeArg::Village => Self::Village,
            PlaceTypeArg::Farm => Self::Farm,
            PlaceTypeArg::Building => Self::Building,
        }
    }
}

/// CLI mirror of [`EventType`]'s closed variants (keeps clap's `ValueEnum` off the domain type).
/// The domain's `Custom` escape is not exposed on the CLI yet.
#[derive(Clone, Copy, ValueEnum)]
enum EventTypeArg {
    /// Birth.
    Birth,
    /// Death.
    Death,
    /// Marriage.
    Marriage,
    /// Baptism / christening.
    Baptism,
    /// Burial.
    Burial,
    /// Census enumeration.
    Census,
    /// Residence.
    Residence,
    /// Immigration.
    Immigration,
    /// Emigration.
    Emigration,
}

impl From<EventTypeArg> for EventType {
    fn from(value: EventTypeArg) -> Self {
        match value {
            EventTypeArg::Birth => Self::Birth,
            EventTypeArg::Death => Self::Death,
            EventTypeArg::Marriage => Self::Marriage,
            EventTypeArg::Baptism => Self::Baptism,
            EventTypeArg::Burial => Self::Burial,
            EventTypeArg::Census => Self::Census,
            EventTypeArg::Residence => Self::Residence,
            EventTypeArg::Immigration => Self::Immigration,
            EventTypeArg::Emigration => Self::Emigration,
        }
    }
}

/// CLI mirror of [`ParticipantRole`]'s closed variants (keeps clap's `ValueEnum` off the domain
/// type). The domain's `Custom` escape is not exposed on the CLI yet.
#[derive(Clone, Copy, ValueEnum)]
enum ParticipantRoleArg {
    /// The principal of the event.
    Primary,
    /// A witness.
    Witness,
    /// An officiator (e.g. clergy).
    Officiator,
    /// The father.
    Father,
    /// The mother.
    Mother,
    /// A parent (neutral).
    Parent,
    /// A child.
    Child,
    /// A godparent.
    Godparent,
    /// The bride.
    Bride,
    /// The groom.
    Groom,
}

impl From<ParticipantRoleArg> for ParticipantRole {
    fn from(value: ParticipantRoleArg) -> Self {
        match value {
            ParticipantRoleArg::Primary => Self::Primary,
            ParticipantRoleArg::Witness => Self::Witness,
            ParticipantRoleArg::Officiator => Self::Officiator,
            ParticipantRoleArg::Father => Self::Father,
            ParticipantRoleArg::Mother => Self::Mother,
            ParticipantRoleArg::Parent => Self::Parent,
            ParticipantRoleArg::Child => Self::Child,
            ParticipantRoleArg::Godparent => Self::Godparent,
            ParticipantRoleArg::Bride => Self::Bride,
            ParticipantRoleArg::Groom => Self::Groom,
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    // Honor RUST_LOG when set; otherwise show errors but silence i18n-embed's benign
    // "unable to parse locale" message that fires for the C/POSIX locale on every run.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error,i18n_embed::requester=off"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    run(Cli::parse()).await
}

/// Resolves the workspace and dispatches the parsed command, rendering output and errors through
/// the localizer that has the most context available (workspace-aware for person commands).
async fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Init { name, path } => {
            let localizer = Localizer::baseline();
            report(&localizer, init(&localizer, name, path).await)
        }
        Command::Person { command } => {
            let baseline = Localizer::baseline();
            let workspace = cli.workspace.or_else(workspace_from_env);
            match resolve(workspace.as_deref()) {
                Ok((config, dir)) => {
                    let localizer = Localizer::for_workspace(&dir);
                    let result = run_person_command(&config, &dir, command, &localizer).await;
                    report(&localizer, result)
                }
                Err(error) => report(&baseline, Err(error)),
            }
        }
        Command::Family { command } => {
            let baseline = Localizer::baseline();
            let workspace = cli.workspace.or_else(workspace_from_env);
            match resolve(workspace.as_deref()) {
                Ok((config, dir)) => {
                    let localizer = Localizer::for_workspace(&dir);
                    let result = run_family_command(&config, &dir, command, &localizer).await;
                    report(&localizer, result)
                }
                Err(error) => report(&baseline, Err(error)),
            }
        }
        Command::Place { command } => {
            let baseline = Localizer::baseline();
            let workspace = cli.workspace.or_else(workspace_from_env);
            match resolve(workspace.as_deref()) {
                Ok((config, dir)) => {
                    let localizer = Localizer::for_workspace(&dir);
                    let result = run_place_command(&config, &dir, command, &localizer).await;
                    report(&localizer, result)
                }
                Err(error) => report(&baseline, Err(error)),
            }
        }
        Command::Source { command } => {
            let baseline = Localizer::baseline();
            let workspace = cli.workspace.or_else(workspace_from_env);
            match resolve(workspace.as_deref()) {
                Ok((config, dir)) => {
                    let localizer = Localizer::for_workspace(&dir);
                    let result = run_source_command(&config, &dir, command, &localizer).await;
                    report(&localizer, result)
                }
                Err(error) => report(&baseline, Err(error)),
            }
        }
        Command::Citation { command } => {
            let baseline = Localizer::baseline();
            let workspace = cli.workspace.or_else(workspace_from_env);
            match resolve(workspace.as_deref()) {
                Ok((config, dir)) => {
                    let localizer = Localizer::for_workspace(&dir);
                    let result = run_citation_command(&config, &dir, command, &localizer).await;
                    report(&localizer, result)
                }
                Err(error) => report(&baseline, Err(error)),
            }
        }
        Command::Event { command } => {
            let baseline = Localizer::baseline();
            let workspace = cli.workspace.or_else(workspace_from_env);
            match resolve(workspace.as_deref()) {
                Ok((config, dir)) => {
                    let localizer = Localizer::for_workspace(&dir);
                    let result = run_event_command(&config, &dir, command, &localizer).await;
                    report(&localizer, result)
                }
                Err(error) => report(&baseline, Err(error)),
            }
        }
    }
}

/// Renders an error to stderr through `localizer` and maps the outcome to an exit code.
fn report(localizer: &Localizer, result: Result<(), AppError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", localizer.error(&error));
            ExitCode::FAILURE
        }
    }
}

/// The workspace name from `GENEALOGY_WORKSPACE`, if set.
fn workspace_from_env() -> Option<String> {
    std::env::var("GENEALOGY_WORKSPACE").ok().filter(|s| !s.is_empty())
}

/// Bootstraps the global config, registers `name` → `path`, and creates the workspace + database.
async fn init(localizer: &Localizer, name: String, path: PathBuf) -> Result<(), AppError> {
    let config_path = config::config_path()?;
    let mut config = load_or_bootstrap(&config_path)?;
    if config.workspaces.contains_key(&name) {
        return Err(AppError::Config(format!("workspace {name:?} is already registered")));
    }

    Workspace::init(&path, &config.operator, &config.defaults)?;
    config.register_workspace(name.clone(), path.clone());
    config::save(&config_path, &config)?;
    // Open once to create the database file and record the operator in the manifest.
    Workspace::open(&path, &config.operator, &config.workspace_defaults).await?;

    println!("{}", localizer.init_success(&name, &path.display().to_string()));
    println!("{}", localizer.config_line(&config_path.display().to_string()));
    Ok(())
}

/// Loads config and resolves the workspace directory (by name) for a non-`init` command.
fn resolve(workspace: Option<&str>) -> Result<(Config, PathBuf), AppError> {
    let config = load(&config::config_path()?)?;
    let dir = config.resolve_workspace(workspace)?;
    Ok((config, dir))
}

/// Opens the resolved workspace and runs a person subcommand against it.
async fn run_person_command(
    config: &Config,
    dir: &Path,
    command: PersonCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    let session = Session::new(config.operator_agent());
    match command {
        PersonCmd::Create {
            id,
            given,
            surname,
            evidence,
        } => {
            let human_id = create_person(
                &workspace,
                &session,
                NewPerson {
                    human_id: id,
                    given,
                    surname,
                    evidence_level: evidence.into(),
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        PersonCmd::AddName {
            human_id,
            given,
            surname,
            citations,
        } => {
            add_name(&workspace, &session, &human_id, given, surname, &citations).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PersonCmd::AddParticipation { human_id, event, role } => {
            assert_participation(&workspace, &session, &human_id, &event, role.into()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PersonCmd::Show { human_id } => show(&workspace, &human_id, localizer).await,
        PersonCmd::List => list(&workspace, localizer).await,
    }
}

/// Renders one person, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_person(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::PersonNotFound(human_id.to_owned())),
    }
}

/// Renders every person, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let people = list_persons(workspace).await?;
    if people.is_empty() {
        println!("{}", localizer.list_empty());
        return Ok(());
    }
    for summary in &people {
        println!("{}", localizer.summary_line(summary));
    }
    Ok(())
}

/// Opens the resolved workspace and runs a family subcommand against it.
async fn run_family_command(
    config: &Config,
    dir: &Path,
    command: FamilyCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    let session = Session::new(config.operator_agent());
    match command {
        FamilyCmd::Create => {
            let human_id = create_family(&workspace, &session).await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        FamilyCmd::AddPartner { family_id, person_id } => {
            add_partner(&workspace, &session, &family_id, &person_id).await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::RemovePartner { family_id, person_id } => {
            remove_partner(&workspace, &session, &family_id, &person_id).await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::AddChild {
            family_id,
            person_id,
            relationship,
        } => {
            add_child(&workspace, &session, &family_id, &person_id, relationship.into()).await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::RemoveChild { family_id, person_id } => {
            remove_child(&workspace, &session, &family_id, &person_id).await?;
            println!("{}", localizer.updated(&family_id));
            Ok(())
        }
        FamilyCmd::Show { human_id } => show_one_family(&workspace, &human_id, localizer).await,
        FamilyCmd::List => list_all_families(&workspace, localizer).await,
    }
}

/// Renders one family, or errors if absent.
async fn show_one_family(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_family(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.family_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::FamilyNotFound(human_id.to_owned())),
    }
}

/// Renders every family, ordered by human id.
async fn list_all_families(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let families = list_families(workspace).await?;
    if families.is_empty() {
        println!("{}", localizer.family_list_empty());
        return Ok(());
    }
    for summary in &families {
        println!("{}", localizer.family_summary_line(summary));
    }
    Ok(())
}

/// Opens the resolved workspace and runs a place subcommand against it.
async fn run_place_command(
    config: &Config,
    dir: &Path,
    command: PlaceCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    let session = Session::new(config.operator_agent());
    match command {
        PlaceCmd::Create { id, r#type, name } => {
            let human_id = create_place(
                &workspace,
                &session,
                NewPlace {
                    human_id: id,
                    place_type: r#type.into(),
                    name,
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        PlaceCmd::SetType { human_id, r#type } => {
            set_place_type(&workspace, &session, &human_id, r#type.into()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::AddName { human_id, name } => {
            add_place_name(&workspace, &session, &human_id, name).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::Show { human_id } => match show_place(&workspace, &human_id).await? {
            Some(summary) => {
                println!("{}", localizer.place_summary_line(&summary));
                Ok(())
            }
            None => Err(AppError::PlaceNotFound(human_id)),
        },
        PlaceCmd::List => {
            let places = list_places(&workspace).await?;
            if places.is_empty() {
                println!("{}", localizer.place_list_empty());
                return Ok(());
            }
            for summary in &places {
                println!("{}", localizer.place_summary_line(summary));
            }
            Ok(())
        }
    }
}

/// Opens the resolved workspace and runs a source subcommand against it.
async fn run_source_command(
    config: &Config,
    dir: &Path,
    command: SourceCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    let session = Session::new(config.operator_agent());
    match command {
        SourceCmd::Create { id, title } => {
            let human_id = create_source(&workspace, &session, NewSource { human_id: id, title }).await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        SourceCmd::SetTitle { human_id, title } => {
            set_title(&workspace, &session, &human_id, title).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::Show { human_id } => match show_source(&workspace, &human_id).await? {
            Some(summary) => {
                println!("{}", localizer.source_summary_line(&summary));
                Ok(())
            }
            None => Err(AppError::SourceNotFound(human_id)),
        },
        SourceCmd::List => {
            let sources = list_sources(&workspace).await?;
            if sources.is_empty() {
                println!("{}", localizer.source_list_empty());
                return Ok(());
            }
            for summary in &sources {
                println!("{}", localizer.source_summary_line(summary));
            }
            Ok(())
        }
    }
}

/// Opens the resolved workspace and runs a citation subcommand against it.
async fn run_citation_command(
    config: &Config,
    dir: &Path,
    command: CitationCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    let session = Session::new(config.operator_agent());
    match command {
        CitationCmd::Create { source, id, page } => {
            let human_id = create_citation(
                &workspace,
                &session,
                NewCitation {
                    human_id: id,
                    source,
                    page,
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        CitationCmd::SetPage { human_id, page } => {
            set_page(&workspace, &session, &human_id, page).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::Show { human_id } => match show_citation(&workspace, &human_id).await? {
            Some(summary) => {
                println!("{}", localizer.citation_summary_line(&summary));
                Ok(())
            }
            None => Err(AppError::CitationNotFound(human_id)),
        },
        CitationCmd::List => {
            let citations = list_citations(&workspace).await?;
            if citations.is_empty() {
                println!("{}", localizer.citation_list_empty());
                return Ok(());
            }
            for summary in &citations {
                println!("{}", localizer.citation_summary_line(summary));
            }
            Ok(())
        }
    }
}

/// Opens the resolved workspace and runs an event subcommand against it.
async fn run_event_command(
    config: &Config,
    dir: &Path,
    command: EventCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    let session = Session::new(config.operator_agent());
    match command {
        EventCmd::Create { r#type, id } => {
            let human_id = create_event(
                &workspace,
                &session,
                NewEvent {
                    human_id: id,
                    event_type: r#type.into(),
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        EventCmd::SetType { human_id, r#type } => {
            set_event_type(&workspace, &session, &human_id, r#type.into()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::AssertDate {
            human_id,
            year,
            month,
            day,
        } => {
            assert_event_date(&workspace, &session, &human_id, DateParts { year, month, day }).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::LinkPlace { human_id, place_id } => {
            link_place(&workspace, &session, &human_id, &place_id).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::Show { human_id } => match show_event(&workspace, &human_id).await? {
            Some(summary) => {
                println!("{}", localizer.event_summary_line(&summary));
                Ok(())
            }
            None => Err(AppError::EventNotFound(human_id)),
        },
        EventCmd::List => {
            let events = list_events(&workspace).await?;
            if events.is_empty() {
                println!("{}", localizer.event_list_empty());
                return Ok(());
            }
            for summary in &events {
                println!("{}", localizer.event_summary_line(summary));
            }
            Ok(())
        }
    }
}
