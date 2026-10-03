//! Digitalarkivet assisted-import plugin (ADR 0017 §5, §6): one long `run-assisted` invocation drives
//! a record-by-record review-and-import session over the sandboxed host capabilities.
//!
//! Flow (prototype-proven, `sort-inbox.py`): classify the request URL, fetch the page(s) over `net`,
//! parse them with the pure `vitni-digitalarkivet` crate, present each record to the user through
//! `present` (suspending until they confirm or skip), file the scan once per source page through
//! `media-store`, and submit the confirmed record as one record graph through `staging` — the host
//! writes it as low-confidence Software-agent assertions, resolving the person by `ExternalId` so a
//! re-run imports no duplicates, and writing only the identity of a person another dataset made
//! (ADR 0040). Besides the person, a record yields its birth, and either its part in the census of its
//! residence (with its place in the household's family) or its part in a church-book event
//! ([`graph`]). The source, its repository, the scan, the census, the residence, its municipality,
//! the household and the church-book event are records of their own, submitted with each record and
//! referenced from its graph by origin, so every record of a page — and of a later session — shares
//! them, and a withheld record withholds them too.
//!
//! - **Census residence** (`/census/{rural,urban}-residence/`): fetch the household page, fetch and
//!   parse each linked person page, present the records list, then review each picked record.
//! - **Census person / church-book record**: a single record — straight to the confirm stage.
//! - **Church-book scans** are served through the new IIIF viewer, which carries no permanent image
//!   ([`ParseError::ImageUrlNotFound`]); the record is presented and imported **without a scan**.
//!
//! AI interpretation (`ai`) is granted and available but not invoked in this flow: census HTML
//! transcription is reliable and the church-book path has no resolvable scan to interpret. The `ai`
//! seam stays wired for a future gothic-transcription path (ADR 0017 §4).

wit_bindgen::generate!({
    world: "assisted-import",
    path: "../../crates/vitni-plugin-host/wit",
    with: {
        "vitni:host-api/types@0.29.0": vitni_plugin_api::types,
        "vitni:host-api/log@0.29.0": vitni_plugin_api::log,
        "vitni:host-api/query@0.29.0": vitni_plugin_api::query,
        "vitni:host-api/staging@0.29.0": vitni_plugin_api::staging,
        "vitni:host-api/progress@0.29.0": vitni_plugin_api::progress,
        "vitni:host-api/net@0.29.0": vitni_plugin_api::net,
        "vitni:host-api/media-store@0.29.0": vitni_plugin_api::media_store,
        "vitni:host-api/ai@0.29.0": vitni_plugin_api::ai,
        "vitni:host-api/present@0.29.0": vitni_plugin_api::present,
    },
});

use vitni_digitalarkivet::{
    AUTHORITY, PageKind, ParseError, PersonRecord, REPOSITORY, census_year, classify_url, extract_urn,
    parse_person_page, parse_residence_page, parse_viewer_page, slugify, suggest_filename,
};
use vitni_plugin_api::staging::{
    EntityFields, EntityKind, EntityRef, LinkKind, MediaLink, PairLink, RepositoryLink, StagedCitation, StagedMedia,
    StagedPerson, StagedRepository, StagedSource, SubmitOutcome,
};
use vitni_plugin_api::types::{
    Confidence, ExternalId, Fact, FactType, MediaCrop, NameType, PersonName, SourceMediaType,
};
use vitni_plugin_api::{Graph, log_info, log_warn, media_store, origin_ref, report};

mod contract;
mod graph;

use contract::{Payload, Response, Suggestion};
use graph::{References, add_record_content};

/// The numbered media-library category folders offered in the save-scan dialog (the owner's archive
/// convention; the host `media-store` is convention-free). Unioned with existing folders wizard-side.
const CATEGORIES: &[&str] = &[
    "01_kirkebok",
    "02_folketelling",
    "03_emigrasjon",
    "04_skifter",
    "05_personbilder",
    "06_gravminner",
    "07_dokumenter",
    "99_inbox",
];

struct Importer;

