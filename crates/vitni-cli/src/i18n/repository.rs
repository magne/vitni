use super::{Localizer, RepositoryError, RepositorySummary, RepositoryType, fl};

impl Localizer {
    /// `No repositories yet.`
    #[must_use]
    pub fn repository_list_empty(&self) -> String {
        fl!(self.loader, "repository-list-empty")
    }

    /// One repository line: `R0001  Riksarkivet  type: archive  addresses: 1  urls: 2`.
    #[must_use]
    pub fn repository_summary_line(&self, summary: &RepositorySummary) -> String {
        let name = match &summary.name {
            Some(name) => name.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let repository_type = match &summary.repository_type {
            Some(repository_type) => self.repository_type(repository_type),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "repository-summary",
            id = summary.human_id.clone(),
            name = name,
            repository_type = repository_type,
            addresses = summary.addresses.len().to_string(),
            urls = summary.urls.len().to_string()
        )
    }

    /// The localized repository-type label; a custom value renders verbatim.
    fn repository_type(&self, repository_type: &RepositoryType) -> String {
        match repository_type {
            RepositoryType::Library => fl!(self.loader, "repository-type-library"),
            RepositoryType::Archive => fl!(self.loader, "repository-type-archive"),
            RepositoryType::Church => fl!(self.loader, "repository-type-church"),
            RepositoryType::Cemetery => fl!(self.loader, "repository-type-cemetery"),
            RepositoryType::Museum => fl!(self.loader, "repository-type-museum"),
            RepositoryType::Website => fl!(self.loader, "repository-type-website"),
            RepositoryType::Collection => fl!(self.loader, "repository-type-collection"),
            RepositoryType::Custom(value) => value.clone(),
        }
    }

    pub(super) fn repository_error(&self, error: &RepositoryError) -> String {
        match error {
            RepositoryError::NotFound(id) => fl!(self.loader, "err-repository-not-exist", id = id.to_string()),
            RepositoryError::AlreadyExists(id) => fl!(self.loader, "err-repository-exists", id = id.to_string()),
            RepositoryError::EmptyName => fl!(self.loader, "err-repository-empty-name"),
            RepositoryError::RetractsMissingAssertion(id) | RepositoryError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
