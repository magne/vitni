//! `genealogy-gramps-xml` — pure Gramps XML parse/emit over an intermediate model.
//!
//! Mirrors `genealogy-gedcom`: the format logic of the Gramps import/export plugins, free of any
//! WASM or host types, so it is unit-tested through the normal `cargo --workspace` path. The wasm
//! glue crates (`plugins/gramps-*`) depend on it and only bridge its [`Database`] to the host
//! capabilities.
//!
//! Gramps XML is reference-based: a `<person>` holds `<eventref hlink="...">`, `<citationref>`,
//! `<noteref>`, `<objref>`, and `<personref>` pointers into the top-level `<events>`, `<citations>`,
//! `<notes>`, `<objects>` lists. The [`Database`] preserves that shape (records keyed by `handle`),
//! so emit reproduces it; the plugin resolves the `hlink`s. Real `.gramps` files are gzipped;
//! [`parse`] sniffs the gzip magic and inflates before parsing. [`emit`] writes plain (uncompressed)
//! XML.

mod emit;
mod model;
mod parse;
mod xml;

pub use emit::emit;
pub use model::{
    ChildRef, Citation, Database, Event, EventRef, EventRefAttribute, Family, Gender, Header, MediaObject, MediaRef,
    Note, Person, PersonRef, Place, Region, RepoRef, Repository, Source, Tag,
};
pub use parse::{GrampsError, parse};

#[cfg(test)]
mod tests {
    use super::{
        ChildRef, Citation, Database, Event, EventRef, EventRefAttribute, Family, Gender, Header, MediaObject,
        MediaRef, Note, Person, PersonRef, Place, Region, RepoRef, Repository, Source, Tag, emit, parse,
    };
    use genealogy_interchange::{
        AssociationKind, Calendar, Date, DateModifier, DatePoint, DateQuality, EventKind, Name, SourceMediaKind,
    };

    fn exact(year: i32) -> Date {
        Date {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: DateModifier::Exact(DatePoint {
                year: Some(year),
                month: None,
                day: None,
            }),
            new_year_begins: None,
            original: String::new(),
        }
    }

    fn sample_people() -> Vec<Person> {
        vec![
            Person {
                handle: "_p1".to_owned(),
                gramps_id: Some("I0001".to_owned()),
                names: vec![Name {
                    given: Some("John".to_owned()),
                    surname: Some("Smith".to_owned()),
                    ..Name::default()
                }],
                gender: Some(Gender::Male),
                event_refs: vec![EventRef::bare("_e1")],
                citation_refs: vec!["_c1".to_owned()],
                note_refs: vec!["_n1".to_owned()],
                media_refs: vec![
                    MediaRef {
                        hlink: "_o1".to_owned(),
                        region: Some(Region {
                            left: 10,
                            top: 20,
                            width: 30,
                            height: 40,
                        }),
                    },
                    MediaRef::bare("_o2"),
                ],
                person_refs: vec![PersonRef {
                    hlink: "_p2".to_owned(),
                    rel: Some(AssociationKind::Godparent),
                }],
                tag_refs: vec!["_t1".to_owned()],
                private: true,
            },
            Person {
                handle: "_p2".to_owned(),
                gramps_id: Some("I0002".to_owned()),
                names: vec![Name {
                    given: Some("Jane".to_owned()),
                    surname: Some("Doe".to_owned()),
                    ..Name::default()
                }],
                gender: Some(Gender::Female),
                ..Person::default()
            },
        ]
    }

