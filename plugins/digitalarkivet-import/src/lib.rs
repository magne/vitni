//! Digitalarkivet assisted-import plugin (ADR 0017 §5, §6): one long `run-assisted` invocation drives
//! a record-by-record review-and-import session over the sandboxed host capabilities.
//!
//! Flow (prototype-proven, `sort-inbox.py`): classify the request URL, fetch the page(s) over `net`,
//! parse them with the pure `genealogy-digitalarkivet` crate, present each record to the user through
//! `present` (suspending until they confirm or skip), file the scan once per source page through
//! `media-store`, and record the confirmed record as low-confidence Software-agent assertions through
//! `commands`, resolving-or-creating by `ExternalId` so a re-run imports no duplicates.
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
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.21.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.21.0": genealogy_plugin_api::log,
        "genealogy:host-api/query@0.21.0": genealogy_plugin_api::query,
        "genealogy:host-api/commands@0.21.0": genealogy_plugin_api::commands,
        "genealogy:host-api/progress@0.21.0": genealogy_plugin_api::progress,
        "genealogy:host-api/net@0.21.0": genealogy_plugin_api::net,
        "genealogy:host-api/media-store@0.21.0": genealogy_plugin_api::media_store,
        "genealogy:host-api/ai@0.21.0": genealogy_plugin_api::ai,
        "genealogy:host-api/present@0.21.0": genealogy_plugin_api::present,
    },
});

use genealogy_digitalarkivet::{
    AUTHORITY, PageKind, ParseError, PersonRecord, REPOSITORY, census_year, classify_url, extract_urn,
    parse_person_page, parse_residence_page, parse_viewer_page, slugify, suggest_filename,
};
use genealogy_plugin_api::types::{Confidence, ExternalId, FactType, MediaCrop, NameType, PersonName, SourceMediaType};
use genealogy_plugin_api::{commands, log_info, log_warn, media_store, query, report};

mod contract;

use contract::{Payload, Response, Suggestion};

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

/// The running session's cross-record state: the scan filed once per source page, the deduped
/// source/repository/media ids, and the summary accumulator.
#[derive(Default)]
struct Session {
    /// The scan stored for this source page (filed once, reused by every record on it).
    stored: Option<StoredScan>,
    /// The source human id (deduped by title within the run and against existing sources).
    source: Option<String>,
    /// The managing repository human id (deduped by name).
    repository: Option<String>,
    /// The media human id for the stored scan (deduped by path).
    media: Option<String>,
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
        let Some(record) = values.row.and_then(|row| records.iter().find(|r| r.external_id.value == row)) else {
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
        match fetch(link).and_then(|html| {
            parse_person_page(&html, link).map_err(|error| format!("parsing {link} failed: {error}"))
        }) {
            Ok(record) => records.push(record),
            Err(error) => log_warn(&format!("skipping a household member: {error}")),
        }
    }
    Ok(records)
}

/// Presents one record's confirm stage and, on import, files the scan (once) and records the
/// person, source, citation, and media through `commands`. Returns which outcome the user chose.
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
/// write), then resolves-or-creates the person by `ExternalId` and — only on first creation — the
/// source, citation, and media, keeping a re-run idempotent.
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

    let name = field_value(values, "name").unwrap_or_else(|| record.name.clone());
    let person = commands::create_person(person_name(&name).as_ref(), Some(&external_id(record)))
        .map_err(|error| format!("create-person failed: {error:?}"))?;
    if person.created {
        record_claims(&person.human_id, record, effective, values, stored.as_ref(), session)?;
    }
    session.imported.push((person.human_id, name));
    Ok(Outcome::Imported)
}