/// The running session's cross-record state: the scan filed once per source page, and the summary
/// accumulator.
#[derive(Default)]
struct Session {
    /// The scan stored for this source page (filed once, reused by every record on it).
    stored: Option<StoredScan>,
    /// The imported records, for the summary (human id + display name).
    imported: Vec<(String, String)>,
    /// How many records the user skipped.
    skipped: u32,
}

/// A scan filed into the media library through `media-store`.
#[derive(Clone)]
struct StoredScan {
    relative_path: String,
    checksum: String,
    mime: String,
}

/// The outcome of reviewing one record.
enum Outcome {
    /// The record was imported.
    Imported,
    /// The user skipped the record.
    Skipped,
    /// The user cancelled the session.
    Cancelled,
    /// The user pressed Back on the confirm stage — return to the records list (residence flow).
    Back,
    /// The user pressed Back on the save-scan dialog — re-present the same record's confirm stage.
    /// Consumed inside [`review`]; never escapes to the flow.
    BackToConfirm,
}

/// The result of the save-scan step: a filed scan, a cancelled session, or a Back to the confirm stage.
enum ScanStep {
    /// The scan was filed under the media library.
    Stored(StoredScan),
    /// The user cancelled the session from the save-scan dialog.
    Cancelled,
    /// The user pressed Back on the save-scan dialog — re-present the confirm stage.
    Back,
}

impl Guest for Importer {
    fn run_assisted(request: String) -> Result<String, String> {
        let request: contract::Request =
            serde_json::from_str(&request).map_err(|error| format!("invalid assisted-import request: {error}"))?;
        if request.kind != "url" {
            return Err(format!("unsupported request kind: {}", request.kind));
        }
        log_info(&format!("assisted import: {}", request.url));
        let mut session = Session::default();
        match page_kind_of(&request) {
            PageKind::CensusResidence => residence_flow(&request.url, &mut session)?,
            PageKind::CensusPerson | PageKind::ChurchbookRecord => single_flow(&request.url, &mut session)?,
            PageKind::Unknown => return Err(format!("not a recognized Digitalarkivet record URL: {}", request.url)),
        }
        summary(&session)
    }
}

/// The page kind driving the flow: the request's explicit `page` override when present, else
/// [`classify_url`] on the request URL (the GUI's path).
fn page_kind_of(request: &contract::Request) -> PageKind {
    match request.page.as_deref() {
        Some("census-person") => PageKind::CensusPerson,
        Some("census-residence") => PageKind::CensusResidence,
        Some("churchbook-record") => PageKind::ChurchbookRecord,
        Some(_) => PageKind::Unknown,
        None => classify_url(&request.url),
    }
}

/// The single-record flow (a census person or a church-book record): fetch, parse, review, import.
fn single_flow(url: &str, session: &mut Session) -> Result<(), String> {
    let html = fetch(url)?;
    let record = parse_person_page(&html, url).map_err(|error| format!("parsing {url} failed: {error}"))?;
    let scan_url = resolve_scan_url(&record);
    review(&record, scan_url.as_deref(), session)?;
    Ok(())
}

/// The residence flow: fetch the household page, fetch and parse each linked person page, then loop —
/// present the records list, review each picked record — until the user finishes or cancels.
fn residence_flow(url: &str, session: &mut Session) -> Result<(), String> {
    let html = fetch(url)?;
    let residence = parse_residence_page(&html, url).map_err(|error| format!("parsing {url} failed: {error}"))?;
    let records = fetch_household(&residence.person_links)?;
    if records.is_empty() {
        return Ok(());
    }
    loop {
        let response = show(&Payload::records(url, &records))?;
        let response: Response = parse_response(&response)?;
        let Response::Submit { action, values } = response else {
            return Ok(()); // cancel from the records list ends the session
        };
        if action != "select" {
            return Ok(()); // "done" (or any non-select) finishes the session
        }
        let Some(record) = values
            .row
            .and_then(|row| records.iter().find(|r| r.external_id.value == row))
        else {
            continue;
        };
        let scan_url = resolve_scan_url(record);
        if matches!(review(record, scan_url.as_deref(), session)?, Outcome::Cancelled) {
            return Ok(());
        }
    }
}

