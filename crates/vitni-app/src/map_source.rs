//! The resolved map source (ADR 0025 §3, ADR 0033): the frontend-neutral basemap descriptor a
//! renderer mounts, resolved from a configured [`MapProvider`] — substituting the `{key}` env
//! placeholder for a `MapLibre` style, or minting a Google Map Tiles session. Map network I/O lives
//! here, in the app layer (ADR 0008): `vitni-ui-dioxus` never fetches, it only renders whatever
//! [`MapSource`] this module hands back.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;

use crate::config::MapProvider;
use crate::error::AppError;

/// The tile/style source a renderer mounts, resolved from a [`MapProvider`]. Unlike the configured
/// provider (which may name a style URL with an unresolved `{key}` placeholder, or no tile URL at
/// all for Google), this is exactly what the map fetches — a renderer paints this and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapBasemap {
    /// A raster XYZ tile source.
    Raster {
        /// The resolved `{z}/{x}/{y}` tile URL template.
        tile_url: String,
        /// The tile image's pixel size (square).
        tile_size: u32,
        /// The last zoom the source serves; the map overzooms beyond it rather than 404ing.
        max_zoom: u8,
    },
    /// A whole `MapLibre` style document.
    Style {
        /// The resolved style URL (any `{key}` placeholder already substituted).
        style_url: String,
    },
}

/// The resolved basemap plus the attribution to display over it (ADR 0025 §3). Resolved as a pair so
/// a raster source and a vector style can never be paired with the wrong provider's credit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSource {
    /// What the renderer mounts.
    pub basemap: MapBasemap,
    /// The credit shown over the map, per the provider's terms. For a live Google session this is
    /// only the placeholder shown until the first viewport response lands — see
    /// [`google_viewport_copyright`].
    pub attribution: String,
}

/// The XYZ raster tile pixel size the built-in OSM default and every other raster provider serve
/// (256px, the universal slippy-map tile size).
const DEFAULT_RASTER_TILE_SIZE: u32 = 256;

/// The last zoom the built-in OSM default (and any other raster provider naming no zoom of its own)
/// serves. Matches the map's own camera ceiling (`vitni_ui::ZOOM_RANGE.1`) today — kept as a
/// literal here since `vitni-app` sits below `vitni-ui` (ADR 0008) and cannot import it.
const DEFAULT_RASTER_MAX_ZOOM: u8 = 19;

/// The last zoom Google's Map Tiles API serves (`developers.google.com/maps/documentation/tile/2d-tiles-overview`).
const GOOGLE_MAX_ZOOM: u8 = 22;

/// How long a minted Google session is trusted before this process mints a fresh one. Google's own
/// tokens are valid two weeks; this is set well under that so a long-running app never presents an
/// expired session, while still minting at most once per run in the common case.
const GOOGLE_SESSION_TTL: Duration = Duration::from_hours(24 * 7);

/// The `{key}` placeholder substituted into a [`MapProvider::MaplibreStyle`] URL from its
/// `api_key_env` environment variable.
const KEY_PLACEHOLDER: &str = "{key}";

/// Resolves `provider` into the [`MapSource`] a renderer mounts.
///
/// # Errors
///
/// [`AppError::Config`] if a `MapLibre` style names an `api_key_env` that is unset, or names one with
/// no `{key}` placeholder in its URL; or if the Google adapter's session request fails (missing
/// `GOOGLE_MAPS_KEY`-named env var, a network failure, or an unparsable response).
pub async fn resolve_map_source(provider: &MapProvider) -> Result<MapSource, AppError> {
    match provider {
        MapProvider::OsmRaster { tile_url, attribution } => Ok(MapSource {
            basemap: MapBasemap::Raster {
                tile_url: tile_url.clone(),
                tile_size: DEFAULT_RASTER_TILE_SIZE,
                max_zoom: DEFAULT_RASTER_MAX_ZOOM,
            },
            attribution: attribution.clone(),
        }),
        MapProvider::MaplibreStyle {
            style_url,
            attribution,
            api_key_env,
        } => Ok(MapSource {
            basemap: MapBasemap::Style {
                style_url: resolve_style_url(style_url, api_key_env.as_deref())?,
            },
            attribution: attribution.clone(),
        }),
        MapProvider::Google {
            api_key_env,
            attribution,
        } => {
            let api_key = required_env(api_key_env)?;
            let session = google_session(&api_key).await?;
            Ok(MapSource {
                basemap: MapBasemap::Raster {
                    tile_url: google_tile_url(&session.session, &api_key),
                    tile_size: session.tile_width,
                    max_zoom: GOOGLE_MAX_ZOOM,
                },
                attribution: attribution.clone(),
            })
        }
    }
}

