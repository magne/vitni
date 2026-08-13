//! Serializes the intermediate [`Tree`] back to GEDCOM text.

use std::collections::HashMap;
use std::fmt::Write as _;

use vitni_interchange::age_value;

use crate::model::{
    Address, Age, AssociationKind, Calendar, Citation, Date, DateModifier, DatePoint, DateQuality, Event,
    EventAssociation, EventKind, Fact, FactKind, Individual, MediaObject, Name, NameKind, Place, RepositoryCitation,
    Restriction, Sex, SourceMediaKind, Tree,
};

/// The GEDCOM tags partners are emitted under, in order (first partner → `HUSB`, second → `WIFE`).
const PARTNER_TAGS: [&str; 2] = ["HUSB", "WIFE"];

/// GEDCOM month abbreviations, 1-indexed.
const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// Emits `tree` as a GEDCOM document (5.5-style header, the records, and a trailer).
#[must_use]
pub fn emit(tree: &Tree) -> String {
    let mut out = String::new();
    out.push_str("0 HEAD\n1 SOUR vitni\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n");
    if let Some(date) = &tree.header.date {
        let _ = writeln!(out, "1 DATE {}", date_value(date));
    }

    let (fams, famc) = family_back_refs(tree);
    for individual in &tree.individuals {
        emit_individual(&mut out, individual, &fams, &famc);
    }

    for family in &tree.families {
        let _ = writeln!(out, "0 @{}@ FAM", family.xref);
        if let Some(uid) = &family.uid {
            let _ = writeln!(out, "1 _UID {uid}");
        }
        emit_resn(&mut out, &family.restrictions);
        for (index, partner) in family.partners.iter().enumerate() {
            let tag = PARTNER_TAGS.get(index).copied().unwrap_or("HUSB");
            let _ = writeln!(out, "1 {tag} @{partner}@");
        }
        for child in &family.children {
            let _ = writeln!(out, "1 CHIL @{}@", child.xref);
            if let Some(frel) = &child.father_relationship {
                let _ = writeln!(out, "2 _FREL {frel}");
            }
            if let Some(mrel) = &child.mother_relationship {
                let _ = writeln!(out, "2 _MREL {mrel}");
            }
        }
        for event in &family.events {
            emit_event(&mut out, event);
        }
        for citation in &family.citations {
            emit_citation(&mut out, citation);
        }
        for media in &family.media {
            emit_media(&mut out, media);
        }
        for note in &family.notes {
            let _ = writeln!(out, "1 NOTE {note}");
        }
    }

    for source in &tree.sources {
        let _ = writeln!(out, "0 @{}@ SOUR", source.xref);
        if let Some(title) = &source.title {
            let _ = writeln!(out, "1 TITL {title}");
        }
        if let Some(author) = &source.author {
            let _ = writeln!(out, "1 AUTH {author}");
        }
        if let Some(pub_info) = &source.pub_info {
            let _ = writeln!(out, "1 PUBL {pub_info}");
        }
        if let Some(abbrev) = &source.abbrev {
            let _ = writeln!(out, "1 ABBR {abbrev}");
        }
        for citation in &source.repository_refs {
            emit_repository_citation(&mut out, citation);
        }
    }

    for repository in &tree.repositories {
        let _ = writeln!(out, "0 @{}@ REPO", repository.xref);
        if let Some(name) = &repository.name {
            let _ = writeln!(out, "1 NAME {name}");
        }
    }

    out.push_str("0 TRLR\n");
    out
}

/// An individual's xref mapped to the xrefs of the families it is linked from (`FAMS`/`FAMC`).
type FamilyXrefMap<'a> = HashMap<&'a str, Vec<&'a str>>;