/// Fetches and parses every linked household person page, reporting progress and tolerating a page
/// that fails to fetch or parse (logged and skipped, never fatal).
fn fetch_household(links: &[String]) -> Result<Vec<PersonRecord>, String> {
    let total = links.len() as u32;
    let mut records = Vec::new();
    for (index, link) in links.iter().enumerate() {
        if !report("fetching household", index as u32, Some(total))? {
            break; // the frontend cancelled the fetch
        }
        match fetch(link)
            .and_then(|html| parse_person_page(&html, link).map_err(|error| format!("parsing {link} failed: {error}")))
        {
            Ok(record) => records.push(record),
            Err(error) => log_warn(&format!("skipping a household member: {error}")),
        }
    }
    Ok(records)
}

/// Presents one record's confirm stage and, on import, files the scan (once) and submits the record
/// through `staging`. Returns which outcome the user chose.
fn review(record: &PersonRecord, scan_url: Option<&str>, session: &mut Session) -> Result<Outcome, String> {
    loop {
        let response = show(&Payload::confirm(record, scan_url))?;
        let Response::Submit { action, values } = parse_response(&response)? else {
            return Ok(Outcome::Cancelled);
        };
        match action.as_str() {
            "back" => return Ok(Outcome::Back),
            "import" => match import(record, scan_url, &values, session)? {
                // Back from the save-scan dialog: re-present this record's confirm stage.
                Outcome::BackToConfirm => continue,
                outcome => return Ok(outcome),
            },
            _ => {
                session.skipped += 1;
                return Ok(Outcome::Skipped);
            }
        }
    }
}

/// Records a confirmed record: files the scan first (so cancelling the save dialog aborts before any
/// write), then submits the record's graph — the person with its occupation, the citation of the
/// source, the scan, the birth, and the census (with the household's family) or the church-book event
/// — under the record's origin, so a re-run writes only what changed (ADR 0037 §4). The source,
/// repository, scan, events, places and family it references go with it, so a record the host
/// withholds withholds them too. The host may ask the user about the record's possible matches first
/// (ADR 0040 §4); a record skipped or a session cancelled there writes nothing.
fn import(
    record: &PersonRecord,
    scan_url: Option<&str>,
    values: &contract::Values,
    session: &mut Session,
) -> Result<Outcome, String> {
    // The user may paste or edit the scan URL on the confirm form (e.g. a 1910 page the plugin could
    // not resolve a scan for); their value wins over the auto-resolved one.
    let effective = values
        .scan_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .or(scan_url);
    let stored = match effective {
        Some(url) => match ensure_scan(url, record, session)? {
            ScanStep::Stored(stored) => Some(stored),
            ScanStep::Cancelled => return Ok(Outcome::Cancelled),
            ScanStep::Back => return Ok(Outcome::BackToConfirm),
        },
        None => None,
    };

    let record = edited(record, values);
    let mut references = References::default();
    let repository = origin_ref(EntityKind::Repository, REPOSITORY_RECORD, None);
    references.add(REPOSITORY_RECORD, repository_graph);
    let source_key = format!("source:{}", source_title(&record));
    references.add(&source_key, |key| source_graph(key, &record, repository));
    let source = origin_ref(EntityKind::Source, &source_key, None);
    let media = stored.as_ref().map(|stored| {
        let key = media_key(stored, effective);
        references.add(&key, |key| media_graph(key, stored));
        origin_ref(EntityKind::Media, &key, None)
    });
    let mut graph = Graph::new(&record.external_id.value);
    let person = graph.entity(None, person_fields(&record));
    let citation = graph.entity(
        Some("citation"),
        EntityFields::Citation(StagedCitation {
            source,
            page: Some(citation_locator(&record, effective)),
            confidence: Some(confidence(values.confidence.as_deref())),
            restrictions: Vec::new(),
        }),
    );
    graph.link(
        None,
        LinkKind::CitationOf(PairLink {
            owner: person.clone(),
            target: citation,
        }),
    );
    if let Some(media) = media {
        graph.link(
            None,
            LinkKind::MediaOf(MediaLink {
                owner: person.clone(),
                media,
                crop: values.region.map(to_crop),
                caption: None,
            }),
        );
    }
    add_record_content(&mut graph, &mut references, &record, &person);
    let human_id = match graph.submit_with(references.into_graphs())? {
        SubmitOutcome::Skipped => {
            session.skipped += 1;
            return Ok(Outcome::Skipped);
        }
        SubmitOutcome::Cancelled => return Ok(Outcome::Cancelled),
        outcome @ (SubmitOutcome::Staged | SubmitOutcome::Committed(_)) => {
            committed(&outcome, &record.external_id.value)?
        }
    };
    session.imported.push((human_id, record.name));
    Ok(Outcome::Imported)
}

