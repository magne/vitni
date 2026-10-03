//! The similar-record hint every create verb gives (ADR 0038 §8): the stored records the new one looks
//! like, on stderr, so stdout stays the plain `Created …` line. It never blocks — the record is created
//! whatever it finds.

use vitni_app::{DecidableKind, MatchBand, MatchableKind, Workspace, find_similar};

use crate::i18n::Localizer;

/// The most similar records a hint names.
const HINT_LIMIT: usize = 3;

/// Prints one line on stderr per stored record at least possibly the same as the new record
/// `human_id` (a tag's id) of `kind`. A failed look is reported there too, since the create itself
/// succeeded.
pub async fn hint_similar(workspace: &Workspace, kind: MatchableKind, human_id: &str, localizer: &Localizer) {
    match find_similar(workspace, kind, human_id, MatchBand::Possible, HINT_LIMIT).await {
        Ok(similar) => {
            let decidable = DecidableKind::from_matchable(kind);
            for record in &similar {
                eprintln!("{}", localizer.similar_hint(decidable, human_id, record));
            }
        }
        Err(error) => eprintln!("{}", localizer.similar_hint_failed(&error)),
    }
}