/// Builds the `FAMS`/`FAMC` back-reference maps from the family list: each individual's xref maps
/// to the families where it is a partner (`FAMS`) or a child (`FAMC`). Computed fresh from
/// `Family.partners`/`.children` rather than modeled on `Individual` — a GEDCOM back-reference is
/// derived/redundant data (`FAM.HUSB`/`WIFE`/`CHIL` already carries the same membership), kept only
/// for compatibility with readers that expect it, so there is nothing new for `parse` to capture on
/// re-import.
fn family_back_refs(tree: &Tree) -> (FamilyXrefMap<'_>, FamilyXrefMap<'_>) {
    let mut fams: FamilyXrefMap<'_> = HashMap::new();
    let mut famc: FamilyXrefMap<'_> = HashMap::new();
    for family in &tree.families {
        for partner in &family.partners {
            fams.entry(partner.as_str()).or_default().push(family.xref.as_str());
        }
        for child in &family.children {
            famc.entry(child.xref.as_str()).or_default().push(family.xref.as_str());
        }
    }
    (fams, famc)
}

/// Emits one `INDI` record and all its sub-structures, including its `FAMS`/`FAMC` back-references
/// (looked up by its own xref in `fams`/`famc`, built once by [`family_back_refs`]).
fn emit_individual(out: &mut String, individual: &Individual, fams: &FamilyXrefMap<'_>, famc: &FamilyXrefMap<'_>) {
    let _ = writeln!(out, "0 @{}@ INDI", individual.xref);
    for name in &individual.names {
        emit_name(out, name);
    }
    if let Some(sex) = individual.sex {
        let _ = writeln!(out, "1 SEX {}", sex_value(sex));
    }
    if let Some(uid) = &individual.uid {
        let _ = writeln!(out, "1 _UID {uid}");
    }
    emit_resn(out, &individual.restrictions);
    for family_xref in famc.get(individual.xref.as_str()).into_iter().flatten() {
        let _ = writeln!(out, "1 FAMC @{family_xref}@");
    }
    for family_xref in fams.get(individual.xref.as_str()).into_iter().flatten() {
        let _ = writeln!(out, "1 FAMS @{family_xref}@");
    }
    for event in &individual.events {
        emit_event(out, event);
    }
    for fact in &individual.facts {
        emit_fact(out, fact);
    }
    for association in &individual.associations {
        let _ = writeln!(out, "1 ASSO @{}@", association.other_xref);
        if let Some(role) = &association.role {
            let _ = writeln!(out, "2 ROLE {}", association_role(role));
        }
    }
    for citation in &individual.citations {
        emit_citation(out, citation);
    }
    for media in &individual.media {
        emit_media(out, media);
    }
    for note in &individual.notes {
        let _ = writeln!(out, "1 NOTE {note}");
    }
}

/// Emits an `OBJE` media object (`1 OBJE`, then `FILE`/`TITL`/`FORM`/`CAPT` — shared by `INDI` and
/// `FAM`).
fn emit_media(out: &mut String, media: &MediaObject) {
    let _ = writeln!(out, "1 OBJE");
    if let Some(file) = &media.file {
        let _ = writeln!(out, "2 FILE {file}");
    }
    if let Some(title) = &media.title {
        let _ = writeln!(out, "2 TITL {title}");
    }
    if let Some(mime) = &media.mime {
        let _ = writeln!(out, "2 FORM {mime}");
    }
    if let Some(caption) = &media.caption {
        let _ = writeln!(out, "2 CAPT {caption}");
    }
}

/// Emits a `SOUR.REPO` citation: the repository xref, plus `CALN`/`CALN.MEDI` if a call number is
/// present (GEDCOM nests `MEDI` under `CALN`, so a medium with no call number cannot round-trip —
/// see [`RepositoryCitation`]'s doc comment).
fn emit_repository_citation(out: &mut String, citation: &RepositoryCitation) {
    let _ = writeln!(out, "1 REPO @{}@", citation.xref);
    if let Some(call_number) = &citation.call_number {
        let _ = writeln!(out, "2 CALN {call_number}");
        if let Some(medium) = &citation.medium {
            let _ = writeln!(out, "3 MEDI {}", medi_value(medium));
            if let SourceMediaKind::Other(text) = medium
                && !text.is_empty()
            {
                let _ = writeln!(out, "4 PHRASE {text}");
            }
        }
    }
}

