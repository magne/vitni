//! `genealogy-gedcom` — pure GEDCOM parse/emit over a small intermediate model.
//!
//! This crate holds the format logic of the GEDCOM import/export plugins, free of any WASM or host
//! types, so it is unit-tested and linted through the normal `cargo --workspace` path. The wasm glue
//! crates (`plugins/gedcom-*`) depend on it and only bridge its [`Tree`] to the host capabilities.
//!
//! It parses `INDI`/`FAM`/`SOUR` records: structured `NAME` sub-records, the full `DATE` grammar
//! (calendars, modifiers, dual dates), events with `PLAC`/`ADDR`, INDI-attribute facts, `ASSO`
//! associations, citations, media, and notes (data-model F′). Broader Gramps XML mapping is later
//! Phase 4 work (ADR 0013).

mod emit;
mod model;
mod parse;

pub use emit::emit;
pub use model::{
    Address, Age, AgeBound, Association, AssociationKind, Calendar, ChildRef, Citation, Date, DateModifier, DatePoint,
    DateQuality, Event, EventAssociation, EventKind, Fact, FactKind, Family, Header, Individual, MediaObject, Name,
    NameKind, Place, Restriction, Sex, Source, Tree,
};
pub use parse::{GedcomError, parse};

#[cfg(test)]
mod tests {
    use super::{
        Address, Age, AgeBound, Association, AssociationKind, Calendar, ChildRef, Citation, Date, DateModifier,
        DatePoint, DateQuality, Event, EventAssociation, EventKind, Fact, FactKind, Family, Header, Individual,
        MediaObject, Name, NameKind, Place, Restriction, Sex, Source, Tree, emit, parse,
    };

    /// An exact Gregorian date with the given parts and a matching `original`.
    fn exact(year: i32, month: Option<u8>, day: Option<u8>, original: &str) -> Date {
        Date {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: DateModifier::Exact(DatePoint {
                year: Some(year),
                month,
                day,
            }),
            new_year_begins: None,
            original: original.to_owned(),
        }
    }

    /// An event carrying only the new participant-age / witness fields at their empty defaults, for
    /// struct-update of the older event literals (`Event` has no `Default` — `EventKind` has none).
    fn event_defaults() -> Event {
        Event {
            kind: EventKind::Birth,
            date: None,
            place: None,
            address: None,
            age: None,
            husband_age: None,
            wife_age: None,
            associations: Vec::new(),
        }
    }

    /// A name with only a given name and a primary surname.
    fn simple_name(given: &str, surname: &str) -> Name {
        Name {
            given: Some(given.to_owned()),
            surname: Some(surname.to_owned()),
            ..Name::default()
        }
    }

