use super::{Localizer, SourceError, SourceSummary, fl};

impl Localizer {
    /// `No sources yet.`
    #[must_use]
    pub fn source_list_empty(&self) -> String {
        fl!(self.loader, "source-list-empty")
    }

    /// One source line: `S0001  Folketelling 1801  author: Riksarkivet  repos: 1  attrs: 2`.
    #[must_use]
    pub fn source_summary_line(&self, summary: &SourceSummary) -> String {
        let title = match &summary.title {
            Some(title) => title.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let author = match &summary.author {
            Some(author) => author.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "source-summary",
            id = summary.human_id.clone(),
            title = title,
            author = author,
            repositories = summary.repositories.len().to_string(),
            attributes = summary.attributes.len().to_string()
        )
    }

    pub(super) fn source_error(&self, error: &SourceError) -> String {
        match error {
            SourceError::NotFound(id) => fl!(self.loader, "err-source-not-exist", id = id.to_string()),
            SourceError::AlreadyExists(id) => fl!(self.loader, "err-source-exists", id = id.to_string()),
            SourceError::RetractsMissingAssertion(id) | SourceError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
            SourceError::UnknownRepository(id) => {
                fl!(self.loader, "err-source-unknown-repository", id = id.to_string())
            }
        }
    }
}