/// Renders a [`SourceMediaKind`] as its GEDCOM `MEDI` value; `Other`'s verbatim text is emitted
/// separately as a `PHRASE` substructure.
fn medi_value(kind: &SourceMediaKind) -> &str {
    match kind {
        SourceMediaKind::Audio => "AUDIO",
        SourceMediaKind::Book => "BOOK",
        SourceMediaKind::Card => "CARD",
        SourceMediaKind::Electronic => "ELECTRONIC",
        SourceMediaKind::Fiche => "FICHE",
        SourceMediaKind::Film => "FILM",
        SourceMediaKind::Magazine => "MAGAZINE",
        SourceMediaKind::Manuscript => "MANUSCRIPT",
        SourceMediaKind::Map => "MAP",
        SourceMediaKind::Newspaper => "NEWSPAPER",
        SourceMediaKind::Photo => "PHOTO",
        SourceMediaKind::Tombstone => "TOMBSTONE",
        SourceMediaKind::Video => "VIDEO",
        SourceMediaKind::Other(_) => "OTHER",
    }
}

/// Emits a `NAME` and its structured sub-records.
fn emit_name(out: &mut String, name: &Name) {
    let _ = writeln!(
        out,
        "1 NAME {}",
        slash_name(name.given.as_deref(), name.surname.as_deref())
    );
    if let Some(name_type) = &name.name_type {
        let _ = writeln!(out, "2 TYPE {}", name_type_value(name_type));
    }
    if let Some(given) = &name.given {
        let _ = writeln!(out, "2 GIVN {given}");
    }
    if let Some(prefix) = &name.surname_prefix {
        let _ = writeln!(out, "2 SPFX {prefix}");
    }
    if let Some(surname) = &name.surname {
        let _ = writeln!(out, "2 SURN {surname}");
    }
    if let Some(nickname) = &name.nickname {
        let _ = writeln!(out, "2 NICK {nickname}");
    }
    if let Some(prefix) = &name.prefix {
        let _ = writeln!(out, "2 NPFX {prefix}");
    }
    if let Some(suffix) = &name.suffix {
        let _ = writeln!(out, "2 NSFX {suffix}");
    }
}

/// Emits one citation (`1 SOUR @S..@`, then `2 PAGE` when present).
fn emit_citation(out: &mut String, citation: &Citation) {
    let _ = writeln!(out, "1 SOUR @{}@", citation.source_xref);
    if let Some(page) = &citation.page {
        let _ = writeln!(out, "2 PAGE {page}");
    }
}

/// Emits one event record (`1 TAG`, then `DATE`/`PLAC`/`ADDR`, participant ages, and event-level
/// `ASSO` witnesses when present).
fn emit_event(out: &mut String, event: &Event) {
    let _ = writeln!(out, "1 {}", event_tag(event.kind));
    if let Some(date) = &event.date {
        let _ = writeln!(out, "2 DATE {}", date_value(date));
    }
    if let Some(place) = &event.place {
        emit_place(out, place);
    }
    if let Some(address) = &event.address {
        emit_address(out, address);
    }
    if let Some(age) = &event.age {
        let _ = writeln!(out, "2 AGE {}", age_value(age));
    }
    emit_partner_age(out, "HUSB", event.husband_age.as_ref());
    emit_partner_age(out, "WIFE", event.wife_age.as_ref());
    for association in &event.associations {
        emit_event_association(out, association);
    }
}

