use super::{Localizer, MatchDataError, PackError, fl};

impl Localizer {
    pub(super) fn match_data_error(&self, error: &MatchDataError) -> String {
        match error {
            MatchDataError::Read { path, source } => fl!(
                self.loader,
                "err-match-data-read",
                path = path.display().to_string(),
                detail = source.to_string()
            ),
            MatchDataError::Pack(PackError::Parse { name, error }) => fl!(
                self.loader,
                "err-match-data-parse",
                name = name.clone(),
                detail = error.to_string()
            ),
            MatchDataError::Pack(PackError::IdMismatch { name, id }) => {
                fl!(self.loader, "err-match-data-id", name = name.clone(), id = id.clone())
            }
        }
    }
}
