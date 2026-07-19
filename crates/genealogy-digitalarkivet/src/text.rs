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

/// Slugify text for a media-library path segment: lowercase, keeping alphanumerics (so the
/// Norwegian letters `æøå` survive), turning spaces and commas into a single `-`, and dropping every
/// other character. Leading, trailing, and repeated separators collapse away.
///
/// The plugin proposes media filenames (`media-store` is convention-free — ADR 0017 §3); this is the
/// guest-side naming rule, kept here so it is unit-tested through `--workspace` rather than only in
/// the wasm component. It mirrors the GUI's `genealogy_ui::slugify`, which the sandboxed plugin
/// cannot link.
#[must_use]
pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut pending_separator = false;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            if pending_separator && !out.is_empty() {
                out.push('-');
            }
            pending_separator = false;
            out.extend(ch.to_lowercase());
        } else if ch == ' ' || ch == ',' {
            pending_separator = true;
        }
    }
    out
}

/// Propose a `{year}_{place}_{event}_{name}.{ext}` filename from record metadata, slugified, with
/// empty parts skipped and the date shortened to its census year. Returns just the stem when no
/// extension is given, and an empty string when there is nothing to name.
#[must_use]
pub fn suggest_filename(date: &str, place: &str, event: &str, name: &str, ext: &str) -> String {
    let mut stem_parts = Vec::new();
    for part in [
        slugify(census_year(date)),
        slugify(place),
        slugify(event),
        slugify(name),
    ] {
        if !part.is_empty() {
            stem_parts.push(part);
        }
    }
    let stem = stem_parts.join("_");
    let ext = slugify(ext.trim().trim_start_matches('.'));
    match (stem.is_empty(), ext.is_empty()) {
        (true, _) => String::new(),
        (false, true) => stem,
        (false, false) => format!("{stem}.{ext}"),
    }
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
    use crate::text::{census_year, extract_urn, normalize_ws, slugify, suggest_filename};

    #[test]
    fn slugify_keeps_norwegian_letters_and_separates_on_space_and_comma() {
        assert_eq!(slugify("Bergstøl, Asbjørn"), "bergstøl-asbjørn");
        assert_eq!(slugify("Størdal Åsen"), "størdal-åsen");
        assert_eq!(slugify("St. Olav"), "st-olav");
        assert_eq!(slugify(", ,"), "");
    }

    #[test]
    fn suggest_filename_joins_slugified_parts_and_shortens_the_year() {
        assert_eq!(
            suggest_filename("1920-12-01", "Greipstad", "folketelling", "Asbjørn Olsen", "jpg"),
            "1920_greipstad_folketelling_asbjørn-olsen.jpg"
        );
        assert_eq!(suggest_filename("", "Bergen", "", "Ada", "png"), "bergen_ada.png");
        assert_eq!(suggest_filename("", "", "", "", "jpg"), "");
        assert_eq!(suggest_filename("", "", "", "Ada", ""), "ada");
    }

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
