//! A tiny generic XML element tree and the quick-xml reader/writer plumbing.
//!
//! Gramps XML is small and regular, so [`parse`](crate::parse) interprets a generic [`Element`] tree
//! rather than driving the event reader directly — the same two-layer shape `vitni-gedcom` uses
//! (a generic node tree, then a typed model). `read_tree` sniffs the gzip magic and inflates a
//! `.gramps` file before parsing; plain XML is parsed as-is.

use std::io::Read;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::parse::GrampsError;

/// A generic XML element: tag name, attributes, trimmed text content, and child elements.
#[derive(Debug, Clone, Default)]
pub struct Element {
    /// The local tag name (namespace prefix stripped).
    pub name: String,
    /// The attributes, in document order.
    pub attrs: Vec<(String, String)>,
    /// The concatenated, trimmed text content.
    pub text: String,
    /// The child elements, in document order.
    pub children: Vec<Element>,
}

impl Element {
    /// The value of attribute `key`, if present.
    #[must_use]
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// The first child with the given tag name, if any.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Every child with the given tag name, in document order.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.children.iter().filter(move |c| c.name == name)
    }
}

/// Inflates `bytes` if gzipped (magic `1f 8b`), then parses the XML into its root [`Element`].
///
/// # Errors
/// [`GrampsError::Gzip`] if a gzipped stream cannot be inflated, or [`GrampsError::Xml`] if the XML
/// is malformed.
pub fn read_tree(bytes: &[u8]) -> Result<Element, GrampsError> {
    let inflated;
    let xml_bytes = if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut decoder = flate2::read::GzDecoder::new(bytes);
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .map_err(|error| GrampsError::Gzip(error.to_string()))?;
        inflated = out;
        inflated.as_slice()
    } else {
        bytes
    };
    let text = String::from_utf8_lossy(xml_bytes);
    build_tree(&text)
}

/// Builds the element tree from XML `text`.
fn build_tree(text: &str) -> Result<Element, GrampsError> {
    // Text is kept whole and trimmed once its element ends: quick-xml reports an entity reference
    // (`&amp;`) as its own event between two text events, so trimming each event would eat the
    // spaces around it.
    let mut reader = Reader::from_str(text);
    // A synthetic root so we never pop an empty stack; the real document root is its first child.
    let mut stack: Vec<Element> = vec![Element::default()];
    loop {
        match reader
            .read_event()
            .map_err(|error| GrampsError::Xml(error.to_string()))?
        {
            Event::Start(start) => {
                stack.push(element_from_start(&start)?);
            }
            Event::Empty(start) => {
                let element = element_from_start(&start)?;
                push_child(&mut stack, element)?;
            }
            Event::End(_) => {
                let mut finished = stack
                    .pop()
                    .ok_or_else(|| GrampsError::Xml("unbalanced end tag".to_owned()))?;
                trim_in_place(&mut finished.text);
                push_child(&mut stack, finished)?;
            }
            Event::Text(bytes_text) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&bytes_text.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                let resolved = resolve_reference(&reference)?;
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&resolved);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut root = stack
        .pop()
        .ok_or_else(|| GrampsError::Xml("no root element".to_owned()))?;
    Ok(root.children.pop().unwrap_or_default())
}

/// Strips `text`'s leading and trailing whitespace without reallocating.
fn trim_in_place(text: &mut String) {
    text.truncate(text.trim_end().len());
    let leading = text.len() - text.trim_start().len();
    text.drain(..leading);
}

/// The text an entity or character reference (`&amp;`, `&#233;`) stands for.
fn resolve_reference(reference: &quick_xml::events::BytesRef<'_>) -> Result<String, GrampsError> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|error| GrampsError::Xml(error.to_string()))?
    {
        return Ok(character.to_string());
    }
    let name = reference.xml10_content();
    quick_xml::escape::resolve_predefined_entity(&name)
        .map(str::to_owned)
        .ok_or_else(|| GrampsError::Xml(format!("unknown entity &{name};")))
}

/// Builds an [`Element`] from a start/empty tag, decoding its name and attributes.
fn element_from_start(start: &quick_xml::events::BytesStart<'_>) -> Result<Element, GrampsError> {
    let name = local_name(start.name().as_ref());
    let mut attrs = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| GrampsError::Xml(error.to_string()))?;
        let key = local_name(attribute.key.as_ref());
        let value = quick_xml::escape::unescape(&attribute.value)
            .map_err(|error| GrampsError::Xml(error.to_string()))?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(Element {
        name,
        attrs,
        text: String::new(),
        children: Vec::new(),
    })
}

/// Pushes `element` as a child of the element on top of the stack.
fn push_child(stack: &mut [Element], element: Element) -> Result<(), GrampsError> {
    stack
        .last_mut()
        .ok_or_else(|| GrampsError::Xml("element outside any parent".to_owned()))?
        .children
        .push(element);
    Ok(())
}

/// Strips a namespace prefix (`gramps:person` -> `person`).
fn local_name(name: &str) -> String {
    match name.rsplit_once(':') {
        Some((_, local)) => local.to_owned(),
        None => name.to_owned(),
    }
}
