use std::collections::BTreeSet;

use super::{Localizer, PersonError, PersonSummary, Restriction, Sex, fl};

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
        let restrictions = self.restrictions_tag(&summary.restrictions);
        fl!(
            self.loader,
            "summary",
            id = summary.human_id.clone(),
            name = name,
            sex = sex,
            restrictions = restrictions
        )
    }

    /// Renders a record's privacy restrictions as ` [label, …]`, or empty when unrestricted.
    pub(super) fn restrictions_tag(&self, restrictions: &BTreeSet<Restriction>) -> String {
        if restrictions.is_empty() {
            return String::new();
        }
        let value = restrictions
            .iter()
            .map(|&restriction| self.restriction_label(restriction))
            .collect::<Vec<_>>()
            .join(", ");
        fl!(self.loader, "restrictions-tag", value = value)
    }

    /// The localized label for a single [`Restriction`].
    fn restriction_label(&self, restriction: Restriction) -> String {
        match restriction {
            Restriction::Confidential => fl!(self.loader, "restriction-confidential"),
            Restriction::Locked => fl!(self.loader, "restriction-locked"),
            Restriction::Privacy => fl!(self.loader, "restriction-privacy"),
        }
    }

    /// The localized sex label; a custom [`Sex::Other`] value renders verbatim.
    fn sex(&self, sex: &Sex) -> String {
        match sex {
            Sex::Male => fl!(self.loader, "sex-male"),
            Sex::Female => fl!(self.loader, "sex-female"),
            Sex::Unknown => fl!(self.loader, "sex-unknown"),
            Sex::Intersex => fl!(self.loader, "sex-intersex"),
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
