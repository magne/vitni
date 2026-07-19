//! HTML parsers for census and church-book pages and the scan viewer.
//!
//! Selectors mirror the owner's prototype (`sort-inbox.py`), verified against live
//! markup: the focal person is `div.data-item.current`, key/value rows pair a label
//! column with an `.ssp-semibold` value column, the scan button is
//! `a#scannedImageLink`, and the legacy viewer exposes `input#permanent_image_link`.

use scraper::{ElementRef, Html, Selector};

use crate::classify::{classify_url, record_id, resolve, resolve_and_dedup};
use crate::error::{PageContext, ParseError};
use crate::model::{ExternalId, Field, PageKind, PersonRecord, ResidenceRecord, SourceMetadata};
use crate::text::{REPOSITORY, normalize_ws};

/// Compile a static selector, surfacing a compile failure as a typed error rather
/// than an `unwrap`/`expect` (which `-D warnings` would reject).
fn sel(css: &'static str) -> Result<Selector, ParseError> {
    Selector::parse(css).map_err(|_| ParseError::Selector(css))
}

/// Whitespace-normalized concatenated text of an element.
fn text_of(el: ElementRef<'_>) -> String {
    normalize_ws(&el.text().collect::<String>())
}

/// The value of `attr` on the first element matching `css`.
fn attr_of(doc: &Html, css: &'static str, attr: &str) -> Result<Option<String>, ParseError> {
    let selector = sel(css)?;
    Ok(doc
        .select(&selector)
        .next()
        .and_then(|el| el.value().attr(attr))
        .map(str::to_owned))
}

