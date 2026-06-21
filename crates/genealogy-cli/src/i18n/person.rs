use super::{Localizer, PersonError, PersonSummary, Sex, fl};

impl Localizer {
    /// `No persons yet.`
    #[must_use]
    pub fn list_empty(&self) -> String {
        fl!(self.loader, "list-empty")
    }

    /// One person line: `I0001  Ada Lovelace  sex: female [private]`.
    #[must_use]
    pub fn summary_line(&self, summary: &PersonSummary) -> String {
        let name = match &summary.display_name {
            Some(name) => name.clone(),
            None => fl!(self.loader, "no-name"),
        };
        let sex = match &summary.sex {
            Some(sex) => self.sex(sex),
            None => fl!(self.loader, "no-value"),
        };
        let private = if summary.private {
            fl!(self.loader, "private-tag")
        } else {
            String::new()
        };
        fl!(
            self.loader,
            "summary",
            id = summary.human_id.clone(),
            name = name,
            sex = sex,
            private = private
        )
    }

    /// The localized sex label; a custom [`Sex::Other`] value renders verbatim.
    fn sex(&self, sex: &Sex) -> String {
        match sex {
            Sex::Male => fl!(self.loader, "sex-male"),
            Sex::Female => fl!(self.loader, "sex-female"),
            Sex::Unknown => fl!(self.loader, "sex-unknown"),
            Sex::Other(value) => value.clone(),
        }
    }

    pub(super) fn person_error(&self, error: &PersonError) -> String {
        match error {
            PersonError::NotFound(id) => fl!(self.loader, "err-person-not-exist", id = id.to_string()),
            PersonError::AlreadyExists(id) => fl!(self.loader, "err-person-exists", id = id.to_string()),
            PersonError::EmptyName => fl!(self.loader, "err-empty-name"),
            PersonError::RetractsMissingAssertion(id) | PersonError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
            PersonError::InvalidDate(detail) => fl!(self.loader, "err-invalid-date", detail = detail.clone()),
            PersonError::MergeConflict {
                surviving,
                merged,
                reason,
            } => fl!(
                self.loader,
                "err-merge-conflict",
                surviving = surviving.to_string(),
                merged = merged.to_string(),
                reason = reason.clone()
            ),
            PersonError::SelfAssociation(id) => fl!(self.loader, "err-self-association", id = id.to_string()),
        }
    }
}