    fn sample() -> Database {
        Database {
            header: Header {
                date: Some(DatePoint {
                    year: Some(2019),
                    month: Some(5),
                    day: Some(4),
                }),
            },
            people: sample_people(),
            families: vec![Family {
                handle: "_f1".to_owned(),
                gramps_id: Some("F0001".to_owned()),
                father: Some("_p1".to_owned()),
                mother: Some("_p2".to_owned()),
                child_refs: vec![ChildRef {
                    hlink: "_p3".to_owned(),
                    mother_relationship: Some("Birth".to_owned()),
                    father_relationship: Some("Adopted".to_owned()),
                }],
                event_refs: vec![EventRef::bare("_e2")],
                tag_refs: vec!["_t1".to_owned()],
                private: true,
            }],
            events: vec![
                Event {
                    handle: "_e1".to_owned(),
                    gramps_id: Some("E0001".to_owned()),
                    kind: EventKind::Birth,
                    date: Some(exact(1850)),
                    place_ref: Some("_pl1".to_owned()),
                    description: None,
                },
                Event {
                    handle: "_e2".to_owned(),
                    gramps_id: Some("E0002".to_owned()),
                    kind: EventKind::Marriage,
                    date: Some(exact(1870)),
                    place_ref: None,
                    description: None,
                },
            ],
            places: vec![Place {
                handle: "_pl1".to_owned(),
                gramps_id: Some("P0001".to_owned()),
                name: Some("Bergen".to_owned()),
                place_type: None,
                enclosed_by: Vec::new(),
                longitude: Some("5.322054".to_owned()),
                latitude: Some("60.391262".to_owned()),
            }],
            sources: vec![Source {
                handle: "_s1".to_owned(),
                gramps_id: Some("S0001".to_owned()),
                title: Some("Census 1801".to_owned()),
                author: Some("Statistics".to_owned()),
                pub_info: None,
                abbrev: Some("1801 Census".to_owned()),
                repository_refs: vec![RepoRef {
                    hlink: "_r1".to_owned(),
                    call_number: Some("6Mi5202".to_owned()),
                    medium: Some(SourceMediaKind::Film),
                }],
            }],
            citations: vec![Citation {
                handle: "_c1".to_owned(),
                gramps_id: Some("C0001".to_owned()),
                source_ref: Some("_s1".to_owned()),
                page: Some("p. 5".to_owned()),
                confidence: Some(2),
            }],
            repositories: vec![Repository {
                handle: "_r1".to_owned(),
                gramps_id: Some("R0001".to_owned()),
                name: Some("National Archive".to_owned()),
            }],
            objects: vec![MediaObject {
                handle: "_o1".to_owned(),
                gramps_id: Some("O0001".to_owned()),
                file: Some("photo.jpg".to_owned()),
                mime: Some("image/jpeg".to_owned()),
            }],
            notes: vec![Note {
                handle: "_n1".to_owned(),
                gramps_id: Some("N0001".to_owned()),
                text: Some("A research note.".to_owned()),
            }],
            tags: vec![Tag {
                handle: "_t1".to_owned(),
                name: Some("Important".to_owned()),
            }],
        }
    }

    #[test]
    fn round_trips_a_rich_database_through_emit_and_parse() {
        let db = sample();
        let bytes = emit(&db);
        let parsed = parse(&bytes).expect("parse");
        assert_eq!(parsed, db);
    }

    #[test]
    fn parses_a_hand_written_coord_element() {
        // A Gramps document's own `<coord>` (plain signed decimal degrees — ADR 0024 §4).
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<places>
<placeobj handle="_pl1" id="P0001">
<pname value="Vagsbygd"/>
<coord long="7.9585" lat="58.1281"/>
</placeobj>
</places>
</database>
"#;
        let db = parse(xml).expect("parse");
        let place = &db.places[0];
        assert_eq!(place.longitude.as_deref(), Some("7.9585"));
        assert_eq!(place.latitude.as_deref(), Some("58.1281"));

        let reparsed = parse(&emit(&db)).expect("reparse");
        assert_eq!(reparsed, db, "<coord> round-trips (ADR 0024 §4)");
    }

    #[test]
    fn a_place_with_no_coord_emits_no_coord_element() {
        let db = Database {
            places: vec![Place {
                handle: "_pl1".to_owned(),
                name: Some("Vagsbygd".to_owned()),
                ..Place::default()
            }],
            ..Database::default()
        };
        let bytes = emit(&db);
        assert!(
            !String::from_utf8_lossy(&bytes).contains("coord"),
            "no <coord> element when no coordinates are recorded"
        );
    }

    #[test]
    fn objref_region_round_trips_the_crop() {
        let parsed = parse(&emit(&sample())).expect("parse");
        assert_eq!(
            parsed.people[0].media_refs,
            vec![
                MediaRef {
                    hlink: "_o1".to_owned(),
                    region: Some(Region {
                        left: 10,
                        top: 20,
                        width: 30,
                        height: 40,
                    }),
                },
                MediaRef::bare("_o2"),
            ]
        );
    }

