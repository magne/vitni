//! CLI mirrors of the closed domain enums, keeping clap's `ValueEnum` off the domain types.
//!
//! Each domain enum carries a `Custom(String)` escape the CLI does not expose yet; these mirrors
//! cover the closed variants and convert into the domain type via `From`.

use std::collections::BTreeSet;

use clap::{Args, ValueEnum};
use vitni_app::{
    AppError, ChildParentRelationship, ChromosomeSide, ColumnType, Confidence, DnaGenomeBuild, DnaProvider,
    DnaTestType, EventType, EvidenceKind, FileConfigStore, FilterOp, FilterValue, GenealogicalDate, InformationKind,
    ListFilter, ListPage, ListQuery, ListQueryError, ListSort, ListedKind, NoteType, ParticipantRole, PlaceKind,
    PlaceLevel, RepositoryType, Restriction, SortDirection, SourceMediaType, SourceQuality, SuccessionKind, Workspace,
    gregorian_date, reference_id, runnable_saved_query,
};
use vitni_core::enums::EvidenceLevel;

/// CLI mirror of [`Confidence`] — the operator's surety in an assertion (data-model §8).
#[derive(Clone, Copy, ValueEnum)]
pub enum ConfidenceArg {
    /// Lowest surety.
    VeryLow,
    /// Low surety.
    Low,
    /// The default, middling surety.
    Normal,
    /// High surety.
    High,
    /// Highest surety.
    VeryHigh,
}

impl From<ConfidenceArg> for Confidence {
    fn from(value: ConfidenceArg) -> Self {
        match value {
            ConfidenceArg::VeryLow => Self::VeryLow,
            ConfidenceArg::Low => Self::Low,
            ConfidenceArg::Normal => Self::Normal,
            ConfidenceArg::High => Self::High,
            ConfidenceArg::VeryHigh => Self::VeryHigh,
        }
    }
}

/// CLI mirror of [`DnaProvider`]'s closed variants. The domain's `Custom` escape is not exposed yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum DnaProviderArg {
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
}

impl From<DnaProviderArg> for DnaProvider {
    fn from(value: DnaProviderArg) -> Self {
        match value {
            DnaProviderArg::AncestryDna => Self::AncestryDna,
            DnaProviderArg::TwentyThreeAndMe => Self::TwentyThreeAndMe,
            DnaProviderArg::MyHeritage => Self::MyHeritage,
            DnaProviderArg::FamilyTreeDna => Self::FamilyTreeDna,
            DnaProviderArg::GedMatch => Self::GedMatch,
            DnaProviderArg::LivingDna => Self::LivingDna,
        }
    }
}

/// CLI mirror of [`ChromosomeSide`].
#[derive(Clone, Copy, ValueEnum)]
pub enum ChromosomeSideArg {
    /// The maternal side.
    Maternal,
    /// The paternal side.
    Paternal,
    /// Unassigned / unknown.
    Unknown,
}

impl From<ChromosomeSideArg> for ChromosomeSide {
    fn from(value: ChromosomeSideArg) -> Self {
        match value {
            ChromosomeSideArg::Maternal => Self::Maternal,
            ChromosomeSideArg::Paternal => Self::Paternal,
            ChromosomeSideArg::Unknown => Self::Unknown,
        }
    }
}

/// CLI mirror of [`NoteType`]'s closed variants. The domain's `Custom` escape is not exposed yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum NoteTypeArg {
    /// A general note.
    General,
    /// A research note.
    Research,
    /// A source transcript.
    Transcript,
    /// A citation note.
    Citation,
}

impl From<NoteTypeArg> for NoteType {
    fn from(value: NoteTypeArg) -> Self {
        match value {
            NoteTypeArg::General => Self::General,
            NoteTypeArg::Research => Self::Research,
            NoteTypeArg::Transcript => Self::Transcript,
            NoteTypeArg::Citation => Self::Citation,
        }
    }
}

/// CLI mirror of [`DnaTestType`].
#[derive(Clone, Copy, ValueEnum)]
pub enum DnaTestTypeArg {
    /// Autosomal (atDNA).
    Autosomal,
    /// Y-chromosome.
    YDna,
    /// Mitochondrial.
    MtDna,
    /// X-chromosome.
    XDna,
}

