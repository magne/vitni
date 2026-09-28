//! `cargo xtask backup-fixture` — regenerates the golden backup fixtures (ADR 0041 §5).
//!
//! Builds the log of **invented data** that holds every event variant of every aggregate, with fixed
//! ids and instants, inserts it into a scratch workspace, rebuilds the projections and writes the
//! current format's archive to `crates/vitni-app/tests/fixtures/backup/v<format>/`. Two runs produce
//! no diff. It then restores every fixture directory's archive, older formats included, and rewrites
//! each one's `projection-digest.txt`: a projection change legitimately moves the digests, so review
//! that diff rather than accepting it blindly. Older archives are never rewritten.
//!
//! Each aggregate's events live in their own module; each pushes rows through [`Builder`], which
//! stamps the provenance envelope and numbers every stream.

mod citation;
mod dna_match;
mod dna_test;
mod event;
mod family;
mod media;
mod note;
mod person;
mod place;
mod repository;
mod research_note;
mod source;
mod tag;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cqrs_es::{Aggregate, DomainEvent};
use time::OffsetDateTime;
use time::macros::datetime;
use uuid::Uuid;
use vitni_app::{
    AppDefaults, BackupRequest, OperatorConfig, RestoreRequest, Workspace, WorkspaceDefaults, create_backup,
    projection_digest, restore_backup,
};
use vitni_core::ids::{
    AgentId, AssertionId, CitationId, DnaMatchId, DnaTestId, EventId, FamilyId, MediaId, NoteId, PersonId, PlaceId,
    RepositoryId, SourceId, TagId,
};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use vitni_db::RawEvent;

/// Where the fixtures live, relative to the repository root.
const FIXTURES: &str = "crates/vitni-app/tests/fixtures/backup";
/// The archive file inside each `v<format>/` directory.
const ARCHIVE: &str = "workspace.vitni-backup";
/// The expected projection digest beside it.
const DIGEST: &str = "projection-digest.txt";
/// The instant the fixture's first assertion is recorded; each later one is a minute on.
const EPOCH: OffsetDateTime = datetime!(2026-01-05 09:00:00 UTC);

/// The media-library file the fixture's media record points at, and its bytes.
pub(crate) const MEDIA_FILE: &str = "fixture/letter.txt";
/// The bytes of [`MEDIA_FILE`].
pub(crate) const MEDIA_BYTES: &[u8] = b"Dear Ingrid, the harvest was good this year.\n";

/// The aggregates created so far, for later modules to reference. Each module sets its own field.
#[derive(Default)]
pub(crate) struct Ids {
    pub(crate) tag: Option<TagId>,
    pub(crate) note: Option<NoteId>,
    pub(crate) repository: Option<RepositoryId>,
    pub(crate) source: Option<SourceId>,
    pub(crate) citation: Option<CitationId>,
    pub(crate) place: Option<PlaceId>,
    pub(crate) media: Option<MediaId>,
    pub(crate) persons: Vec<PersonId>,
    pub(crate) event: Option<EventId>,
    pub(crate) family: Option<FamilyId>,
    pub(crate) dna_test: Option<DnaTestId>,
    pub(crate) dna_match: Option<DnaMatchId>,
}

/// Accumulates the fixture log: deterministic ids and instants, one stream sequence per aggregate.
pub(crate) struct Builder {
    rows: Vec<RawEvent>,
    sequences: BTreeMap<(&'static str, String), i64>,
    minted: u64,
    /// The aggregates created so far.
    pub(crate) ids: Ids,
}

impl Builder {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            sequences: BTreeMap::new(),
            minted: 0,
            ids: Ids::default(),
        }
    }

    /// A fresh UUID v7, deterministic: its timestamp and counter bits advance with each call.
    pub(crate) fn uuid(&mut self) -> Uuid {
        self.minted += 1;
        let millis = u64::try_from(EPOCH.unix_timestamp()).unwrap_or(0) * 1000 + self.minted;
        let mut counter = [0_u8; 10];
        counter[2..].copy_from_slice(&self.minted.to_be_bytes());
        uuid::Builder::from_unix_timestamp_millis(millis, &counter).into_uuid()
    }

    /// The provenance for the next assertion: a fresh assertion id, the fixture operator, the next
    /// minute, and a normal confidence.
    pub(crate) fn meta(&mut self) -> AssertionMeta {
        let assertion_id = AssertionId::from_uuid(self.uuid());
        let minutes = i64::try_from(self.rows.len()).unwrap_or(0);
        AssertionMeta {
            assertion_id,
            context: EventContext {
                operator: Agent {
                    kind: AgentKind::Human,
                    id: AgentId::from_uuid(Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_0001)),
                    display: Some("Fixture Researcher".to_owned()),
                },
                occurred_at: Timestamp::new(EPOCH + time::Duration::minutes(minutes)),
                rationale: None,
                confidence: Some(Confidence::Normal),
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
    }

    /// Appends `event` to aggregate `A`'s stream `aggregate_id`, at that stream's next sequence.
    #[expect(
        clippy::expect_used,
        reason = "a domain event always serializes; the fixture aborts if not"
    )]
    pub(crate) fn push<A: Aggregate>(&mut self, aggregate_id: impl std::fmt::Display + Copy, event: A::Event) {
        let aggregate_id = aggregate_id.to_string();
        let sequence = self.sequences.entry((A::TYPE, aggregate_id.clone())).or_insert(0);
        *sequence += 1;
        let event_type = event.event_type();
        let event_version = event.event_version();
        self.rows.push(RawEvent {
            aggregate_type: A::TYPE.to_owned(),
            aggregate_id,
            sequence: *sequence,
            event_type,
            event_version,
            payload: serde_json::to_value(event).expect("a domain event serializes"),
            metadata: serde_json::json!({}),
        });
    }
}