/// Emits an event's place (`2 PLAC`), plus its point (`3 MAP` / `4 LATI` / `4 LONG`) when present
/// (ADR 0024 §4).
fn emit_place(out: &mut String, place: &Place) {
    let _ = writeln!(out, "2 PLAC {}", place.name);
    if place.latitude.is_none() && place.longitude.is_none() {
        return;
    }
    let _ = writeln!(out, "3 MAP");
    if let Some(latitude) = &place.latitude {
        let _ = writeln!(out, "4 LATI {latitude}");
    }
    if let Some(longitude) = &place.longitude {
        let _ = writeln!(out, "4 LONG {longitude}");
    }
}

/// Emits a `FAM`-event partner age (`2 HUSB` / `3 AGE`, `2 WIFE` / `3 AGE`) when present.
fn emit_partner_age(out: &mut String, tag: &str, age: Option<&Age>) {
    if let Some(age) = age {
        let _ = writeln!(out, "2 {tag}");
        let _ = writeln!(out, "3 AGE {}", age_value(age));
    }
}

/// Emits one event-level `ASSO` witness (`2 ASSO @x@`, then `3 ROLE`, nested `3 SOUR`/`4 PAGE`, and
/// `3 NOTE`).
fn emit_event_association(out: &mut String, association: &EventAssociation) {
    let _ = writeln!(out, "2 ASSO @{}@", association.other_xref);
    if let Some(role) = &association.role {
        let _ = writeln!(out, "3 ROLE {}", association_role(role));
    }
    for citation in &association.citations {
        let _ = writeln!(out, "3 SOUR @{}@", citation.source_xref);
        if let Some(page) = &citation.page {
            let _ = writeln!(out, "4 PAGE {page}");
        }
    }
    for note in &association.notes {
        let _ = writeln!(out, "3 NOTE {note}");
    }
}

/// Emits one INDI-attribute fact (`1 TAG value`, then `2 DATE` when present).
fn emit_fact(out: &mut String, fact: &Fact) {
    match &fact.value {
        Some(value) => {
            let _ = writeln!(out, "1 {} {value}", fact_tag(fact.kind));
        }
        None => {
            let _ = writeln!(out, "1 {}", fact_tag(fact.kind));
        }
    }
    if let Some(date) = &fact.date {
        let _ = writeln!(out, "2 DATE {}", date_value(date));
    }
}

/// Emits the `ADDR` structure and the contact subtags beside it.
fn emit_address(out: &mut String, address: &Address) {
    match address.lines.split_first() {
        Some((first, rest)) => {
            let _ = writeln!(out, "2 ADDR {first}");
            for line in rest {
                let _ = writeln!(out, "3 CONT {line}");
            }
        }
        None => {
            let _ = writeln!(out, "2 ADDR");
        }
    }
    if let Some(locality) = &address.locality {
        let _ = writeln!(out, "3 CITY {locality}");
    }
    if let Some(region) = &address.region {
        let _ = writeln!(out, "3 STAE {region}");
    }
    if let Some(postal_code) = &address.postal_code {
        let _ = writeln!(out, "3 POST {postal_code}");
    }
    if let Some(country) = &address.country {
        let _ = writeln!(out, "3 CTRY {country}");
    }
    if let Some(phone) = &address.phone {
        let _ = writeln!(out, "2 PHON {phone}");
    }
    if let Some(email) = &address.email {
        let _ = writeln!(out, "2 EMAIL {email}");
    }
    if let Some(fax) = &address.fax {
        let _ = writeln!(out, "2 FAX {fax}");
    }
    if let Some(www) = &address.www {
        let _ = writeln!(out, "2 WWW {www}");
    }
}