/// Records the owned claims for a newly-created person: an occupation fact, the source + citation
/// (confidence from the confirm form), and — when a scan was filed — the media with the user's crop.
fn record_claims(
    person: &str,
    record: &PersonRecord,
    scan_url: Option<&str>,
    values: &contract::Values,
    stored: Option<&StoredScan>,
    session: &mut Session,
) -> Result<(), String> {
    if let Some(occupation) = field_value(values, "occupation").filter(|value| !value.trim().is_empty()) {
        commands::assert_fact(person, &FactType::Occupation, Some(&occupation), None)
            .map_err(|error| format!("assert-fact failed: {error:?}"))?;
    }
    let source = ensure_source(record, session)?;
    let citation = commands::create_citation(&source, Some(&citation_locator(record, scan_url)))
        .map_err(|error| format!("create-citation failed: {error:?}"))?;
    commands::set_citation_confidence(&citation, confidence(values.confidence.as_deref()))
        .map_err(|error| format!("set-citation-confidence failed: {error:?}"))?;
    commands::attach_person_citation(person, &citation).map_err(|error| format!("attach-citation failed: {error:?}"))?;
    if let Some(stored) = stored {
        let media = ensure_media(stored, session)?;
        let crop = values.region.map(to_crop);
        commands::attach_person_media(person, &media, crop, None)
            .map_err(|error| format!("attach-media failed: {error:?}"))?;
    }
    Ok(())
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

/// Resolves-or-creates the citing source, deduping by title within the run and against existing
/// sources, and links its managing repository. Cached on the session for the rest of the page.
fn ensure_source(record: &PersonRecord, session: &mut Session) -> Result<String, String> {
    if let Some(source) = &session.source {
        return Ok(source.clone());
    }
    let title = record.source.title.clone().unwrap_or_else(|| record.record_url.clone());
    if let Ok(sources) = query::list_sources()
        && let Some(existing) = sources.iter().find(|source| source.title.as_deref() == Some(title.as_str()))
    {
        session.source = Some(existing.human_id.clone());
        return Ok(existing.human_id.clone());
    }
    let source = commands::create_source(Some(&title)).map_err(|error| format!("create-source failed: {error:?}"))?;
    let repository = ensure_repository(session)?;
    // No call number or medium concept in a Digitalarkivet page; both default (unspecified).
    commands::link_source_repository(&source, &repository, None, &SourceMediaType::Custom(String::new()))
        .map_err(|error| format!("link-source-repository failed: {error:?}"))?;
    session.source = Some(source.clone());
    Ok(source)
}

/// Resolves-or-creates the managing repository (`Digitalarkivet (Arkivverket)`), deduping by name.
fn ensure_repository(session: &mut Session) -> Result<String, String> {
    if let Some(repository) = &session.repository {
        return Ok(repository.clone());
    }
    if let Ok(repositories) = query::list_repositories()
        && let Some(existing) = repositories.iter().find(|repo| repo.name.as_deref() == Some(REPOSITORY))
    {
        session.repository = Some(existing.human_id.clone());
        return Ok(existing.human_id.clone());
    }
    let repository = commands::create_repository(REPOSITORY).map_err(|error| format!("create-repository failed: {error:?}"))?;
    session.repository = Some(repository.clone());
    Ok(repository)
}

/// Resolves-or-creates the media object for the stored scan, deduping by path (within the run and
/// against existing media), and setting its MIME type.
fn ensure_media(stored: &StoredScan, session: &mut Session) -> Result<String, String> {
    if let Some(media) = &session.media {
        return Ok(media.clone());
    }
    if let Ok(objects) = query::list_media()
        && let Some(existing) = objects.iter().find(|media| media.path.as_deref() == Some(stored.relative_path.as_str()))
    {
        session.media = Some(existing.human_id.clone());
        return Ok(existing.human_id.clone());
    }
    let media = commands::create_media(Some(&stored.relative_path)).map_err(|error| format!("create-media failed: {error:?}"))?;
    commands::set_media_mime(&media, &stored.mime).map_err(|error| format!("set-media-mime failed: {error:?}"))?;
    log_info(&format!("stored scan {} ({})", stored.relative_path, stored.checksum));
    session.media = Some(media.clone());
    Ok(media)
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
    let bytes = genealogy_plugin_api::fetch_bytes(url)?;
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
    genealogy_plugin_api::present(&json)
}

/// The value of a confirm field by key (the user's edited value).
fn field_value(values: &contract::Values, key: &str) -> Option<String> {
    values
        .fields
        .iter()
        .find(|field| field.key == key)
        .map(|field| field.value.clone())
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