/// Substitutes [`KEY_PLACEHOLDER`] in `style_url` from the environment variable named by
/// `api_key_env`, or returns `style_url` verbatim when no `api_key_env` is configured.
///
/// # Errors
///
/// [`AppError::Config`] naming `api_key_env` if it is set but the variable is unset/empty, or if
/// `style_url` has no `{key}` placeholder for it to substitute.
fn resolve_style_url(style_url: &str, api_key_env: Option<&str>) -> Result<String, AppError> {
    let Some(env_name) = api_key_env else {
        return Ok(style_url.to_owned());
    };
    if !style_url.contains(KEY_PLACEHOLDER) {
        return Err(AppError::Config(format!(
            "the map style URL names api-key-env {env_name:?} but has no {KEY_PLACEHOLDER} placeholder to substitute"
        )));
    }
    let key = required_env(env_name)?;
    Ok(style_url.replace(KEY_PLACEHOLDER, &key))
}

/// Reads `name` from the environment, or a named [`AppError::Config`] if it is unset or empty.
fn required_env(name: &str) -> Result<String, AppError> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(AppError::Config(format!(
            "the map provider needs the {name:?} environment variable, which is unset"
        ))),
    }
}

/// The shared HTTP client for every Google Map Tiles request this process makes.
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// A minted Google Map Tiles session (`developers.google.com/maps/documentation/tile/session_tokens`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleSession {
    session: String,
    #[expect(
        dead_code,
        reason = "carried for completeness/parity with the API response; not read yet"
    )]
    expiry: String,
    tile_width: u32,
    #[expect(
        dead_code,
        reason = "carried for completeness/parity with the API response; not read yet"
    )]
    tile_height: u32,
    #[expect(
        dead_code,
        reason = "carried for completeness/parity with the API response; not read yet"
    )]
    image_format: String,
}

/// One process-lifetime cached Google session, keyed by the API key it was minted for.
struct CachedSession {
    api_key: String,
    session: GoogleSession,
    minted_at: Instant,
}

/// The process-lifetime Google session cache (see [`GOOGLE_SESSION_TTL`]'s doc comment) — one mint per
/// run is the target, not one per provider switch.
fn session_cache() -> &'static Mutex<Option<CachedSession>> {
    static CACHE: OnceLock<Mutex<Option<CachedSession>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// The `createSession` request URL (`developers.google.com/maps/documentation/tile/session_tokens`).
fn google_create_session_url(api_key: &str) -> String {
    format!("https://tile.googleapis.com/v1/createSession?key={api_key}")
}

/// The `createSession` request body: a roadmap session in `language`, region `US` (the plan's fixed
/// default — this resolver has no UI-language input to thread through).
fn google_create_session_body() -> serde_json::Value {
    json!({ "mapType": "roadmap", "language": "en", "region": "US" })
}

/// The `{z}/{x}/{y}` tile URL for a minted `session` and `api_key`
/// (`developers.google.com/maps/documentation/tile/roadmap`).
fn google_tile_url(session: &str, api_key: &str) -> String {
    format!("https://tile.googleapis.com/v1/2dtiles/{{z}}/{{x}}/{{y}}?session={session}&key={api_key}")
}

