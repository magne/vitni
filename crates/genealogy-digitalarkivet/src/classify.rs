//! URL classification and link resolution.

use url::Url;

use crate::model::PageKind;

/// True when `host` is `digitalarkivet.no` or a subdomain of it.
fn is_digitalarkivet_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "digitalarkivet.no" || host.ends_with(".digitalarkivet.no")
}

/// Classify a Digitalarkivet URL by host and path.
///
/// Any non-`digitalarkivet.no` host, or an unrecognized path, is
/// [`PageKind::Unknown`]. Classification is scheme-agnostic (http and https both
/// classify); the `net` capability enforces HTTPS at fetch time.
#[must_use]
pub fn classify_url(url: &str) -> PageKind {
    let Ok(parsed) = Url::parse(url) else {
        return PageKind::Unknown;
    };
    let Some(host) = parsed.host_str() else {
        return PageKind::Unknown;
    };
    if !is_digitalarkivet_host(host) {
        return PageKind::Unknown;
    }
    let path = parsed.path();
    if path.contains("/census/person/") {
        return PageKind::CensusPerson;
    }
    if path.contains("/census/rural-residence/") || path.contains("/census/urban-residence/") {
        return PageKind::CensusResidence;
    }
    if is_churchbook_record_path(path) {
        return PageKind::ChurchbookRecord;
    }
    PageKind::Unknown
}

/// A church-book record path is `/view/<segment>/pd<id>` (an optional two-letter
/// locale prefix such as `/en` or `/nn` is allowed).
fn is_churchbook_record_path(path: &str) -> bool {
    let mut segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments
        .first()
        .is_some_and(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()))
    {
        segments.remove(0);
    }
    match segments.as_slice() {
        ["view", _, last, ..] => last.starts_with("pd"),
        _ => false,
    }
}

/// The last non-empty path segment of a URL — the record id (e.g.
/// `pf01073902000464`, `pd00000020636420`, `bf01073902000463`).
#[must_use]
pub fn record_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    parsed.path_segments()?.rfind(|s| !s.is_empty()).map(str::to_owned)
}

/// Resolve `href` against `base`, returning an absolute URL string.
#[must_use]
pub fn resolve(base: &str, href: &str) -> Option<String> {
    let base = Url::parse(base).ok()?;
    base.join(href).ok().map(String::from)
}

/// Resolve every `href` against `base`, keeping only those whose absolute path
/// contains `needle`, in first-seen order with duplicates removed.
#[must_use]
pub fn resolve_and_dedup<'a>(base: &str, hrefs: impl IntoIterator<Item = &'a str>, needle: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for href in hrefs {
        let Some(absolute) = resolve(base, href) else {
            continue;
        };
        if absolute.contains(needle) && !seen.contains(&absolute) {
            seen.push(absolute);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use crate::classify::{classify_url, record_id, resolve, resolve_and_dedup};
    use crate::model::PageKind;

    #[test]
    fn classifies_census_person() {
        assert_eq!(
            classify_url("https://www.digitalarkivet.no/census/person/pf01073902000464"),
            PageKind::CensusPerson
        );
    }

    #[test]
    fn classifies_both_residence_kinds() {
        assert_eq!(
            classify_url("https://www.digitalarkivet.no/census/rural-residence/bf01"),
            PageKind::CensusResidence
        );
        assert_eq!(
            classify_url("https://www.digitalarkivet.no/census/urban-residence/bu01"),
            PageKind::CensusResidence
        );
    }

    #[test]
    fn classifies_churchbook_record_with_and_without_locale() {
        assert_eq!(
            classify_url("https://www.digitalarkivet.no/view/255/pd00000020636420"),
            PageKind::ChurchbookRecord
        );
        assert_eq!(
            classify_url("https://www.digitalarkivet.no/nn/view/255/pd00000020636420"),
            PageKind::ChurchbookRecord
        );
    }

    #[test]
    fn http_classifies_like_https() {
        assert_eq!(
            classify_url("http://www.digitalarkivet.no/census/person/pf01"),
            PageKind::CensusPerson
        );
    }

    #[test]
    fn bare_and_sub_hosts_are_digitalarkivet() {
        assert_eq!(
            classify_url("https://digitalarkivet.no/census/person/pf01"),
            PageKind::CensusPerson
        );
        assert_eq!(
            classify_url("https://media.digitalarkivet.no/census/person/pf01"),
            PageKind::CensusPerson
        );
    }

    #[test]
    fn trailing_junk_still_classifies() {
        assert_eq!(
            classify_url("https://www.digitalarkivet.no/census/person/pf01?foo=bar#frag"),
            PageKind::CensusPerson
        );
    }

    #[test]
    fn non_digitalarkivet_host_is_unknown() {
        assert_eq!(
            classify_url("https://www.example.com/census/person/pf01"),
            PageKind::Unknown
        );
        // A look-alike suffix must not match.
        assert_eq!(
            classify_url("https://digitalarkivet.no.evil.com/census/person/pf01"),
            PageKind::Unknown
        );
    }

    #[test]
    fn garbage_and_media_view_are_unknown() {
        assert_eq!(classify_url("not a url"), PageKind::Unknown);
        assert_eq!(classify_url(""), PageKind::Unknown);
        // A media-host viewer path (`/view/<n>/<n>`) is not a church-book record.
        assert_eq!(
            classify_url("https://media.digitalarkivet.no/view/73902/1192"),
            PageKind::Unknown
        );
    }

    #[test]
    fn record_id_is_last_segment() {
        assert_eq!(
            record_id("https://www.digitalarkivet.no/census/person/pf01073902000464"),
            Some("pf01073902000464".to_owned())
        );
        assert_eq!(
            record_id("https://www.digitalarkivet.no/view/255/pd00000020636420"),
            Some("pd00000020636420".to_owned())
        );
    }

    #[test]
    fn resolve_makes_relative_absolute() {
        assert_eq!(
            resolve("https://www.digitalarkivet.no/view/255/pd1", "/census/person/pf2"),
            Some("https://www.digitalarkivet.no/census/person/pf2".to_owned())
        );
        assert_eq!(
            resolve("https://www.digitalarkivet.no/a/b", "https://media.digitalarkivet.no/x"),
            Some("https://media.digitalarkivet.no/x".to_owned())
        );
    }

    #[test]
    fn resolve_and_dedup_filters_and_dedupes() {
        let hrefs = [
            "/census/person/pf1",
            "https://www.digitalarkivet.no/census/person/pf1",
            "/census/person/pf2",
            "/census/district/tf9",
        ];
        let out = resolve_and_dedup("https://www.digitalarkivet.no/x", hrefs, "/census/person/");
        assert_eq!(
            out,
            vec![
                "https://www.digitalarkivet.no/census/person/pf1".to_owned(),
                "https://www.digitalarkivet.no/census/person/pf2".to_owned(),
            ]
        );
    }
}
