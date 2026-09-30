//! The source, repository and citation assessments' table cases (ADR 0038 §2, §4).

use proptest::prelude::{Strategy, prop, prop_assert, prop_assert_eq, proptest};
use uuid::Uuid;

use crate::address::Address;
use crate::ids::{RepositoryId, SourceId};
use crate::matching::profile::{CitationProfile, RepositoryProfile, SourceProfile};
use crate::matching::tests::{DATA, feature, household, point};
use crate::matching::{
    CultureId, Feature, MatchBand, MatchSettings, Outcome, assess_citations, assess_repositories, assess_sources,
};

fn settings() -> MatchSettings {
    MatchSettings::default()
}

fn archive(name: &str, locality: &str) -> RepositoryProfile {
    RepositoryProfile {
        id: Some(RepositoryId::from_uuid(Uuid::now_v7())),
        name: Some(name.to_owned()),
        addresses: vec![Address {
            locality: Some(locality.to_owned()),
            country: Some("Norge".to_owned()),
            ..Address::default()
        }],
        origins: Vec::new(),
    }
}

fn church_book(title: &str) -> SourceProfile {
    SourceProfile {
        id: None,
        title: Some(title.to_owned()),
        author: Some("Ringsaker prestegjeld".to_owned()),
        publication: None,
        repositories: vec![archive("Statsarkivet på Hamar", "Hamar")],
        origins: Vec::new(),
    }
}

fn cite(source: SourceProfile, page: &str) -> CitationProfile {
    CitationProfile {
        source: Some(source),
        page: Some(page.to_owned()),
        date: Some(point(1877, Some(10), Some(14))),
        origins: Vec::new(),
    }
}

#[test]
fn one_church_book_titled_two_ways_is_probable() {
    let a = church_book("Ministerialbok for Ringsaker 1870-1880");
    let b = SourceProfile {
        repositories: a.repositories.clone(),
        ..church_book("Ringsaker ministerialbok 1870-1880")
    };
    let assessment = assess_sources(&a, &b, &DATA, &settings());
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    assert!(feature(&assessment, Feature::Title).weight > 0.0);
    assert_eq!(feature(&assessment, Feature::Author).outcome, Outcome::Agree);
    assert_eq!(feature(&assessment, Feature::Repository).outcome, Outcome::Agree);
    let cultures: Vec<&str> = assessment.cultures.iter().map(CultureId::as_str).collect();
    assert_eq!(cultures, ["universal", "da", "no"]);
}

#[test]
fn another_parish_book_disagrees_on_the_title() {
    let a = church_book("Ministerialbok for Ringsaker 1870-1880");
    let b = church_book("Ministerialbok for Voss 1820-1830");
    let assessment = assess_sources(&a, &b, &DATA, &settings());
    assert_eq!(
        feature(&assessment, Feature::Title).outcome,
        Outcome::Disagree,
        "{assessment:#?}"
    );
    assert!(assessment.band < MatchBand::Possible, "{assessment:#?}");
}

#[test]
fn a_copy_held_elsewhere_or_published_otherwise_is_no_evidence_against() {
    let a = church_book("Ministerialbok for Ringsaker 1870-1880");
    let b = SourceProfile {
        repositories: vec![archive("Digitalarkivet", "Oslo")],
        publication: Some("Skannet 2012".to_owned()),
        ..a.clone()
    };
    let a = SourceProfile {
        publication: Some("Original".to_owned()),
        ..a
    };
    let assessment = assess_sources(&a, &b, &DATA, &settings());
    for wanted in [Feature::Repository, Feature::Publication] {
        let term = feature(&assessment, wanted);
        assert!(term.weight.abs() < f64::EPSILON, "{term:?}");
    }
}

#[test]
fn one_archive_named_two_ways_is_probable_and_another_town_disagrees() {
    let a = archive("Statsarkivet på Hamar", "Hamar");
    let b = RepositoryProfile {
        id: None,
        ..archive("Statsarkivet i Hamar", "Hamar")
    };
    let assessment = assess_repositories(&a, &b, &DATA, &settings());
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    assert_eq!(feature(&assessment, Feature::Address).outcome, Outcome::Agree);
    let elsewhere = assess_repositories(&a, &archive("Statsarkivet i Bergen", "Bergen"), &DATA, &settings());
    assert_eq!(feature(&elsewhere, Feature::Address).outcome, Outcome::Disagree);
    assert!(elsewhere.band < MatchBand::Probable, "{elsewhere:#?}");
}

#[test]
fn one_citation_of_one_page_is_probable_and_keeps_the_source_assessment() {
    let book = church_book("Ministerialbok for Ringsaker 1870-1880");
    let a = cite(book.clone(), "s. 45, nr. 12");
    let b = cite(book, "side 45 nr 12");
    let assessment = assess_citations(&a, &b, &DATA, &settings());
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    assert_eq!(feature(&assessment, Feature::Page).outcome, Outcome::Agree);
    assert_eq!(feature(&assessment, Feature::Source).outcome, Outcome::Agree);
    assert_eq!(feature(&assessment, Feature::Date).outcome, Outcome::Agree);
    assert_eq!(assessment.parts.len(), 1);
    assert_eq!(
        assessment.parts[0].band,
        MatchBand::Probable,
        "the source pair's own assessment explains the Source term"
    );
}