/// The viewport `copyright` request URL for the camera `bounds` (`north, south, east, west`) at
/// `zoom`, over `session`/`api_key`.
fn google_viewport_url(session: &str, api_key: &str, zoom: f64, bounds: (f64, f64, f64, f64)) -> String {
    let (north, south, east, west) = bounds;
    format!(
        "https://tile.googleapis.com/tile/v1/viewport?session={session}&key={api_key}&zoom={zoom}\
         &north={north}&south={south}&east={east}&west={west}"
    )
}

/// The reusable session for `api_key`: the process-lifetime cache when it is fresh, else a newly
/// minted one (cached for the next call).
///
/// # Errors
///
/// [`AppError::Config`] if the `createSession` request fails or its response cannot be parsed.
async fn google_session(api_key: &str) -> Result<GoogleSession, AppError> {
    if let Some(cached) = cached_session(api_key) {
        return Ok(cached);
    }
    let minted = google_create_session(api_key).await?;
    let mut cache = session_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *cache = Some(CachedSession {
        api_key: api_key.to_owned(),
        session: minted.clone(),
        minted_at: Instant::now(),
    });
    Ok(minted)
}

/// The cached session for `api_key`, if one exists and is still within [`GOOGLE_SESSION_TTL`].
fn cached_session(api_key: &str) -> Option<GoogleSession> {
    let cache = session_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cached = cache.as_ref()?;
    (cached.api_key == api_key && cached.minted_at.elapsed() < GOOGLE_SESSION_TTL).then(|| cached.session.clone())
}

/// How much of a failed response's body is kept in the error message.
const MAX_ERROR_BODY_CHARS: usize = 400;

/// Google's standard error envelope, the body every Map Tiles 4xx/5xx carries.
#[derive(Debug, Deserialize)]
struct GoogleErrorEnvelope {
    error: GoogleErrorBody,
}

/// The envelope's inner error; only its human-readable `message` is surfaced.
#[derive(Debug, Deserialize)]
struct GoogleErrorBody {
    message: String,
}

/// The error message for a failed Google Map Tiles request: the status plus **Google's own message**,
/// which is the only part that names the cause. A bare status cannot distinguish the three ways a 403
/// happens here — the Map Tiles API not enabled on the project, no billing account, or an
/// HTTP-referrer-restricted key rejecting this server-side call — and the first report of exactly that
/// left a user with nothing to act on. The API key travels in the request URL and is never echoed in a
/// response body, so nothing secret reaches this string.
fn google_failure_message(what: &str, status: reqwest::StatusCode, body: &str) -> String {
    match google_error_detail(body) {
        Some(detail) => format!("the Google Maps {what} request failed with {status}: {detail}"),
        None => format!("the Google Maps {what} request failed with {status}"),
    }
}

/// The reportable detail in a failed response's `body`: Google's `error.message` when the body is its
/// standard envelope, else the raw text — either way truncated to [`MAX_ERROR_BODY_CHARS`] characters
/// (not bytes, so a multi-byte character is never split). `None` for an empty body.
fn google_error_detail(body: &str) -> Option<String> {
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let detail = serde_json::from_str::<GoogleErrorEnvelope>(body)
        .map_or_else(|_| body.to_owned(), |envelope| envelope.error.message);
    let truncated: String = detail.chars().take(MAX_ERROR_BODY_CHARS).collect();
    Some(truncated)
}

/// The [`AppError`] for a non-success Map Tiles response, consuming it to read the body
/// [`google_failure_message`] reports.
async fn google_request_error(what: &str, response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    AppError::Config(google_failure_message(what, status, &body))
}

