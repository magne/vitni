//! Guest-side support for the Vitni plugins (ADR 0013).
//!
//! This crate generates the host-API **import** bindings once from the shared WIT (`host-imports`
//! world) and re-exports each capability module, so a plugin component maps the shared interfaces to
//! this crate via `with` and only generates its own export. On top of the raw bindings it provides
//! the boilerplate every bulk plugin repeats: draining the host-opened import source, writing the
//! host-resolved export sink, reporting progress, and logging. The host owns the actual path; a
//! plugin reads or writes through these helpers without ever naming a file.

wit_bindgen::generate!({
    world: "host-imports",
    path: "../../crates/vitni-plugin-host/wit",
});

pub use vitni::host_api::{
    ai, commands, export_sink, import_source, log, media_store, net, present, progress, query, types,
};

pub mod convert;

/// The chunk size used when draining the import source.
const CHUNK: u32 = 64 * 1024;

/// Logs `message` at info level through the host `log` capability.
pub fn log_info(message: &str) {
    log::log(log::Level::Info, message);
}

/// Logs `message` at warn level through the host `log` capability.
pub fn log_warn(message: &str) {
    log::log(log::Level::Warn, message);
}

/// Reads the entire host-opened import source into memory (ADR 0013), a chunk at a time.
///
/// # Errors
/// Returns a message if the source cannot be opened or read.
pub fn read_source_to_end() -> Result<Vec<u8>, String> {
    import_source::open().map_err(|error| format!("opening import source failed: {error:?}"))?;
    let mut data = Vec::new();
    loop {
        let chunk = import_source::read(CHUNK).map_err(|error| format!("reading import source failed: {error:?}"))?;
        if chunk.is_empty() {
            break;
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

/// Reads the import source and decodes it as UTF-8, lossily.
///
/// Real-world exports (MyHeritage, older Gramps) declare UTF-8 but occasionally carry a stray
/// non-UTF-8 byte; decoding lossily (invalid bytes become U+FFFD) lets the rest of the document
/// import rather than failing the whole run on one bad byte.
///
/// # Errors
/// Returns a message if the source cannot be read.
pub fn read_source_to_string() -> Result<String, String> {
    let bytes = read_source_to_end()?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Writes `bytes` to the host-resolved export sink under the proposed `suggested_name` (ADR 0013).
///
/// # Errors
/// Returns a message if the sink cannot be opened, written, or flushed.
pub fn write_export(suggested_name: &str, bytes: &[u8]) -> Result<(), String> {
    export_sink::open(suggested_name).map_err(|error| format!("opening export sink failed: {error:?}"))?;
    export_sink::write(bytes).map_err(|error| format!("writing export failed: {error:?}"))?;
    export_sink::finish().map_err(|error| format!("finishing export failed: {error:?}"))
}

/// GETs `url` through the host `net` capability (ADR 0017 §2) and returns the response body,
/// discarding the status and headers. A convenience for the common "fetch a page, hand the bytes to
/// a parser" flow; use [`net::fetch`] directly when the status or headers matter.
///
/// # Errors
/// Returns a message if the host denies the capability or the fetch violates the net policy.
pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    net::fetch(url)
        .map(|response| response.body)
        .map_err(|error| format!("fetching {url} failed: {error:?}"))
}

/// Interprets the media file at `media_path` (a `media-store` relative path) with `prompt` through the
/// host `ai` capability (ADR 0017 §4), using the provider named by `provider` (or the configured
/// default when `None`). Returns the model's raw text; the caller owns any JSON extraction.
///
/// # Errors
/// Returns a message if the host denies the capability, the provider is unknown, or the provider
/// fails.
pub fn interpret(provider: Option<&str>, media_path: &str, prompt: &str) -> Result<String, String> {
    ai::interpret_media(provider, media_path, prompt).map_err(|error| format!("interpreting {media_path} failed: {error:?}"))
}

/// Shows `payload` to the frontend through the host `present` capability (ADR 0017 §5) and suspends
/// until the user answers, returning their response verbatim. Both strings are the typed
/// assisted-import presentation contract (`vitni-ui`), opaque to the host; the plugin owns
/// serializing the payload and parsing the response.
///
/// # Errors
/// Returns a message if the host denies the capability or the frontend channel is unavailable
/// (a cancelled/gone wizard surfaces as a `backend` error).
pub fn present(payload: &str) -> Result<String, String> {
    present::show(payload).map_err(|error| format!("presenting to the frontend failed: {error:?}"))
}

/// Reports progress of a bulk operation through the host `progress` capability (ADR 0013). `total`
/// is `None` when the count is not yet known. Returns `true` to keep going, `false` if the frontend
/// asked to cancel — a bulk plugin must stop promptly when it sees `false`, returning what it has
/// done so far.
///
/// # Errors
/// Returns a message if the host rejects the report.
pub fn report(step: &str, processed: u32, total: Option<u32>) -> Result<bool, String> {
    match progress::report(step, processed, total) {
        Ok(progress::Control::Proceed) => Ok(true),
        Ok(progress::Control::Cancel) => Ok(false),
        Err(error) => Err(format!("reporting progress failed: {error:?}")),
    }
}
