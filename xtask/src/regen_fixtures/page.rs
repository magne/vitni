//! One fetched page turned into a bundled fixture: pruned, substituted and written out.

use std::collections::{BTreeSet, HashSet};
use std::fmt;

use anyhow::{Result, anyhow};
use ego_tree::{NodeId, NodeRef};
use scraper::node::Element;
use scraper::{Html, Node, Selector};
use vitni_digitalarkivet::html::{NESTED_ELEMENTS, PAGE_ELEMENTS};
use vitni_digitalarkivet::normalize_ws;

use crate::regen_fixtures::manifest::Page;

/// Elements dropped wherever they are, even inside a kept subtree: none holds anything the parser reads.
const DROPPED: [&str; 4] = ["script", "style", "noscript", "template"];

/// Elements with no end tag.
const VOID: [&str; 6] = ["meta", "input", "img", "link", "br", "hr"];

/// Attributes whose values the parser reads as data, so each is substituted like a text node.
const VALUE_ATTRIBUTES: [&str; 4] = ["href", "src", "content", "value"];

/// The page's pruned values that have neither a substitution nor a place in the vocabulary.
#[derive(Debug)]
pub struct Unmapped {
    pub page: String,
    pub values: BTreeSet<String>,
}

impl fmt::Display for Unmapped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}: {} value(s) with no substitution — add each to the page's `[page.substitute]` (real → \
             invented), or to `vocabulary` if it names no one:",
            self.page,
            self.values.len()
        )?;
        for value in &self.values {
            writeln!(f, "  {value:?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Unmapped {}

/// The class names, ids and `property` values the parser's selectors name. A structural attribute
/// keeps only these, so the selectors still match and nothing else of the page survives in it.
#[derive(Default)]
struct Structural {
    classes: BTreeSet<String>,
    ids: BTreeSet<String>,
    properties: BTreeSet<String>,
}

impl Structural {
    fn from_selectors<'a>(selectors: impl IntoIterator<Item = &'a &'a str>) -> Self {
        let mut structural = Self::default();
        for selector in selectors {
            let mut rest = *selector;
            while let Some(at) = rest.find(['.', '#', '"']) {
                let (marker, after) = (&rest[at..=at], &rest[at + 1..]);
                if marker == "\"" {
                    let Some(end) = after.find('"') else {
                        break;
                    };
                    if rest[..at].ends_with("property=") {
                        structural.properties.insert(after[..end].to_owned());
                    }
                    rest = &after[end + 1..];
                    continue;
                }
                let end = after
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                    .unwrap_or(after.len());
                let set = if marker == "." {
                    &mut structural.classes
                } else {
                    &mut structural.ids
                };
                set.insert(after[..end].to_owned());
                rest = &after[end..];
            }
        }
        structural
    }
}

/// Prunes and substitutes fetched pages, with the parser's selectors compiled once.
pub struct Pruner {
    selectors: Vec<Selector>,
    structural: Structural,
}

impl Pruner {
    /// Compiles [`PAGE_ELEMENTS`] and collects the structural names every parser selector uses.
    ///
    /// # Errors
    /// Returns an error when one of the crate's selectors does not compile.
    pub fn new() -> Result<Self> {
        let mut selectors = Vec::new();
        for css in PAGE_ELEMENTS {
            selectors.push(Selector::parse(css).map_err(|error| anyhow!("selector {css:?}: {error}"))?);
        }
        Ok(Self {
            selectors,
            structural: Structural::from_selectors(PAGE_ELEMENTS.iter().chain(NESTED_ELEMENTS)),
        })
    }

    /// `html` pruned to the elements the parser reads, with every remaining text and value
    /// replaced through `page.substitute` or kept because it is in `vocabulary`.
    ///
    /// # Errors
    /// Returns every value that is in neither, so one run lists all the mapping a page still needs.
    pub fn regenerate(&self, page: &Page, html: &str, vocabulary: &BTreeSet<String>) -> Result<String, Unmapped> {
        let doc = Html::parse_document(html);
        let mut keep = HashSet::new();
        for selector in &self.selectors {
            for matched in doc.select(selector) {
                keep.extend(matched.descendants().map(|node| node.id()));
                keep.extend(matched.ancestors().map(|node| node.id()));
            }
        }
        let mut writer = Writer {
            page,
            vocabulary,
            structural: &self.structural,
            keep: &keep,
            out: header(&page.id),
            unmapped: BTreeSet::new(),
        };
        writer.element(*doc.root_element(), Some(0));
        writer.out.push('\n');
        if writer.unmapped.is_empty() {
            Ok(writer.out)
        } else {
            Err(Unmapped {
                page: page.id.clone(),
                values: writer.unmapped,
            })
        }
    }
}

/// The doctype and the comment that says where the file came from and how to change it.
fn header(id: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<!--\n  Generated by `cargo xtask regen-fixtures` from the external page `{id}`\n  \
         (ADR 0042 §4): pruned to the elements vitni-digitalarkivet's parser reads, with every value\n  \
         replaced by an invented one. Do not edit: change the page's substitutions in\n  \
         tests/external/manifest.toml and regenerate.\n-->\n"
    )
}

