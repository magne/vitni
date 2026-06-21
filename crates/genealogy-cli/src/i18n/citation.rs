use super::{CitationError, CitationSummary, Localizer, fl};

impl Localizer {
    /// `No citations yet.`
    #[must_use]
    pub fn citation_list_empty(&self) -> String {
        fl!(self.loader, "citation-list-empty")
    }

    /// One citation line: `C0001  source: S0001  page: p. 42`.
    #[must_use]
    pub fn citation_summary_line(&self, summary: &CitationSummary) -> String {
        let source = match &summary.source {
            Some(source) => source.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let page = match &summary.page {
            Some(page) => page.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let date = match &summary.date {
            Some(date) => self.date(date),
            None => fl!(self.loader, "no-value"),
        };
        let confidence = match summary.confidence {
            Some(confidence) => self.confidence(confidence),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "citation-summary",
            id = summary.human_id.clone(),
            source = source,
            page = page,
            date = date,
            confidence = confidence
        )
    }

    pub(super) fn citation_error(&self, error: &CitationError) -> String {
        match error {
            CitationError::NotFound(id) => fl!(self.loader, "err-citation-not-exist", id = id.to_string()),
            CitationError::AlreadyExists(id) => fl!(self.loader, "err-citation-exists", id = id.to_string()),
            CitationError::UnknownSource(id) => fl!(self.loader, "err-unknown-source", id = id.to_string()),
            CitationError::RetractsMissingAssertion(id) | CitationError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
