//! Framework-free media-save naming (ADR 0017 §3): the pure logic behind the "add file to the media
//! library" dialog. It proposes where a scan/photo should be filed — a numbered category folder, an
//! optional subfolder, and a slugified `{date}_{place}_{event}_{name}.{ext}` filename following the
//! owner's archive convention — and renders the workspace-media-relative target path the host's
//! `media-store` writes under. Slugging keeps the Norwegian letters `æøå`, and census dates are filed
//! by year. Unit-tested here so the dialog stays a thin binding over tested math.

/// The numbered category folders the media library is organised into (the owner's archive
/// convention). Offered first in the save dialog's category picker, unioned with any folders that
/// already exist under `<workspace>/media/`; the operator may also type a free-text category.
pub const CATEGORY_CONVENTION: &[&str] = &[
    "01_kirkebok",
    "02_folketelling",
    "03_emigrasjon",
    "04_skifter",
    "05_personbilder",
    "06_gravminner",
    "07_dokumenter",
    "99_inbox",
];

/// The metadata hints a filename is proposed from — each already the plain display value; empty parts
/// are skipped. `date` is shortened to its year (census records are filed by year); the rest are
/// slugified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilenameHints {
    /// A date or year (a full census date is shortened to its leading year).
    pub date: String,
    /// A place name (e.g. a parish or municipality).
    pub place: String,
    /// An event kind (e.g. `dåp`, `folketelling`).
    pub event: String,
    /// A person or family name.
    pub name: String,
    /// The file extension, without the leading dot (e.g. `jpg`).
    pub ext: String,
}

/// The editable state of the save dialog: the chosen category folder, an optional subfolder, and the
/// filename. Its [`Self::target_rel_path`] is the live path preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaSaveDraft {
    /// The category folder (a convention entry, an existing folder, or free text).
    pub category: String,
    /// An optional subfolder within the category.
    pub subfolder: String,
    /// The filename (proposed by [`suggest_filename`], operator-editable).
    pub filename: String,
}

impl MediaSaveDraft {
    /// The workspace-media-relative target path — the non-empty `category`/`subfolder`/`filename`
    /// parts joined with `/`. This is what the dialog emits and the host `media-store` writes under
    /// `<workspace>/media/`; the host enforces path safety.
    #[must_use]
    pub fn target_rel_path(&self) -> String {
        let mut parts = Vec::new();
        for part in [self.category.trim(), self.subfolder.trim(), self.filename.trim()] {
            if !part.is_empty() {
                parts.push(part);
            }
        }
        parts.join("/")
    }
}

/// Proposes a filename from record metadata: `{date}_{place}_{event}_{name}.{ext}`, slugified, with
/// empty parts skipped and the date shortened to its year. Returns just the stem when no extension is
/// given, and an empty string when there is nothing to name.
#[must_use]
pub fn suggest_filename(hints: &FilenameHints) -> String {
    let mut stem_parts = Vec::new();
    for part in [
        slugify(census_year(&hints.date)),
        slugify(&hints.place),
        slugify(&hints.event),
        slugify(&hints.name),
    ] {
        if !part.is_empty() {
            stem_parts.push(part);
        }
    }
    let stem = stem_parts.join("_");
    let ext = normalize_ext(&hints.ext);
    match (stem.is_empty(), ext.is_empty()) {
        (true, _) => String::new(),
        (false, true) => stem,
        (false, false) => format!("{stem}.{ext}"),
    }
}

/// Slugifies text for a path segment: lowercase, keeping alphanumerics (so the Norwegian letters
/// `æøå` survive), turning spaces and commas into a single `-`, and dropping every other character.
/// Leading, trailing, and repeated separators collapse away.
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
        // Any other character is dropped, without introducing a separator.
    }
    out
}