impl From<DnaTestTypeArg> for DnaTestType {
    fn from(value: DnaTestTypeArg) -> Self {
        match value {
            DnaTestTypeArg::Autosomal => Self::Autosomal,
            DnaTestTypeArg::YDna => Self::YDna,
            DnaTestTypeArg::MtDna => Self::MtDna,
            DnaTestTypeArg::XDna => Self::XDna,
        }
    }
}

/// CLI mirror of `SourceQuality` (Evidence Explained's source axis).
#[derive(Clone, Copy, ValueEnum)]
pub enum SourceQualityArg {
    /// An original record.
    Original,
    /// A derivative (copy, transcription, abstract).
    Derivative,
}

impl From<SourceQualityArg> for SourceQuality {
    fn from(value: SourceQualityArg) -> Self {
        match value {
            SourceQualityArg::Original => Self::Original,
            SourceQualityArg::Derivative => Self::Derivative,
        }
    }
}

/// CLI mirror of [`DnaGenomeBuild`].
#[derive(Clone, Copy, ValueEnum)]
pub enum DnaGenomeBuildArg {
    /// `GRCh37` / hg19.
    Grch37,
    /// `GRCh38` / hg38.
    Grch38,
}

impl From<DnaGenomeBuildArg> for DnaGenomeBuild {
    fn from(value: DnaGenomeBuildArg) -> Self {
        match value {
            DnaGenomeBuildArg::Grch37 => Self::GRCh37,
            DnaGenomeBuildArg::Grch38 => Self::GRCh38,
        }
    }
}

/// CLI mirror of `InformationKind` (Evidence Explained's information axis).
#[derive(Clone, Copy, ValueEnum)]
pub enum InformationKindArg {
    /// Primary (firsthand) information.
    Primary,
    /// Secondary (secondhand) information.
    Secondary,
}

impl From<InformationKindArg> for InformationKind {
    fn from(value: InformationKindArg) -> Self {
        match value {
            InformationKindArg::Primary => Self::Primary,
            InformationKindArg::Secondary => Self::Secondary,
        }
    }
}

/// CLI mirror of `EvidenceKind` (Evidence Explained's evidence axis).
#[derive(Clone, Copy, ValueEnum)]
pub enum EvidenceKindArg {
    /// Direct evidence.
    Direct,
    /// Indirect evidence.
    Indirect,
    /// Negative evidence.
    Negative,
}

impl From<EvidenceKindArg> for EvidenceKind {
    fn from(value: EvidenceKindArg) -> Self {
        match value {
            EvidenceKindArg::Direct => Self::Direct,
            EvidenceKindArg::Indirect => Self::Indirect,
            EvidenceKindArg::Negative => Self::Negative,
        }
    }
}