/// Serializes the kept nodes, substituting as it goes.
struct Writer<'a> {
    page: &'a Page,
    vocabulary: &'a BTreeSet<String>,
    structural: &'a Structural,
    keep: &'a HashSet<NodeId>,
    out: String,
    unmapped: BTreeSet<String>,
}

impl Writer<'_> {
    /// Writes `node` indented at `depth`, or inline when `depth` is `None`. An element whose kept
    /// children include text is written inline, so its text is never padded with indentation.
    fn element(&mut self, node: NodeRef<'_, Node>, depth: Option<usize>) {
        let Node::Element(element) = node.value() else {
            return;
        };
        let name = element.name();
        if DROPPED.contains(&name) {
            return;
        }
        self.out.push('<');
        self.out.push_str(name);
        self.attributes(element);
        self.out.push('>');
        if VOID.contains(&name) {
            return;
        }
        let children: Vec<NodeRef<'_, Node>> = node
            .children()
            .filter(|child| self.keep.contains(&child.id()))
            .collect();
        let has_text = children.iter().any(|child| {
            if let Node::Text(text) = child.value() {
                !text.trim().is_empty()
            } else {
                false
            }
        });
        match depth {
            Some(depth) if !has_text => self.block_children(&children, depth),
            Some(_) | None => self.inline_children(&children),
        }
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push('>');
    }

    /// Each element child on its own line, one level deeper; blank text between them is dropped.
    fn block_children(&mut self, children: &[NodeRef<'_, Node>], depth: usize) {
        let mut wrote = false;
        for child in children {
            if let Node::Element(element) = child.value()
                && !DROPPED.contains(&element.name())
            {
                self.newline(depth + 1);
                self.element(*child, Some(depth + 1));
                wrote = true;
            }
        }
        if wrote {
            self.newline(depth);
        }
    }

    /// Children written as they run, each text's edge whitespace collapsed to one space and dropped at
    /// the element's own edges.
    fn inline_children(&mut self, children: &[NodeRef<'_, Node>]) {
        let last = children.len().saturating_sub(1);
        for (index, child) in children.iter().enumerate() {
            match child.value() {
                Node::Text(text) => self.text(text, index == 0, index == last),
                Node::Element(_) => self.element(*child, None),
                Node::Document
                | Node::Fragment
                | Node::Doctype(_)
                | Node::Comment(_)
                | Node::ProcessingInstruction(_) => {}
            }
        }
    }

    fn text(&mut self, text: &str, first: bool, last: bool) {
        let value = normalize_ws(text);
        if value.is_empty() {
            if !first && !last && !text.is_empty() {
                self.out.push(' ');
            }
            return;
        }
        if !first && text.starts_with(char::is_whitespace) {
            self.out.push(' ');
        }
        let substituted = self.substitute(value);
        self.out.push_str(&escape_text(&substituted));
        if !last && text.ends_with(char::is_whitespace) {
            self.out.push(' ');
        }
    }

    /// The kept attributes in name order: values substituted, structural ones filtered.
    fn attributes(&mut self, element: &Element) {
        let mut attributes: Vec<(&str, &str)> = element.attrs().collect();
        attributes.sort_unstable();
        for (name, value) in attributes {
            let kept = if VALUE_ATTRIBUTES.contains(&name) {
                let value = normalize_ws(value);
                if value.is_empty() {
                    Some(value)
                } else {
                    Some(self.substitute(value))
                }
            } else if name == "class" {
                let classes: Vec<&str> = value
                    .split_whitespace()
                    .filter(|class| self.structural.classes.contains(*class))
                    .collect();
                (!classes.is_empty()).then(|| classes.join(" "))
            } else if name == "id" {
                self.structural.ids.contains(value).then(|| value.to_owned())
            } else if name == "property" {
                self.structural.properties.contains(value).then(|| value.to_owned())
            } else {
                None
            };
            if let Some(kept) = kept {
                self.out.push(' ');
                self.out.push_str(name);
                self.out.push_str("=\"");
                self.out.push_str(&escape_attribute(&kept));
                self.out.push('"');
            }
        }
    }

    /// The invented value for `value`, or `value` itself when the vocabulary lists it. Anything else
    /// is recorded as unmapped and written as is, since the run fails before any file is written.
    fn substitute(&mut self, value: String) -> String {
        if let Some(invented) = self.page.substitute.get(&value) {
            return invented.clone();
        }
        if !self.vocabulary.contains(&value) {
            self.unmapped.insert(value.clone());
        }
        value
    }

    fn newline(&mut self, depth: usize) {
        self.out.push('\n');
        for _ in 0..depth {
            self.out.push_str("  ");
        }
    }
}

/// `value` escaped for a text node.
pub fn escape_text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `value` escaped for a double-quoted attribute.
pub fn escape_attribute(value: &str) -> String {
    escape_text(value).replace('"', "&quot;")
}
