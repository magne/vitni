//! Norwegian archival constants and small text helpers.
//!
//! Crate data, not UI strings — no Fluent here. The plugin owns localization.

/// The managing repository for Digitalarkivet scans (citation attribution; the
/// archive asks to be named when a non-restricted scan is reused).
pub const REPOSITORY: &str = "Digitalarkivet (Arkivverket)";

/// The external-id authority for records resolved from this archive.
pub const AUTHORITY: &str = "digitalarkivet";

/// Common Norwegian church-book / census event terms, offered as a vocabulary
/// for classifying a record's event (mirrors the owner's prototype menu).
pub const COMMON_EVENTS: &[&str] = &[
    "dåp",
    "fødsel",
    "konfirmasjon",
    "vielse",
    "død",
    "begravelse",
    "folketelling",
    "utreise",
    "ankomst",
    "skifte",
    "gravminne",
];

/// Image file extensions a permanent scan URL may end in.
const IMAGE_EXTENSIONS: &[&str] = &[".jpeg", ".jpg", ".png", ".tiff", ".tif"];

/// Shorten a census date to its year.
///
/// Norwegian census records carry a full census date (e.g. `1920-12-01`) but are
/// conventionally filed and cited by year. Returns the leading four-digit year
/// when the string starts with one, else the input unchanged.
#[must_use]
pub fn census_year(date: &str) -> &str {
    let trimmed = date.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 4 && bytes[..4].iter().all(u8::is_ascii_digit) {
        return &trimmed[..4];
    }
    trimmed
}

/// Extract a `URN:NBN:…` identifier from a string, dropping any trailing image
/// extension.
///
/// From `https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10771822220997.jpg`
/// this yields `URN:NBN:no-a1450-fs10771822220997` — the stable scan identifier
/// used as the citation's archival reference.
#[must_use]
pub fn extract_urn(s: &str) -> Option<String> {
    let start = s.find("URN:NBN:")?;
    let rest = &s[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-')))
        .unwrap_or(rest.len());
    let mut urn = &rest[..end];
    for ext in IMAGE_EXTENSIONS {
        if let Some(stripped) = urn.strip_suffix(ext) {
            urn = stripped;
            break;
        }
    }
    Some(urn.to_owned())
}

/// Collapse runs of whitespace (including non-breaking spaces) to single spaces
/// and trim the ends.
#[must_use]
pub fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() || ch == '\u{a0}' {
            in_space = true;
        } else {
            if in_space && !out.is_empty() {
                out.push(' ');
            }
            in_space = false;
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::text::{census_year, extract_urn, normalize_ws};

    #[test]
    fn census_year_shortens_full_date() {
        assert_eq!(census_year("1920-12-01"), "1920");
        assert_eq!(census_year("1920"), "1920");
        assert_eq!(census_year(" 1885-31-12 "), "1885");
    }

    #[test]
    fn census_year_passes_through_non_year() {
        assert_eq!(census_year("udatert"), "udatert");
        assert_eq!(census_year("18"), "18");
        assert_eq!(census_year(""), "");
    }

    #[test]
    fn extract_urn_strips_extension() {
        assert_eq!(
            extract_urn("https://urn.digitalarkivet.no/URN:NBN:no-a1450-fs10771822220997.jpg"),
            Some("URN:NBN:no-a1450-fs10771822220997".to_owned())
        );
    }

    #[test]
    fn extract_urn_bare() {
        assert_eq!(
            extract_urn("URN:NBN:no-a1450-fs123"),
            Some("URN:NBN:no-a1450-fs123".to_owned())
        );
    }

    #[test]
    fn extract_urn_absent() {
        assert_eq!(extract_urn("https://example.com/foo.jpg"), None);
    }

    #[test]
    fn normalize_ws_collapses_nbsp_and_runs() {
        assert_eq!(normalize_ws("0036\u{a0}Bergstøl"), "0036 Bergstøl");
        assert_eq!(normalize_ws("  a   b\n c\t"), "a b c");
        assert_eq!(normalize_ws(""), "");
    }
}