/// `record` with the user's edits on the confirm form: an edited field replaces the parsed value,
/// and an emptied one clears it (the name excepted, which keeps the parsed name).
fn edited(record: &PersonRecord, values: &contract::Values) -> PersonRecord {
    let mut record = record.clone();
    for field in &values.fields {
        let value = Some(field.value.trim().to_owned()).filter(|value| !value.is_empty());
        let slot = match field.key.as_str() {
            "name" => {
                if let Some(name) = value {
                    record.name = name;
                }
                continue;
            }
            "birth" => &mut record.birth,
            "birthplace" => &mut record.birthplace,
            "residence" => &mut record.residence,
            "role" => &mut record.role,
            "marital-status" => &mut record.marital_status,
            "occupation" => &mut record.occupation,
            _ => continue,
        };
        *slot = value;
    }
    record
}

/// The person a record names: its name, occupation, and external id.
fn person_fields(record: &PersonRecord) -> EntityFields {
    EntityFields::Person(StagedPerson {
        names: person_name(&record.name).into_iter().collect(),
        sex: None,
        facts: record
            .occupation
            .iter()
            .map(|occupation| Fact {
                fact_type: FactType::Occupation,
                value: Some(occupation.clone()),
                date: None,
            })
            .collect(),
        external_ids: vec![external_id(record)],
        restrictions: Vec::new(),
    })
}

/// The human id the host gave the own entity (local id 0) of the submitted record `record`.
fn committed(outcome: &SubmitOutcome, record: &str) -> Result<String, String> {
    let SubmitOutcome::Committed(entities) = outcome else {
        return Err("the host held an assisted record instead of writing it".to_owned());
    };
    entities
        .iter()
        .find(|entity| entity.record == record && entity.local_id == 0)
        .map(|entity| entity.human_id.clone())
        .ok_or_else(|| format!("the host wrote no record for {record}"))
}

/// Files the scan into the media library, once per source page: presents the save-scan dialog (only
/// the first time), then downloads and stores the permanent image through `media-store`. Returns
/// `None` when the user cancels the dialog (import is aborted).
fn ensure_scan(scan_url: &str, record: &PersonRecord, session: &mut Session) -> Result<ScanStep, String> {
    if let Some(stored) = &session.stored {
        return Ok(ScanStep::Stored(stored.clone()));
    }
    let response = show(&Payload::save_scan(suggestion(record), CATEGORIES))?;
    let Response::Submit { action, values } = parse_response(&response)? else {
        return Ok(ScanStep::Cancelled);
    };
    if action == "back" {
        return Ok(ScanStep::Back); // re-present the confirm stage
    }
    let Some(save) = values.save else {
        return Ok(ScanStep::Cancelled);
    };
    let stored = media_store::fetch_and_store(scan_url, &save.rel_path())
        .map_err(|error| format!("fetch-and-store failed: {error:?}"))?;
    let scan = StoredScan {
        relative_path: media_root_relative(stored.relative_path),
        checksum: stored.checksum,
        mime: stored.mime,
    };
    log_info(&format!("stored scan {} ({})", scan.relative_path, scan.checksum));
    session.stored = Some(scan.clone());
    Ok(ScanStep::Stored(scan))
}

