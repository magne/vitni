//! Conversions between the format-neutral [`genealogy_interchange`] value types and the host's WIT
//! types (ADR 0013). Every import/export plugin maps the same leaf vocabulary — sex, names, dates,
//! addresses, event/fact/association kinds — to and from the `commands`/`query` capability records,
//! so the mapping lives here once rather than in each plugin.
//!
//! `*_to_wit` converts an interchange value into the WIT record an importer submits; `*_from_wit`
//! converts a WIT record an exporter read back into an interchange value.

use genealogy_interchange::{
    Address, AssociationKind, Calendar, Date, DateModifier, DatePoint, DateQuality, EventKind, FactKind, Name, NameKind,
    Restriction, Sex,
};

use crate::types;

/// Maps interchange restrictions onto the host capability's `restriction` list (GEDCOM `RESN`).
#[must_use]
pub fn restrictions_to_wit(restrictions: &[Restriction]) -> Vec<types::Restriction> {
    restrictions.iter().map(|&r| restriction_to_wit(r)).collect()
}

/// Maps the host capability's `restriction` list onto interchange restrictions.
#[must_use]
pub fn restrictions_from_wit(restrictions: &[types::Restriction]) -> Vec<Restriction> {
    restrictions.iter().map(|&r| restriction_from_wit(r)).collect()
}

/// Maps a Gramps boolean `priv` flag onto the host `restriction` list (lossy — data-model §16):
/// a private record becomes a single `Privacy` restriction.
#[must_use]
pub fn private_to_wit(private: bool) -> Vec<types::Restriction> {
    if private {
        vec![types::Restriction::Privacy]
    } else {
        Vec::new()
    }
}

/// Maps a host `restriction` list back onto a Gramps boolean `priv` flag (lossy): any restriction
/// marks the record private.
#[must_use]
pub fn private_from_wit(restrictions: &[types::Restriction]) -> bool {
    !restrictions.is_empty()
}

fn restriction_to_wit(restriction: Restriction) -> types::Restriction {
    match restriction {
        Restriction::Confidential => types::Restriction::Confidential,
        Restriction::Locked => types::Restriction::Locked,
        Restriction::Privacy => types::Restriction::Privacy,
    }
}

fn restriction_from_wit(restriction: types::Restriction) -> Restriction {
    match restriction {
        types::Restriction::Confidential => Restriction::Confidential,
        types::Restriction::Locked => Restriction::Locked,
        types::Restriction::Privacy => Restriction::Privacy,
    }
}

/// Maps an interchange sex onto the host capability's `sex`.
#[must_use]
pub fn sex_to_wit(sex: Sex) -> types::Sex {
    match sex {
        Sex::Male => types::Sex::Male,
        Sex::Female => types::Sex::Female,
        Sex::Intersex => types::Sex::Intersex,
        Sex::Unknown => types::Sex::Unknown,
    }
}

/// Maps the host capability's `sex` onto an interchange sex.
#[must_use]
pub fn sex_from_wit(sex: types::Sex) -> Sex {
    match sex {
        types::Sex::Male => Sex::Male,
        types::Sex::Female => Sex::Female,
        types::Sex::Intersex => Sex::Intersex,
        types::Sex::Unknown => Sex::Unknown,
    }
}

/// Maps an interchange [`Name`] onto the host capability's `person-name` record (an absent type
/// defaults to a birth name).
#[must_use]
pub fn name_to_wit(name: &Name) -> types::PersonName {
    types::PersonName {
        name_type: name.name_type.as_ref().map_or(types::NameType::BirthName, name_type_to_wit),
        given: name.given.clone(),
        surname_prefix: name.surname_prefix.clone(),
        surname: name.surname.clone(),
        nickname: name.nickname.clone(),
        prefix: name.prefix.clone(),
        suffix: name.suffix.clone(),
    }
}

/// Maps an interchange name kind onto the host capability's `name-type`.
#[must_use]
pub fn name_type_to_wit(kind: &NameKind) -> types::NameType {
    match kind {
        NameKind::BirthName => types::NameType::BirthName,
        NameKind::MarriedName => types::NameType::MarriedName,
        NameKind::Maiden => types::NameType::Maiden,
        NameKind::Immigrant => types::NameType::Immigrant,
        NameKind::Professional => types::NameType::Professional,
        NameKind::AlsoKnownAs => types::NameType::AlsoKnownAs,
        NameKind::ReligiousName => types::NameType::ReligiousName,
        NameKind::Other(value) => types::NameType::Custom(value.clone()),
    }
}

