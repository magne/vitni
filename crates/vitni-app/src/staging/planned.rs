//! What a plan's summary says of each record it changes (ADR 0040 §4): the record, whether the import
//! updates it or reuses it, and the fields its writes add — the field keys the dry run names, read as
//! [`PlannedField`]s a frontend can label.

use vitni_core::enums::FactType;
use vitni_core::matching::MatchableKind;

/// One record a plan changes: one this dataset made that the import updates, or one an entity was
/// resolved onto that the import adds to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRecord {
    /// The record's kind.
    pub kind: MatchableKind,
    /// The incoming record's label (a person's name, a source's title), empty for a kind without one.
    pub label: String,
    /// The changed record's human id.
    pub human_id: String,
    /// Whether the import updates the record or reuses it.
    pub change: PlannedChange,
    /// The fields the writes add, in key order, each once.
    pub fields: Vec<PlannedField>,
    /// The field keys the writes assert (`person.FactAsserted.Occupation`), as the dry run names them.
    pub keys: Vec<String>,
}

/// How a plan changes a [`PlannedRecord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedChange {
    /// A record this dataset made, which the import adds to or changes.
    Updates,
    /// A record already in the tree, which the import resolves onto and adds what it lacks to.
    Reuses,
}

/// A field a planned write adds, read from its field key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedField {
    /// A name.
    Name,
    /// A person's sex.
    Sex,
    /// A person's fact of a built-in type.
    Fact(FactType),
    /// A person's fact of a custom type.
    CustomFact,
    /// A person's part in an event, or an event of a family.
    Event,
    /// An association with another person.
    Association,
    /// A family's partner.
    Partner,
    /// A family's child.
    Child,
    /// How a child is related to a family's partners.
    ChildRelationship,
    /// A date.
    Date,
    /// An address.
    Address,
    /// The place an event happened at.
    Place,
    /// A place's or a note's type.
    Type,
    /// The place a place lies in.
    Enclosure,
    /// Where a place lies.
    Coordinates,
    /// A source's title.
    Title,
    /// A source's author.
    Author,
    /// A source's publication info.
    Publication,
    /// A source's abbreviation.
    Abbreviation,
    /// A repository holding a source.
    Repository,
    /// A citation's page.
    Page,
    /// A citation's confidence.
    Confidence,
    /// A media file's MIME type.
    Mime,
    /// A note's text.
    Text,
    /// Restrictions.
    Restrictions,
    /// A citation.
    Citation,
    /// A media file.
    Media,
    /// A note.
    Note,
    /// A tag.
    Tag,
    /// Anything else the record gains.
    Other,
}

impl PlannedField {
    /// The field a field key (`source.AuthorSet`, `person.FactAsserted.Occupation`) names.
    #[must_use]
    pub fn of(key: &str) -> Self {
        let mut parts = key.splitn(3, '.');
        let (Some(aggregate), Some(event)) = (parts.next(), parts.next()) else {
            return Self::Other;
        };
        match (aggregate, event) {
            ("person", "FactAsserted") => parts.next().map_or(Self::Other, fact),
            (_, "NameAsserted" | "NameSet") => Self::Name,
            (_, "SexAsserted") => Self::Sex,
            (_, "ParticipationAsserted" | "FamilyEventLinked") => Self::Event,
            (_, "AssociationAsserted") => Self::Association,
            (_, "PartnerAdded") => Self::Partner,
            (_, "ChildAdded") => Self::Child,
            (_, "ChildRelationshipAsserted") => Self::ChildRelationship,
            (_, "DateAsserted") => Self::Date,
            (_, "AddressAdded") => Self::Address,
            (_, "PlaceLinked") => Self::Place,
            (_, "PlaceTypeSet" | "NoteTypeSet") => Self::Type,
            (_, "EnclosedByAsserted") => Self::Enclosure,
            (_, "CoordinatesAsserted") => Self::Coordinates,
            (_, "TitleSet") => Self::Title,
            (_, "AuthorSet") => Self::Author,
            (_, "PubInfoSet") => Self::Publication,
            (_, "AbbrevSet") => Self::Abbreviation,
            (_, "RepositoryLinked") => Self::Repository,
            (_, "PageSet") => Self::Page,
            (_, "ConfidenceSet") => Self::Confidence,
            (_, "MimeSet") => Self::Mime,
            (_, "RichTextSet") => Self::Text,
            (_, "RestrictionsChanged") => Self::Restrictions,
            (_, "CitationAdded") => Self::Citation,
            (_, "MediaAttached") => Self::Media,
            (_, "NoteAttached") => Self::Note,
            (_, "Tagged") => Self::Tag,
            _ => Self::Other,
        }
    }