/// Strips a single leading `media/` segment from a `media-store` result path. `media-store` returns a
/// **workspace-relative** path (`media/…`), but `MediaPath::File` and the GUI asset handler expect a
/// **media-root-relative** path (`02_folketelling/…`); persisting the `media/`-prefixed form doubles
/// the segment and the served image 404s. (A future "add file to media library" action must do the
/// same at its own `media-store` boundary.)
fn media_root_relative(path: String) -> String {
    match path.strip_prefix("media/") {
        Some(rest) => rest.to_owned(),
        None => path,
    }
}

/// The record of the managing repository's graph.
const REPOSITORY_RECORD: &str = "repository:arkivverket";

/// The citing source's title. A listing's source has no record id on the page, so its title is its
/// record (`source:{title}`).
fn source_title(record: &PersonRecord) -> String {
    record.source.title.clone().unwrap_or_else(|| record.record_url.clone())
}

/// The citing source's graph `key`, held by `repository`.
fn source_graph(key: &str, record: &PersonRecord, repository: EntityRef) -> Graph {
    let mut graph = Graph::new(key);
    let entity = graph.entity(
        None,
        EntityFields::Source(StagedSource {
            title: Some(source_title(record)),
            author: None,
            pub_info: None,
            abbrev: None,
            restrictions: Vec::new(),
        }),
    );
    // No call number or medium concept in a Digitalarkivet page; both default (unspecified).
    graph.link(
        None,
        LinkKind::SourceRepository(RepositoryLink {
            source: entity,
            repository,
            call_number: None,
            media_type: SourceMediaType::Custom(String::new()),
        }),
    );
    graph
}

/// The managing repository's graph (`Digitalarkivet (Arkivverket)`).
fn repository_graph(key: &str) -> Graph {
    let mut graph = Graph::new(key);
    graph.entity(
        None,
        EntityFields::Repository(StagedRepository {
            name: REPOSITORY.to_owned(),
            restrictions: Vec::new(),
        }),
    );
    graph
}

/// The stored scan's record: the scan's URL, since the filing path is the operator's choice and
/// changes between runs.
fn media_key(stored: &StoredScan, scan_url: Option<&str>) -> String {
    match scan_url {
        Some(url) => format!("scan:{url}"),
        None => format!("file:{}", stored.relative_path),
    }
}

/// The stored scan's media graph `key`, with its MIME type.
fn media_graph(key: &str, stored: &StoredScan) -> Graph {
    let mut graph = Graph::new(key);
    graph.entity(
        None,
        EntityFields::Media(StagedMedia {
            path: Some(stored.relative_path.clone()),
            mime: Some(stored.mime.clone()),
            restrictions: Vec::new(),
        }),
    );
    graph
}

/// Presents the session summary and returns it as the invocation result.
fn summary(session: &Session) -> Result<String, String> {
    let payload = Payload::summary(&session.imported, session.skipped);
    // The wizard shows the summary; its response (done/cancel) does not change the outcome.
    let _ = show(&payload)?;
    serde_json::to_string(&payload).map_err(|error| format!("serializing summary failed: {error}"))
}

/// Resolves the permanent scan image URL from a record's viewer page, degrading gracefully: a
/// church-book IIIF viewer (no permanent image) or any viewer failure yields `None` (import without a
/// scan).
fn resolve_scan_url(record: &PersonRecord) -> Option<String> {
    let viewer_url = record.scan_viewer_url.as_deref()?;
    let html = match fetch(viewer_url) {
        Ok(html) => html,
        Err(error) => {
            log_warn(&format!("fetching the scan viewer failed: {error}"));
            return None;
        }
    };
    match parse_viewer_page(&html, viewer_url) {
        Ok(image_url) => Some(image_url),
        Err(ParseError::ImageUrlNotFound { .. }) => {
            log_warn("church-book IIIF viewer carries no permanent image; importing without a scan");
            None
        }
        Err(error) => {
            log_warn(&format!("resolving the scan image failed: {error}"));
            None
        }
    }
}