/// CLI mirror of [`EvidenceLevel`].
#[derive(Clone, Copy, ValueEnum)]
pub enum EvidenceArg {
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

/// CLI mirror of [`ChildParentRelationship`].
#[derive(Clone, Copy, ValueEnum)]
pub enum RelationshipArg {
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

/// CLI mirror of [`SuccessionKind`] — how a place's identity changed (ADR 0026 §2–§3). A closed
/// domain set, so this mirror is exhaustive and carries no `Custom` escape.
#[derive(Clone, Copy, ValueEnum)]
pub enum SuccessionKindArg {
    /// Two or more places merged into one (many→one).
    Merged,
    /// One place split into two or more (one→many).
    Split,
    /// One place was absorbed into another, which continues under its own identity.
    Absorbed,
    /// One place was elevated to a new administrative level, becoming a new identity.
    Elevated,
    /// One place's identity was replaced by a new one (not a same-place rename).
    Renamed,
}

impl From<SuccessionKindArg> for SuccessionKind {
    fn from(value: SuccessionKindArg) -> Self {
        match value {
            SuccessionKindArg::Merged => Self::Merged,
            SuccessionKindArg::Split => Self::Split,
            SuccessionKindArg::Absorbed => Self::Absorbed,
            SuccessionKindArg::Elevated => Self::Elevated,
            SuccessionKindArg::Renamed => Self::Renamed,
        }
    }
}

/// CLI mirror of [`PlaceKind`] — what sort of structure a place is (ADR 0031 §3).
#[derive(Clone, Copy, ValueEnum)]
pub enum PlaceKindArg {
    /// A civil administrative unit.
    Administrative,
    /// A church unit.
    Ecclesiastical,
    /// A court district.
    Judicial,
    /// A civil-registration district.
    Registration,
    /// A tax or fiscal unit.
    Fiscal,
    /// A military district.
    Military,
    /// A manorial unit.
    Manorial,
    /// A land-register unit.
    Cadastral,
    /// A statistical unit.
    Statistical,
    /// A populated place.
    Settlement,
    /// A building or structure.
    Building,
    /// A natural feature.
    Natural,
}

impl From<PlaceKindArg> for PlaceKind {
    fn from(value: PlaceKindArg) -> Self {
        match value {
            PlaceKindArg::Administrative => Self::Administrative,
            PlaceKindArg::Ecclesiastical => Self::Ecclesiastical,
            PlaceKindArg::Judicial => Self::Judicial,
            PlaceKindArg::Registration => Self::Registration,
            PlaceKindArg::Fiscal => Self::Fiscal,
            PlaceKindArg::Military => Self::Military,
            PlaceKindArg::Manorial => Self::Manorial,
            PlaceKindArg::Cadastral => Self::Cadastral,
            PlaceKindArg::Statistical => Self::Statistical,
            PlaceKindArg::Settlement => Self::Settlement,
            PlaceKindArg::Building => Self::Building,
            PlaceKindArg::Natural => Self::Natural,
        }
    }
}

/// CLI mirror of [`PlaceLevel`] — a place's rank in a hierarchy (ADR 0031 §3).
#[derive(Clone, Copy, ValueEnum)]
pub enum PlaceLevelArg {
    /// A country.
    Country,
    /// First administrative level.
    Adm1,
    /// Second administrative level.
    Adm2,
    /// Third administrative level.
    Adm3,
    /// Fourth administrative level.
    Adm4,
    /// Fifth administrative level.
    Adm5,
    /// A settlement.
    Settlement,
    /// A part of a settlement.
    SubSettlement,
}

impl From<PlaceLevelArg> for PlaceLevel {
    fn from(value: PlaceLevelArg) -> Self {
        match value {
            PlaceLevelArg::Country => Self::Country,
            PlaceLevelArg::Adm1 => Self::Adm1,
            PlaceLevelArg::Adm2 => Self::Adm2,
            PlaceLevelArg::Adm3 => Self::Adm3,
            PlaceLevelArg::Adm4 => Self::Adm4,
            PlaceLevelArg::Adm5 => Self::Adm5,
            PlaceLevelArg::Settlement => Self::Settlement,
            PlaceLevelArg::SubSettlement => Self::SubSettlement,
        }
    }
}

/// CLI mirror of [`EventType`]'s closed variants. The domain's `Custom` escape is not exposed yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum EventTypeArg {
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

/// CLI mirror of [`ParticipantRole`]'s closed variants. The domain's `Custom` escape is not exposed
/// yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum ParticipantRoleArg {
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

/// CLI mirror of [`RepositoryType`]'s closed variants. The domain's `Custom` escape is not exposed
/// yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum RepositoryTypeArg {
    /// A library.
    Library,
    /// An archive.
    Archive,
    /// A church / parish.
    Church,
    /// A cemetery.
    Cemetery,
    /// A museum.
    Museum,
    /// A website / online collection.
    Website,
    /// A private or personal collection.
    Collection,
}

impl From<RepositoryTypeArg> for RepositoryType {
    fn from(value: RepositoryTypeArg) -> Self {
        match value {
            RepositoryTypeArg::Library => Self::Library,
            RepositoryTypeArg::Archive => Self::Archive,
            RepositoryTypeArg::Church => Self::Church,
            RepositoryTypeArg::Cemetery => Self::Cemetery,
            RepositoryTypeArg::Museum => Self::Museum,
            RepositoryTypeArg::Website => Self::Website,
            RepositoryTypeArg::Collection => Self::Collection,
        }
    }
}

/// CLI mirror of [`SourceMediaType`]'s closed variants. The domain's `Custom` escape is not exposed
/// yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum SourceMediaTypeArg {
    /// A book.
    Book,
    /// A card / index card.
    Card,
    /// An electronic / digital record.
    Electronic,
    /// Microfiche.
    Fiche,
    /// Microfilm.
    Film,
    /// A magazine / periodical.
    Magazine,
    /// A manuscript.
    Manuscript,
    /// A map.
    Map,
    /// A newspaper.
    Newspaper,
    /// A photograph.
    Photo,
    /// A tombstone / grave marker.
    Tombstone,
    /// A video recording.
    Video,
    /// An audio recording.
    Audio,
}