/// Maps the host capability's `name-type` onto an interchange name kind.
#[must_use]
pub fn name_type_from_wit(name_type: types::NameType) -> NameKind {
    match name_type {
        types::NameType::BirthName => NameKind::BirthName,
        types::NameType::MarriedName => NameKind::MarriedName,
        types::NameType::Maiden => NameKind::Maiden,
        types::NameType::Immigrant => NameKind::Immigrant,
        types::NameType::Professional => NameKind::Professional,
        types::NameType::AlsoKnownAs => NameKind::AlsoKnownAs,
        types::NameType::ReligiousName => NameKind::ReligiousName,
        types::NameType::Custom(value) => NameKind::Other(value),
    }
}

/// Maps an interchange [`Date`] onto the host capability's `genealogical-date` record.
#[must_use]
pub fn date_to_wit(date: &Date) -> types::GenealogicalDate {
    types::GenealogicalDate {
        calendar: calendar_to_wit(date.calendar),
        quality: quality_to_wit(date.quality),
        modifier: modifier_to_wit(&date.modifier),
        new_year_begins: date.new_year_begins,
        original_text: Some(date.original.clone()),
    }
}

/// Maps the host capability's `genealogical-date` onto an interchange [`Date`].
#[must_use]
pub fn date_from_wit(date: &types::GenealogicalDate) -> Date {
    Date {
        calendar: calendar_from_wit(date.calendar),
        quality: quality_from_wit(date.quality),
        modifier: modifier_from_wit(&date.modifier),
        new_year_begins: date.new_year_begins,
        original: date.original_text.clone().unwrap_or_default(),
    }
}

/// Maps an interchange calendar onto the host capability's `date-calendar`.
#[must_use]
pub fn calendar_to_wit(calendar: Calendar) -> types::DateCalendar {
    match calendar {
        Calendar::Gregorian => types::DateCalendar::Gregorian,
        Calendar::Julian => types::DateCalendar::Julian,
        Calendar::Hebrew => types::DateCalendar::Hebrew,
        Calendar::FrenchRepublican => types::DateCalendar::FrenchRepublican,
        Calendar::Islamic => types::DateCalendar::Islamic,
        Calendar::Swedish => types::DateCalendar::Swedish,
    }
}

/// Maps the host capability's `date-calendar` onto an interchange calendar.
#[must_use]
pub fn calendar_from_wit(calendar: types::DateCalendar) -> Calendar {
    match calendar {
        types::DateCalendar::Gregorian => Calendar::Gregorian,
        types::DateCalendar::Julian => Calendar::Julian,
        types::DateCalendar::Hebrew => Calendar::Hebrew,
        types::DateCalendar::FrenchRepublican => Calendar::FrenchRepublican,
        types::DateCalendar::Islamic => Calendar::Islamic,
        types::DateCalendar::Swedish => Calendar::Swedish,
    }
}

/// Maps an interchange date quality onto the host capability's `date-quality`.
#[must_use]
pub fn quality_to_wit(quality: DateQuality) -> types::DateQuality {
    match quality {
        DateQuality::Normal => types::DateQuality::Normal,
        DateQuality::Estimated => types::DateQuality::Estimated,
        DateQuality::Calculated => types::DateQuality::Calculated,
    }
}

/// Maps the host capability's `date-quality` onto an interchange date quality.
#[must_use]
pub fn quality_from_wit(quality: types::DateQuality) -> DateQuality {
    match quality {
        types::DateQuality::Normal => DateQuality::Normal,
        types::DateQuality::Estimated => DateQuality::Estimated,
        types::DateQuality::Calculated => DateQuality::Calculated,
    }
}

/// Maps an interchange date modifier onto the host capability's `date-modifier`.
#[must_use]
pub fn modifier_to_wit(modifier: &DateModifier) -> types::DateModifier {
    match modifier {
        DateModifier::Exact(point) => types::DateModifier::Exact(point_to_wit(point)),
        DateModifier::Before(point) => types::DateModifier::Before(point_to_wit(point)),
        DateModifier::After(point) => types::DateModifier::After(point_to_wit(point)),
        DateModifier::About(point) => types::DateModifier::About(point_to_wit(point)),
        DateModifier::Range { start, end } => types::DateModifier::Range(types::DateRange {
            start: point_to_wit(start),
            end: point_to_wit(end),
        }),
        DateModifier::Span { start, end } => types::DateModifier::Span(types::DateRange {
            start: point_to_wit(start),
            end: point_to_wit(end),
        }),
        DateModifier::From(point) => types::DateModifier::FromDate(point_to_wit(point)),
        DateModifier::To(point) => types::DateModifier::ToDate(point_to_wit(point)),
        DateModifier::Interpreted { date, phrase } => types::DateModifier::Interpreted(types::InterpretedDate {
            date: point_to_wit(date),
            phrase: phrase.clone(),
        }),
        DateModifier::TextOnly(text) => types::DateModifier::TextOnly(text.clone()),
    }
}