    #[test]
    fn parses_a_hand_written_region_into_a_crop() {
        // A Gramps document's own `<region>` (corners in percent) lands as a top-left origin + extent.
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<objref hlink="_o1"><region corner1_x="25" corner1_y="30" corner2_x="75" corner2_y="90"/></objref>
</person>
</people>
</database>
"#;
        let parsed = parse(xml).expect("parse");
        assert_eq!(
            parsed.people[0].media_refs[0].region,
            Some(Region {
                left: 25,
                top: 30,
                width: 50,
                height: 60,
            })
        );
    }

    #[test]
    fn a_bare_objref_has_no_region() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001"><objref hlink="_o1"/></person>
</people>
</database>
"#;
        let parsed = parse(xml).expect("parse");
        assert_eq!(parsed.people[0].media_refs, vec![MediaRef::bare("_o1")]);
    }

    #[test]
    fn parses_gender_and_names() {
        let parsed = parse(&emit(&sample())).expect("parse");
        let john = &parsed.people[0];
        assert_eq!(john.gender, Some(Gender::Male));
        assert_eq!(john.names.first().and_then(|n| n.given.as_deref()), Some("John"));
        assert_eq!(john.names.first().and_then(|n| n.surname.as_deref()), Some("Smith"));
    }

    #[test]
    fn a_second_name_is_kept_as_an_alternate_and_round_trips() {
        let text = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<name><first>Jane</first><surname>Smith</surname></name>
<name alt="1"><first>Jane</first><surname>Doe</surname></name>
</person>
</people>
</database>
"#;
        let db = parse(text).expect("parse");
        let names = &db.people[0].names;
        assert_eq!(names.len(), 2, "both <name> elements are kept, not just the last");
        assert_eq!(names[0].surname.as_deref(), Some("Smith"));
        assert_eq!(names[1].surname.as_deref(), Some("Doe"));

        let reparsed = parse(&emit(&db)).expect("reparse");
        assert_eq!(reparsed, db, "both <name> elements round-trip");
    }

    #[test]
    fn resolves_hlinks_and_keeps_top_level_records() {
        let parsed = parse(&emit(&sample())).expect("parse");
        assert_eq!(parsed.people[0].event_refs, vec![EventRef::bare("_e1")]);
        assert_eq!(parsed.families[0].father.as_deref(), Some("_p1"));
        assert_eq!(parsed.events.len(), 2);
        assert_eq!(parsed.events[0].place_ref.as_deref(), Some("_pl1"));
        assert_eq!(parsed.citations[0].source_ref.as_deref(), Some("_s1"));
    }

    #[test]
    fn parses_and_round_trips_a_tagref_on_person_and_family() {
        let text = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<people>
<person handle="_p1" id="I0001">
<tagref hlink="_t1"/>
</person>
</people>
<families>
<family handle="_f1" id="F0001">
<tagref hlink="_t1"/>
</family>
</families>
<tags>
<tag handle="_t1" name="Direct line"/>
</tags>
</database>
"#;
        let db = parse(text).expect("parse");
        assert_eq!(db.people[0].tag_refs, vec!["_t1".to_owned()]);
        assert_eq!(db.families[0].tag_refs, vec!["_t1".to_owned()]);

        let reparsed = parse(&emit(&db)).expect("reparse");
        assert_eq!(reparsed, db, "person/family <tagref> round-trips");
    }

    #[test]
    fn parses_and_round_trips_a_source_abbreviation() {
        let text = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<sources>
<source handle="_s1" id="S0001">
<stitle>Census 1801</stitle>
<sabbrev>1801 Census</sabbrev>
</source>
</sources>
</database>
"#;
        let db = parse(text).expect("parse");
        assert_eq!(db.sources[0].abbrev.as_deref(), Some("1801 Census"));

        let reparsed = parse(&emit(&db)).expect("reparse");
        assert_eq!(reparsed, db, "<sabbrev> round-trips");
    }

    #[test]
    fn parses_and_round_trips_a_reporef_call_number_and_medium() {
        let text = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<sources>
<source handle="_s1" id="S0001">
<stitle>Death certificate</stitle>
<reporef hlink="_r1" callno="6Mi5202" medium="Film"/>
</source>
</sources>
</database>
"#;
        let db = parse(text).expect("parse");
        let reporef = &db.sources[0].repository_refs[0];
        assert_eq!(reporef.call_number.as_deref(), Some("6Mi5202"));
        assert_eq!(reporef.medium, Some(SourceMediaKind::Film));

        let reparsed = parse(&emit(&db)).expect("reparse");
        assert_eq!(reparsed, db, "<reporef callno medium> round-trips");
    }

    #[test]
    fn an_unrecognized_medium_is_kept_verbatim() {
        let text = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<sources>
<source handle="_s1" id="S0001">
<reporef hlink="_r1" medium="Bound ledger"/>
</source>
</sources>
</database>
"#;
        let db = parse(text).expect("parse");
        assert_eq!(
            db.sources[0].repository_refs[0].medium,
            Some(SourceMediaKind::Other("Bound ledger".to_owned()))
        );

        let reparsed = parse(&emit(&db)).expect("reparse");
        assert_eq!(reparsed, db, "an unrecognized medium round-trips verbatim");
    }

    #[test]
    fn inflates_gzipped_input() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let xml = emit(&sample());
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&xml).expect("gz write");
        let gzipped = encoder.finish().expect("gz finish");
        assert_eq!(&gzipped[..2], &[0x1f, 0x8b], "gzip magic");
        let parsed = parse(&gzipped).expect("parse gzipped");
        assert_eq!(parsed, sample());
    }

    #[test]
    fn an_empty_document_parses_to_an_empty_database() {
        let parsed = parse(&emit(&Database::default())).expect("parse");
        assert_eq!(parsed, Database::default());
    }

    #[test]
    fn parses_the_header_export_date() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<header>