/// Shortens a date to its leading four-digit year (census records are filed and cited by year),
/// leaving a non-year string unchanged. Mirrors `vitni-digitalarkivet`'s `census_year` so this
/// crate stays free of that dependency.
fn census_year(date: &str) -> &str {
    let trimmed = date.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 4 && bytes[..4].iter().all(u8::is_ascii_digit) {
        return &trimmed[..4];
    }
    trimmed
}

/// Normalises a file extension: trims, drops a leading dot, lowercases, and keeps only alphanumerics.
fn normalize_ext(ext: &str) -> String {
    let mut out = String::new();
    for ch in ext.trim().trim_start_matches('.').chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{FilenameHints, MediaSaveDraft, slugify, suggest_filename};

    #[test]
    fn slugify_keeps_norwegian_letters_and_separates_on_space_and_comma() {
        assert_eq!(slugify("Bergstøl, Asbjørn"), "bergstøl-asbjørn");
        assert_eq!(slugify("Størdal Åsen"), "størdal-åsen");
    }

    #[test]
    fn slugify_drops_other_punctuation_without_a_separator() {
        // A dot or apostrophe is dropped with no separator; only spaces/commas separate.
        assert_eq!(slugify("a/b.c"), "abc");
        assert_eq!(slugify("O'Brien"), "obrien");
        assert_eq!(slugify("St. Olav"), "st-olav");
    }

    #[test]
    fn slugify_collapses_and_trims_separators() {
        assert_eq!(slugify("  Oslo,,  Norway  "), "oslo-norway");
        assert_eq!(slugify(", ,"), "");
    }

    #[test]
    fn suggest_filename_joins_slugified_parts_with_underscores() {
        let hints = FilenameHints {
            date: "1900".to_owned(),
            place: "Trinity".to_owned(),
            event: "baptism".to_owned(),
            name: "John Smith".to_owned(),
            ext: "jpg".to_owned(),
        };
        assert_eq!(suggest_filename(&hints), "1900_trinity_baptism_john-smith.jpg");
    }

    #[test]
    fn suggest_filename_skips_empty_parts() {
        let hints = FilenameHints {
            date: String::new(),
            place: "Bergen".to_owned(),
            event: String::new(),
            name: "Ada".to_owned(),
            ext: "png".to_owned(),
        };
        assert_eq!(suggest_filename(&hints), "bergen_ada.png");
    }

    #[test]
    fn suggest_filename_shortens_a_census_date_to_its_year() {
        let hints = FilenameHints {
            date: "1920-12-01".to_owned(),
            place: "Oslo".to_owned(),
            event: "folketelling".to_owned(),
            name: String::new(),
            ext: "jpg".to_owned(),
        };
        assert_eq!(suggest_filename(&hints), "1920_oslo_folketelling.jpg");
    }

    #[test]
    fn suggest_filename_without_metadata_is_empty() {
        let hints = FilenameHints {
            ext: "jpg".to_owned(),
            ..FilenameHints::default()
        };
        assert_eq!(suggest_filename(&hints), "");
    }

    #[test]
    fn suggest_filename_without_an_extension_is_just_the_stem() {
        let hints = FilenameHints {
            name: "Ada".to_owned(),
            ..FilenameHints::default()
        };
        assert_eq!(suggest_filename(&hints), "ada");
    }

    #[test]
    fn target_rel_path_joins_non_empty_parts() {
        let draft = MediaSaveDraft {
            category: "02_folketelling".to_owned(),
            subfolder: "1900".to_owned(),
            filename: "oslo_ada.jpg".to_owned(),
        };
        assert_eq!(draft.target_rel_path(), "02_folketelling/1900/oslo_ada.jpg");
    }

    #[test]
    fn target_rel_path_skips_a_blank_subfolder() {
        let draft = MediaSaveDraft {
            category: "05_personbilder".to_owned(),
            subfolder: "  ".to_owned(),
            filename: "ada.jpg".to_owned(),
        };
        assert_eq!(draft.target_rel_path(), "05_personbilder/ada.jpg");
    }
}