/// Maps the host capability's `date-modifier` onto an interchange date modifier.
#[must_use]
pub fn modifier_from_wit(modifier: &types::DateModifier) -> DateModifier {
    match modifier {
        types::DateModifier::Exact(point) => DateModifier::Exact(point_from_wit(point)),
        types::DateModifier::Before(point) => DateModifier::Before(point_from_wit(point)),
        types::DateModifier::After(point) => DateModifier::After(point_from_wit(point)),
        types::DateModifier::About(point) => DateModifier::About(point_from_wit(point)),
        types::DateModifier::Range(range) => DateModifier::Range {
            start: point_from_wit(&range.start),
            end: point_from_wit(&range.end),
        },
        types::DateModifier::Span(range) => DateModifier::Span {
            start: point_from_wit(&range.start),
            end: point_from_wit(&range.end),
        },
        types::DateModifier::FromDate(point) => DateModifier::From(point_from_wit(point)),
        types::DateModifier::ToDate(point) => DateModifier::To(point_from_wit(point)),
        types::DateModifier::Interpreted(interpreted) => DateModifier::Interpreted {
            date: point_from_wit(&interpreted.date),
            phrase: interpreted.phrase.clone(),
        },
        types::DateModifier::TextOnly(text) => DateModifier::TextOnly(text.clone()),
    }
}

/// Maps an interchange date point onto the host capability's `date-point`.
#[must_use]
pub fn point_to_wit(point: &DatePoint) -> types::DatePoint {
    types::DatePoint {
        year: point.year,
        month: point.month,
        day: point.day,
    }
}

/// Maps the host capability's `date-point` onto an interchange date point.
#[must_use]
pub fn point_from_wit(point: &types::DatePoint) -> DatePoint {
    DatePoint {
        year: point.year,
        month: point.month,
        day: point.day,
    }
}

/// Maps an interchange [`Address`] onto the host capability's `address` record.
#[must_use]
pub fn address_to_wit(address: &Address) -> types::Address {
    types::Address {
        lines: address.lines.clone(),
        locality: address.locality.clone(),
        region: address.region.clone(),
        postal_code: address.postal_code.clone(),
        country: address.country.clone(),
        phone: address.phone.clone(),
        email: address.email.clone(),
        fax: address.fax.clone(),
        www: address.www.clone(),
        original_text: address.original_text.clone(),
    }
}

/// Maps the host capability's `address` onto an interchange [`Address`].
#[must_use]
pub fn address_from_wit(address: &types::Address) -> Address {
    Address {
        lines: address.lines.clone(),
        locality: address.locality.clone(),
        region: address.region.clone(),
        postal_code: address.postal_code.clone(),
        country: address.country.clone(),
        phone: address.phone.clone(),
        email: address.email.clone(),
        fax: address.fax.clone(),
        www: address.www.clone(),
        original_text: address.original_text.clone(),
    }
}

/// Maps an interchange event kind onto the host capability's `event-type`.
#[must_use]
pub fn event_type_to_wit(kind: EventKind) -> types::EventType {
    match kind {
        EventKind::Birth => types::EventType::Birth,
        EventKind::Death => types::EventType::Death,
        EventKind::Marriage => types::EventType::Marriage,
        EventKind::Baptism => types::EventType::Baptism,
        EventKind::Christening => types::EventType::Christening,
        EventKind::Burial => types::EventType::Burial,
        EventKind::Cremation => types::EventType::Cremation,
        EventKind::Census => types::EventType::Census,
        EventKind::Residence => types::EventType::Residence,
        EventKind::Immigration => types::EventType::Immigration,
        EventKind::Emigration => types::EventType::Emigration,
        EventKind::Adoption => types::EventType::Adoption,
        EventKind::Confirmation => types::EventType::Confirmation,
        EventKind::BarMitzvah => types::EventType::BarMitzvah,
        EventKind::BasMitzvah => types::EventType::BasMitzvah,
        EventKind::FirstCommunion => types::EventType::FirstCommunion,
        EventKind::Graduation => types::EventType::Graduation,
        EventKind::Naturalization => types::EventType::Naturalization,
        EventKind::Ordination => types::EventType::Ordination,
        EventKind::Probate => types::EventType::Probate,
        EventKind::Retirement => types::EventType::Retirement,
        EventKind::Will => types::EventType::Will,
        EventKind::Engagement => types::EventType::Engagement,
        EventKind::Annulment => types::EventType::Annulment,
        EventKind::Divorce => types::EventType::Divorce,
        EventKind::DivorceFiled => types::EventType::DivorceFiled,
        EventKind::MarriageBanns => types::EventType::MarriageBanns,
        EventKind::MarriageContract => types::EventType::MarriageContract,
        EventKind::MarriageLicense => types::EventType::MarriageLicense,
        EventKind::MarriageSettlement => types::EventType::MarriageSettlement,
    }
}