    fn sample() -> Tree {
        Tree {
            header: Header {
                date: Some(exact(2006, Some(3), Some(27), "27 MAR 2006")),
            },
            individuals: vec![
                Individual {
                    xref: "I0001".to_owned(),
                    uid: Some("D02D344F-F781-4337-BCF1-0A1A1A548280".to_owned()),
                    name: Some(Name {
                        name_type: Some(NameKind::BirthName),
                        given: Some("John".to_owned()),
                        surname_prefix: Some("van".to_owned()),
                        surname: Some("Smith".to_owned()),
                        nickname: Some("Jack".to_owned()),
                        prefix: Some("Dr".to_owned()),
                        suffix: Some("Jr".to_owned()),
                    }),
                    sex: Some(Sex::Male),
                    events: vec![Event {
                        kind: EventKind::Birth,
                        date: Some(exact(1970, Some(4), Some(5), "5 APR 1970")),
                        place: Some(Place {
                            name: "Mandal".to_owned(),
                            latitude: Some("N58.028".to_owned()),
                            longitude: Some("E7.462".to_owned()),
                        }),
                        address: Some(Address {
                            lines: vec!["1 Main St".to_owned()],
                            locality: Some("Bergen".to_owned()),
                            postal_code: Some("5003".to_owned()),
                            country: Some("Norway".to_owned()),
                            phone: Some("+47 555".to_owned()),
                            ..Address::default()
                        }),
                        ..event_defaults()
                    }],
                    facts: vec![Fact {
                        kind: FactKind::Occupation,
                        value: Some("Carpenter".to_owned()),
                        date: None,
                    }],
                    associations: vec![Association {
                        other_xref: "I0002".to_owned(),
                        role: Some(AssociationKind::Witness),
                    }],
                    citations: vec![Citation {
                        source_xref: "S0001".to_owned(),
                        page: Some("p. 5".to_owned()),
                    }],
                    media: vec![MediaObject {
                        file: Some("https://example.test/photo.jpg".to_owned()),
                        title: Some("Portrait".to_owned()),
                        mime: Some("image/jpeg".to_owned()),
                    }],
                    notes: vec!["A research note.".to_owned()],
                    restrictions: vec![Restriction::Confidential, Restriction::Privacy],
                },
                Individual {
                    xref: "I0002".to_owned(),
                    name: Some(simple_name("Jane", "Doe")),
                    sex: Some(Sex::Female),
                    ..Individual::default()
                },
                Individual {
                    xref: "I0003".to_owned(),
                    uid: Some("A673BB63-328E-4F79-B4E3-ABCF43460749".to_owned()),
                    name: Some(simple_name("Sam", "Smith")),
                    ..Individual::default()
                },
            ],
            families: vec![Family {
                xref: "F0001".to_owned(),
                uid: Some("11111111-2222-3333-4444-555555555555".to_owned()),
                partners: vec!["I0001".to_owned(), "I0002".to_owned()],
                children: vec![ChildRef {
                    xref: "I0003".to_owned(),
                    father_relationship: Some("Birth".to_owned()),
                    mother_relationship: Some("Adopted".to_owned()),
                }],
                events: vec![Event {
                    kind: EventKind::Marriage,
                    date: Some(exact(1995, None, None, "1995")),
                    place: None,
                    address: None,
                    ..event_defaults()
                }],
                restrictions: vec![Restriction::Privacy],
            }],
            sources: vec![Source {
                xref: "S0001".to_owned(),
                title: Some("Census 1801".to_owned()),
                author: Some("Statistics Norway".to_owned()),
                pub_info: None,
            }],
        }
    }

    #[test]
    fn parses_individuals_names_and_family_structure() {
        let text = "\
0 HEAD
1 SOUR test
0 @I1@ INDI
1 NAME John /Smith/
0 @I2@ INDI
1 NAME Jane /Doe/
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 CHIL @I3@
0 TRLR
";
        let tree = parse(text).expect("parse");
        assert_eq!(tree.individuals.len(), 2);
        let name = tree.individuals[0].name.as_ref().expect("name");
        assert_eq!(name.given.as_deref(), Some("John"));
        assert_eq!(name.surname.as_deref(), Some("Smith"));
        assert_eq!(tree.families.len(), 1);
        assert_eq!(tree.families[0].partners, vec!["I1".to_owned(), "I2".to_owned()]);
        assert_eq!(
            tree.families[0].children,
            vec![ChildRef {
                xref: "I3".to_owned(),
                father_relationship: None,
                mother_relationship: None,
            }]
        );
    }

