use super::{FamilyError, FamilySummary, Localizer, fl};

impl Localizer {
    /// `No families yet.`
    #[must_use]
    pub fn family_list_empty(&self) -> String {
        fl!(self.loader, "family-list-empty")
    }

    /// One family line: `F0001  partners: I0001, I0002  children: I0003 [private]`.
    #[must_use]
    pub fn family_summary_line(&self, summary: &FamilySummary) -> String {
        let partners = self.members(&summary.partners);
        let children = self.members(&summary.children);
        let private = if summary.private {
            fl!(self.loader, "private-tag")
        } else {
            String::new()
        };
        fl!(
            self.loader,
            "family-summary",
            id = summary.human_id.clone(),
            partners = partners,
            children = children,
            private = private
        )
    }

    /// Renders a member id list, or the localized `(none)` placeholder when empty.
    fn members(&self, ids: &[String]) -> String {
        if ids.is_empty() {
            fl!(self.loader, "family-none")
        } else {
            ids.join(", ")
        }
    }

    pub(super) fn family_error(&self, error: &FamilyError) -> String {
        match error {
            FamilyError::NotFound(id) => fl!(self.loader, "err-family-not-exist", id = id.to_string()),
            FamilyError::AlreadyExists(id) => fl!(self.loader, "err-family-exists", id = id.to_string()),
            FamilyError::PartnerAlreadyPresent(id) => fl!(self.loader, "err-partner-present", id = id.to_string()),
            FamilyError::PartnerNotPresent(id) => fl!(self.loader, "err-partner-absent", id = id.to_string()),
            FamilyError::ChildAlreadyPresent(id) => fl!(self.loader, "err-child-present", id = id.to_string()),
            FamilyError::ChildNotPresent(id) => fl!(self.loader, "err-child-absent", id = id.to_string()),
            FamilyError::RetractsMissingAssertion(id) | FamilyError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