/// The first preceding element sibling of `el`.
fn prev_element(el: ElementRef<'_>) -> Option<ElementRef<'_>> {
    el.prev_siblings().find_map(ElementRef::wrap)
}

/// The record page URL: `og:url` when present, else the fetched URL.
fn record_url(doc: &Html, url: &str) -> Result<String, ParseError> {
    Ok(attr_of(doc, r#"meta[property="og:url"]"#, "content")?.unwrap_or_else(|| url.to_owned()))
}

/// Every transcribed key/value row inside the focal element, in document order.
fn extract_fields(focal: ElementRef<'_>) -> Result<Vec<Field>, ParseError> {
    let value_sel = sel("div.ssp-semibold")?;
    let mut fields = Vec::new();
    for value_el in focal.select(&value_sel) {
        let Some(label_el) = prev_element(value_el) else {
            continue;
        };
        let key = text_of(label_el).trim_end_matches(':').trim().to_owned();
        if key.is_empty() {
            continue;
        }
        fields.push(Field {
            key,
            value: text_of(value_el),
        });
    }
    Ok(fields)
}

/// The value of the first field whose key case-insensitively matches a candidate,
/// treating the archive's `-` placeholder and empty strings as absent.
fn field_value(fields: &[Field], candidates: &[&str]) -> Option<String> {
    for field in fields {
        for candidate in candidates {
            if field.key.eq_ignore_ascii_case(candidate) {
                let value = field.value.trim();
                if value.is_empty() || value == "-" {
                    return None;
                }
                return Some(value.to_owned());
            }
        }
    }
    None
}

/// The focal person's name: the `Navn` field, else the `h4` anchor text with its
/// de-emphasized ordinal (e.g. `001`, `Løpenr`) removed.
fn focal_name(focal: ElementRef<'_>, fields: &[Field]) -> Result<String, ParseError> {
    if let Some(navn) = field_value(fields, &["Navn"]) {
        return Ok(navn);
    }
    let anchor_sel = sel("h4 a")?;
    let de_sel = sel("span.de-emphasized")?;
    let Some(anchor) = focal.select(&anchor_sel).next() else {
        return Ok(String::new());
    };
    let full = text_of(anchor);
    let de = anchor.select(&de_sel).next().map(text_of).unwrap_or_default();
    Ok(full.strip_prefix(&de).unwrap_or(&full).trim().to_owned())
}

/// Resolve the scan-viewer URL from a person page, prototype selector chain.
fn scan_viewer_url(doc: &Html, base: &str) -> Result<Option<String>, ParseError> {
    if let Some(href) = attr_of(doc, "a#scannedImageLink", "href")? {
        return Ok(resolve(base, &href));
    }
    let anchor_sel = sel("a[href]")?;
    let anchors: Vec<ElementRef<'_>> = doc.select(&anchor_sel).collect();
    for anchor in &anchors {
        if anchor.value().attr("data-scans").is_some()
            && let Some(href) = anchor.value().attr("href")
        {
            return Ok(resolve(base, href));
        }
    }
    for anchor in &anchors {
        let text = text_of(*anchor).to_ascii_lowercase();
        if (text.contains("skannet") || text.contains("scanned"))
            && let Some(href) = anchor.value().attr("href")
        {
            return Ok(resolve(base, href));
        }
    }
    for anchor in &anchors {
        if let Some(href) = anchor.value().attr("href")
            && href.contains("media.digitalarkivet.no")
        {
            return Ok(resolve(base, href));
        }
    }
    Ok(None)
}

/// Href of every `.data-item` participant/household anchor, absolute and deduped.
fn household_links(doc: &Html, base: &str) -> Result<Vec<String>, ParseError> {
    let anchor_sel = sel("div.data-item h4 a[href]")?;
    let hrefs: Vec<&str> = doc.select(&anchor_sel).filter_map(|a| a.value().attr("href")).collect();
    Ok(resolve_and_dedup(base, hrefs, ""))
}

/// The four-digit year (1500–2099) appearing first in `title`.
fn year_in(title: &str) -> Option<String> {
    let bytes = title.as_bytes();
    for window in bytes.windows(4) {
        if window.iter().all(u8::is_ascii_digit) {
            let year: i32 = std::str::from_utf8(window).ok()?.parse().ok()?;
            if (1500..=2099).contains(&year) {
                return std::str::from_utf8(window).ok().map(str::to_owned);
            }
        }
    }
    None
}

/// The source title: the ` - `-delimited segment before the trailing
/// `Digitalarkivet` site name.
fn source_title(title: &str) -> Option<String> {
    let parts: Vec<&str> = title.split(" - ").map(str::trim).collect();
    if parts.len() >= 2 && parts.last() == Some(&"Digitalarkivet") {
        return parts.get(parts.len() - 2).map(|s| normalize_ws(s));
    }
    None
}

/// Source/citation metadata from the page title and `.parent-post` headings.
fn source_metadata(doc: &Html) -> Result<SourceMetadata, ParseError> {
    let title_sel = sel("title")?;
    let title = doc.select(&title_sel).next().map(text_of).unwrap_or_default();
    let heading_title = source_title(&title);
    let year = heading_title.as_deref().and_then(year_in);

    let heading_sel = sel("div.parent-post h4")?;
    let mut headings = Vec::new();
    for heading in doc.select(&heading_sel) {
        let text = text_of(heading);
        if let Some((label, value)) = text.split_once(':') {
            let key = label.trim().to_owned();
            let value = normalize_ws(value);
            if !key.is_empty() && !value.is_empty() {
                headings.push(Field { key, value });
            }
        }
    }
    Ok(SourceMetadata {
        title: heading_title,
        year,
        repository: REPOSITORY,
        headings,
    })
}

/// Parse a census or church-book person/record page.
///
/// # Errors
/// Returns [`ParseError::MissingElement`] when the focal person block
/// (`div.data-item.current`) is absent, or [`ParseError::Selector`] on an internal
/// selector fault.
pub fn parse_person_page(html: &str, url: &str) -> Result<PersonRecord, ParseError> {
    let doc = Html::parse_document(html);
    let page_kind = classify_url(url);
    let page = match page_kind {
        PageKind::ChurchbookRecord => PageContext::ChurchbookRecord,
        PageKind::CensusPerson | PageKind::CensusResidence | PageKind::Unknown => PageContext::CensusPerson,
    };

    let record_url = record_url(&doc, url)?;
    let rid = record_id(&record_url).or_else(|| record_id(url)).unwrap_or_default();

    let focal_sel = sel("div.data-item.current")?;
    let focal = doc.select(&focal_sel).next().ok_or(ParseError::MissingElement {
        page,
        what: "focal person (.data-item.current)",
    })?;

    let fields = extract_fields(focal)?;
    let name = focal_name(focal, &fields)?;

    Ok(PersonRecord {
        page_kind,
        external_id: ExternalId::digitalarkivet(rid),
        name,
        birth: field_value(&fields, &["Alder/født", "Fødselsdato", "Fødselsår"]),
        birthplace: field_value(&fields, &["Fødested"]),
        residence: field_value(&fields, &["Bosted"]),
        role: field_value(&fields, &["Familiestilling", "Rolle"]),
        marital_status: field_value(&fields, &["Sivilstand"]),
        occupation: field_value(&fields, &["Yrke", "Stilling/stand"]),
        scan_viewer_url: scan_viewer_url(&doc, &record_url)?,
        household: household_links(&doc, &record_url)?,
        source: source_metadata(&doc)?,
        fields,
        record_url,
    })
}

/// Parse a census residence/household page into its person links and source
/// metadata.
///
/// # Errors
/// Returns [`ParseError::Selector`] on an internal selector fault. A page with no
/// person links yields an empty `person_links` (the honest empty result).
pub fn parse_residence_page(html: &str, url: &str) -> Result<ResidenceRecord, ParseError> {
    let doc = Html::parse_document(html);
    let record_url = record_url(&doc, url)?;
    let rid = record_id(&record_url).or_else(|| record_id(url)).unwrap_or_default();

    let anchor_sel = sel("a[href]")?;
    let hrefs: Vec<&str> = doc.select(&anchor_sel).filter_map(|a| a.value().attr("href")).collect();
    let person_links = resolve_and_dedup(&record_url, hrefs, "/census/person/");

    Ok(ResidenceRecord {
        external_id: ExternalId::digitalarkivet(rid),
        person_links,
        source: source_metadata(&doc)?,
        record_url,
    })
}

/// Extract the permanent scan image URL from a viewer page.
///
/// Resolution chain: `input#permanent_image_link` value → `og:image` (when it is
/// an image) → a `urn.digitalarkivet.no` `.jpg` anchor.
///
/// # Errors
/// Returns [`ParseError::ImageUrlNotFound`] when none resolves — notably the new
/// `nye.digitalarkivet.no` IIIF viewer used for church-book scans, which serves
/// tiles through a manifest rather than a permanent `.jpg`.
pub fn parse_viewer_page(html: &str, url: &str) -> Result<String, ParseError> {
    let doc = Html::parse_document(html);
    if let Some(value) = attr_of(&doc, "input#permanent_image_link", "value")?
        && !value.trim().is_empty()
    {
        return Ok(resolve(url, &value).unwrap_or(value));
    }
    if let Some(image) = attr_of(&doc, r#"meta[property="og:image"]"#, "content")?
        && is_image_url(&image)
    {
        return Ok(resolve(url, &image).unwrap_or(image));
    }
    let anchor_sel = sel("a[href]")?;
    for anchor in doc.select(&anchor_sel) {
        if let Some(href) = anchor.value().attr("href")
            && href.contains("urn.digitalarkivet.no")
            && is_image_url(href)
        {
            return Ok(resolve(url, href).unwrap_or_else(|| href.to_owned()));
        }
    }
    Err(ParseError::ImageUrlNotFound {
        page: PageContext::Viewer,
    })
}

/// True when `url`'s path ends in a recognized raster-image extension.
fn is_image_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    ["jpg", "jpeg", "png", "tif", "tiff"].contains(&ext.as_str())
}
