//! The Fellegi–Sunter weights (ADR 0038 §3, §9): log₂ Bayes factors, reviewed constants until the
//! evaluation corpus measures them.
//!
//! A feature that agrees adds its `agree` weight to the match weight and one that disagrees adds its
//! `disagree` weight; a partial agreement falls between them along its similarity. The match weight
//! starts from [`PRIOR`], the log-odds that two candidate records are one individual before any
//! evidence is seen.

/// The weights of one feature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Weights {
    /// The weight when the values agree.
    pub agree: f64,
    /// The weight when the values disagree.
    pub disagree: f64,
}

impl Weights {
    /// The weight at position `t` in `0..=1` between disagreement and agreement.
    pub fn at(self, t: f64) -> f64 {
        self.disagree + (self.agree - self.disagree) * t.clamp(0.0, 1.0)
    }
}

/// The prior log-odds of a candidate pair being one individual.
pub(crate) const PRIOR: f64 = -4.0;

/// Given names: strong evidence either way.
pub(crate) const GIVEN_NAME: Weights = Weights {
    agree: 6.0,
    disagree: -5.0,
};

/// Inherited surnames.
pub(crate) const SURNAME: Weights = Weights {
    agree: 3.0,
    disagree: -3.0,
};

/// Patronymic or residence surnames: agreement helps, a mismatch is not held against the pair.
pub(crate) const WEAK_SURNAME: Weights = Weights {
    agree: 2.0,
    disagree: 0.0,
};

/// Sex: half of all pairs agree by chance, so agreement is weak evidence; a mismatch is a conflict.
pub(crate) const SEX_AGREE: f64 = 1.0;

/// Birth date, or a baptism standing in for it.
pub(crate) const BIRTH: Weights = Weights {
    agree: 5.0,
    disagree: -6.0,
};

/// Death date, or a burial standing in for it.
pub(crate) const DEATH: Weights = Weights {
    agree: 4.0,
    disagree: -5.0,
};

/// Birth or death place.
pub(crate) const PLACE: Weights = Weights {
    agree: 2.0,
    disagree: -1.5,
};

/// A father or a mother: a stated parent who cannot be the other's is strong evidence of two people.
pub(crate) const PARENT: Weights = Weights {
    agree: 4.0,
    disagree: -8.0,
};

/// A partner: sources list different partners, so only agreement counts.
pub(crate) const PARTNER: Weights = Weights {
    agree: 4.0,
    disagree: 0.0,
};

/// A child: sources list different children, so only agreement counts.
pub(crate) const CHILD: Weights = Weights {
    agree: 3.0,
    disagree: 0.0,
};

/// A patronymic checked against the other record's father.
pub(crate) const PATRONYMIC: Weights = Weights {
    agree: 2.0,
    disagree: -3.0,
};

/// A marriage's or any event's date: the same event is recorded on the same day.
pub(crate) const EVENT_DATE: Weights = Weights {
    agree: 4.0,
    disagree: -5.0,
};

/// An event type in common: weak, since candidate events are mostly of one type already; a different
/// type is a conflict.
pub(crate) const EVENT_TYPE_AGREE: f64 = 1.0;

/// The most one family partner can support a pair: a remarriage shares a partner too, so only both
/// partners agreeing make one family. A partner who is someone else is not capped.
pub(crate) const PARTNER_SUPPORT: f64 = 6.0;

/// An event's principal: an event with another principal is another event.
pub(crate) const PRINCIPAL: Weights = Weights {
    agree: 4.0,
    disagree: -8.0,
};

/// An event's other participants: sources list different witnesses and godparents, so only agreement
/// counts.
pub(crate) const PARTICIPANT: Weights = Weights {
    agree: 2.0,
    disagree: 0.0,
};

/// An occupation in common: weak, and occupations change, so a difference is not evidence.
pub(crate) const OCCUPATION_AGREE: f64 = 1.0;

/// The weight of a hard conflict: overwhelming, whatever else agrees.
pub(crate) const CONFLICT: f64 = -20.0;

/// The highest score a pair with a hard conflict can have.
pub(crate) const CONFLICT_CAP: f64 = 0.01;

/// Below this Jaro–Winkler similarity two names disagree.
pub(crate) const NAME_FLOOR: f64 = 0.75;

/// Below this similarity two place names disagree.
pub(crate) const PLACE_NAME_FLOOR: f64 = 0.85;

/// A place's names, compared as a place record rather than as a person's birthplace.
pub(crate) const PLACE_NAME: Weights = Weights {
    agree: 5.0,
    disagree: -5.0,
};

/// A place's type: sources type one place differently (a farm, a village), so a difference is mild.
pub(crate) const PLACE_TYPE: Weights = Weights {
    agree: 1.0,
    disagree: -2.0,
};

/// Where a place lies: the same enclosing place, or one enclosing the other.
pub(crate) const ENCLOSURE: Weights = Weights {
    agree: 3.0,
    disagree: -3.0,
};

/// A place's coordinates: two places of one name far apart are two places.
pub(crate) const COORDINATES: Weights = Weights {
    agree: 4.0,
    disagree: -6.0,
};

/// A source's title: the one feature that names the source.
pub(crate) const TITLE: Weights = Weights {
    agree: 7.0,
    disagree: -5.0,
};

/// A source's author.
pub(crate) const AUTHOR: Weights = Weights {
    agree: 2.0,
    disagree: -2.0,
};

/// A source's publication information: a copy is published otherwise, so only agreement counts.
pub(crate) const PUBLICATION: Weights = Weights {
    agree: 1.0,
    disagree: 0.0,
};

/// A repository holding a source: a copy is held elsewhere, so only agreement counts.
pub(crate) const REPOSITORY: Weights = Weights {
    agree: 2.0,
    disagree: 0.0,
};

/// A repository's name.
pub(crate) const REPOSITORY_NAME: Weights = Weights {
    agree: 6.0,
    disagree: -5.0,
};

/// A repository's address.
pub(crate) const ADDRESS: Weights = Weights {
    agree: 3.0,
    disagree: -2.0,
};

/// The most a citation's source can support a pair: one source holds many citations, so only its page
/// makes two citations one.
pub(crate) const SOURCE_SUPPORT: f64 = 4.0;

/// A citation's page: another page of one source is another citation.
pub(crate) const PAGE: Weights = Weights {
    agree: 6.0,
    disagree: -6.0,
};

/// The date of a citation's entry.
pub(crate) const CITATION_DATE: Weights = Weights {
    agree: 2.0,
    disagree: -3.0,
};

/// Below this share of shared letters two free texts disagree.
pub(crate) const TEXT_FLOOR: f64 = 0.6;

/// A media object's checksum: equal bytes are one file, other bytes another.
pub(crate) const CHECKSUM: Weights = Weights {
    agree: 12.0,
    disagree: -12.0,
};

/// A media object's file name: weak, and a file is renamed, so only agreement counts.
pub(crate) const FILE_NAME_AGREE: f64 = 2.0;

/// A note's text.
pub(crate) const NOTE_TEXT: Weights = Weights {
    agree: 10.0,
    disagree: -6.0,
};

/// A tag's name: a tag is its name.
pub(crate) const TAG_NAME: Weights = Weights {
    agree: 10.0,
    disagree: -10.0,
};
