//! Host-mediated HTTP for the `net` capability (ADR 0017 §2).
//!
//! [`NetPolicy`] is the caller-supplied policy threaded onto an [`Invocation`](crate::Invocation):
//! a host allowlist ([`HostPattern`], exact or `*.suffix`), a response-size cap, a separate larger
//! binary cap (used by `media-store.fetch-and-store`), and a per-request timeout. Every fetch is
//! GET-only, HTTPS-only (in production), refuses userinfo and IP-literal hosts, and re-checks the
//! allowlist on **every** redirect hop by following redirects manually (the client itself does not
//! follow any). The User-Agent is honest and non-crawler (`genealogy/<version>`) — the archive's
//! `robots.txt` blocks named crawler agents, so the client must not impersonate one.

use std::net::IpAddr;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Url;
use reqwest::header::{CONTENT_TYPE, LOCATION};

/// Default cap on an in-memory response body (HTML/JSON pages).
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;
/// Default cap on a streamed binary download (`media-store.fetch-and-store`).
const DEFAULT_MAX_BINARY_BYTES: u64 = 64 * 1024 * 1024;
/// Default per-request timeout.
const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// The maximum number of redirects the host follows before giving up.
const MAX_REDIRECTS: usize = 10;

/// A single entry in a [`NetPolicy`] allowlist: either an exact host or a `*.suffix` wildcard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    /// Matches exactly this host (case-insensitive).
    Exact(String),
    /// Matches any strict subdomain of this suffix — `*.digitalarkivet.no` matches
    /// `www.digitalarkivet.no` but neither the apex `digitalarkivet.no` nor `evildigitalarkivet.no`.
    Suffix(String),
}

impl HostPattern {
    /// Parses a pattern string: a leading `*.` makes it a [`HostPattern::Suffix`], otherwise
    /// [`HostPattern::Exact`]. Stored lowercased for case-insensitive matching.
    #[must_use]
    pub fn parse(pattern: &str) -> Self {
        match pattern.strip_prefix("*.") {
            Some(suffix) => Self::Suffix(suffix.to_ascii_lowercase()),
            None => Self::Exact(pattern.to_ascii_lowercase()),
        }
    }

    /// Whether `host` matches this pattern (case-insensitive).
    fn matches(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        match self {
            Self::Exact(exact) => *exact == host,
            Self::Suffix(suffix) => host
                .strip_suffix(suffix.as_str())
                .and_then(|prefix| prefix.strip_suffix('.'))
                .is_some(),
        }
    }
}

/// The caller-supplied network policy for one invocation (ADR 0017 §2). Empty `allowed_hosts` denies
/// every host (deny-by-default).
#[derive(Debug, Clone)]
pub struct NetPolicy {
    /// The hosts a fetch may reach; checked on the initial URL and every redirect hop.
    pub allowed_hosts: Vec<HostPattern>,
    /// Cap on an in-memory response body (`net.fetch`).
    pub max_response_bytes: u64,
    /// Cap on a streamed binary download (`media-store.fetch-and-store`).
    pub max_binary_bytes: u64,
    /// Per-request timeout.
    pub timeout: Duration,
    /// Whether only `https` is permitted. Production keeps this `true` (ADR 0017 §2); the
    /// capability tests set it `false` to reach a local mock HTTP server.
    pub require_https: bool,
}

impl Default for NetPolicy {
    fn default() -> Self {
        Self::deny_all()
    }
}

impl NetPolicy {
    /// A policy that denies every host — the default for runs that were granted no `net` access.
    #[must_use]
    pub fn deny_all() -> Self {
        Self {
            allowed_hosts: Vec::new(),
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            max_binary_bytes: DEFAULT_MAX_BINARY_BYTES,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            require_https: true,
        }
    }

    /// A production policy allowing exactly `allowed_hosts` over HTTPS, with the default caps and
    /// timeout.
    #[must_use]
    pub fn allow(allowed_hosts: Vec<HostPattern>) -> Self {
        Self {
            allowed_hosts,
            ..Self::deny_all()
        }
    }

    /// Whether `host` is permitted by the allowlist.
    fn host_allowed(&self, host: &str) -> bool {
        self.allowed_hosts.iter().any(|pattern| pattern.matches(host))
    }
}

/// Why a host-mediated fetch failed. Policy rejections and transport failures are distinguished so
/// the caller can map them onto the right `capability-error` (invalid-input vs backend).
#[derive(Debug)]
pub enum NetError {
    /// The URL or a redirect hop violated the policy (scheme, host, userinfo, IP literal).
    Policy(String),
    /// The URL could not be parsed.
    InvalidUrl(String),
    /// The response (or download) exceeded its size cap.
    TooLarge,
    /// The request exceeded the policy timeout.
    Timeout,
    /// A transport or I/O failure.
    Backend(String),
}

