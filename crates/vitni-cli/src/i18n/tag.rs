use super::{Localizer, TagError, TagSummary, fl};

impl Localizer {
    /// `No tags yet.`
    #[must_use]
    pub fn tag_list_empty(&self) -> String {
        fl!(self.loader, "tag-list-empty")
    }

    /// One tag line: `<uuid>  name  color: #1f77b4  priority: 5`.
    #[must_use]
    pub fn tag_summary_line(&self, summary: &TagSummary) -> String {
        let name = match &summary.name {
            Some(name) => name.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let color = match &summary.color {
            Some(color) => color.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let priority = match summary.priority {
            Some(priority) => priority.to_string(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "tag-summary",
            id = summary.id.clone(),
            name = name,
            color = color,
            priority = priority
        )
    }

    pub(super) fn tag_error(&self, error: &TagError) -> String {
        match error {
            TagError::NotFound(id) => fl!(self.loader, "err-tag-not-exist", id = id.to_string()),
            TagError::AlreadyExists(id) => fl!(self.loader, "err-tag-exists", id = id.to_string()),
            TagError::EmptyName => fl!(self.loader, "err-tag-empty-name"),
        }
    }
}