#[test]
fn another_page_of_one_book_is_another_citation() {
    let book = church_book("Ministerialbok for Ringsaker 1870-1880");
    let assessment = assess_citations(&cite(book.clone(), "s. 45"), &cite(book, "s. 46"), &DATA, &settings());
    assert_eq!(feature(&assessment, Feature::Page).outcome, Outcome::Disagree);
    assert!(assessment.band < MatchBand::Probable, "{assessment:#?}");
}

#[test]
fn one_page_of_another_book_is_another_citation() {
    let a = cite(church_book("Ministerialbok for Ringsaker 1870-1880"), "s. 45");
    let b = cite(church_book("Folketelling 1865 for Voss"), "s. 45");
    let assessment = assess_citations(&a, &b, &DATA, &settings());
    let source = feature(&assessment, Feature::Source);
    assert_eq!(source.outcome, Outcome::Disagree, "{assessment:#?}");
    assert!(source.weight < 0.0);
    assert!(assessment.band < MatchBand::Probable, "{assessment:#?}");
}

#[test]
fn a_page_is_read_by_its_digits_as_written() {
    let book = church_book("Ministerialbok for Ringsaker 1870-1880");
    let page = |x: &str, y: &str| {
        let assessment = assess_citations(&cite(book.clone(), x), &cite(book.clone(), y), &DATA, &settings());
        feature(&assessment, Feature::Page).outcome
    };
    assert_eq!(page("s. 11", "s. 1"), Outcome::Disagree);
    assert_eq!(page("nr. 100", "nr. 10"), Outcome::Disagree);
    assert_eq!(page("s.45", "s. 45"), Outcome::Agree);
    assert_eq!(page("s. 045", "side 45"), Outcome::Agree);
}

#[test]
fn two_citations_of_one_source_record_share_its_source_whatever_it_holds() {
    let untitled = SourceProfile {
        id: Some(SourceId::from_uuid(Uuid::now_v7())),
        ..SourceProfile::default()
    };
    let assessment = assess_citations(
        &cite(untitled.clone(), "s. 45"),
        &cite(untitled, "s. 45"),
        &DATA,
        &settings(),
    );
    assert_eq!(feature(&assessment, Feature::Source).outcome, Outcome::Agree);
}

#[test]
fn a_page_without_numbers_compares_by_its_words() {
    let book = church_book("Ministerialbok for Ringsaker 1870-1880");
    let a = cite(book.clone(), "Døde, innledning");
    let b = cite(book, "døde innledning");
    let page = feature(&assess_citations(&a, &b, &DATA, &settings()), Feature::Page).clone();
    assert_eq!(page.outcome, Outcome::Agree, "{page:?}");
}

#[test]
fn a_shared_origin_is_deterministic_and_two_items_of_one_record_conflict() {
    let book = church_book("Ministerialbok for Ringsaker 1870-1880");
    let (mut a, mut b) = (cite(book.clone(), "s. 45"), cite(book, "s. 45"));
    a.origins.push(household(Some("citation:1")));
    b.origins.push(household(Some("citation:2")));
    let apart = assess_citations(&a, &b, &DATA, &settings());
    assert_eq!(feature(&apart, Feature::Record).outcome, Outcome::Conflict);
    b.origins = vec![household(Some("citation:1"))];
    assert_eq!(
        assess_citations(&a, &b, &DATA, &settings()).band,
        MatchBand::Deterministic
    );
}

fn a_citation() -> impl Strategy<Value = CitationProfile> {
    let titles = prop::sample::select(vec![
        "Ministerialbok for Ringsaker 1870-1880",
        "Ringsaker ministerialbok 1870-1880",
        "Folketelling 1865 for Voss",
    ]);
    let pages = prop::sample::select(vec!["s. 45", "side 45", "s. 46", "innledning"]);
    (prop::option::of(titles), prop::option::of(pages), prop::bool::ANY).prop_map(|(title, page, dated)| {
        CitationProfile {
            source: title.map(church_book),
            page: page.map(ToOwned::to_owned),
            date: dated.then(|| point(1877, None, None)),
            origins: Vec::new(),
        }
    })
}

proptest! {
    #[test]
    fn a_citation_assessment_is_a_symmetric_probability(a in a_citation(), b in a_citation()) {
        let (ab, ba) = (
            assess_citations(&a, &b, &DATA, &settings()),
            assess_citations(&b, &a, &DATA, &settings()),
        );
        prop_assert!((0.0..=1.0).contains(&ab.score), "{}", ab.score);
        prop_assert!((ab.score - ba.score).abs() < 1e-9, "{} vs {}", ab.score, ba.score);
        prop_assert_eq!(ab.band, ba.band);
    }

    #[test]
    fn a_citation_matches_itself_at_least_as_well_as_any_other(a in a_citation(), b in a_citation()) {
        let (itself, other) = (
            assess_citations(&a, &a, &DATA, &settings()),
            assess_citations(&a, &b, &DATA, &settings()),
        );
        prop_assert!(itself.score >= other.score, "self {} < other {}", itself.score, other.score);
    }
}

#[test]
fn a_page_and_entry_swapped_is_another_citation() {
    let book = church_book("Ministerialbok for Ringsaker 1870-1880");
    let a = cite(book.clone(), "s. 12, nr. 45");
    let b = cite(book, "s. 45, nr. 12");
    let page = feature(&assess_citations(&a, &b, &DATA, &settings()), Feature::Page).clone();
    assert_eq!(page.outcome, Outcome::Disagree, "{page:?}");
}