/// Maps the host capability's `event-type` onto an interchange event kind.
#[must_use]
pub fn event_type_from_wit(event_type: types::EventType) -> EventKind {
    match event_type {
        types::EventType::Birth => EventKind::Birth,
        types::EventType::Death => EventKind::Death,
        types::EventType::Marriage => EventKind::Marriage,
        types::EventType::Baptism => EventKind::Baptism,
        types::EventType::Christening => EventKind::Christening,
        types::EventType::Burial => EventKind::Burial,
        types::EventType::Cremation => EventKind::Cremation,
        types::EventType::Census => EventKind::Census,
        types::EventType::Residence => EventKind::Residence,
        types::EventType::Immigration => EventKind::Immigration,
        types::EventType::Emigration => EventKind::Emigration,
        types::EventType::Adoption => EventKind::Adoption,
        types::EventType::Confirmation => EventKind::Confirmation,
        types::EventType::BarMitzvah => EventKind::BarMitzvah,
        types::EventType::BasMitzvah => EventKind::BasMitzvah,
        types::EventType::FirstCommunion => EventKind::FirstCommunion,
        types::EventType::Graduation => EventKind::Graduation,
        types::EventType::Naturalization => EventKind::Naturalization,
        types::EventType::Ordination => EventKind::Ordination,
        types::EventType::Probate => EventKind::Probate,
        types::EventType::Retirement => EventKind::Retirement,
        types::EventType::Will => EventKind::Will,
        types::EventType::Engagement => EventKind::Engagement,
        types::EventType::Annulment => EventKind::Annulment,
        types::EventType::Divorce => EventKind::Divorce,
        types::EventType::DivorceFiled => EventKind::DivorceFiled,
        types::EventType::MarriageBanns => EventKind::MarriageBanns,
        types::EventType::MarriageContract => EventKind::MarriageContract,
        types::EventType::MarriageLicense => EventKind::MarriageLicense,
        types::EventType::MarriageSettlement => EventKind::MarriageSettlement,
    }
}

/// Maps an interchange fact kind onto the host capability's `fact-type` variant.
#[must_use]
pub fn fact_type_to_wit(kind: FactKind) -> types::FactType {
    match kind {
        FactKind::Occupation => types::FactType::Occupation,
        FactKind::Religion => types::FactType::Religion,
        FactKind::Education => types::FactType::Education,
        FactKind::Caste => types::FactType::Caste,
        FactKind::PhysicalDescription => types::FactType::PhysicalDescription,
        FactKind::Ethnicity => types::FactType::Ethnicity,
        FactKind::NationalId => types::FactType::NationalId,
        FactKind::Nationality => types::FactType::Nationality,
        FactKind::NumberOfChildren => types::FactType::NumberOfChildren,
        FactKind::NumberOfMarriages => types::FactType::NumberOfMarriages,
        FactKind::Property => types::FactType::Property,
        FactKind::SocialSecurityNumber => types::FactType::SocialSecurityNumber,
        FactKind::NobilityTitle => types::FactType::NobilityTitle,
    }
}

/// Maps the host capability's `fact-type` onto an interchange fact kind; event-like and custom
/// values have no INDI-attribute fact kind and return `None`.
#[must_use]
pub fn fact_type_from_wit(fact_type: types::FactType) -> Option<FactKind> {
    let kind = match fact_type {
        types::FactType::Occupation => FactKind::Occupation,
        types::FactType::Religion => FactKind::Religion,
        types::FactType::Education => FactKind::Education,
        types::FactType::Caste => FactKind::Caste,
        types::FactType::PhysicalDescription => FactKind::PhysicalDescription,
        types::FactType::Ethnicity => FactKind::Ethnicity,
        types::FactType::NationalId => FactKind::NationalId,
        types::FactType::Nationality => FactKind::Nationality,
        types::FactType::NumberOfChildren => FactKind::NumberOfChildren,
        types::FactType::NumberOfMarriages => FactKind::NumberOfMarriages,
        types::FactType::Property => FactKind::Property,
        types::FactType::SocialSecurityNumber => FactKind::SocialSecurityNumber,
        types::FactType::NobilityTitle => FactKind::NobilityTitle,
        types::FactType::Birth
        | types::FactType::Death
        | types::FactType::Baptism
        | types::FactType::Burial
        | types::FactType::Residence
        | types::FactType::Custom(_) => return None,
    };
    Some(kind)
}