/// A fetched response returned across the `net` boundary.
#[derive(Debug)]
pub struct Fetched {
    /// The HTTP status code.
    pub status: u16,
    /// The URL after all redirects.
    pub final_url: String,
    /// The response headers, in wire order.
    pub headers: Vec<(String, String)>,
    /// The full response body (under the response-size cap).
    pub body: Vec<u8>,
}

/// Metadata a streamed download reports back to `media-store` (the bytes went to a caller-provided
/// sink, not here).
#[derive(Debug)]
pub struct DownloadMeta {
    /// The response `Content-Type`, if any.
    pub content_type: Option<String>,
    /// The number of bytes streamed.
    pub size: u64,
}

/// The honest, non-crawler User-Agent every request carries.
fn user_agent() -> String {
    format!("genealogy/{}", env!("CARGO_PKG_VERSION"))
}

/// The shared HTTP client: no automatic redirects (the host follows them manually to re-check the
/// allowlist per hop) and the honest User-Agent. Reused across invocations — the client holds no
/// per-invocation policy. Also reused by the `ai` vision-api provider (ADR 0017 §4), which sets its
/// own per-request timeout.
pub(crate) fn client() -> &'static reqwest::Client {
    #[expect(
        clippy::expect_used,
        reason = "building a reqwest client with a static rustls config and no proxies cannot fail at runtime"
    )]
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(user_agent())
            .build()
            .expect("static reqwest client builds")
    });
    &CLIENT
}

/// Checks a URL (initial or a redirect hop) against the policy: scheme, no userinfo, no IP-literal
/// host, and the host allowlist.
fn validate_url(policy: &NetPolicy, url: &Url) -> Result<(), NetError> {
    match url.scheme() {
        "https" => {}
        "http" if !policy.require_https => {}
        other => return Err(NetError::Policy(format!("scheme `{other}` is not permitted"))),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(NetError::Policy("URLs with userinfo are not permitted".to_owned()));
    }
    let host = url
        .host_str()
        .ok_or_else(|| NetError::Policy("URL has no host".to_owned()))?;
    let bare = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    if bare.parse::<IpAddr>().is_ok() {
        return Err(NetError::Policy("IP-literal hosts are not permitted".to_owned()));
    }
    if !policy.host_allowed(host) {
        return Err(NetError::Policy(format!("host `{host}` is not on the allowlist")));
    }
    Ok(())
}

/// Sends a GET and follows redirects manually, re-checking each hop against the policy. Returns the
/// final URL and the response whose body is still unread.
async fn send_following_redirects(policy: &NetPolicy, url: &str) -> Result<(Url, reqwest::Response), NetError> {
    let mut current = Url::parse(url).map_err(|error| NetError::InvalidUrl(error.to_string()))?;
    validate_url(policy, &current)?;
    for _ in 0..=MAX_REDIRECTS {
        let response = client()
            .get(current.clone())
            .send()
            .await
            .map_err(|error| NetError::Backend(error.to_string()))?;
        if !response.status().is_redirection() {
            return Ok((current, response));
        }
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| NetError::Backend("redirect response without a Location header".to_owned()))?;
        let next = current
            .join(location)
            .map_err(|error| NetError::InvalidUrl(error.to_string()))?;
        validate_url(policy, &next)?;
        current = next;
    }
    Err(NetError::Policy(format!("more than {MAX_REDIRECTS} redirects")))
}

/// The whole request-and-drain operation, bounded by `policy.timeout`. Each body chunk is handed to
/// `on_chunk`; the running total is capped at `cap`.
async fn download<F>(policy: &NetPolicy, url: &str, cap: u64, mut on_chunk: F) -> Result<DownloadMeta, NetError>
where
    F: FnMut(&[u8]) -> Result<(), NetError> + Send,
{
    let run = async {
        let (_final_url, mut response) = send_following_redirects(policy, url).await?;
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut size: u64 = 0;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| NetError::Backend(error.to_string()))?
        {
            size += chunk.len() as u64;
            if size > cap {
                return Err(NetError::TooLarge);
            }
            on_chunk(&chunk)?;
        }
        Ok(DownloadMeta { content_type, size })
    };
    match tokio::time::timeout(policy.timeout, run).await {
        Ok(result) => result,
        Err(_) => Err(NetError::Timeout),
    }
}

