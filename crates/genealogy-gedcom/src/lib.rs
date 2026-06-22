//! `genealogy-gedcom` — pure GEDCOM parse/emit over a small intermediate model.
//!
//! This crate holds the format logic of the GEDCOM import/export plugins, free of any WASM or host
//! types, so it is unit-tested and linted through the normal `cargo --workspace` path. The wasm glue
//! crates (`plugins/gedcom-*`) depend on it and only bridge its [`Tree`] to the host capabilities.
//!
//! The supported subset is deliberately minimal (roadmap Spike C): `INDI` + `NAME` → individual;
//! `FAM` + `HUSB`/`WIFE`/`CHIL` → family. Broader GEDCOM 7 / Gramps XML mapping is Phase 4 (ADR 0013).

mod emit;
mod model;
mod parse;

pub use emit::emit;
pub use model::{Citation, Date, Event, EventKind, Family, Individual, MediaObject, Sex, Source, Tree};
pub use parse::{GedcomError, parse};

#[cfg(test)]
mod tests {
    use super::{Citation, Date, Event, EventKind, Family, Individual, MediaObject, Sex, Source, Tree, emit, parse};

    fn sample() -> Tree {
        Tree {
            individuals: vec![
                Individual {
                    xref: "I0001".to_owned(),
                    uid: Some("D02D344F-F781-4337-BCF1-0A1A1A548280".to_owned()),
                    given: Some("John".to_owned()),
                    surname: Some("Smith".to_owned()),
                    sex: Some(Sex::Male),
                    events: vec![Event {
                        kind: EventKind::Birth,
                        date: Some(Date {
                            year: 1970,
                            month: Some(4),
                            day: Some(5),
                        }),
                        place: Some("Mandal".to_owned()),
                    }],
                    citations: vec![Citation {
                        source_xref: "S0001".to_owned(),
                        page: Some("p. 5".to_owned()),
                    }],
                    media: vec![MediaObject {
                        file: Some("https://example.test/photo.jpg".to_owned()),
                        title: Some("Portrait".to_owned()),
                    }],
                    notes: vec!["A research note.".to_owned()],
                },
                Individual {
                    xref: "I0002".to_owned(),
                    uid: None,
                    given: Some("Jane".to_owned()),
                    surname: Some("Doe".to_owned()),
                    sex: Some(Sex::Female),
                    events: Vec::new(),
                    citations: Vec::new(),
                    media: Vec::new(),
                    notes: Vec::new(),
                },
                Individual {
                    xref: "I0003".to_owned(),
                    uid: Some("A673BB63-328E-4F79-B4E3-ABCF43460749".to_owned()),
                    given: Some("Sam".to_owned()),
                    surname: Some("Smith".to_owned()),
                    sex: None,
                    events: Vec::new(),
                    citations: Vec::new(),
                    media: Vec::new(),
                    notes: Vec::new(),
                },
            ],
            families: vec![Family {
                xref: "F0001".to_owned(),
                uid: Some("11111111-2222-3333-4444-555555555555".to_owned()),
                partners: vec!["I0001".to_owned(), "I0002".to_owned()],
                children: vec!["I0003".to_owned()],
                events: vec![Event {
                    kind: EventKind::Marriage,
                    date: Some(Date {
                        year: 1995,
                        month: None,
                        day: None,
                    }),
                    place: None,
                }],
            }],
            sources: vec![Source {
                xref: "S0001".to_owned(),
                title: Some("Census 1801".to_owned()),
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
        assert_eq!(tree.individuals[0].given.as_deref(), Some("John"));
        assert_eq!(tree.individuals[0].surname.as_deref(), Some("Smith"));
        assert_eq!(tree.families.len(), 1);
        assert_eq!(tree.families[0].partners, vec!["I1".to_owned(), "I2".to_owned()]);
        assert_eq!(tree.families[0].children, vec!["I3".to_owned()]);
    }

    #[test]
    fn round_trips_through_emit_and_parse() {
        let tree = sample();
        let reparsed = parse(&emit(&tree)).expect("reparse");
        assert_eq!(reparsed, tree, "emit then parse must reproduce the tree");
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
        assert_eq!(tree.individuals[0].given.as_deref(), Some("Madonna"));
        assert_eq!(tree.individuals[0].surname, None);
    }

    #[test]
    fn an_empty_document_parses_to_an_empty_tree() {
        assert_eq!(parse("").expect("parse"), Tree::default());
        assert_eq!(parse("0 HEAD\n0 TRLR\n").expect("parse"), Tree::default());
    }

    #[test]
    fn skips_unknown_tags_but_keeps_the_record() {
        let tree = parse("0 @I1@ INDI\n1 SEX M\n1 NOTE ignored\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals.len(), 1);
        assert_eq!(tree.individuals[0].given, None);
    }

    #[test]
    fn tolerates_crlf_line_endings() {
        let tree = parse("0 @I1@ INDI\r\n1 NAME Ada /Lovelace/\r\n0 TRLR\r\n").expect("parse");
        assert_eq!(tree.individuals[0].surname.as_deref(), Some("Lovelace"));
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
1 DEAT
2 DATE ABT 2020
0 @F1@ FAM
1 MARR
2 DATE 1995
0 TRLR
";
        let tree = parse(text).expect("parse");
        let events = &tree.individuals[0].events;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::Birth);
        assert_eq!(
            events[0].date,
            Some(Date {
                year: 1970,
                month: Some(4),
                day: Some(5)
            })
        );
        assert_eq!(events[0].place.as_deref(), Some("Mandal"));
        assert_eq!(events[1].kind, EventKind::Death);
        assert_eq!(events[1].date.map(|d| d.year), Some(2020));
        assert_eq!(tree.families[0].events[0].kind, EventKind::Marriage);
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
        let tree = parse("0 @I1@ INDI\n1 SEX F\n0 @I2@ INDI\n1 SEX M\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals[0].sex, Some(Sex::Female));
        assert_eq!(tree.individuals[1].sex, Some(Sex::Male));
    }

    #[test]
    fn strips_a_leading_utf8_bom() {
        let tree = parse("\u{feff}0 @I1@ INDI\n1 NAME Ada /Lovelace/\n0 TRLR\n").expect("parse");
        assert_eq!(tree.individuals.len(), 1);
        assert_eq!(tree.individuals[0].surname.as_deref(), Some("Lovelace"));
    }
}
