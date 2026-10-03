//! The similar-record hint a create prints on stderr (ADR 0038 §8).

use vitni_app::{AppError, DecidableKind, SimilarRecord};

use super::{Localizer, fl};

impl Localizer {
    /// `Possibly the same as I0001 (87%)`, with the `vitni match show` command comparing the two when
    /// the kind can be decided about.
    #[must_use]
    pub fn similar_hint(&self, kind: Option<DecidableKind>, new: &str, similar: &SimilarRecord) -> String {
        let id = similar.record.human_id.clone();
        let score = similar.assessment.evidence().percent();
        match kind {
            Some(kind) => fl!(
                self.loader,
                "similar-hint-compare",
                id = id,
                score = score,
                kind = kind.as_str(),
                new = new
            ),
            None => fl!(self.loader, "similar-hint", id = id, score = score),
        }
    }

    /// The look for similar records failed; the record was created all the same.
    #[must_use]
    pub fn similar_hint_failed(&self, error: &AppError) -> String {
        fl!(self.loader, "similar-hint-failed", error = self.error(error))
    }
}
