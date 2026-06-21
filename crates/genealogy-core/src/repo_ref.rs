//! [`RepoRef`] — a link from a [`Source`](crate::source) to a [`Repository`](crate::repository)
//! holding it (data-model §6, §7).
//!
//! The same work can sit in several repositories, each with its own call number and medium, so the
//! call number and media type belong on the *link*, not on the repository. The link is an id (ADR
//! 0002 self-contained events).

use serde::{Deserialize, Serialize};

use crate::enums::SourceMediaType;
use crate::ids::RepositoryId;

/// A source's holding in one repository: a call number and the medium (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRef {
    /// The repository holding the source.
    pub repository_id: RepositoryId,
    /// The call number / shelf mark within that repository.
    pub call_number: Option<String>,
    /// The medium the source is held as.
    pub media_type: SourceMediaType,
}

#[cfg(test)]
mod tests {
    use super::RepoRef;
    use crate::enums::SourceMediaType;
    use crate::ids::RepositoryId;
    use uuid::Uuid;

    #[test]
    fn repo_ref_round_trips_through_json() {
        let reference = RepoRef {
            repository_id: RepositoryId::from_uuid(Uuid::from_u128(0x7)),
            call_number: Some("MS 1234".to_owned()),
            media_type: SourceMediaType::Film,
        };
        let json = serde_json::to_string(&reference).unwrap();
        let back: RepoRef = serde_json::from_str(&json).unwrap();
        assert_eq!(reference, back);
    }
}
