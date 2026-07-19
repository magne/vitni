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
    ChildRef, Citation, Database, Event, EventRef, EventRefAttribute, Family, Gender, MediaObject, MediaRef, Note,
    Person, PersonRef, Place, Region, Repository, Source, Tag,
};
pub use parse::{GrampsError, parse};

#[cfg(test)]
mod tests {
    use super::{
        ChildRef, Citation, Database, Event, EventRef, EventRefAttribute, Family, Gender, MediaObject, MediaRef, Note,
        Person, PersonRef, Place, Region, Repository, Source, Tag, emit, parse,
    };
    use genealogy_interchange::{
        AssociationKind, Calendar, Date, DateModifier, DatePoint, DateQuality, EventKind, Name,
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
                name: Some(Name {
                    given: Some("John".to_owned()),
                    surname: Some("Smith".to_owned()),
                    ..Name::default()
                }),
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
                private: true,
            },
            Person {
                handle: "_p2".to_owned(),
                gramps_id: Some("I0002".to_owned()),
                name: Some(Name {
                    given: Some("Jane".to_owned()),
                    surname: Some("Doe".to_owned()),
                    ..Name::default()
                }),
                gender: Some(Gender::Female),
                ..Person::default()
            },
        ]
    }

    fn sample() -> Database {
        Database {
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
            }],
            sources: vec![Source {
                handle: "_s1".to_owned(),
                gramps_id: Some("S0001".to_owned()),
                title: Some("Census 1801".to_owned()),
                author: Some("Statistics".to_owned()),
                pub_info: None,
                repository_refs: vec!["_r1".to_owned()],
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
        assert_eq!(john.name.as_ref().and_then(|n| n.given.as_deref()), Some("John"));
        assert_eq!(john.name.as_ref().and_then(|n| n.surname.as_deref()), Some("Smith"));
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