impl From<SourceMediaTypeArg> for SourceMediaType {
    fn from(value: SourceMediaTypeArg) -> Self {
        match value {
            SourceMediaTypeArg::Book => Self::Book,
            SourceMediaTypeArg::Card => Self::Card,
            SourceMediaTypeArg::Electronic => Self::Electronic,
            SourceMediaTypeArg::Fiche => Self::Fiche,
            SourceMediaTypeArg::Film => Self::Film,
            SourceMediaTypeArg::Magazine => Self::Magazine,
            SourceMediaTypeArg::Manuscript => Self::Manuscript,
            SourceMediaTypeArg::Map => Self::Map,
            SourceMediaTypeArg::Newspaper => Self::Newspaper,
            SourceMediaTypeArg::Photo => Self::Photo,
            SourceMediaTypeArg::Tombstone => Self::Tombstone,
            SourceMediaTypeArg::Video => Self::Video,
            SourceMediaTypeArg::Audio => Self::Audio,
        }
    }
}

/// Which conclusion-bearing aggregate a `--subject` `human_id` names (ADR 0028 §2). Not a mirror of
/// a `vitni-core` enum — `SubjectRef` bakes the kind and the id together; this is the CLI's own
/// way to ask "which kind of id did you just give me".
#[derive(Clone, Copy, ValueEnum)]
pub enum SubjectKindArg {
    /// A Person, by its `human_id`.
    Person,
    /// A Family, by its `human_id`.
    Family,
    /// An Event, by its `human_id`.
    Event,
    /// A Place, by its `human_id`.
    Place,
}

/// CLI mirror of [`Restriction`] — a record's privacy restriction (GEDCOM v7 `RESN`, data-model §6).
#[derive(Clone, Copy, ValueEnum)]
pub enum RestrictionArg {
    /// Hide from general view.
    Confidential,
    /// Protected from edits.
    Locked,
    /// Living-person privacy.
    Privacy,
}

impl From<RestrictionArg> for Restriction {
    fn from(value: RestrictionArg) -> Self {
        match value {
            RestrictionArg::Confidential => Self::Confidential,
            RestrictionArg::Locked => Self::Locked,
            RestrictionArg::Privacy => Self::Privacy,
        }
    }
}

/// The full restriction set a `set-restrictions` verb writes: the listed restrictions, or none with
/// `--clear`. It replaces the record's current set rather than adding to it.
#[derive(Args)]
pub struct RestrictionsArgs {
    /// A restriction to set; repeat the flag or separate with commas (`privacy,locked`).
    #[arg(
        long = "restriction",
        value_enum,
        value_delimiter = ',',
        required_unless_present = "clear"
    )]
    restrictions: Vec<RestrictionArg>,
    /// Remove every restriction from the record.
    #[arg(long, conflicts_with = "restrictions")]
    clear: bool,
}

impl RestrictionsArgs {
    /// The restriction set to write — empty after `--clear`.
    #[must_use]
    pub fn into_set(self) -> BTreeSet<Restriction> {
        let mut set = BTreeSet::new();
        if self.clear {
            return set;
        }
        for restriction in self.restrictions {
            set.insert(restriction.into());
        }
        set
    }
}

/// The query and window of a `list` verb (ADR 0052 §9): narrow by `--search` and `--filter`, order by
/// `--sort`, then skip `--offset` records and print at most `--limit` of them.
#[derive(Args)]
pub struct ListArgs {
    /// Print at most this many records (at least one).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    limit: Option<u32>,
    /// Skip this many records first.
    #[arg(long, default_value_t = 0)]
    offset: u32,
    /// Start from the saved search of this name (any case); `--filter` adds to it, `--search` and `--sort` replace
    /// its own.
    #[arg(long, value_name = "NAME")]
    saved: Option<String>,
    #[command(flatten)]
    clauses: QueryClauses,
}

/// The `--search`, `--filter` and `--sort` clauses shared by `list` and `saved-search save`.
#[derive(Args)]
pub struct QueryClauses {
    /// List only records whose text columns or id contain this, in any case.
    #[arg(long)]
    search: Option<String>,
    /// List only records passing `<column>:<op>:<value>` (e.g. `birth:before:1800`); repeat to combine.
    #[arg(long = "filter", value_name = "COLUMN:OP:VALUE", value_parser = parse_filter)]
    filters: Vec<FilterArg>,
    /// Order by `<column>`, or `<column>:desc` for largest first.
    #[arg(long, value_name = "COLUMN[:desc]", value_parser = parse_sort)]
    sort: Option<SortArg>,
}