/// Mints a fresh Google Map Tiles session.
///
/// # Errors
///
/// [`AppError::Config`] if the request fails, the response is not a success status, or the body does
/// not parse as a session.
async fn google_create_session(api_key: &str) -> Result<GoogleSession, AppError> {
    let response = http_client()
        .post(google_create_session_url(api_key))
        .json(&google_create_session_body())
        .send()
        .await
        .map_err(|error| AppError::Config(format!("requesting a Google Maps session: {error}")))?;
    if !response.status().is_success() {
        return Err(google_request_error("session", response).await);
    }
    response
        .json()
        .await
        .map_err(|error| AppError::Config(format!("parsing the Google Maps session response: {error}")))
}

/// The response to a viewport `copyright` request.
#[derive(Debug, Deserialize)]
struct GoogleViewportResponse {
    copyright: String,
}

/// Refreshes the attribution Google's Map Tiles policy requires for the camera at `zoom`/`bounds`
/// (`north, south, east, west`), given the `session`/`api_key` the basemap was resolved with.
///
/// Google's Map Tiles terms require this **variable** string be displayed over the map — a
/// provider-level static `attribution` (as every other kind carries) is only the placeholder shown
/// until this call's first response lands.
///
/// # Errors
///
/// [`AppError::Config`] if the request fails or the response cannot be parsed.
pub async fn google_viewport_copyright(
    session: &str,
    api_key: &str,
    zoom: f64,
    bounds: (f64, f64, f64, f64),
) -> Result<String, AppError> {
    let response = http_client()
        .get(google_viewport_url(session, api_key, zoom, bounds))
        .send()
        .await
        .map_err(|error| AppError::Config(format!("requesting the Google Maps viewport copyright: {error}")))?;
    if !response.status().is_success() {
        return Err(google_request_error("viewport copyright", response).await);
    }
    let parsed: GoogleViewportResponse = response
        .json()
        .await
        .map_err(|error| AppError::Config(format!("parsing the Google Maps viewport copyright response: {error}")))?;
    Ok(parsed.copyright)
}