    #[test]
    fn round_trips_a_rich_tree_through_emit_and_parse() {
        let tree = sample();
        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "emit then parse must reproduce the tree");
    }

    #[test]
    fn parses_structured_name_sub_records() {
        let text = "\
0 @I1@ INDI
1 NAME John /Smith/
2 TYPE birth
2 GIVN Johnny
2 SPFX van
2 SURN Smithson
2 NICK Jack
2 NPFX Dr
2 NSFX Jr
0 TRLR
";
        let tree = parse(text).expect("parse");
        let name = tree.individuals[0].name.as_ref().expect("name");
        // Structured sub-records override the slash form.
        assert_eq!(name.given.as_deref(), Some("Johnny"));
        assert_eq!(name.surname.as_deref(), Some("Smithson"));
        assert_eq!(name.surname_prefix.as_deref(), Some("van"));
        assert_eq!(name.nickname.as_deref(), Some("Jack"));
        assert_eq!(name.prefix.as_deref(), Some("Dr"));
        assert_eq!(name.suffix.as_deref(), Some("Jr"));
        assert_eq!(name.name_type, Some(NameKind::BirthName));
    }

    #[test]
    fn captures_the_stable_uid_on_individuals_and_families() {
        let text = "\
0 @I37@ INDI
1 NAME Magne /Rasmussen/
1 _UID A673BB63-328E-4F79-B4E3-ABCF43460749
0 @F12@ FAM
1 HUSB @I37@
1 _UID 11111111-2222-3333-4444-555555555555
0 TRLR
";
        let tree = parse(text).expect("parse");
        assert_eq!(
            tree.individuals[0].uid.as_deref(),
            Some("A673BB63-328E-4F79-B4E3-ABCF43460749")
        );
        assert_eq!(
            tree.families[0].uid.as_deref(),
            Some("11111111-2222-3333-4444-555555555555")
        );
    }

    #[test]
    fn handles_a_given_only_name() {
        let tree = parse("0 @I1@ INDI\n1 NAME Madonna\n0 TRLR\n").expect("parse");
        let name = tree.individuals[0].name.as_ref().expect("name");
        assert_eq!(name.given.as_deref(), Some("Madonna"));
        assert_eq!(name.surname, None);
    }

    #[test]
    fn an_empty_document_parses_to_an_empty_tree() {
        assert_eq!(parse("").expect("parse"), Tree::default());
        assert_eq!(parse("0 HEAD\n0 TRLR\n").expect("parse"), Tree::default());
    }

    #[test]
    fn parses_the_head_export_date() {
        let tree = parse("0 HEAD\n1 DATE 27 MAR 2006\n0 TRLR\n").expect("parse");
        assert_eq!(tree.header.date, Some(exact(2006, Some(3), Some(27), "27 MAR 2006")));
    }

    #[test]
    fn a_head_with_no_date_line_has_no_export_date() {
        let tree = parse("0 HEAD\n1 SOUR test\n0 TRLR\n").expect("parse");
        assert_eq!(tree.header.date, None);
    }

    #[test]
    fn an_unparseable_head_date_degrades_to_text_without_breaking_the_rest_of_the_parse() {
        let text = "\
0 HEAD
1 DATE the spring of the great flood
0 @I1@ INDI
1 NAME Ada /Lovelace/
0 TRLR
";
        let tree = parse(text).expect("parse");
        assert_eq!(
            tree.header.date.map(|d| d.modifier),
            Some(DateModifier::TextOnly("the spring of the great flood".to_owned()))
        );
        // An unparseable HEAD date must not derail parsing the rest of the document (ADR 0029 §3).
        assert_eq!(tree.individuals.len(), 1);
    }

    #[test]
    fn head_export_date_round_trips_through_emit_and_parse() {
        let tree = Tree {
            header: Header {
                date: Some(exact(2006, Some(3), Some(27), "27 MAR 2006")),
            },
            ..Tree::default()
        };
        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "the HEAD export date round-trips (ADR 0029 §2)");
    }

    #[test]
    fn a_missing_head_date_emits_no_date_line() {
        assert!(
            !emit(&Tree::default()).contains("DATE"),
            "no HEAD DATE line when no export date is recorded"
        );
    }

    #[test]
    fn skips_unknown_tags_but_keeps_the_record() {
        let tree = parse("0 @I1@ INDI\n1 SEX M\n1 FOOO ignored\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals.len(), 1);
        assert_eq!(tree.individuals[0].name, None);
    }

    #[test]
    fn tolerates_crlf_line_endings() {
        let tree = parse("0 @I1@ INDI\r\n1 NAME Ada /Lovelace/\r\n0 TRLR\r\n").expect("parse");
        let name = tree.individuals[0].name.as_ref().expect("name");
        assert_eq!(name.surname.as_deref(), Some("Lovelace"));
    }

    #[test]
    fn rejects_a_line_without_a_numeric_level() {
        assert!(parse("INDI\n").is_err());
    }

    #[test]
    fn parses_events_with_date_and_place() {
        let text = "\
0 @I1@ INDI
1 BIRT
2 DATE 5 APR 1970
2 PLAC Mandal
0 @F1@ FAM
1 MARR
2 DATE 1995
0 TRLR
";
        let tree = parse(text).expect("parse");
        let events = &tree.individuals[0].events;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Birth);
        assert_eq!(events[0].date, Some(exact(1970, Some(4), Some(5), "5 APR 1970")));
        assert_eq!(events[0].place.as_ref().map(|p| p.name.as_str()), Some("Mandal"));
        assert_eq!(tree.families[0].events[0].kind, EventKind::Marriage);
    }

    #[test]
    fn parses_and_round_trips_a_place_with_map_coordinates() {
        let text = "\
0 @I1@ INDI
1 BIRT
2 PLAC Mandal
3 MAP
4 LATI N58.028
4 LONG E7.462
0 TRLR
";
        let tree = parse(text).expect("parse");
        let place = tree.individuals[0].events[0].place.as_ref().expect("place");
        assert_eq!(place.name, "Mandal");
        assert_eq!(place.latitude.as_deref(), Some("N58.028"));
        assert_eq!(place.longitude.as_deref(), Some("E7.462"));

        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "PLAC.MAP round-trips (ADR 0024 §4)");
    }

    #[test]
    fn a_place_with_no_map_has_no_map_line_on_emit() {
        let tree = Tree {
            individuals: vec![Individual {
                xref: "I1".to_owned(),
                events: vec![Event {
                    kind: EventKind::Birth,
                    date: None,
                    place: Some(Place {
                        name: "Mandal".to_owned(),
                        latitude: None,
                        longitude: None,
                    }),
                    address: None,
                    ..event_defaults()
                }],
                ..Individual::default()
            }],
            ..Tree::default()
        };
        assert!(
            !emit(&tree).contains("MAP"),
            "no MAP line when no coordinates are recorded"
        );
    }

    #[test]
    fn parses_the_full_date_grammar() {
        let cases = [
            (
                "ABT 1850",
                DateModifier::About(DatePoint {
                    year: Some(1850),
                    month: None,
                    day: None,
                }),
            ),
            (
                "BEF 1900",
                DateModifier::Before(DatePoint {
                    year: Some(1900),
                    month: None,
                    day: None,
                }),
            ),
            (
                "AFT 1800",
                DateModifier::After(DatePoint {
                    year: Some(1800),
                    month: None,
                    day: None,
                }),
            ),
        ];
        for (input, expected) in cases {
            let date = date_of(input);
            assert_eq!(date.modifier, expected, "{input}");
            assert_eq!(date.original, input);
        }

        let range = date_of("BET 1850 AND 1860");
        assert_eq!(
            range.modifier,
            DateModifier::Range {
                start: DatePoint {
                    year: Some(1850),
                    month: None,
                    day: None
                },
                end: DatePoint {
                    year: Some(1860),
                    month: None,
                    day: None
                },
            }
        );

        let span = date_of("FROM 1900 TO 1910");
        assert_eq!(
            span.modifier,
            DateModifier::Span {
                start: DatePoint {
                    year: Some(1900),
                    month: None,
                    day: None
                },
                end: DatePoint {
                    year: Some(1910),
                    month: None,
                    day: None
                },
            }
        );

        assert_eq!(date_of("EST 1855").quality, DateQuality::Estimated);
        assert_eq!(date_of("CAL 1855").quality, DateQuality::Calculated);
    }

    #[test]
    fn parses_a_non_gregorian_calendar() {
        let date = date_of("@#DJULIAN@ 12 MAR 1700");
        assert_eq!(date.calendar, Calendar::Julian);
        assert_eq!(
            date.modifier,
            DateModifier::Exact(DatePoint {
                year: Some(1700),
                month: Some(3),
                day: Some(12)
            })
        );
    }

    #[test]
    fn parses_a_dual_year() {
        let date = date_of("1735/6");
        assert_eq!(date.new_year_begins, Some(3));
        assert_eq!(
            date.modifier,
            DateModifier::Exact(DatePoint {
                year: Some(1735),
                month: None,
                day: None
            })
        );
    }

    #[test]
    fn an_unparseable_date_is_kept_as_text() {
        let date = date_of("the spring of the great flood");
        assert_eq!(
            date.modifier,
            DateModifier::TextOnly("the spring of the great flood".to_owned())
        );
    }

    #[test]
    fn parses_an_interpreted_date() {
        let date = date_of("INT 1850 (about mid-century)");
        assert_eq!(
            date.modifier,
            DateModifier::Interpreted {
                date: DatePoint {
                    year: Some(1850),
                    month: None,
                    day: None
                },
                phrase: "about mid-century".to_owned(),
            }
        );
    }

    #[test]
    fn dates_round_trip_through_emit_and_parse() {
        for input in [
            "5 APR 1970",
            "ABT 1850",
            "BEF 1900",
            "BET 1850 AND 1860",
            "FROM 1900 TO 1910",
            "EST 1855",
            "@#DJULIAN@ 1700",
        ] {
            let date = date_of(input);
            let tree = Tree {
                individuals: vec![Individual {
                    xref: "I1".to_owned(),
                    events: vec![Event {
                        kind: EventKind::Birth,
                        date: Some(date.clone()),
                        place: None,
                        address: None,
                        ..event_defaults()
                    }],
                    ..Individual::default()
                }],
                ..Tree::default()
            };
            let reparsed = parse(&emit(&tree)).expect("reparse");
            let back = reparsed.individuals[0].events[0].date.clone().expect("date");
            assert_eq!(back.modifier, date.modifier, "modifier of {input}");
            assert_eq!(back.calendar, date.calendar, "calendar of {input}");
            assert_eq!(back.quality, date.quality, "quality of {input}");
        }
    }

    #[test]
    fn parses_a_residence_address() {
        let text = "\
0 @I1@ INDI
1 RESI
2 ADDR 12 Market Square
3 CITY Bergen
3 STAE Vestland
3 POST 5003
3 CTRY Norway
2 PHON +47 555 1234
0 TRLR
";
        let tree = parse(text).expect("parse");
        let address = tree.individuals[0].events[0].address.as_ref().expect("address");
        assert_eq!(address.lines, vec!["12 Market Square".to_owned()]);
        assert_eq!(address.locality.as_deref(), Some("Bergen"));
        assert_eq!(address.region.as_deref(), Some("Vestland"));
        assert_eq!(address.postal_code.as_deref(), Some("5003"));
        assert_eq!(address.country.as_deref(), Some("Norway"));
        assert_eq!(address.phone.as_deref(), Some("+47 555 1234"));
    }

    #[test]
    fn parses_indi_attribute_facts() {
        let text = "\
0 @I1@ INDI
1 OCCU Carpenter
1 RELI Lutheran
1 NCHI 3
0 TRLR
";
        let tree = parse(text).expect("parse");
        let facts = &tree.individuals[0].facts;
        assert_eq!(facts.len(), 3);
        assert_eq!(facts[0].kind, FactKind::Occupation);
        assert_eq!(facts[0].value.as_deref(), Some("Carpenter"));
        assert_eq!(facts[1].kind, FactKind::Religion);
        assert_eq!(facts[2].kind, FactKind::NumberOfChildren);
        assert_eq!(facts[2].value.as_deref(), Some("3"));
    }

    #[test]
    fn parses_associations_with_a_role() {
        let text = "\
0 @I1@ INDI
1 ASSO @I2@
2 ROLE WITN
0 TRLR
";
        let tree = parse(text).expect("parse");
        let associations = &tree.individuals[0].associations;
        assert_eq!(associations.len(), 1);
        assert_eq!(associations[0].other_xref, "I2");
        assert_eq!(associations[0].role, Some(AssociationKind::Witness));
    }

    #[test]
    fn parses_media_and_notes() {
        let text = "\
0 @I1@ INDI
1 OBJE
2 FILE https://example.test/p.jpg
2 TITL Portrait
1 NOTE A note.
0 TRLR
";
        let tree = parse(text).expect("parse");
        assert_eq!(tree.individuals[0].media.len(), 1);
        assert_eq!(
            tree.individuals[0].media[0].file.as_deref(),
            Some("https://example.test/p.jpg")
        );
        assert_eq!(tree.individuals[0].notes, vec!["A note.".to_owned()]);
    }

    #[test]
    fn parses_sources_and_citations() {
        let text = "\
0 @I1@ INDI
1 NAME Ada /Lovelace/
1 SOUR @S1@
2 PAGE p. 12
0 @S1@ SOUR
1 TITL Census 1801
0 TRLR
";
        let tree = parse(text).expect("parse");
        assert_eq!(tree.sources.len(), 1);
        assert_eq!(tree.sources[0].xref, "S1");
        assert_eq!(tree.sources[0].title.as_deref(), Some("Census 1801"));
        let citations = &tree.individuals[0].citations;
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_xref, "S1");
        assert_eq!(citations[0].page.as_deref(), Some("p. 12"));
    }

    #[test]
    fn parses_sex() {
        let tree = parse("0 @I1@ INDI\n1 SEX F\n0 @I2@ INDI\n1 SEX X\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals[0].sex, Some(Sex::Female));
        assert_eq!(tree.individuals[1].sex, Some(Sex::Intersex));
    }

    #[test]
    fn strips_a_leading_utf8_bom() {
        let tree = parse("\u{feff}0 @I1@ INDI\n1 NAME Ada /Lovelace/\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals.len(), 1);
        let name = tree.individuals[0].name.as_ref().expect("name");
        assert_eq!(name.surname.as_deref(), Some("Lovelace"));
    }

    #[test]
    fn parses_and_round_trips_an_indi_event_age() {
        let text = "\
0 @I1@ INDI
1 CENS
2 DATE 1900
2 AGE 45y
0 TRLR
";
        let tree = parse(text).expect("parse");
        let event = &tree.individuals[0].events[0];
        assert_eq!(
            event.age,
            Some(Age {
                years: Some(45),
                ..Age::default()
            })
        );
        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "INDI AGE round-trips");
    }

    #[test]
    fn parses_and_round_trips_family_partner_ages() {
        let text = "\
0 @I1@ INDI
1 NAME John /Smith/
0 @I2@ INDI
1 NAME Jane /Doe/
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 MARR
2 DATE 1848
2 HUSB
3 AGE 25y
2 WIFE
3 AGE < 24y 6m
0 TRLR
";
        let tree = parse(text).expect("parse");
        let marriage = &tree.families[0].events[0];
        assert_eq!(
            marriage.husband_age,
            Some(Age {
                years: Some(25),
                ..Age::default()
            })
        );
        assert_eq!(
            marriage.wife_age,
            Some(Age {
                bound: Some(AgeBound::LessThan),
                years: Some(24),
                months: Some(6),
                ..Age::default()
            })
        );
        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "HUSB/WIFE ages round-trip");
    }

    #[test]
    fn parses_and_round_trips_an_event_level_association() {
        let text = "\
0 @I1@ INDI
1 BIRT
2 DATE 1850
2 ASSO @I2@
3 ROLE WITN
3 SOUR @S1@
4 PAGE p. 3
3 NOTE Witnessed the baptism.
0 @I2@ INDI
1 NAME Pat /Vitne/
0 @S1@ SOUR
1 TITL Parish register
0 TRLR
";
        let tree = parse(text).expect("parse");
        let birth = &tree.individuals[0].events[0];
        assert_eq!(
            birth.associations,
            vec![EventAssociation {
                other_xref: "I2".to_owned(),
                role: Some(AssociationKind::Witness),
                citations: vec![Citation {
                    source_xref: "S1".to_owned(),
                    page: Some("p. 3".to_owned()),
                }],
                notes: vec!["Witnessed the baptism.".to_owned()],
            }]
        );
        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "event-level ASSO witness round-trips");
    }

    /// Parses a single `DATE` value through a one-individual document.
    fn date_of(value: &str) -> Date {
        let text = format!("0 @I1@ INDI\n1 BIRT\n2 DATE {value}\n0 TRLR\n");
        parse(&text)
            .expect("parse")
            .individuals
            .swap_remove(0)
            .events
            .swap_remove(0)
            .date
            .expect("date")
    }
}
