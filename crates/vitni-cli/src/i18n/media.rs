use super::{Localizer, MediaError, MediaSummary, fl};

impl Localizer {
    /// `No media yet.`
    #[must_use]
    pub fn media_list_empty(&self) -> String {
        fl!(self.loader, "media-list-empty")
    }

    /// One media line: `O0001  path: photos/ada.jpg  checksum: -  attributes: 0`.
    #[must_use]
    pub fn media_summary_line(&self, summary: &MediaSummary) -> String {
        let path = match &summary.path {
            Some(path) => path.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let checksum = match &summary.checksum {
            Some(checksum) => checksum.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "media-summary",
            id = summary.human_id.clone(),
            path = path,
            checksum = checksum,
            attributes = summary.attributes.len().to_string()
        )
    }

    pub(super) fn media_error(&self, error: &MediaError) -> String {
        match error {
            MediaError::NotFound(id) => fl!(self.loader, "err-media-not-exist", id = id.to_string()),
            MediaError::AlreadyExists(id) => fl!(self.loader, "err-media-exists", id = id.to_string()),
            MediaError::RetractsMissingAssertion(id) | MediaError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