    /// The stable name of a [`PlannedField::Fact`]'s type (`Occupation`), the selector a frontend picks
    /// its label by; `None` for any other field.
    #[must_use]
    pub fn fact_name(&self) -> Option<String> {
        let Self::Fact(fact) = self else {
            return None;
        };
        serde_json::to_value(fact)
            .ok()?
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    }

    /// The fields `keys` name, in order, each once.
    pub(crate) fn all(keys: &[String]) -> Vec<Self> {
        let mut fields: Vec<Self> = Vec::new();
        for key in keys {
            let field = Self::of(key);
            if !fields.contains(&field) {
                fields.push(field);
            }
        }
        fields
    }
}

/// The fact a `person.FactAsserted.<type>` key names: its built-in type, or a custom one.
fn fact(fact_type: &str) -> PlannedField {
    serde_json::from_value(serde_json::json!({ "type": fact_type }))
        .map_or(PlannedField::CustomFact, PlannedField::Fact)
}

#[cfg(test)]
mod tests {
    use super::PlannedField;
    use vitni_core::enums::FactType;

    #[test]
    fn every_key_the_staging_writer_asserts_names_its_field() {
        let cases = [
            ("person.NameAsserted", PlannedField::Name),
            ("place.NameAsserted", PlannedField::Name),
            ("repository.NameSet", PlannedField::Name),
            ("person.SexAsserted", PlannedField::Sex),
            (
                "person.FactAsserted.Occupation",
                PlannedField::Fact(FactType::Occupation),
            ),
            ("person.FactAsserted.Custom", PlannedField::CustomFact),
            ("person.ParticipationAsserted", PlannedField::Event),
            ("family.FamilyEventLinked", PlannedField::Event),
            ("person.AssociationAsserted", PlannedField::Association),
            ("family.PartnerAdded", PlannedField::Partner),
            ("family.ChildAdded", PlannedField::Child),
            ("family.ChildRelationshipAsserted", PlannedField::ChildRelationship),
            ("event.DateAsserted", PlannedField::Date),
            ("event.AddressAdded", PlannedField::Address),
            ("event.PlaceLinked", PlannedField::Place),
            ("place.PlaceTypeSet", PlannedField::Type),
            ("note.NoteTypeSet", PlannedField::Type),
            ("place.EnclosedByAsserted", PlannedField::Enclosure),
            ("place.CoordinatesAsserted", PlannedField::Coordinates),
            ("source.TitleSet", PlannedField::Title),
            ("source.AuthorSet", PlannedField::Author),
            ("source.PubInfoSet", PlannedField::Publication),
            ("source.AbbrevSet", PlannedField::Abbreviation),
            ("source.RepositoryLinked", PlannedField::Repository),
            ("citation.PageSet", PlannedField::Page),
            ("citation.ConfidenceSet", PlannedField::Confidence),
            ("media.MimeSet", PlannedField::Mime),
            ("note.RichTextSet", PlannedField::Text),
            ("event.RestrictionsChanged", PlannedField::Restrictions),
            ("person.CitationAdded", PlannedField::Citation),
            ("person.MediaAttached", PlannedField::Media),
            ("family.NoteAttached", PlannedField::Note),
            ("person.Tagged", PlannedField::Tag),
            ("record", PlannedField::Other),
            ("error", PlannedField::Other),
            ("person.Unheard", PlannedField::Other),
        ];
        for (key, field) in cases {
            assert_eq!(PlannedField::of(key), field, "{key}");
        }
    }

    #[test]
    fn a_fact_field_names_its_type_and_no_other_field_does() {
        assert_eq!(
            PlannedField::Fact(FactType::NobilityTitle).fact_name().as_deref(),
            Some("NobilityTitle")
        );
        assert_eq!(PlannedField::Author.fact_name(), None);
    }

    #[test]
    fn a_field_two_keys_name_is_listed_once() {
        let keys = ["person.NameAsserted", "person.Tagged", "person.NameAsserted"].map(str::to_owned);
        assert_eq!(PlannedField::all(&keys), [PlannedField::Name, PlannedField::Tag]);
    }
}