/// The canonical GEDCOM tag for an event kind.
fn event_tag(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Birth => "BIRT",
        EventKind::Death => "DEAT",
        EventKind::Marriage => "MARR",
        EventKind::Baptism => "BAPM",
        EventKind::Christening => "CHR",
        EventKind::Burial => "BURI",
        EventKind::Cremation => "CREM",
        EventKind::Census => "CENS",
        EventKind::Residence => "RESI",
        EventKind::Immigration => "IMMI",
        EventKind::Emigration => "EMIG",
        EventKind::Adoption => "ADOP",
        EventKind::Confirmation => "CONF",
        EventKind::BarMitzvah => "BARM",
        EventKind::BasMitzvah => "BASM",
        EventKind::FirstCommunion => "FCOM",
        EventKind::Graduation => "GRAD",
        EventKind::Naturalization => "NATU",
        EventKind::Ordination => "ORDN",
        EventKind::Probate => "PROB",
        EventKind::Retirement => "RETI",
        EventKind::Will => "WILL",
        EventKind::Engagement => "ENGA",
        EventKind::Annulment => "ANUL",
        EventKind::Divorce => "DIV",
        EventKind::DivorceFiled => "DIVF",
        EventKind::MarriageBanns => "MARB",
        EventKind::MarriageContract => "MARC",
        EventKind::MarriageLicense => "MARL",
        EventKind::MarriageSettlement => "MARS",
    }
}

/// The canonical GEDCOM tag for an INDI-attribute fact kind.
fn fact_tag(kind: FactKind) -> &'static str {
    match kind {
        FactKind::Occupation => "OCCU",
        FactKind::Religion => "RELI",
        FactKind::Education => "EDUC",
        FactKind::Caste => "CAST",
        FactKind::PhysicalDescription => "DSCR",
        FactKind::Ethnicity => "ETHN",
        FactKind::NationalId => "IDNO",
        FactKind::Nationality => "NATI",
        FactKind::NumberOfChildren => "NCHI",
        FactKind::NumberOfMarriages => "NMR",
        FactKind::Property => "PROP",
        FactKind::SocialSecurityNumber => "SSN",
        FactKind::NobilityTitle => "TITL",
    }
}

/// The GEDCOM `ROLE` token for an association kind.
fn association_role(role: &AssociationKind) -> String {
    match role {
        AssociationKind::Clergy => "CLERGY".to_owned(),
        AssociationKind::Friend => "FRIEND".to_owned(),
        AssociationKind::Godparent => "GODP".to_owned(),
        AssociationKind::Neighbour => "NGHBR".to_owned(),
        AssociationKind::Officiator => "OFFICIATOR".to_owned(),
        AssociationKind::Witness => "WITN".to_owned(),
        AssociationKind::Child => "CHIL".to_owned(),
        AssociationKind::Father => "FATH".to_owned(),
        AssociationKind::Mother => "MOTH".to_owned(),
        AssociationKind::Parent => "PARENT".to_owned(),
        AssociationKind::Husband => "HUSB".to_owned(),
        AssociationKind::Wife => "WIFE".to_owned(),
        AssociationKind::Spouse => "SPOU".to_owned(),
        AssociationKind::Multiple => "MULTIPLE".to_owned(),
        AssociationKind::Other(value) => value.clone(),
    }
}

/// The GEDCOM `NAME.TYPE` token for a name kind.
fn name_type_value(name_type: &NameKind) -> String {
    match name_type {
        NameKind::BirthName => "BIRTH".to_owned(),
        NameKind::MarriedName => "MARRIED".to_owned(),
        NameKind::Maiden => "MAIDEN".to_owned(),
        NameKind::Immigrant => "IMMIGRANT".to_owned(),
        NameKind::Professional => "PROFESSIONAL".to_owned(),
        NameKind::AlsoKnownAs => "AKA".to_owned(),
        NameKind::ReligiousName => "RELIGIOUS".to_owned(),
        NameKind::Other(value) => value.clone(),
    }
}

