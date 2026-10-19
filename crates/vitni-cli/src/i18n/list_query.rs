//! Messages for list queries (ADR 0052): a query that matched nothing, and the clause the catalogue
//! cannot evaluate.

use super::{ListQueryError, Localizer, fl};

impl Localizer {
    pub(super) fn list_query_error(&self, error: &ListQueryError) -> String {
        match error {
            ListQueryError::UnknownColumn { kind, column } => {
                let columns: Vec<&str> = kind.columns().iter().map(|column| column.id).collect();
                fl!(
                    self.loader,
                    "err-list-unknown-column",
                    kind = kind.as_str(),
                    column = column.clone(),
                    columns = columns.join(", ")
                )
            }
            ListQueryError::UnknownOperator { column, op } => fl!(
                self.loader,
                "err-list-unknown-operator",
                column = column.clone(),
                op = op.clone()
            ),
            ListQueryError::UnsupportedOperator { column, op } => fl!(
                self.loader,
                "err-list-unsupported-operator",
                column = column.clone(),
                op = op.as_str()
            ),
            ListQueryError::WrongValue { column, op } => fl!(
                self.loader,
                "err-list-wrong-value",
                column = column.clone(),
                op = op.as_str()
            ),
            ListQueryError::UnknownChoice { column, value } => fl!(
                self.loader,
                "err-list-unknown-choice",
                column = column.clone(),
                value = value.clone()
            ),
            ListQueryError::UnknownReference { column, value } => fl!(
                self.loader,
                "err-list-unknown-reference",
                column = column.clone(),
                value = value.clone()
            ),
            ListQueryError::Unsortable { column } => fl!(self.loader, "err-list-unsortable", column = column.clone()),
            ListQueryError::KindMismatch { expected, found } => fl!(
                self.loader,
                "err-list-kind-mismatch",
                expected = expected.as_str(),
                found = found.as_str()
            ),
        }
    }
}