/// Refreshes the live per-viewport attribution for `provider` at the camera `zoom`/`bounds`, or `None`
/// for every kind but [`MapProvider::Google`] — only Google's terms require a dynamic credit
/// ([`google_viewport_copyright`]'s doc comment). Reuses [`google_session`]'s process-lifetime cache
/// rather than minting a fresh session, so the raw API key never has to leave this module: a caller
/// (the renderer) passes back only the `provider` it was already handed, never a session or key.
///
/// # Errors
///
/// [`AppError::Config`] if `provider` is [`MapProvider::Google`] and its `api_key_env` is unset, or
/// the viewport request fails.
pub async fn refresh_map_attribution(
    provider: &MapProvider,
    zoom: f64,
    bounds: (f64, f64, f64, f64),
) -> Result<Option<String>, AppError> {
    let MapProvider::Google { api_key_env, .. } = provider else {
        return Ok(None);
    };
    let api_key = required_env(api_key_env)?;
    let session = google_session(&api_key).await?;
    google_viewport_copyright(&session.session, &api_key, zoom, bounds)
        .await
        .map(Some)
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::{
        DEFAULT_RASTER_MAX_ZOOM, DEFAULT_RASTER_TILE_SIZE, GOOGLE_MAX_ZOOM, MAX_ERROR_BODY_CHARS, MapBasemap,
        google_create_session_url, google_error_detail, google_failure_message, google_tile_url, google_viewport_url,
        refresh_map_attribution, resolve_map_source, resolve_style_url,
    };
    use crate::config::MapProvider;
    use crate::error::AppError;

    fn osm() -> MapProvider {
        MapProvider::default_osm()
    }

    #[tokio::test]
    async fn an_osm_raster_provider_resolves_verbatim_with_no_io() {
        let source = resolve_map_source(&osm()).await.expect("resolve");
        assert_eq!(
            source.basemap,
            MapBasemap::Raster {
                tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_owned(),
                tile_size: DEFAULT_RASTER_TILE_SIZE,
                max_zoom: DEFAULT_RASTER_MAX_ZOOM,
            }
        );
        assert_eq!(source.attribution, "© OpenStreetMap contributors");
    }

    #[test]
    fn a_style_url_with_no_api_key_env_is_used_verbatim() {
        let resolved = resolve_style_url("https://example.test/style.json", None).expect("resolve");
        assert_eq!(resolved, "https://example.test/style.json");
    }

    /// Each of this module's env-touching tests uses its own uniquely named variable (never a shared
    /// one like `PATH`), so they need no cross-test serialization even under `cargo test`'s default
    /// parallelism — nothing else reads these names.
    #[test]
    fn a_configured_api_key_env_substitutes_the_key_placeholder() {
        unsafe { std::env::set_var("VITNI_TEST_MAP_KEY_SUBSTITUTE", "secret-123") };
        let resolved = resolve_style_url(
            "https://example.test/style.json?key={key}",
            Some("VITNI_TEST_MAP_KEY_SUBSTITUTE"),
        )
        .expect("resolve");
        assert_eq!(resolved, "https://example.test/style.json?key=secret-123");
        unsafe { std::env::remove_var("VITNI_TEST_MAP_KEY_SUBSTITUTE") };
    }

    #[test]
    fn an_unset_api_key_env_is_a_named_config_error() {
        unsafe { std::env::remove_var("VITNI_TEST_MAP_KEY_UNSET") };
        let error = resolve_style_url(
            "https://example.test/style.json?key={key}",
            Some("VITNI_TEST_MAP_KEY_UNSET"),
        )
        .expect_err("the env var is unset");
        let AppError::Config(message) = error else {
            panic!("expected AppError::Config, got {error:?}");
        };
        assert!(
            message.contains("VITNI_TEST_MAP_KEY_UNSET"),
            "names the missing variable: {message}"
        );
    }

    #[test]
    fn an_api_key_env_with_no_placeholder_in_the_url_is_a_named_config_error() {
        unsafe { std::env::set_var("VITNI_TEST_MAP_KEY_NO_PLACEHOLDER", "secret-123") };
        let error = resolve_style_url(
            "https://example.test/style.json",
            Some("VITNI_TEST_MAP_KEY_NO_PLACEHOLDER"),
        )
        .expect_err("the URL has no {key} placeholder");
        let AppError::Config(message) = error else {
            panic!("expected AppError::Config, got {error:?}");
        };
        assert!(
            message.contains("VITNI_TEST_MAP_KEY_NO_PLACEHOLDER"),
            "names the offending env var: {message}"
        );
        unsafe { std::env::remove_var("VITNI_TEST_MAP_KEY_NO_PLACEHOLDER") };
    }

    #[tokio::test]
    async fn a_maplibre_provider_naming_an_unset_env_var_fails_before_any_network_call() {
        let provider = MapProvider::MaplibreStyle {
            style_url: "https://example.test/style.json?key={key}".to_owned(),
            attribution: "© Example".to_owned(),
            api_key_env: Some("VITNI_TEST_MAP_KEY_ALSO_UNSET".to_owned()),
        };
        let error = resolve_map_source(&provider).await.expect_err("the env var is unset");
        assert!(matches!(error, AppError::Config(_)));
    }

    #[test]
    fn the_create_session_url_carries_the_api_key() {
        assert_eq!(
            google_create_session_url("KEY123"),
            "https://tile.googleapis.com/v1/createSession?key=KEY123"
        );
    }

    #[test]
    fn the_tile_url_carries_the_session_and_key_over_the_2d_tiles_template() {
        assert_eq!(
            google_tile_url("SESSION1", "KEY123"),
            "https://tile.googleapis.com/v1/2dtiles/{z}/{x}/{y}?session=SESSION1&key=KEY123"
        );
    }

    #[test]
    fn the_viewport_url_carries_the_camera_bounds_and_zoom() {
        let url = google_viewport_url("SESSION1", "KEY123", 12.5, (60.0, 59.0, 11.0, 10.0));
        assert!(url.starts_with("https://tile.googleapis.com/tile/v1/viewport?"));
        for fragment in [
            "session=SESSION1",
            "key=KEY123",
            "zoom=12.5",
            "north=60",
            "south=59",
            "east=11",
            "west=10",
        ] {
            assert!(url.contains(fragment), "{url} names {fragment}");
        }
    }

    #[test]
    fn googles_ceiling_is_higher_than_the_built_in_rasters() {
        // Google's Map Tiles API serves finer detail than the OSM raster default overzooms past.
        const { assert!(GOOGLE_MAX_ZOOM > DEFAULT_RASTER_MAX_ZOOM) };
    }

    #[test]
    fn a_google_session_response_parses_the_fields_this_module_uses() {
        let json = r#"{"session":"abc","expiry":"1700000000","tileWidth":256,"tileHeight":256,"imageFormat":"png"}"#;
        let parsed: super::GoogleSession = serde_json::from_str(json).expect("parses");
        assert_eq!(parsed.session, "abc");
        assert_eq!(parsed.tile_width, 256);
    }

    #[test]
    fn a_viewport_response_parses_its_copyright() {
        let json = r#"{"copyright":"© 2026 Google","maxZoom":22}"#;
        let parsed: super::GoogleViewportResponse = serde_json::from_str(json).expect("parses");
        assert_eq!(parsed.copyright, "© 2026 Google");
    }

    /// The failure a 403 actually is: the status alone cannot tell an unenabled Map Tiles API from an
    /// unbilled project from a referrer-restricted key, and Google names which one in the body.
    #[test]
    fn a_failed_request_reports_googles_own_error_message() {
        let body = r#"{"error":{"code":403,"message":"Map Tiles API has not been used in project 42 before or it is disabled.","status":"PERMISSION_DENIED"}}"#;
        let message = google_failure_message("session", StatusCode::FORBIDDEN, body);
        assert!(message.contains("403"), "keeps the status: {message}");
        assert!(
            message.contains("Map Tiles API has not been used in project 42"),
            "names Google's own cause: {message}"
        );
        assert!(
            !message.contains("PERMISSION_DENIED"),
            "reports the human-readable message, not the whole envelope: {message}"
        );
    }

    #[test]
    fn a_body_that_is_not_googles_envelope_is_reported_verbatim() {
        let message = google_failure_message("viewport copyright", StatusCode::BAD_GATEWAY, "upstream is down");
        assert!(message.contains("upstream is down"), "{message}");
    }

    #[test]
    fn an_empty_body_leaves_the_status_to_speak_for_itself() {
        let message = google_failure_message("session", StatusCode::FORBIDDEN, "   ");
        assert_eq!(message, "the Google Maps session request failed with 403 Forbidden");
    }

    /// Truncation counts characters, not bytes — a body of multi-byte characters must not be cut
    /// mid-character (which would panic on a byte slice).
    #[test]
    fn a_long_body_is_truncated_on_a_character_boundary() {
        let body = "©".repeat(MAX_ERROR_BODY_CHARS * 2);
        let detail = google_error_detail(&body).expect("a non-empty body has a detail");
        assert_eq!(detail.chars().count(), MAX_ERROR_BODY_CHARS);
    }

    #[tokio::test]
    async fn every_non_google_provider_refreshes_no_attribution() {
        for provider in [
            osm(),
            MapProvider::MaplibreStyle {
                style_url: "https://example.test/style.json".to_owned(),
                attribution: "© Example".to_owned(),
                api_key_env: None,
            },
        ] {
            let refreshed = refresh_map_attribution(&provider, 10.0, (60.0, 59.0, 11.0, 10.0))
                .await
                .expect("no I/O for a non-Google provider");
            assert_eq!(refreshed, None);
        }
    }
}