/// Renders a `DATE` value from the structured form (calendar prefix, modifier keyword, points).
fn date_value(date: &Date) -> String {
    let body = match &date.modifier {
        DateModifier::Exact(point) => quality_prefixed(date.quality, &point_value(point, date.new_year_begins)),
        DateModifier::Before(point) => format!("BEF {}", point_value(point, date.new_year_begins)),
        DateModifier::After(point) => format!("AFT {}", point_value(point, date.new_year_begins)),
        DateModifier::About(point) => format!("ABT {}", point_value(point, date.new_year_begins)),
        DateModifier::Range { start, end } => format!(
            "BET {} AND {}",
            point_value(start, date.new_year_begins),
            point_value(end, date.new_year_begins)
        ),
        DateModifier::Span { start, end } => format!(
            "FROM {} TO {}",
            point_value(start, date.new_year_begins),
            point_value(end, date.new_year_begins)
        ),
        DateModifier::From(point) => format!("FROM {}", point_value(point, date.new_year_begins)),
        DateModifier::To(point) => format!("TO {}", point_value(point, date.new_year_begins)),
        DateModifier::Interpreted { date: point, phrase } => {
            format!("INT {} ({phrase})", point_value(point, date.new_year_begins))
        }
        DateModifier::TextOnly(text) => return text.clone(),
    };
    match calendar_escape(date.calendar) {
        Some(escape) => format!("{escape} {body}"),
        None => body,
    }
}

/// Prefixes a date body with its `EST`/`CAL` quality keyword (Normal has none).
fn quality_prefixed(quality: DateQuality, body: &str) -> String {
    match quality {
        DateQuality::Normal => body.to_owned(),
        DateQuality::Estimated => format!("EST {body}"),
        DateQuality::Calculated => format!("CAL {body}"),
    }
}

/// Renders a single date point as `[DAY] [MON] YEAR`, with dual-dating when `new_year_begins` is set.
fn point_value(point: &DatePoint, new_year_begins: Option<u8>) -> String {
    let month = point
        .month
        .and_then(|m| MONTHS.get((m as usize).wrapping_sub(1)).copied());
    let year = match point.year {
        Some(year) if new_year_begins.is_some() => format!("{year}/{:02}", (year + 1).rem_euclid(100)),
        Some(year) => year.to_string(),
        None => String::new(),
    };
    match (point.day, month) {
        (Some(day), Some(month)) => format!("{day} {month} {year}").trim().to_owned(),
        (None, Some(month)) => format!("{month} {year}").trim().to_owned(),
        _ => year,
    }
}

/// The calendar escape prefix, or `None` for the default Gregorian.
fn calendar_escape(calendar: Calendar) -> Option<&'static str> {
    match calendar {
        Calendar::Gregorian => None,
        Calendar::Julian => Some("@#DJULIAN@"),
        Calendar::Hebrew => Some("@#DHEBREW@"),
        Calendar::FrenchRepublican => Some("@#DFRENCH R@"),
        Calendar::Islamic => Some("@#DISLAMIC@"),
        Calendar::Swedish => Some("@#DSWEDISH@"),
    }
}

/// Emits a `1 RESN` line (GEDCOM v7 privacy restrictions) when the set is non-empty.
fn emit_resn(out: &mut String, restrictions: &[Restriction]) {
    if restrictions.is_empty() {
        return;
    }
    let value = restrictions
        .iter()
        .map(|restriction| match restriction {
            Restriction::Confidential => "CONFIDENTIAL",
            Restriction::Locked => "LOCKED",
            Restriction::Privacy => "PRIVACY",
        })
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "1 RESN {value}");
}

/// Renders a GEDCOM `SEX` value.
fn sex_value(sex: Sex) -> &'static str {
    match sex {
        Sex::Male => "M",
        Sex::Female => "F",
        Sex::Intersex => "X",
        Sex::Unknown => "U",
    }
}

/// Renders a GEDCOM `NAME` value: `Given /Surname/`, omitting an absent part.
fn slash_name(given: Option<&str>, surname: Option<&str>) -> String {
    match (given, surname) {
        (Some(given), Some(surname)) => format!("{given} /{surname}/"),
        (Some(given), None) => given.to_owned(),
        (None, Some(surname)) => format!("/{surname}/"),
        (None, None) => String::new(),
    }
}