/// Maps an interchange association kind onto the host capability's `association-role` variant (an
/// absent role becomes a custom `associate`).
#[must_use]
pub fn association_role_to_wit(role: Option<&AssociationKind>) -> types::AssociationRole {
    match role {
        Some(AssociationKind::Clergy) => types::AssociationRole::Clergy,
        Some(AssociationKind::Friend) => types::AssociationRole::Friend,
        Some(AssociationKind::Godparent) => types::AssociationRole::Godparent,
        Some(AssociationKind::Neighbour) => types::AssociationRole::Neighbour,
        Some(AssociationKind::Officiator) => types::AssociationRole::Officiator,
        Some(AssociationKind::Witness) => types::AssociationRole::Witness,
        Some(AssociationKind::Child) => types::AssociationRole::Child,
        Some(AssociationKind::Father) => types::AssociationRole::Father,
        Some(AssociationKind::Mother) => types::AssociationRole::Mother,
        Some(AssociationKind::Parent) => types::AssociationRole::Parent,
        Some(AssociationKind::Husband) => types::AssociationRole::Husband,
        Some(AssociationKind::Wife) => types::AssociationRole::Wife,
        Some(AssociationKind::Spouse) => types::AssociationRole::Spouse,
        Some(AssociationKind::Multiple) => types::AssociationRole::Multiple,
        Some(AssociationKind::Other(value)) => types::AssociationRole::Custom(value.clone()),
        None => types::AssociationRole::Custom("associate".to_owned()),
    }
}

/// Maps the host capability's `association-role` onto an interchange association kind.
#[must_use]
pub fn association_role_from_wit(role: types::AssociationRole) -> AssociationKind {
    match role {
        types::AssociationRole::Clergy => AssociationKind::Clergy,
        types::AssociationRole::Friend => AssociationKind::Friend,
        types::AssociationRole::Godparent => AssociationKind::Godparent,
        types::AssociationRole::Neighbour => AssociationKind::Neighbour,
        types::AssociationRole::Officiator => AssociationKind::Officiator,
        types::AssociationRole::Witness => AssociationKind::Witness,
        types::AssociationRole::Child => AssociationKind::Child,
        types::AssociationRole::Father => AssociationKind::Father,
        types::AssociationRole::Mother => AssociationKind::Mother,
        types::AssociationRole::Parent => AssociationKind::Parent,
        types::AssociationRole::Husband => AssociationKind::Husband,
        types::AssociationRole::Wife => AssociationKind::Wife,
        types::AssociationRole::Spouse => AssociationKind::Spouse,
        types::AssociationRole::Multiple => AssociationKind::Multiple,
        types::AssociationRole::Custom(value) => AssociationKind::Other(value),
    }
}

/// Maps a WIT `child-relationship` to its canonical GEDCOM/Gramps string (`_FREL`/`_MREL`,
/// `frel`/`mrel`), for export.
#[must_use]
pub fn child_relationship_from_wit(relationship: &types::ChildRelationship) -> String {
    match relationship {
        types::ChildRelationship::Birth => "Birth".to_owned(),
        types::ChildRelationship::Adopted => "Adopted".to_owned(),
        types::ChildRelationship::Foster => "Foster".to_owned(),
        types::ChildRelationship::Step => "Stepchild".to_owned(),
        types::ChildRelationship::Sealed => "Sealed".to_owned(),
        types::ChildRelationship::Unknown => "Unknown".to_owned(),
        types::ChildRelationship::Custom(value) => value.clone(),
    }
}

/// Maps a raw GEDCOM/Gramps child-relationship string back to a WIT `child-relationship`
/// (case-insensitive), for import. An unrecognized value becomes a `custom`.
#[must_use]
pub fn child_relationship_to_wit(value: &str) -> types::ChildRelationship {
    match value.trim().to_ascii_lowercase().as_str() {
        "birth" => types::ChildRelationship::Birth,
        "adopted" => types::ChildRelationship::Adopted,
        "foster" => types::ChildRelationship::Foster,
        "step" | "stepchild" => types::ChildRelationship::Step,
        "sealed" => types::ChildRelationship::Sealed,
        "unknown" => types::ChildRelationship::Unknown,
        _ => types::ChildRelationship::Custom(value.to_owned()),
    }
}