/// One `--filter` clause, split into its parts; the catalogue checks them when the query is built.
#[derive(Clone, Debug)]
pub struct FilterArg {
    column: String,
    op: String,
    value: String,
}

/// Splits `<column>:<op>[:<value>]`; the value may itself hold colons.
fn parse_filter(raw: &str) -> Result<FilterArg, String> {
    let mut parts = raw.splitn(3, ':');
    let column = parts.next().unwrap_or_default().trim();
    let op = parts.next().unwrap_or_default().trim();
    if column.is_empty() || op.is_empty() {
        return Err(format!("expected COLUMN:OP:VALUE, got `{raw}`"));
    }
    Ok(FilterArg {
        column: column.to_owned(),
        op: op.to_owned(),
        value: parts.next().unwrap_or_default().trim().to_owned(),
    })
}

/// A `--sort` column and direction.
#[derive(Clone, Debug)]
pub struct SortArg {
    column: String,
    direction: SortDirection,
}

/// Splits `<column>[:asc|:desc]`.
fn parse_sort(raw: &str) -> Result<SortArg, String> {
    let (column, direction) = match raw.split_once(':') {
        None => (raw, SortDirection::Ascending),
        Some((column, "asc")) => (column, SortDirection::Ascending),
        Some((column, "desc")) => (column, SortDirection::Descending),
        Some(_) => return Err(format!("expected COLUMN, COLUMN:asc or COLUMN:desc, got `{raw}`")),
    };
    if column.trim().is_empty() {
        return Err(format!("expected COLUMN, COLUMN:asc or COLUMN:desc, got `{raw}`"));
    }
    Ok(SortArg {
        column: column.trim().to_owned(),
        direction,
    })
}

/// Whether `query` narrows the list, so an empty one means no match.
#[must_use]
pub fn narrows(query: &ListQuery) -> bool {
    !query.search.trim().is_empty() || !query.filters.is_empty()
}

impl ListArgs {
    /// The page of the list to read.
    #[must_use]
    pub const fn page(&self) -> ListPage {
        ListPage {
            offset: self.offset,
            limit: self.limit,
        }
    }

    /// The list query for `kind`: the `--saved` search if given, with `--search` and `--sort` replacing its own and
    /// each `--filter` added, every clause checked against its catalogue: a tag named by its name and a record by
    /// its `human_id` become their ids.
    ///
    /// # Errors
    ///
    /// [`AppError::ListQuery`] naming the clause the catalogue cannot take or the kind `--saved` lists instead,
    /// [`AppError::SavedSearch`] for an unknown or stale saved search, or a store error.
    pub async fn query(&self, workspace: &Workspace, kind: ListedKind) -> Result<ListQuery, AppError> {
        let query = match &self.saved {
            Some(name) => saved_query(workspace, name, kind)?,
            None => ListQuery::new(kind),
        };
        self.clauses.apply(workspace, kind, query).await
    }
}

impl QueryClauses {
    /// `query` with `--search` and `--sort` replacing its own and each `--filter` added, every clause checked
    /// against the catalogue of `kind`.
    async fn apply(
        &self,
        workspace: &Workspace,
        kind: ListedKind,
        mut query: ListQuery,
    ) -> Result<ListQuery, AppError> {
        if let Some(search) = &self.search {
            query.search.clone_from(search);
        }
        for filter in &self.filters {
            query.filters.push(filter.to_filter(workspace, kind).await?);
        }
        if let Some(sort) = &self.sort {
            query.sort = Some(ListSort {
                column: sort.column.clone(),
                direction: sort.direction,
            });
        }
        Ok(query)
    }
}

/// The runnable query of the saved search `name`, which must list `kind`.
fn saved_query(workspace: &Workspace, name: &str, kind: ListedKind) -> Result<ListQuery, AppError> {
    let store = FileConfigStore::for_workspace(workspace.dir().to_path_buf());
    runnable_saved_query(&store, name, kind)
}

/// Reads a record kind as the name `list` prints it by.
pub fn parse_kind(raw: &str) -> Result<ListedKind, String> {
    for kind in ListedKind::ALL {
        if kind.as_str() == raw {
            return Ok(*kind);
        }
    }
    let kinds: Vec<&str> = ListedKind::ALL.iter().map(|kind| kind.as_str()).collect();
    Err(format!("expected one of {}, got `{raw}`", kinds.join(", ")))
}

