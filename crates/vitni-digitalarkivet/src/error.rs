//! Parse errors — typed and contextual, never a panic on malformed HTML.

use thiserror::Error;

/// Which page a parser was asked to read, for error context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageContext {
    /// A `/census/person/` page.
    CensusPerson,
    /// A `/census/{rural,urban}-residence/` page.
    CensusResidence,
    /// A church-book record (`/view/<n>/pd…`) page.
    ChurchbookRecord,
    /// A scan-viewer page (media/goto host).
    Viewer,
}

impl std::fmt::Display for PageContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            PageContext::CensusPerson => "census person page",
            PageContext::CensusResidence => "census residence page",
            PageContext::ChurchbookRecord => "church-book record page",
            PageContext::Viewer => "scan-viewer page",
        };
        f.write_str(name)
    }
}

/// A failure while parsing Digitalarkivet HTML.
///
/// Malformed or unexpected markup yields one of these; parsers never panic.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    /// A required element was absent from the page.
    #[error("{page}: missing required element ({what})")]
    MissingElement {
        /// The page kind the parser was reading.
        page: PageContext,
        /// A human description of the element that was expected.
        what: &'static str,
    },

    /// A scan-viewer page carried no resolvable permanent image URL.
    ///
    /// The new `nye.digitalarkivet.no` IIIF viewer (church-book scans) hits this:
    /// it serves tiles through a IIIF manifest, not a permanent `.jpg`.
    #[error("{page}: no permanent image URL (IIIF-only viewer?)")]
    ImageUrlNotFound {
        /// Always [`PageContext::Viewer`].
        page: PageContext,
    },

    /// A static CSS selector failed to compile — a programmer error, surfaced
    /// as a typed error rather than an `unwrap`/`panic` in library code.
    #[error("internal: invalid selector `{0}`")]
    Selector(&'static str),
}