/// GET `url` under `policy`, returning the whole response in memory (capped at
/// `policy.max_response_bytes`). The request and the body drain share one policy timeout.
pub async fn fetch(policy: &NetPolicy, url: &str) -> Result<Fetched, NetError> {
    let run = async {
        let (final_url, mut response) = send_following_redirects(policy, url).await?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect();
        let mut body = Vec::new();
        let mut total: u64 = 0;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| NetError::Backend(error.to_string()))?
        {
            total += chunk.len() as u64;
            if total > policy.max_response_bytes {
                return Err(NetError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Fetched {
            status,
            final_url: final_url.to_string(),
            headers,
            body,
        })
    };
    match tokio::time::timeout(policy.timeout, run).await {
        Ok(result) => result,
        Err(_) => Err(NetError::Timeout),
    }
}

/// Streams a download to `on_chunk` under `policy` (capped at `policy.max_binary_bytes`), returning
/// the response metadata. Used by `media-store.fetch-and-store` to write bytes straight to disk.
pub async fn stream_to<F>(policy: &NetPolicy, url: &str, on_chunk: F) -> Result<DownloadMeta, NetError>
where
    F: FnMut(&[u8]) -> Result<(), NetError> + Send,
{
    download(policy, url, policy.max_binary_bytes, on_chunk).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(hosts: &[&str]) -> NetPolicy {
        NetPolicy::allow(hosts.iter().map(|host| HostPattern::parse(host)).collect())
    }

    #[test]
    fn exact_pattern_matches_only_that_host() {
        let pattern = HostPattern::parse("www.digitalarkivet.no");
        assert!(pattern.matches("www.digitalarkivet.no"));
        assert!(pattern.matches("WWW.DigitalArkivet.NO"), "matching is case-insensitive");
        assert!(!pattern.matches("media.digitalarkivet.no"));
        assert!(!pattern.matches("digitalarkivet.no"));
    }

    #[test]
    fn suffix_pattern_matches_subdomains_only() {
        let pattern = HostPattern::parse("*.digitalarkivet.no");
        assert!(pattern.matches("www.digitalarkivet.no"));
        assert!(pattern.matches("media.digitalarkivet.no"));
        assert!(pattern.matches("urn.digitalarkivet.no"));
        assert!(
            !pattern.matches("digitalarkivet.no"),
            "the wildcard does not match the bare apex"
        );
        assert!(
            !pattern.matches("evildigitalarkivet.no"),
            "the wildcard requires a dot boundary — a look-alike host must not match"
        );
    }

    #[test]
    fn https_allowlisted_host_is_accepted() {
        let policy = policy(&["*.digitalarkivet.no"]);
        let url = Url::parse("https://www.digitalarkivet.no/census/person/1").expect("url");
        assert!(validate_url(&policy, &url).is_ok());
    }

    #[test]
    fn http_is_rejected_when_https_is_required() {
        let policy = policy(&["www.digitalarkivet.no"]);
        let url = Url::parse("http://www.digitalarkivet.no/").expect("url");
        assert!(matches!(validate_url(&policy, &url), Err(NetError::Policy(_))));
    }

    #[test]
    fn non_allowlisted_host_is_rejected() {
        let policy = policy(&["www.digitalarkivet.no"]);
        let url = Url::parse("https://example.com/").expect("url");
        assert!(matches!(validate_url(&policy, &url), Err(NetError::Policy(_))));
    }

    #[test]
    fn userinfo_url_is_rejected() {
        let policy = policy(&["www.digitalarkivet.no"]);
        let url = Url::parse("https://user@www.digitalarkivet.no/").expect("url");
        assert!(matches!(validate_url(&policy, &url), Err(NetError::Policy(_))));
    }

    #[test]
    fn ipv4_literal_host_is_rejected() {
        let mut policy = policy(&["127.0.0.1"]);
        policy.require_https = false;
        let url = Url::parse("http://127.0.0.1:8080/").expect("url");
        assert!(
            matches!(validate_url(&policy, &url), Err(NetError::Policy(_))),
            "IP-literal hosts are refused even if the literal is on the allowlist"
        );
    }

    #[test]
    fn ipv6_literal_host_is_rejected() {
        let mut policy = policy(&["::1"]);
        policy.require_https = false;
        let url = Url::parse("http://[::1]:8080/").expect("url");
        assert!(matches!(validate_url(&policy, &url), Err(NetError::Policy(_))));
    }

    #[test]
    fn redirect_hop_uses_the_same_validation() {
        // The per-hop check in `send_following_redirects` calls the same `validate_url`, so a hop to
        // a non-allowlisted host is rejected exactly as an initial URL to it would be.
        let policy = policy(&["www.digitalarkivet.no"]);
        let hop = Url::parse("https://evil.example.com/").expect("url");
        assert!(matches!(validate_url(&policy, &hop), Err(NetError::Policy(_))));
    }
}