<created date="2019-05-04"/>
</header>
</database>
"#;
        let db = parse(xml).expect("parse");
        assert_eq!(
            db.header.date,
            Some(DatePoint {
                year: Some(2019),
                month: Some(5),
                day: Some(4),
            })
        );
    }

    #[test]
    fn a_header_with_no_created_date_has_no_export_date() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<header>
</header>
</database>
"#;
        let db = parse(xml).expect("parse");
        assert_eq!(db.header.date, None);
    }

    #[test]
    fn an_unparseable_header_date_degrades_to_none_without_breaking_the_rest_of_the_parse() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
<header>
<created date="not-a-date"/>
</header>
<people>
<person handle="_p1" id="I0001"/>
</people>
</database>
"#;
        let db = parse(xml).expect("parse");
        assert_eq!(
            db.header.date, None,
            "an unparseable created date is None (ADR 0029 §3)"
        );
        assert_eq!(db.people.len(), 1, "the rest of the document still parses");
    }

    #[test]
    fn header_export_date_round_trips_through_emit_and_parse() {
        let db = Database {
            header: Header {
                date: Some(DatePoint {
                    year: Some(2019),
                    month: Some(5),
                    day: Some(4),
                }),
            },
            ..Database::default()
        };
        let reparsed = parse(&emit(&db)).expect("reparse");
        assert_eq!(reparsed, db, "the header export date round-trips (ADR 0029 §2)");
    }

    #[test]
    fn a_missing_header_date_emits_no_header_element() {
        let bytes = emit(&Database::default());
        assert!(
            !String::from_utf8_lossy(&bytes).contains("header"),
            "no <header> element when no export date is recorded"
        );
    }

    #[test]
    fn round_trips_an_eventref_with_role_attributes_and_note() {
        let db = Database {
            people: vec![Person {
                handle: "_p1".to_owned(),
                gramps_id: Some("I0001".to_owned()),
                event_refs: vec![EventRef {
                    hlink: "_e1".to_owned(),
                    role: Some("Witness".to_owned()),
                    attributes: vec![
                        EventRefAttribute {
                            attribute_type: "Age".to_owned(),
                            value: "45y".to_owned(),
                        },
                        EventRefAttribute {
                            attribute_type: "Occupation".to_owned(),
                            value: "Clerk".to_owned(),
                        },
                    ],
                    note_refs: vec!["_n1".to_owned()],
                    citation_refs: vec!["_c1".to_owned()],
                }],
                ..Person::default()
            }],
            ..Database::default()
        };
        let parsed = parse(&emit(&db)).expect("parse");
        assert_eq!(parsed.people[0].event_refs, db.people[0].event_refs);
    }

    #[test]
    fn a_bare_eventref_stays_self_closing() {
        let db = Database {
            people: vec![Person {
                handle: "_p1".to_owned(),
                event_refs: vec![EventRef::bare("_e1")],
                ..Person::default()
            }],
            ..Database::default()
        };
        let xml = String::from_utf8(emit(&db)).expect("utf8");
        assert!(
            xml.contains("<eventref hlink=\"_e1\"/>"),
            "bare eventref is self-closing: {xml}"
        );
    }
}