/// Every aggregate's fixture events, in one log. Later aggregates reference earlier ones through
/// [`Builder::ids`], so the order is the dependency order.
fn build_log() -> Vec<RawEvent> {
    let mut builder = Builder::new();
    tag::events(&mut builder);
    note::events(&mut builder);
    repository::events(&mut builder);
    source::events(&mut builder);
    citation::events(&mut builder);
    place::events(&mut builder);
    media::events(&mut builder);
    person::events(&mut builder);
    event::events(&mut builder);
    family::events(&mut builder);
    dna_test::events(&mut builder);
    dna_match::events(&mut builder);
    research_note::events(&mut builder);
    builder.rows
}

/// Runs the command: writes the current fixture, then refreshes every fixture's digest.
pub fn run() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("starting the async runtime")?;
    runtime.block_on(async {
        write_current_fixture().await?;
        refresh_digests().await
    })
}

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_0001)),
        display: Some("Fixture Researcher".to_owned()),
        email: None,
    }
}

/// Seeds a scratch workspace with the log and the media file, and writes its archive into the
/// current format's fixture directory.
async fn write_current_fixture() -> Result<()> {
    let scratch = tempfile::tempdir().context("creating a scratch directory")?;
    let dir = scratch.path().join("fixture");
    Workspace::init(&dir, &operator(), &AppDefaults::default(), None)?;
    let workspace = Workspace::open(&dir, &operator(), &WorkspaceDefaults::default()).await?;
    let rows = build_log();
    workspace.store().insert_raw_events(rows.into_iter().map(Ok)).await?;
    workspace.rebuild_projections().await?;
    let media_path = workspace.media_root().join(MEDIA_FILE);
    fs::create_dir_all(media_path.parent().context("the media file has a directory")?)?;
    fs::write(&media_path, MEDIA_BYTES)?;

    let target = Path::new(FIXTURES).join(format!("v{}", vitni_app::backup::current_format_version()));
    fs::create_dir_all(&target)?;
    let archive = target.join(ARCHIVE);
    if archive.exists() {
        fs::remove_file(&archive).with_context(|| format!("replacing {}", archive.display()))?;
    }
    let request = BackupRequest {
        workspace_name: "fixture",
        destination: &archive,
        with_media: true,
        created_at: EPOCH,
    };
    let report = create_backup(&workspace, &request).await?;
    if !report.media_missing.is_empty() {
        bail!("the fixture's media files are missing: {:?}", report.media_missing);
    }
    println!("backup-fixture: wrote {} ({} events)", archive.display(), report.events);
    Ok(())
}

/// Restores every fixture's archive into a scratch workspace and rewrites its digest.
async fn refresh_digests() -> Result<()> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(FIXTURES).with_context(|| format!("reading {FIXTURES}"))? {
        let path = entry?.path();
        if path.join(ARCHIVE).is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    for dir in dirs {
        let scratch = tempfile::tempdir().context("creating a scratch directory")?;
        let config = scratch.path().join("config.toml");
        let target = scratch.path().join("restored");
        let archive = dir.join(ARCHIVE);
        let request = RestoreRequest {
            config_path: &config,
            archive: &archive,
            name: "fixture",
            dir: Some(&target),
            database_url: None,
        };
        restore_backup(&request)
            .await
            .with_context(|| format!("restoring {}", archive.display()))?;
        let workspace = Workspace::open(&target, &operator(), &WorkspaceDefaults::default()).await?;
        let digest = projection_digest(&workspace).await?;
        fs::write(dir.join(DIGEST), format!("{digest}\n"))?;
        println!("backup-fixture: {} → {digest}", dir.display());
    }
    Ok(())
}