/// The query a `saved-search save` stores: `--filter`, `--search` and `--sort`, as `list` takes them.
#[derive(Args)]
pub struct QueryArgs {
    /// The kind of record the search lists.
    #[arg(long, value_parser = parse_kind)]
    kind: ListedKind,
    #[command(flatten)]
    clauses: QueryClauses,
}

impl QueryArgs {
    /// The query to store, each clause checked against its catalogue.
    ///
    /// # Errors
    ///
    /// [`AppError::ListQuery`] naming the clause the catalogue cannot take, or a store error.
    pub async fn query(&self, workspace: &Workspace) -> Result<ListQuery, AppError> {
        self.clauses
            .apply(workspace, self.kind, ListQuery::new(self.kind))
            .await
    }
}

impl FilterArg {
    /// The clause as the query holds it: its value read by the column's type.
    async fn to_filter(&self, workspace: &Workspace, kind: ListedKind) -> Result<ListFilter, AppError> {
        let column = self.column.clone();
        let op = FilterOp::parse(&self.op).ok_or_else(|| ListQueryError::UnknownOperator {
            column: column.clone(),
            op: self.op.clone(),
        })?;
        let ty = kind
            .column(&column)
            .ok_or_else(|| ListQueryError::UnknownColumn {
                kind,
                column: column.clone(),
            })?
            .ty;
        if !FilterOp::for_type(ty).contains(&op) {
            return Err(ListQueryError::UnsupportedOperator { column, op }.into());
        }
        let wrong = || ListQueryError::WrongValue {
            column: column.clone(),
            op,
        };
        let raw = self.value.as_str();
        let value = match (ty, op) {
            (_, FilterOp::IsEmpty) => FilterValue::None,
            (ColumnType::Text, _) => FilterValue::Text(raw.to_owned()),
            (ColumnType::Date, FilterOp::Between) => {
                let (from, to) = raw.split_once("..").ok_or_else(wrong)?;
                FilterValue::DateRange {
                    from: date(from).ok_or_else(wrong)?,
                    to: date(to).ok_or_else(wrong)?,
                }
            }
            (ColumnType::Date, _) => FilterValue::Date(date(raw).ok_or_else(wrong)?),
            (ColumnType::Number, FilterOp::Between) => {
                let (from, to) = raw.split_once("..").ok_or_else(wrong)?;
                FilterValue::NumberRange {
                    from: from.trim().parse().map_err(|_| wrong())?,
                    to: to.trim().parse().map_err(|_| wrong())?,
                }
            }
            (ColumnType::Number, _) => FilterValue::Number(raw.parse().map_err(|_| wrong())?),
            (ColumnType::Choice, _) => {
                FilterValue::Values(raw.split(',').map(|value| value.trim().to_owned()).collect())
            }
            (ColumnType::Tags, _) => {
                let mut ids = Vec::new();
                for tag in raw.split(',') {
                    ids.push(reference_id(workspace, kind, &self.column, tag.trim()).await?);
                }
                FilterValue::Values(ids)
            }
            (ColumnType::Record, _) => FilterValue::Text(reference_id(workspace, kind, &self.column, raw).await?),
        };
        Ok(ListFilter {
            column: self.column.clone(),
            op,
            value,
        })
    }
}

/// A `YYYY[-MM[-DD]]` filter date.
fn date(raw: &str) -> Option<GenealogicalDate> {
    crate::commands::person::parse_born(raw).ok().map(gregorian_date)
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;
    use vitni_app::{PlaceKind, PlaceLevel};

    use super::{PlaceKindArg, PlaceLevelArg};

    #[test]
    fn the_place_kind_mirror_covers_exactly_the_core_kinds() {
        let mapped: Vec<PlaceKind> = PlaceKindArg::value_variants()
            .iter()
            .copied()
            .map(PlaceKind::from)
            .collect();
        assert_eq!(mapped, PlaceKind::ALL);
    }

    #[test]
    fn the_place_level_mirror_covers_exactly_the_core_levels() {
        let mapped: Vec<PlaceLevel> = PlaceLevelArg::value_variants()
            .iter()
            .copied()
            .map(PlaceLevel::from)
            .collect();
        assert_eq!(mapped, PlaceLevel::ALL);
    }
}