/// GETs `url` over the host `net` capability and decodes the body as UTF-8, lossily.
fn fetch(url: &str) -> Result<String, String> {
    let bytes = vitni_plugin_api::fetch_bytes(url)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Parses a wizard response, mapping a parse failure to a message.
fn parse_response(json: &str) -> Result<Response, String> {
    serde_json::from_str(json).map_err(|error| format!("parsing the wizard response failed: {error}"))
}

/// Serializes and shows `payload`, returning the raw response JSON (a thin wrapper over the host
/// `present` capability).
fn show(payload: &Payload) -> Result<String, String> {
    let json = serde_json::to_string(payload).map_err(|error| format!("serializing the payload failed: {error}"))?;
    vitni_plugin_api::present(&json)
}

/// Builds the `ExternalId` a record resolves-or-creates by: authority `digitalarkivet`, the record
/// id as the value, the page kind as the kind, and the record URL.
fn external_id(record: &PersonRecord) -> ExternalId {
    ExternalId {
        authority: AUTHORITY.to_owned(),
        value: record.external_id.value.clone(),
        kind: Some(page_kind(record.page_kind).to_owned()),
        url: Some(record.record_url.clone()),
    }
}

/// A stable label for a page kind, used as the external id's `kind`.
fn page_kind(kind: PageKind) -> &'static str {
    match kind {
        PageKind::CensusPerson | PageKind::CensusResidence => "census-person",
        PageKind::ChurchbookRecord => "churchbook-record",
        PageKind::Unknown => "record",
    }
}

/// The citation locator: the scan's stable `URN:NBN:…` when a scan resolved, else the record URL.
/// The retrieval date is not embedded — the plugin has no clock; the host stamps each assertion's
/// `occurred_at`, which carries the when.
fn citation_locator(record: &PersonRecord, scan_url: Option<&str>) -> String {
    scan_url
        .and_then(extract_urn)
        .unwrap_or_else(|| record.record_url.clone())
}

/// Splits a full name into a WIT `person-name`: the last whitespace-separated token is the surname,
/// the rest the given name. A single token (or empty) becomes a given name only.
fn person_name(full: &str) -> Option<PersonName> {
    let full = full.trim();
    if full.is_empty() {
        return None;
    }
    let (given, surname) = match full.rsplit_once(char::is_whitespace) {
        Some((given, surname)) if !given.trim().is_empty() && !surname.trim().is_empty() => {
            (Some(given.trim().to_owned()), Some(surname.trim().to_owned()))
        }
        _ => (Some(full.to_owned()), None),
    };
    Some(PersonName {
        name_type: NameType::BirthName,
        given,
        surname_prefix: None,
        surname,
        nickname: None,
        prefix: None,
        suffix: None,
    })
}

/// Maps a wizard confidence token onto the WIT `confidence` enum; the assisted flow defaults to `Low`.
fn confidence(token: Option<&str>) -> Confidence {
    match token {
        Some("very-low") => Confidence::VeryLow,
        Some("normal") => Confidence::Normal,
        Some("high") => Confidence::High,
        Some("very-high") => Confidence::VeryHigh,
        _ => Confidence::Low,
    }
}

/// Maps a confirmed region onto the WIT `media-crop` record.
fn to_crop(region: contract::Region) -> MediaCrop {
    MediaCrop {
        left: region.left,
        top: region.top,
        width: region.width,
        height: region.height,
    }
}

/// The proposed media-library filing target for a record's scan: a numbered category by page kind, a
/// year subfolder, and a `{year}_{place}_{event}_{name}.jpg` filename (slugified, æøå kept).
fn suggestion(record: &PersonRecord) -> Suggestion {
    let (category, event) = match record.page_kind {
        PageKind::ChurchbookRecord => ("01_kirkebok", "kirkebok"),
        PageKind::CensusPerson | PageKind::CensusResidence | PageKind::Unknown => ("02_folketelling", "folketelling"),
    };
    let year = record.source.year.clone().unwrap_or_default();
    let place = record
        .residence
        .clone()
        .or_else(|| record.birthplace.clone())
        .unwrap_or_default();
    let filename = suggest_filename(&year, &place, event, &record.name, "jpg");
    let filename = if filename.is_empty() {
        format!("{}.jpg", slugify(&record.external_id.value))
    } else {
        filename
    };
    Suggestion {
        category: category.to_owned(),
        subfolder: census_year(&year).to_owned(),
        filename,
    }
}

export!(Importer);
