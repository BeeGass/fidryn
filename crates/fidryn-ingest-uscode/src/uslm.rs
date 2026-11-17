//! The publisher's XML: the checks of a document, the classes of its
//! elements, and the reading of text.
//!
//! A document is parsed by `roxmltree`, which refuses a document type
//! declaration and with it every entity declaration. It is refused when its
//! root is not `uscDoc` in the namespace of the publisher's schema, when its
//! schema location names another file than `USLM-1.0.15.xsd`, when its
//! `docPublicationName` is not `Online@` followed by the release, an en dash
//! in the name read as a hyphen, and when it has more than one property
//! `is-positive-law` or one that is neither `yes` nor `no`. A document
//! without that property has no flag.
//!
//! The text of a flow (a text part, a `p`, a note, quoted content, a level
//! of quoted content, a table cell) is read into blocks: text and inline
//! elements gather into the current paragraph, and a block element closes it
//! and is the next block. A paragraph is normalized as a whole; a link, a
//! superscript and a subscript keep the extent of their own words, with
//! white space at their two ends left outside them. A footnote is taken out
//! of the flow where it stands, without closing the paragraph; it is a
//! footnote of its module, numbered in document order. A footnote marker
//! gives no text: it is a `Mark` run that stands directly after the last
//! character before it that is not white space. Quoted content keeps its
//! levels, parts and blocks, and has no address. An element that stands
//! where its class does not allow it, or that is in no class, is refused.

use crate::IngestError;
use crate::operative;
use fidryn_provision::normalize::{WHITE_SPACE, normalize};
use fidryn_provision::workspace::NotImported;
use fidryn_provision::{
    Address, Block, Cell, Footnote, Num, PartRole, Quoted, QuotedItem, QuotedLevel, Row, RowGroup,
    Run, Table, Text,
};
use roxmltree::{Document, Node, NodeId, ParsingOptions};
use std::collections::HashMap;

/// The namespace of the publisher's schema.
pub(crate) const USLM: &str = "http://xml.house.gov/schemas/uslm/1.0";
/// The namespace of XHTML, in which the publisher writes tables.
pub(crate) const XHTML: &str = "http://www.w3.org/1999/xhtml";
/// The namespace of the schema location.
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// The file of the schema every document names.
pub(crate) const SCHEMA_FILE: &str = "USLM-1.0.15.xsd";

/// The 19 values of `status` the schema allows.
pub(crate) const STATUSES: [&str; 19] = [
    "proposed",
    "withdrawn",
    "cancelled",
    "pending",
    "operational",
    "suspended",
    "renumbered",
    "repealed",
    "expired",
    "terminated",
    "hadItsEffect",
    "omitted",
    "notAdopted",
    "transferred",
    "redesignated",
    "reserved",
    "vacant",
    "crossReference",
    "unknown",
];

/// Whether `node` is the element `name` of the publisher's namespace.
pub(crate) fn is(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().namespace() == Some(USLM) && node.tag_name().name() == name
}

/// Whether `node` is the XHTML element `name`.
pub(crate) fn is_xhtml(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element()
        && node.tag_name().namespace() == Some(XHTML)
        && node.tag_name().name() == name
}

/// The local name of `node` when it is an element of the publisher's
/// namespace.
pub(crate) fn uslm_name<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    (node.is_element() && node.tag_name().namespace() == Some(USLM)).then(|| node.tag_name().name())
}

/// The name of an element as a message writes it: its local name in the
/// publisher's namespace, `xhtml:` and its local name in XHTML, and
/// `{namespace}` and its local name in any other.
pub(crate) fn tag(node: Node<'_, '_>) -> String {
    let name = node.tag_name();
    match name.namespace() {
        Some(USLM) => name.name().to_owned(),
        Some(XHTML) => format!("xhtml:{}", name.name()),
        Some(other) => format!("{{{other}}}{}", name.name()),
        None => name.name().to_owned(),
    }
}

/// Whether `node` is a footnote: a `note` of type `footnote`.
pub(crate) fn is_footnote(node: Node<'_, '_>) -> bool {
    is(node, "note") && node.attribute("type") == Some("footnote")
}

/// Whether `node` is a text part: `chapeau`, `content`, `continuation` or
/// `proviso`.
pub(crate) fn text_part_role(node: Node<'_, '_>) -> Option<PartRole> {
    match uslm_name(node)? {
        "chapeau" => Some(PartRole::Chapeau),
        "content" => Some(PartRole::Content),
        "continuation" => Some(PartRole::Continuation),
        "proviso" => Some(PartRole::Proviso),
        _ => None,
    }
}

/// Whether `node` is a `p` of either namespace, which are read alike.
pub(crate) fn is_p(node: Node<'_, '_>) -> bool {
    is(node, "p") || is_xhtml(node, "p")
}

/// Whether `text` is white space only.
fn is_white_space(text: &str) -> bool {
    text.chars().all(|c| WHITE_SPACE.contains(&c))
}

/// Refuses `node` when it is text other than white space: it stands in an
/// element where only elements may stand.
pub(crate) fn only_elements(node: Node<'_, '_>) -> Result<(), IngestError> {
    match node.text() {
        Some(text) if node.is_text() && !is_white_space(text) => Err(IngestError::at(
            node.parent().unwrap_or(node),
            format!("text where only elements may stand: {:?}", normalize(text)),
        )),
        _ => Ok(()),
    }
}

/// The refusal of `node`, which stands in `parent` where its class does not
/// allow it, or is in no class.
pub(crate) fn misplaced(node: Node<'_, '_>, parent: Node<'_, '_>) -> IngestError {
    misplaced_in(node, &format!("<{}>", tag(parent)))
}

/// The refusal of `node`, which stands in `place` where its class does not
/// allow it, or is in no class.
pub(crate) fn misplaced_in(node: Node<'_, '_>, place: &str) -> IngestError {
    let what = if is_known(node) {
        format!("<{}>", tag(node))
    } else {
        format!("<{}> is in no class of the reading, and", tag(node))
    };
    IngestError::at(node, format!("{what} does not stand in {place}"))
}

/// Whether `node` is in a class of the reading.
fn is_known(node: Node<'_, '_>) -> bool {
    const PUBLISHER: &[&str] = &[
        "uscDoc",
        "meta",
        "main",
        "title",
        "appendix",
        "subtitle",
        "chapter",
        "subchapter",
        "part",
        "subpart",
        "division",
        "subdivision",
        "article",
        "courtRules",
        "compiledAct",
        "reorganizationPlans",
        "section",
        "courtRule",
        "reorganizationPlan",
        "level",
        "subsection",
        "paragraph",
        "subparagraph",
        "clause",
        "subclause",
        "item",
        "subitem",
        "subsubitem",
        "num",
        "heading",
        "chapeau",
        "content",
        "continuation",
        "proviso",
        "p",
        "quotedContent",
        "img",
        "signature",
        "subheading",
        "ref",
        "sup",
        "sub",
        "date",
        "i",
        "b",
        "inline",
        "name",
        "role",
        "br",
        "notes",
        "note",
        "sourceCredit",
        "toc",
    ];
    const TABLES: &[&str] = &[
        "p", "table", "sup", "sub", "span", "b", "i", "a", "thead", "tbody", "tfoot", "tr", "th",
        "td", "colgroup", "col",
    ];
    match node.tag_name().namespace() {
        Some(USLM) => PUBLISHER.contains(&node.tag_name().name()),
        Some(XHTML) => TABLES.contains(&node.tag_name().name()),
        _ => false,
    }
}

/// Parses `xml`, refusing a document type declaration.
pub(crate) fn parse(xml: &str) -> Result<Document<'_>, IngestError> {
    let options = ParsingOptions {
        allow_dtd: false,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(xml, options).map_err(|error| match error {
        roxmltree::Error::DtdDetected => IngestError::new(
            "the document has a document type declaration, which the reading refuses",
        ),
        other => IngestError::new(format!("the document is not well-formed XML: {other}")),
    })
}

/// What the document says of itself, and the element its modules are read
/// from.
pub(crate) struct Head<'a, 'i> {
    /// The `title` inside `main`, or the `appendix` under `uscDoc`.
    pub(crate) top: Node<'a, 'i>,
    /// The flag `is-positive-law` of the document, `None` when it has none.
    pub(crate) positive_law: Option<bool>,
}

/// Checks the root, the schema location and the metadata of `document`
/// against the release `release`, and finds its top container.
pub(crate) fn head<'a, 'i>(
    document: &'a Document<'i>,
    release: &str,
) -> Result<Head<'a, 'i>, IngestError> {
    let root = document.root_element();
    if !is(root, "uscDoc") {
        let namespace = root.tag_name().namespace().unwrap_or("no namespace");
        return Err(IngestError::at(
            root,
            format!(
                "the root element is {} in {namespace}, not uscDoc in the namespace {USLM}",
                root.tag_name().name()
            ),
        ));
    }
    schema_location(root)?;
    let mut meta = None;
    let mut top = None;
    for child in root.children() {
        only_elements(child)?;
        if !child.is_element() {
            continue;
        }
        if is(child, "meta") && meta.is_none() {
            meta = Some(child);
        } else if (is(child, "main") || is(child, "appendix")) && top.is_none() {
            top = Some(child);
        } else {
            return Err(misplaced(child, root));
        }
    }
    let Some(meta) = meta else {
        return Err(IngestError::at(root, "the document has no meta"));
    };
    let positive_law = metadata(meta, release)?;
    let top = match top {
        Some(main) if is(main, "main") => title_in(main)?,
        Some(appendix) => appendix,
        None => {
            return Err(IngestError::at(
                root,
                "the document has neither main nor an appendix under uscDoc",
            ));
        }
    };
    Ok(Head { top, positive_law })
}

/// The one `title` in `main`.
fn title_in<'a, 'i>(main: Node<'a, 'i>) -> Result<Node<'a, 'i>, IngestError> {
    let mut title = None;
    for child in main.children() {
        only_elements(child)?;
        if !child.is_element() {
            continue;
        }
        if !is(child, "title") {
            return Err(IngestError::at(
                child,
                format!("the top element in main is {}, not a title", tag(child)),
            ));
        }
        if title.is_some() {
            return Err(IngestError::at(child, "main holds a second title"));
        }
        title = Some(child);
    }
    title.ok_or_else(|| IngestError::at(main, "main holds no title"))
}

/// Refuses a schema location that does not name the publisher's schema file
/// for its namespace.
fn schema_location(root: Node<'_, '_>) -> Result<(), IngestError> {
    let Some(location) = root.attribute((XSI, "schemaLocation")) else {
        return Err(IngestError::at(root, "the document has no schema location"));
    };
    let words: Vec<&str> = location.split_ascii_whitespace().collect();
    let file = words
        .chunks(2)
        .find(|pair| pair[0] == USLM)
        .and_then(|pair| pair.get(1))
        .map(|place| place.rsplit('/').next().unwrap_or(place));
    match file {
        Some(SCHEMA_FILE) => Ok(()),
        Some(other) => Err(IngestError::at(
            root,
            format!("the schema location names {other}, not {SCHEMA_FILE}"),
        )),
        None => Err(IngestError::at(
            root,
            format!("the schema location {location:?} names no file for {USLM}"),
        )),
    }
}

/// The text of `node`: its text nodes and those of its descendants,
/// concatenated.
pub(crate) fn text_content(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|text| text.text())
        .collect()
}

/// Checks the release name of `meta`, each en dash of which is read as a
/// hyphen and no other character as another, and reads its positive-law
/// flag: `None` when it has no property `is-positive-law`.
fn metadata(meta: Node<'_, '_>, release: &str) -> Result<Option<bool>, IngestError> {
    let mut names = meta
        .children()
        .filter(|child| is(*child, "docPublicationName"));
    let (Some(name), None) = (names.next(), names.next()) else {
        return Err(IngestError::at(
            meta,
            "meta holds no docPublicationName, or more than one",
        ));
    };
    let published = text_content(name);
    let expected = format!("Online@{release}");
    if published.replace('\u{2013}', "-") != expected {
        return Err(IngestError::at(
            name,
            format!(
                "the document names its release {published:?}, and a document of release {release} \
                 says {expected}"
            ),
        ));
    }
    let mut flags = meta.children().filter(|child| {
        is(*child, "property") && child.attribute("role") == Some("is-positive-law")
    });
    let Some(flag) = flags.next() else {
        return Ok(None);
    };
    if flags.next().is_some() {
        return Err(IngestError::at(
            meta,
            "meta holds more than one property is-positive-law",
        ));
    }
    match text_content(flag).as_str() {
        "yes" => Ok(Some(true)),
        "no" => Ok(Some(false)),
        other => Err(IngestError::at(
            flag,
            format!("the property is-positive-law is {other:?}, neither yes nor no"),
        )),
    }
}

/// What the reading of one module finds and sets aside, and what it counts.
#[derive(Debug, Default)]
pub(crate) struct Scope {
    pub(crate) counts: NotImported,
    pub(crate) aside: SetAside,
    pub(crate) footnotes: Footnotes,
}

/// The elements of a module that are read after its provisions or its own
/// fields: its notes (the `notes` elements and the notes that stand directly
/// in its levels), in document order and each with the address of the level
/// that holds it, and its source credit.
#[derive(Debug, Default)]
pub(crate) struct SetAside {
    pub(crate) notes: Vec<(NodeId, Address)>,
    pub(crate) credit: Option<NodeId>,
}

/// The footnotes of a module: every footnote that stands in it, outside its
/// tables of contents and, for a container, outside its members, in
/// document order; the ordinal of each is its place from 1.
#[derive(Debug, Default)]
pub(crate) struct Footnotes {
    pub(crate) nodes: Vec<NodeId>,
    ordinals: HashMap<String, u32>,
}

impl Footnotes {
    /// The footnotes of the module whose element is `node`. `is_member` says
    /// whether a child of `node` is a module of its own.
    pub(crate) fn of_module(
        node: Node<'_, '_>,
        is_member: &dyn Fn(Node<'_, '_>) -> bool,
    ) -> Result<Footnotes, IngestError> {
        let mut footnotes = Footnotes::default();
        for child in node.children().filter(|child| !is_member(*child)) {
            footnotes.collect(child)?;
        }
        Ok(footnotes)
    }

    /// Adds the footnotes of `node` and of what stands in it, outside tables
    /// of contents.
    fn collect(&mut self, node: Node<'_, '_>) -> Result<(), IngestError> {
        if !node.is_element() || is(node, "toc") {
            return Ok(());
        }
        if is_footnote(node) {
            let ordinal = u32::try_from(self.nodes.len() + 1)
                .map_err(|_| IngestError::at(node, "a module holds too many footnotes"))?;
            if let Some(id) = node.attribute("id")
                && self.ordinals.insert(id.to_owned(), ordinal).is_some()
            {
                return Err(IngestError::at(
                    node,
                    format!(
                        "two footnotes of the module have the id {id:?}, so a marker cannot \
                         name one"
                    ),
                ));
            }
            self.nodes.push(node.id());
        }
        for child in node.children() {
            self.collect(child)?;
        }
        Ok(())
    }

    /// The ordinal of the footnote whose `id` is `id`.
    fn ordinal(&self, id: &str) -> Option<u32> {
        self.ordinals.get(id).copied()
    }
}

impl Scope {
    /// Sets aside the note or the `notes` element `node`, which stands
    /// directly in the level at `anchor`. A footnote is not set aside: it is
    /// among the module's footnotes already.
    pub(crate) fn set_aside_note(&mut self, node: Node<'_, '_>, anchor: &Address) {
        if !is_footnote(node) {
            self.aside.notes.push((node.id(), anchor.clone()));
        }
    }

    /// Sets aside the source credit `node`, the direct child of its unit or
    /// container; refuses a second one.
    pub(crate) fn set_aside_credit(&mut self, node: Node<'_, '_>) -> Result<(), IngestError> {
        if self.aside.credit.is_some() {
            return Err(IngestError::at(
                node,
                "a second source credit; a unit or a container has one",
            ));
        }
        self.aside.credit = Some(node.id());
        Ok(())
    }
}

/// Counts the table of contents `node`, with the footnotes and links inside
/// it; none of it is imported.
pub(crate) fn table_of_contents(node: Node<'_, '_>, counts: &mut NotImported) {
    for inner in node.descendants() {
        if is(inner, "toc") {
            counts.tables_of_contents += 1;
        } else if is_footnote(inner) {
            counts.footnotes_in_tables_of_contents += 1;
        } else if is(inner, "ref") && inner.has_attribute("href") {
            counts.links_in_tables_of_contents += 1;
        }
    }
}

/// The status of the level `node`: `None` in force, or one of the 19 values
/// of the schema.
pub(crate) fn status(node: Node<'_, '_>) -> Result<Option<String>, IngestError> {
    match node.attribute("status") {
        None => Ok(None),
        Some(status) if STATUSES.contains(&status) => Ok(Some(status.to_owned())),
        Some(status) => Err(IngestError::at(
            node,
            format!("the status {status:?} is not one of the 19 values of the schema"),
        )),
    }
}

/// The number and the heading of a level, read as its children are met.
#[derive(Debug, Default)]
pub(crate) struct Labels {
    pub(crate) num: Option<Num>,
    pub(crate) heading: Option<Text>,
    seen_num: bool,
    seen_heading: bool,
}

impl Labels {
    /// Reads `child` when it is the level's `num` or `heading`, and says
    /// whether it is one. A second of either is refused.
    pub(crate) fn read(
        &mut self,
        child: Node<'_, '_>,
        scope: &mut Scope,
    ) -> Result<bool, IngestError> {
        let seen = match uslm_name(child) {
            Some("num") => &mut self.seen_num,
            Some("heading") => &mut self.seen_heading,
            _ => return Ok(false),
        };
        if std::mem::replace(seen, true) {
            return Err(IngestError::at(
                child,
                format!("a second {}; a level has one", tag(child)),
            ));
        }
        if is(child, "num") {
            self.num = number(child, scope)?;
        } else {
            self.heading = heading(child, scope)?;
        }
        Ok(true)
    }
}

/// The number of a level: its `value` attribute and the text it shows.
/// Absent when it has neither a value nor text and holds no footnote marker.
fn number(node: Node<'_, '_>, scope: &mut Scope) -> Result<Option<Num>, IngestError> {
    let value = node.attribute("value").unwrap_or("");
    let shown = label(node, scope)?;
    if value.is_empty() && shown.runs.is_empty() {
        return Ok(None);
    }
    Ok(Some(Num {
        value: value.to_owned(),
        shown,
    }))
}

/// The text of a heading, a subheading or a source credit: absent when it
/// has no text and holds no footnote marker.
pub(crate) fn heading(node: Node<'_, '_>, scope: &mut Scope) -> Result<Option<Text>, IngestError> {
    let text = label(node, scope)?;
    Ok((!text.runs.is_empty()).then_some(text))
}

/// The text of the label `node`, which holds text and inline elements only.
/// A footnote marker is one of its runs, so a label that holds one is never
/// empty.
fn label(node: Node<'_, '_>, scope: &mut Scope) -> Result<Text, IngestError> {
    let mut pieces = Pieces::default();
    inline_children(node, &mut pieces, Within::Text, scope)?;
    shape(pieces, &mut scope.counts).map_err(|message| IngestError::at(node, message))
}

/// The text of a paragraph before it is normalized: its pieces in document
/// order.
#[derive(Debug, Default)]
struct Pieces {
    pieces: Vec<Piece>,
}

/// A piece of a paragraph, with its words as the source writes them.
#[derive(Debug)]
enum Piece {
    Words(String),
    Link {
        words: String,
        target: String,
    },
    Sup(String),
    Sub(String),
    /// A footnote marker, which gives no text: its own text, and the ordinal
    /// of the footnote it points at. Its place among the pieces is where it
    /// stands.
    Marker {
        shown: String,
        footnote: u32,
    },
}

impl Pieces {
    fn words(&mut self, text: &str) {
        match self.pieces.last_mut() {
            Some(Piece::Words(words)) => words.push_str(text),
            _ => self.pieces.push(Piece::Words(text.to_owned())),
        }
    }

    fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }
}

/// Where inline content is read: in a paragraph or a label, or inside a
/// link, a superscript or a subscript, which holds neither another of the
/// three nor a footnote marker.
#[derive(Clone, Copy)]
enum Within {
    Text,
    Styled(&'static str),
}

/// What an inline element is.
enum Inline<'a> {
    Link(&'a str),
    Marker,
    Sup,
    Sub,
    /// An element whose words are kept and whose markup is dropped.
    Plain,
    Break,
    Footnote,
}

/// The inline class of `node`, or `None` when it is not inline.
fn inline_class<'a>(node: Node<'a, '_>) -> Result<Option<Inline<'a>>, IngestError> {
    if let Some(name) = uslm_name(node) {
        return Ok(match name {
            "ref" => {
                if let Some(target) = node.attribute("href") {
                    Some(Inline::Link(target))
                } else if node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|c| c == "footnoteRef"))
                {
                    Some(Inline::Marker)
                } else {
                    return Err(IngestError::at(
                        node,
                        "a ref that has neither an href nor the class footnoteRef",
                    ));
                }
            }
            "sup" => Some(Inline::Sup),
            "sub" => Some(Inline::Sub),
            "date" | "i" | "b" | "inline" | "name" | "role" => Some(Inline::Plain),
            "br" => Some(Inline::Break),
            "note" if is_footnote(node) => Some(Inline::Footnote),
            _ => None,
        });
    }
    if node.tag_name().namespace() == Some(XHTML) {
        return Ok(match node.tag_name().name() {
            "sup" => Some(Inline::Sup),
            "sub" => Some(Inline::Sub),
            "span" | "b" | "i" | "a" => Some(Inline::Plain),
            _ => None,
        });
    }
    Ok(None)
}

/// Reads the children of `node`, text and inline elements only, into
/// `pieces`.
fn inline_children(
    node: Node<'_, '_>,
    pieces: &mut Pieces,
    within: Within,
    scope: &mut Scope,
) -> Result<(), IngestError> {
    for child in node.children() {
        if child.is_text() {
            pieces.words(child.text().unwrap_or(""));
        } else if child.is_element() && !inline(child, pieces, within, scope)? {
            return Err(misplaced(child, node));
        }
    }
    Ok(())
}

/// Reads `node` into `pieces` when it is an inline element, and says whether
/// it is one.
fn inline(
    node: Node<'_, '_>,
    pieces: &mut Pieces,
    within: Within,
    scope: &mut Scope,
) -> Result<bool, IngestError> {
    let Some(class) = inline_class(node)? else {
        return Ok(false);
    };
    match class {
        Inline::Link(target) => {
            let words = styled(node, "a link", within, scope)?;
            pieces.pieces.push(Piece::Link {
                words,
                target: target.to_owned(),
            });
        }
        Inline::Sup => {
            let words = styled(node, "a superscript", within, scope)?;
            pieces.pieces.push(Piece::Sup(words));
        }
        Inline::Sub => {
            let words = styled(node, "a subscript", within, scope)?;
            pieces.pieces.push(Piece::Sub(words));
        }
        Inline::Marker => {
            if let Within::Styled(outer) = within {
                return Err(IngestError::at(
                    node,
                    format!("a footnote marker inside {outer}"),
                ));
            }
            if node.children().any(|child| child.is_element()) {
                return Err(IngestError::at(
                    node,
                    "a footnote marker that holds an element",
                ));
            }
            let idref = node.attribute("idref").unwrap_or("");
            let Some(footnote) = scope.footnotes.ordinal(idref) else {
                return Err(IngestError::at(
                    node,
                    format!(
                        "a footnote marker whose idref {idref:?} names no footnote of its module"
                    ),
                ));
            };
            pieces.pieces.push(Piece::Marker {
                shown: normalize(&text_content(node)),
                footnote,
            });
        }
        Inline::Plain => inline_children(node, pieces, within, scope)?,
        Inline::Break => {
            if node.has_children() {
                return Err(IngestError::at(
                    node,
                    "a br that holds content; a br is one space",
                ));
            }
            pieces.words(" ");
        }
        // A footnote is taken out of the flow; it is among the module's
        // footnotes already.
        Inline::Footnote => {}
    }
    Ok(true)
}

/// The words of the link, superscript or subscript `node`, called `what`.
fn styled(
    node: Node<'_, '_>,
    what: &'static str,
    within: Within,
    scope: &mut Scope,
) -> Result<String, IngestError> {
    if let Within::Styled(outer) = within {
        return Err(IngestError::at(
            node,
            format!(
                "{what} inside {outer}; a link, a superscript or a subscript does not stand \
                 inside another"
            ),
        ));
    }
    let mut inner = Pieces::default();
    inline_children(node, &mut inner, Within::Styled(what), scope)?;
    Ok(inner
        .pieces
        .into_iter()
        .map(|piece| match piece {
            Piece::Words(words) => words,
            _ => unreachable!("only words are read inside a link, a superscript or a subscript"),
        })
        .collect())
}

/// The text of `pieces`, normalized as a whole. A link, a superscript or a
/// subscript keeps the extent of its own words with white space at its two
/// ends left outside it; a link with no words left is dropped and counted. A
/// footnote marker is a `Mark` run at the offset that the normalized text
/// before it has as its length: directly after the last character before it
/// that is not white space. Refused, with the reason, when a character
/// composes across the end of a piece, so that its text cannot be divided as
/// its pieces are.
fn shape(pieces: Pieces, counts: &mut NotImported) -> Result<Text, String> {
    enum Kind {
        Words,
        Link(String),
        Sup,
        Sub,
    }
    let starts_with_space = |text: &str| {
        text.chars()
            .next()
            .is_some_and(|c| WHITE_SPACE.contains(&c))
    };
    let ends_with_space = |text: &str| {
        text.chars()
            .last()
            .is_some_and(|c| WHITE_SPACE.contains(&c))
    };
    let undivided = |expected: &str| {
        format!(
            "the text {expected:?} cannot be divided into its runs once normalized: a character \
             composes across the end of a link, a superscript, a subscript or a footnote marker"
        )
    };
    let mut whole = String::new();
    let mut runs = Vec::new();
    // The length of the text of `runs`, in characters.
    let mut length = 0;
    let mut started = false;
    let mut space = false;
    let mut links_without_words = 0;
    for piece in pieces.pieces {
        let (raw, kind) = match piece {
            Piece::Marker { shown, footnote } => {
                // The pending space, if any, stands after the marker.
                let before = normalize(&whole);
                if before.chars().count() != length {
                    return Err(undivided(&before));
                }
                runs.push(Run::Mark { shown, footnote });
                continue;
            }
            Piece::Words(words) => (words, Kind::Words),
            Piece::Link { words, target } => (words, Kind::Link(target)),
            Piece::Sup(words) => (words, Kind::Sup),
            Piece::Sub(words) => (words, Kind::Sub),
        };
        whole.push_str(&raw);
        let core = normalize(&raw);
        if core.is_empty() {
            if matches!(kind, Kind::Link(_)) {
                links_without_words += 1;
            }
            space |= started && !raw.is_empty();
            continue;
        }
        if space || (started && starts_with_space(&raw)) {
            runs.push(Run::Words(" ".to_owned()));
            length += 1;
        }
        length += core.chars().count();
        runs.push(match kind {
            Kind::Words => Run::Words(core),
            Kind::Link(target) => Run::Ref {
                words: core,
                target,
            },
            Kind::Sup => Run::Sup(core),
            Kind::Sub => Run::Sub(core),
        });
        started = true;
        space = ends_with_space(&raw);
    }
    let text = Text::from_runs(runs);
    let expected = normalize(&whole);
    if text.plain() != expected {
        return Err(undivided(&expected));
    }
    counts.links_without_words += links_without_words;
    Ok(text)
}

/// What holds a flow, which decides the blocks it holds besides a `p`, a
/// table, quoted content and an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holder {
    /// Operative text, a footnote, or quoted content that no note holds.
    Text,
    /// A note, or quoted content inside one: a `signature`, any `subheading`
    /// and, at any depth, any `heading` other than the note's own (its first
    /// `heading` child, which a note may lack) are blocks of it too.
    Note,
}

/// The blocks of a flow as they are read.
struct Blocks<'a, 'i> {
    /// The element whose flow is read, for messages.
    node: Node<'a, 'i>,
    holder: Holder,
    blocks: Vec<Block>,
    pieces: Pieces,
}

impl<'a, 'i> Blocks<'a, 'i> {
    fn new(node: Node<'a, 'i>, holder: Holder) -> Self {
        Self {
            node,
            holder,
            blocks: Vec::new(),
            pieces: Pieces::default(),
        }
    }

    /// Closes the current paragraph; one whose text is empty is dropped.
    fn close(&mut self, scope: &mut Scope) -> Result<(), IngestError> {
        if self.pieces.is_empty() {
            return Ok(());
        }
        let pieces = std::mem::take(&mut self.pieces);
        let text = shape(pieces, &mut scope.counts)
            .map_err(|message| IngestError::at(self.node, message))?;
        if !text.plain().is_empty() {
            self.blocks.push(Block::Para(text));
        }
        Ok(())
    }

    /// Reads `child`, a child of the flow's element.
    fn child(&mut self, child: Node<'a, 'i>, scope: &mut Scope) -> Result<(), IngestError> {
        if child.is_text() {
            self.pieces.words(child.text().unwrap_or(""));
            return Ok(());
        }
        if !child.is_element() {
            return Ok(());
        }
        if is_p(child) {
            self.close(scope)?;
            let outer = std::mem::replace(&mut self.node, child);
            for inner in child.children() {
                self.child(inner, scope)?;
            }
            self.close(scope)?;
            self.node = outer;
        } else if is_xhtml(child, "table") {
            self.close(scope)?;
            self.blocks
                .push(Block::Table(table(child, self.holder, scope)?));
        } else if is(child, "img") {
            self.close(scope)?;
            self.blocks.push(image(child)?);
        } else if is(child, "quotedContent") {
            self.close(scope)?;
            self.blocks
                .push(Block::Quoted(quoted(child, self.holder, scope)?));
        } else if self.holder == Holder::Note && is(child, "signature") {
            self.close(scope)?;
            self.blocks.extend(signature(child, scope)?);
        } else if self.holder == Holder::Note && (is(child, "subheading") || is(child, "heading")) {
            self.close(scope)?;
            self.blocks
                .extend(heading(child, scope)?.map(Block::Subheading));
        } else if !inline(child, &mut self.pieces, Within::Text, scope)? {
            return Err(misplaced(child, self.node));
        }
        Ok(())
    }

    /// The blocks read so far; the current paragraph is closed first.
    fn take(&mut self, scope: &mut Scope) -> Result<Vec<Block>, IngestError> {
        self.close(scope)?;
        Ok(std::mem::take(&mut self.blocks))
    }
}

/// The blocks of the flow of `node`, held by `holder`.
pub(crate) fn flow(
    node: Node<'_, '_>,
    holder: Holder,
    scope: &mut Scope,
) -> Result<Vec<Block>, IngestError> {
    let mut blocks = Blocks::new(node, holder);
    for child in node.children() {
        blocks.child(child, scope)?;
    }
    blocks.take(scope)
}

/// The parts of the text part `node` of role `role`, held by `holder`, in
/// order. A `continuation` or a `proviso` that stands inside a `content`
/// closes the content part, is a part of its own, and what follows it opens a
/// further content part. A part with no block is dropped.
pub(crate) fn text_part(
    node: Node<'_, '_>,
    role: PartRole,
    holder: Holder,
    scope: &mut Scope,
) -> Result<Vec<(PartRole, Vec<Block>)>, IngestError> {
    let mut parts = Vec::new();
    let mut blocks = Blocks::new(node, holder);
    for child in node.children() {
        let inner = text_part_role(child).filter(|inner| {
            role == PartRole::Content && matches!(inner, PartRole::Continuation | PartRole::Proviso)
        });
        match inner {
            Some(inner) => {
                parts.push((role, blocks.take(scope)?));
                parts.push((inner, flow(child, holder, scope)?));
            }
            None => blocks.child(child, scope)?,
        }
    }
    parts.push((role, blocks.take(scope)?));
    parts.retain(|(_, blocks)| !blocks.is_empty());
    Ok(parts)
}

/// The heading and the blocks of the note `node`: its first `heading` child,
/// and the flow of everything else in it, where a text part that stands
/// directly in the note is replaced by the blocks of its own flow. The first
/// `heading` child of a cross heading is the label of its group, which is not
/// imported: `read_heading` false leaves it unread.
pub(crate) fn note(
    node: Node<'_, '_>,
    read_heading: bool,
    scope: &mut Scope,
) -> Result<(Option<Text>, Vec<Block>), IngestError> {
    let mut first = None;
    let mut seen = false;
    let mut blocks = Blocks::new(node, Holder::Note);
    for child in node.children() {
        if !seen && is(child, "heading") {
            seen = true;
            if read_heading {
                first = heading(child, scope)?;
            }
        } else if let Some(role) = text_part_role(child) {
            blocks.close(scope)?;
            for (_, inner) in text_part(child, role, Holder::Note, scope)? {
                blocks.blocks.extend(inner);
            }
        } else {
            blocks.child(child, scope)?;
        }
    }
    Ok((first, blocks.take(scope)?))
}

/// The footnote `node`, numbered `ordinal`: the text of its `num`, empty when
/// it has none, and the blocks of the flow of the rest.
pub(crate) fn footnote(
    node: Node<'_, '_>,
    ordinal: u32,
    scope: &mut Scope,
) -> Result<Footnote, IngestError> {
    let mut shown = None;
    let mut blocks = Blocks::new(node, Holder::Text);
    for child in node.children() {
        if shown.is_none() && is(child, "num") {
            shown = Some(label(child, scope)?.plain());
        } else {
            blocks.child(child, scope)?;
        }
    }
    Ok(Footnote {
        ordinal,
        shown: shown.unwrap_or_default(),
        blocks: blocks.take(scope)?,
    })
}

/// The paragraphs of the `signature` `node`: one for each `name` and each
/// `role` in it, in document order. A signature that holds anything else, a
/// footnote included, is refused.
fn signature(node: Node<'_, '_>, scope: &mut Scope) -> Result<Vec<Block>, IngestError> {
    let mut blocks = Vec::new();
    for child in node.children() {
        only_elements(child)?;
        if !child.is_element() {
            continue;
        }
        if is_footnote(child) {
            return Err(IngestError::at(
                child,
                "a footnote that stands directly in a signature; a signature holds only name and \
                 role",
            ));
        }
        if !is(child, "name") && !is(child, "role") {
            return Err(IngestError::at(
                child,
                format!(
                    "<{}> in a signature; a signature holds only name and role",
                    tag(child)
                ),
            ));
        }
        let text = label(child, scope)?;
        if !text.plain().is_empty() {
            blocks.push(Block::Para(text));
        }
    }
    Ok(blocks)
}

/// The quoted content `node`, held by `holder`: its origin as the source
/// writes it, and its items.
fn quoted(node: Node<'_, '_>, holder: Holder, scope: &mut Scope) -> Result<Quoted, IngestError> {
    let mut items = Items::new(node, holder);
    for child in node.children() {
        if is(child, "heading") || is(child, "subheading") {
            items.flush(scope)?;
            if let Some(text) = heading(child, scope)? {
                items.items.push(QuotedItem::Block(Block::Subheading(text)));
            }
        } else if !items.structure(child, scope)? {
            items.blocks.child(child, scope)?;
        }
    }
    Ok(Quoted {
        origin: node.attribute("origin").map(str::to_owned),
        items: items.finish(scope)?,
    })
}

/// The level of quoted content `node`, held by `holder`: its kind, number,
/// heading and items. It has no address.
fn quoted_level(
    node: Node<'_, '_>,
    holder: Holder,
    scope: &mut Scope,
) -> Result<QuotedLevel, IngestError> {
    status(node)?;
    let mut labels = Labels::default();
    let mut items = Items::new(node, holder);
    for child in node.children() {
        if !labels.read(child, scope)? && !items.structure(child, scope)? {
            items.blocks.child(child, scope)?;
        }
    }
    Ok(QuotedLevel {
        kind: node.tag_name().name().to_owned(),
        num: labels.num,
        heading: labels.heading,
        items: items.finish(scope)?,
    })
}

/// The items of quoted content or of one of its levels, as they are read:
/// the blocks of its flow, each an item, between its parts, levels and
/// notes.
struct Items<'a, 'i> {
    items: Vec<QuotedItem>,
    blocks: Blocks<'a, 'i>,
}

impl<'a, 'i> Items<'a, 'i> {
    fn new(node: Node<'a, 'i>, holder: Holder) -> Self {
        Self {
            items: Vec::new(),
            blocks: Blocks::new(node, holder),
        }
    }

    /// Adds the blocks read so far as items.
    fn flush(&mut self, scope: &mut Scope) -> Result<(), IngestError> {
        let blocks = self.blocks.take(scope)?;
        self.items.extend(blocks.into_iter().map(QuotedItem::Block));
        Ok(())
    }

    /// Reads `child` when it is a text part, a level or a note, and says
    /// whether it is one. A note inside quoted content is replaced by the
    /// blocks of its own flow, its heading first as a subheading.
    fn structure(&mut self, child: Node<'a, 'i>, scope: &mut Scope) -> Result<bool, IngestError> {
        let holder = self.blocks.holder;
        if let Some(role) = text_part_role(child) {
            self.flush(scope)?;
            for (role, blocks) in text_part(child, role, holder, scope)? {
                self.items.push(QuotedItem::Part { role, blocks });
            }
        } else if operative::is_provision(child) {
            self.flush(scope)?;
            self.items
                .push(QuotedItem::Level(quoted_level(child, holder, scope)?));
        } else if is(child, "note") && !is_footnote(child) {
            self.flush(scope)?;
            let (heading, blocks) = note(child, true, scope)?;
            self.items.extend(
                heading
                    .map(Block::Subheading)
                    .into_iter()
                    .chain(blocks)
                    .map(QuotedItem::Block),
            );
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn finish(mut self, scope: &mut Scope) -> Result<Vec<QuotedItem>, IngestError> {
        self.flush(scope)?;
        Ok(self.items)
    }
}

/// The table `node`, held by `holder`. A row belongs to the group of the
/// `thead`, `tbody` or `tfoot` it stands in, and to the body when it stands
/// in none; column groups and columns hold nothing and are dropped.
fn table(node: Node<'_, '_>, holder: Holder, scope: &mut Scope) -> Result<Table, IngestError> {
    let mut rows = Vec::new();
    for child in node.children() {
        only_elements(child)?;
        if !child.is_element() {
            continue;
        }
        let group = match child.tag_name().name() {
            _ if child.tag_name().namespace() != Some(XHTML) => None,
            "thead" => Some(RowGroup::Head),
            "tbody" => Some(RowGroup::Body),
            "tfoot" => Some(RowGroup::Foot),
            _ => None,
        };
        if let Some(group) = group {
            for row in child.children() {
                only_elements(row)?;
                if !row.is_element() {
                    continue;
                }
                if !is_xhtml(row, "tr") {
                    return Err(misplaced(row, child));
                }
                rows.push(table_row(row, group, holder, scope)?);
            }
        } else if is_xhtml(child, "tr") {
            rows.push(table_row(child, RowGroup::Body, holder, scope)?);
        } else if is_xhtml(child, "colgroup") {
            for column in child.children() {
                if column.is_text() && !is_white_space(column.text().unwrap_or("")) {
                    return Err(IngestError::at(child, "a column group that holds text"));
                }
                if !column.is_element() {
                    continue;
                }
                if !is_xhtml(column, "col") {
                    return Err(misplaced(column, child));
                }
                empty_column(column)?;
            }
        } else if is_xhtml(child, "col") {
            empty_column(child)?;
        } else {
            return Err(misplaced(child, node));
        }
    }
    Ok(Table { rows })
}

/// Refuses a column that holds content.
fn empty_column(node: Node<'_, '_>) -> Result<(), IngestError> {
    if node.has_children() {
        return Err(IngestError::at(
            node,
            "a column that holds content; a column holds no text",
        ));
    }
    Ok(())
}

/// The row `node` of the group `group`.
fn table_row(
    node: Node<'_, '_>,
    group: RowGroup,
    holder: Holder,
    scope: &mut Scope,
) -> Result<Row, IngestError> {
    let mut cells = Vec::new();
    for child in node.children() {
        only_elements(child)?;
        if !child.is_element() {
            continue;
        }
        let header = if is_xhtml(child, "th") {
            true
        } else if is_xhtml(child, "td") {
            false
        } else {
            return Err(misplaced(child, node));
        };
        cells.push(Cell {
            header,
            colspan: span(child, "colspan")?,
            rowspan: span(child, "rowspan")?,
            blocks: flow(child, holder, scope)?,
        });
    }
    Ok(Row { group, cells })
}

/// The span `name` of the cell `node`: 1 when it has none, and otherwise a
/// whole number written in ASCII digits.
fn span(node: Node<'_, '_>, name: &str) -> Result<u32, IngestError> {
    let Some(text) = node.attribute(name) else {
        return Ok(1);
    };
    let refused = || {
        IngestError::at(
            node,
            format!(
                "the {name} {text:?} of a table cell is not a whole number written in ASCII digits"
            ),
        )
    };
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(refused());
    }
    text.parse().map_err(|_| refused())
}

/// The image `node`: the file its `src` names, normalized, and empty when it
/// has none. An `img` with another attribute, with text or with a child is
/// refused.
fn image(node: Node<'_, '_>) -> Result<Block, IngestError> {
    if let Some(other) = node
        .attributes()
        .find(|attribute| attribute.name() != "src" || attribute.namespace().is_some())
    {
        return Err(IngestError::at(
            node,
            format!(
                "an img with the attribute {}, an attribute other than src",
                other.name()
            ),
        ));
    }
    for child in node.children() {
        if child.is_element() {
            return Err(IngestError::at(node, "an img that holds an element"));
        }
        if child.is_text() {
            return Err(IngestError::at(node, "an img that holds text"));
        }
    }
    Ok(Block::Image {
        source: normalize(node.attribute("src").unwrap_or("")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{
        Document as Doc, MEMBER, RELEASE, assert_fixture, fixture, read_fixture, records, refusal,
    };
    use fidryn_provision::{Item, Module, Provision};

    /// The provisions of the one unit of `modules`, in document order.
    fn provisions(modules: &[Module]) -> Vec<&Provision> {
        fn walk<'m>(provision: &'m Provision, out: &mut Vec<&'m Provision>) {
            out.push(provision);
            for item in &provision.items {
                if let Item::Child(child) = item {
                    walk(child, out);
                }
            }
        }
        let mut out = Vec::new();
        for module in modules {
            if let Module::Unit(unit) = module {
                walk(&unit.root, &mut out);
            }
        }
        out
    }

    /// The blocks of the one part of the section of `inner`.
    fn blocks_of(inner: &str) -> Vec<Block> {
        let modules = Doc::with_section(inner)
            .read()
            .unwrap_or_else(|error| panic!("{error}"));
        let section = provisions(&modules)[0].clone();
        match section.items.as_slice() {
            [Item::Part { blocks, .. }] => blocks.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn words(text: &str) -> Run {
        Run::Words(text.to_owned())
    }

    fn para(runs: Vec<Run>) -> Block {
        Block::Para(Text { runs })
    }

    fn mark(shown: &str, footnote: u32) -> Run {
        Run::Mark {
            shown: shown.to_owned(),
            footnote,
        }
    }

    #[test]
    fn a_document_type_declaration_is_refused() {
        let mut document = Doc::with_section("<content>Text.</content>");
        document.doctype = "<!DOCTYPE uscDoc>\n";
        let error = refusal(&document);
        assert!(
            error.message().contains("document type declaration"),
            "{error}"
        );
        assert_eq!(error.member(), Some("usc99.xml"));
    }

    #[test]
    fn an_entity_declaration_is_refused() {
        let mut document = Doc::with_section("<content>A &w;.</content>");
        document.doctype = "<!DOCTYPE uscDoc [<!ENTITY w \"widget\">]>\n";
        let error = refusal(&document);
        assert!(error.message().contains("declaration"), "{error}");
    }

    #[test]
    fn a_root_element_in_another_namespace_is_refused() {
        let mut document = Doc::with_section("<content>Text.</content>");
        document.namespace = "http://example.invalid/other";
        let error = refusal(&document);
        assert!(
            error.message().contains("not uscDoc in the namespace"),
            "{error}"
        );
        assert_eq!(
            error.element(),
            Some("{http://example.invalid/other}uscDoc")
        );
        let mut document = Doc::with_section("<content>Text.</content>");
        document.root = "uscDocument";
        assert!(refusal(&document).message().contains("not uscDoc"));
    }

    #[test]
    fn a_schema_location_that_is_not_in_the_list_is_refused() {
        let mut document = Doc::with_section("<content>Text.</content>");
        document.schema = "USLM-1.0.14.xsd";
        let error = refusal(&document);
        assert!(
            error
                .message()
                .contains("the schema location names USLM-1.0.14.xsd, not USLM-1.0.15.xsd"),
            "{error}"
        );
        document.schema = "http://xml.house.gov/schemas/uslm/1.0/USLM-1.0.15.xsd";
        assert!(document.read().is_ok());
    }

    #[test]
    fn a_release_name_that_differs_from_the_records_is_refused() {
        // White space around the name is part of it.
        let names = [
            "Online@999-2",
            "Print@999-1",
            "Online@999-1 ",
            " Online@999-1",
            "Online@999-1\n",
            "",
        ];
        for release in names {
            let mut document = Doc::with_section("<content>Text.</content>");
            document.release = release;
            let error = refusal(&document);
            assert!(
                error.message().contains("says Online@999-1"),
                "{release}: {error}"
            );
            assert_eq!(error.element(), Some("docPublicationName"));
        }
    }

    #[test]
    fn a_second_release_name_is_refused() {
        for second in ["Online@999-1", "Online@999-2"] {
            let text = Doc::with_section("<content>Text.</content>")
                .text()
                .replace(
                    "</docPublicationName>",
                    &format!(
                        "</docPublicationName><docPublicationName>{second}</docPublicationName>"
                    ),
                );
            let error = crate::ingest_xml(MEMBER, &text, RELEASE)
                .map(|_| ())
                .expect_err("a document with two names is refused");
            assert!(
                error
                    .message()
                    .contains("meta holds no docPublicationName, or more than one"),
                "{error}"
            );
            assert_eq!(error.element(), Some("meta"));
        }
    }

    #[test]
    fn the_name_and_the_flag_are_read_as_their_whole_text() {
        // Markup inside the name or the flag is read as the words it holds.
        let plain = Doc::with_section("<content>Text.</content>")
            .read()
            .unwrap_or_else(|error| panic!("{error}"));
        let mut document = Doc::with_section("<content>Text.</content>");
        document.release = "Online@<i>999</i>-1";
        document.flags = &["y<b>es</b>"];
        let marked = document.read().unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(records(&marked), records(&plain));
        document.flags = &["<i>n</i>o"];
        let modules = document.read().unwrap_or_else(|error| panic!("{error}"));
        assert!(modules.iter().all(|module| match module {
            Module::Unit(unit) => unit.positive_law == Some(false),
            Module::Container(container) => container.positive_law == Some(false),
            Module::Curated(_) => false,
        }));
        document.flags = &["<i>maybe</i>"];
        let error = refusal(&document);
        assert_eq!(
            error.message(),
            "the property is-positive-law is \"maybe\", neither yes nor no"
        );
    }

    #[test]
    fn a_release_name_with_an_en_dash_is_read() {
        let read = |xml: &str| {
            crate::ingest_xml(MEMBER, xml, RELEASE)
                .unwrap_or_else(|error| panic!("{error}"))
                .0
        };
        // The document of release 999-1 named `Online@999–1` reads to the
        // records of the same document named with a hyphen; the en dash of
        // its words stays.
        let dashed = fixture("meta-dash-name");
        assert!(dashed.contains("<docPublicationName>Online@999\u{2013}1</docPublicationName>"));
        let hyphenated = dashed.replace("Online@999\u{2013}1", "Online@999-1");
        assert_ne!(dashed, hyphenated);
        assert_eq!(records(&read(&dashed)), records(&read(&hyphenated)));
        assert_fixture("meta-dash-name");
        assert!(records(&read(&dashed))[1].contains("release 999\u{2013}1 keep"));
        // Any other character in the place of the hyphen is refused, and so
        // is a name with an en dash of another release; the message shows the
        // name as the document writes it.
        let others = [
            "Online@999\u{2014}1",
            "Online@999\u{2012}1",
            "Online@999\u{2010}1",
            "Online@999\u{2011}1",
            "Online@999\u{2212}1",
            "Online@999\u{FE63}1",
            "Online@999\u{FF0D}1",
            "Online@999 1",
            "Online@999_1",
            "Online@9991",
            "Online@999\u{2013}\u{2013}1",
            "Online@999\u{2013}2",
            "Online@99\u{2013}9-1",
        ];
        for release in others {
            let mut document = Doc::with_section("<content>Text.</content>");
            document.release = release;
            let error = refusal(&document);
            assert_eq!(
                error.message(),
                format!(
                    "the document names its release {release:?}, and a document of release \
                     999-1 says Online@999-1"
                ),
                "{release}"
            );
            assert!(error.message().contains(release), "{release}: {error}");
            assert_eq!(error.element(), Some("docPublicationName"));
        }
    }

    #[test]
    fn two_flags_and_another_value_are_still_refused() {
        let mut document = Doc::with_section("<content>Text.</content>");
        let twice: [&'static [&'static str]; 3] = [&["yes", "yes"], &["yes", "no"], &["no", "no"]];
        for flags in twice {
            document.flags = flags;
            let error = refusal(&document);
            assert_eq!(
                error.message(),
                "meta holds more than one property is-positive-law",
                "{flags:?}"
            );
            assert_eq!(error.element(), Some("meta"));
        }
        let others: [&'static [&'static str]; 6] =
            [&["maybe"], &[""], &["Yes"], &["true"], &[" yes"], &["no\n"]];
        for flags in others {
            document.flags = flags;
            let error = refusal(&document);
            assert_eq!(
                error.message(),
                format!(
                    "the property is-positive-law is {:?}, neither yes nor no",
                    flags[0]
                ),
            );
            assert_eq!(error.element(), Some("property"));
        }
        // One `yes` or `no` is the flag of every module of the document, and
        // a document without the property has none.
        let read: [(&'static [&'static str], Option<bool>); 3] =
            [(&["yes"], Some(true)), (&["no"], Some(false)), (&[], None)];
        for (flags, flag) in read {
            document.flags = flags;
            let modules = document.read().unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(modules.len(), 2);
            for module in &modules {
                let positive_law = match module {
                    Module::Unit(unit) => unit.positive_law,
                    Module::Container(container) => container.positive_law,
                    Module::Curated(_) => panic!("{module:?}"),
                };
                assert_eq!(positive_law, flag, "{flags:?}");
            }
        }
    }

    #[test]
    fn an_element_the_ingester_does_not_know_is_refused_and_named() {
        let error = refusal(&Doc::with_section(
            "<content>A <widget>gadget</widget>.</content>",
        ));
        assert_eq!(error.element(), Some("widget"));
        assert_eq!(error.identifier(), Some("/us/usc/t99/s1"));
        assert_eq!(error.member(), Some("usc99.xml"));
        assert!(
            error.message().contains(
                "<widget> is in no class of the reading, and does not stand in <content>"
            ),
            "{error}"
        );
        assert!(
            error
                .to_string()
                .starts_with("usc99.xml: line 5, <widget> in /us/usc/t99/s1: ")
        );
        let error = refusal(&Doc::with_section("<widget>Text.</widget>"));
        assert_eq!(error.element(), Some("widget"));
        let error = refusal(&Doc::with_section(
            "<content><xhtml:p xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">A \
             <xhtml:em>b</xhtml:em></xhtml:p></content>",
        ));
        assert_eq!(error.element(), Some("xhtml:em"));
    }

    #[test]
    fn what_the_rules_refuse_in_operative_text_is_refused() {
        const FOOTNOTE: &str = "<note type=\"footnote\" id=\"f1\"><num>1</num> A footnote.</note>";
        let table = |inner: &str| {
            format!(
                "<content><table xmlns=\"http://www.w3.org/1999/xhtml\">{inner}</table></content>"
            )
        };
        let cases: Vec<(String, &str)> =
            vec![
            (
                "<content>See <ref>this</ref>.</content>".to_owned(),
                "neither an href nor the class footnoteRef",
            ),
            (
                "<content>See <ref href=\"/us/usc/t99/s2\">section <ref href=\"/us/usc/t99/s3\">3\
                 </ref></ref>.</content>"
                    .to_owned(),
                "a link inside a link",
            ),
            (
                "<content>A<sub>b<sup>c</sup></sub>.</content>".to_owned(),
                "a superscript inside a subscript",
            ),
            (
                "<content>A<sup>b<ref href=\"/us/usc/t99/s2\">c</ref></sup>.</content>".to_owned(),
                "a link inside a superscript",
            ),
            (
                format!(
                    "<content>Text<sup><ref class=\"footnoteRef\" idref=\"f1\">1</ref></sup>\
                     {FOOTNOTE}.</content>"
                ),
                "a footnote marker inside a superscript",
            ),
            (
                format!(
                    "<content>Text<ref class=\"footnoteRef\" idref=\"f1\"><i>1</i></ref>\
                     {FOOTNOTE}.</content>"
                ),
                "a footnote marker that holds an element",
            ),
            (
                "<num value=\"2\">§ 2.</num><content>Text.</content>".to_owned(),
                "a second num",
            ),
            (
                "<heading>Again</heading><content>Text.</content>".to_owned(),
                "a second heading",
            ),
            (
                table("<tr><td colspan=\"two\">A</td></tr>"),
                "whole number written in ASCII digits",
            ),
            (
                table("<tr><td rowspan=\"\u{0662}\">A</td></tr>"),
                "whole number written in ASCII digits",
            ),
            (table("<tr><td colspan=\"-1\">A</td></tr>"), "whole number"),
            ("<content>A<br>b</br>c.</content>".to_owned(), "a br that holds content"),
            (
                table("<colgroup><col>A</col></colgroup><tr><td>A</td></tr>"),
                "a column that holds content",
            ),
            (table("<caption>A</caption>"), "<xhtml:caption> is in no class"),
            (
                "<content><img src=\"file:///a.eps\" alt=\"a\"/></content>".to_owned(),
                "attribute other than src",
            ),
            (
                "<content><img src=\"file:///a.eps\">A picture</img></content>".to_owned(),
                "an img that holds text",
            ),
            (
                "<content><img src=\"file:///a.eps\"><i/></img></content>".to_owned(),
                "an img that holds an element",
            ),
            (
                "stray words<content>Text.</content>".to_owned(),
                "text where only elements may stand",
            ),
            (
                "<content><chapeau>Text.</chapeau></content>".to_owned(),
                "<chapeau> does not stand in <content>",
            ),
            (
                "<chapeau>Text.<continuation>More.</continuation></chapeau>".to_owned(),
                "<continuation> does not stand in <chapeau>",
            ),
            (
                "<content><p>A<continuation>b</continuation></p></content>".to_owned(),
                "<continuation> does not stand in <p>",
            ),
            (
                "<subsection><num>A <p>b</p></num><content>Text.</content></subsection>".to_owned(),
                "<p> does not stand in <num>",
            ),
            (
                "<img src=\"file:///a.eps\"/>".to_owned(),
                "<img> does not stand in <section>",
            ),
            (
                "<content><subheading>A</subheading></content>".to_owned(),
                "<subheading> does not stand in <content>",
            ),
            (
                "<content><signature><name>A.</name></signature></content>".to_owned(),
                "<signature> does not stand in <content>",
            ),
            (
                "<content>A<quotedContent><p>B<heading>C</heading></p></quotedContent>\
                 </content>"
                    .to_owned(),
                "<heading> does not stand in <p>",
            ),
            (
                "<content>A<quotedContent><signature><name>B.</name></signature>\
                 </quotedContent></content>"
                    .to_owned(),
                "<signature> does not stand in <quotedContent>",
            ),
            (
                "<content>A<quotedContent><section><heading>B</heading><heading>C</heading>\
                 </section></quotedContent></content>"
                    .to_owned(),
                "a second heading",
            ),
        ];
        for (inner, phrase) in cases {
            let error = refusal(&Doc::with_section(&inner));
            assert!(error.message().contains(phrase), "{inner}: {error}");
            assert_eq!(
                error.identifier(),
                Some("/us/usc/t99/s1"),
                "{inner}: {error}"
            );
        }
        let error = refusal(&with_status("gone"));
        assert!(
            error.message().contains("not one of the 19 values"),
            "{error}"
        );
    }

    /// The document of an empty section with the status `status`.
    fn with_status(status: &str) -> Doc {
        let mut document = Doc::with_section("");
        document.body =
            document
                .body
                .replacen("<section ", &format!("<section status=\"{status}\" "), 1);
        document
    }

    #[test]
    fn every_status_of_the_schema_is_read() {
        for status in STATUSES {
            let modules = with_status(status).read().expect(status);
            assert_eq!(provisions(&modules)[0].status.as_deref(), Some(status));
        }
        assert_fixture("statuses");
    }

    #[test]
    fn links_superscripts_subscripts_and_dropped_typography_keep_their_words() {
        assert_fixture("links");
        assert_fixture("inline");
        // The link whose words are white space is dropped and counted; its
        // space is not left at the end of the paragraph.
        let (modules, not_imported) = crate::ingest_xml(
            crate::testing::MEMBER,
            &crate::testing::fixture("links"),
            crate::testing::RELEASE,
        )
        .expect("read");
        assert_eq!(not_imported.links_without_words, 1);
        assert_eq!(links_of(&modules), 2);
        // White space at the two ends of a link, a superscript and a
        // subscript stands outside it.
        assert_eq!(
            blocks_of(
                "<content>See<ref href=\"/us/usc/t99/s2\"> section 2 </ref>and 10<sup> 3 \
                 </sup>H<sub>2 </sub>O.</content>"
            ),
            [para(vec![
                words("See "),
                Run::Ref {
                    words: "section 2".to_owned(),
                    target: "/us/usc/t99/s2".to_owned(),
                },
                words(" and 10 "),
                Run::Sup("3".to_owned()),
                words(" H"),
                Run::Sub("2".to_owned()),
                words(" O."),
            ])]
        );
        // The target is kept as the source writes it.
        assert_eq!(
            blocks_of("<content><ref href=\"/us/usc/t99/s2/a b\">here</ref></content>"),
            [para(vec![Run::Ref {
                words: "here".to_owned(),
                target: "/us/usc/t99/s2/a b".to_owned(),
            }])]
        );
    }

    /// The links of the paragraphs of the provisions of `modules`.
    fn links_of(modules: &[Module]) -> usize {
        let mut count = 0;
        for provision in provisions(modules) {
            for item in &provision.items {
                if let Item::Part { blocks, .. } = item {
                    for block in blocks {
                        if let Block::Para(text) = block {
                            count += text
                                .runs
                                .iter()
                                .filter(|run| matches!(run, Run::Ref { .. }))
                                .count();
                        }
                    }
                }
            }
        }
        count
    }

    #[test]
    fn a_bare_superscript_is_never_a_footnote_marker() {
        // A superscript that equals the number of a footnote of its unit is a
        // superscript, and part of the text.
        let blocks = blocks_of(
            "<content>Due<ref class=\"footnoteRef\" idref=\"f1\">1</ref><note type=\"footnote\" \
             id=\"f1\"><num>1</num> So in original.</note> by rule<sup>1</sup> and C<sub>3\
             </sub>H<sub>8</sub>.</content>",
        );
        assert_eq!(
            blocks,
            [para(vec![
                words("Due"),
                mark("1", 1),
                words(" by rule"),
                Run::Sup("1".to_owned()),
                words(" and C"),
                Run::Sub("3".to_owned()),
                words("H"),
                Run::Sub("8".to_owned()),
                words("."),
            ])]
        );
        let modules = read_fixture("inline");
        let b = provisions(&modules)
            .into_iter()
            .find(|provision| provision.address.as_str() == "/us/usc/t99/s1/b")
            .expect("(b)");
        let Item::Part { blocks, .. } = &b.items[0] else {
            panic!("{b:?}");
        };
        let Block::Para(text) = &blocks[0] else {
            panic!("{blocks:?}");
        };
        assert_eq!(
            text.plain(),
            "The fee is set by rule1 and is 1⁄4 of a coin."
        );
        // The one marker is the `footnoteRef`, and both superscripts are
        // superscripts.
        assert_eq!(
            text.runs
                .iter()
                .filter(|run| matches!(run, Run::Mark { .. }))
                .collect::<Vec<_>>(),
            [&mark("1", 1)]
        );
        assert_eq!(
            text.runs
                .iter()
                .filter(|run| matches!(run, Run::Sup(words) if words == "1"))
                .count(),
            2
        );
    }

    #[test]
    fn a_footnote_does_not_close_its_paragraph_and_a_marker_gives_no_text() {
        // A marker stands directly after the last character before it that
        // is not white space: between two spaces it leaves one, after it.
        assert_eq!(
            blocks_of(
                "<content>The keeper shall keep<ref class=\"footnoteRef\" idref=\"f\">3</ref>\
                 <note type=\"footnote\" id=\"f\"><num>3</num> So in original.<p>Two.</p></note> \
                 the gate and <ref class=\"footnoteRef\" idref=\"f\"> 3\n</ref> the key<ref \
                 class=\"footnoteRef\" idref=\"f\">*</ref>  </content>"
            ),
            [para(vec![
                words("The keeper shall keep"),
                mark("3", 1),
                words(" the gate and"),
                mark("3", 1),
                words(" the key"),
                mark("*", 1),
            ])]
        );
        // After a link, and at the start of a paragraph.
        assert_eq!(
            blocks_of(
                "<content> <ref class=\"footnoteRef\" idref=\"f\">1</ref> See <ref \
                 href=\"/us/usc/t99/s2\">section 2</ref><ref class=\"footnoteRef\" \
                 idref=\"f\">1</ref>.<note type=\"footnote\" id=\"f\"><num>1</num> X.</note>\
                 </content>"
            ),
            [para(vec![
                mark("1", 1),
                words("See "),
                Run::Ref {
                    words: "section 2".to_owned(),
                    target: "/us/usc/t99/s2".to_owned(),
                },
                mark("1", 1),
                words("."),
            ])]
        );
        // A paragraph that holds a marker and no text is dropped.
        assert_eq!(
            blocks_of(
                "<content><p>Kept.</p><p><ref class=\"footnoteRef\" idref=\"f\">1</ref></p><note \
                 type=\"footnote\" id=\"f\"><num>1</num> X.</note></content>"
            ),
            [para(vec![words("Kept.")])]
        );
        assert_fixture("footnotes");
        assert_fixture("marker-labels");
    }

    #[test]
    fn quoted_content_in_operative_text_is_read() {
        assert_fixture("quoted-operative");
        // A subheading in a note that is quoted in operative text is a
        // subheading, as in any note, and so is a heading that stands directly
        // in the quotation.
        let blocks = blocks_of(
            "<content>Reads: <quotedContent><heading>Part</heading><note \
             topic=\"miscellaneous\"><heading>Instructions</heading><p>Fill.</p><subheading>\
             Signature Line</subheading><p>Sign.</p></note><chapeau>Words—</chapeau>\
             </quotedContent></content>",
        );
        let subheading = |text: &str| {
            QuotedItem::Block(Block::Subheading(Text {
                runs: vec![words(text)],
            }))
        };
        assert_eq!(
            blocks,
            [
                para(vec![words("Reads:")]),
                Block::Quoted(Quoted {
                    origin: None,
                    items: vec![
                        subheading("Part"),
                        subheading("Instructions"),
                        QuotedItem::Block(para(vec![words("Fill.")])),
                        subheading("Signature Line"),
                        QuotedItem::Block(para(vec![words("Sign.")])),
                        QuotedItem::Part {
                            role: PartRole::Chapeau,
                            blocks: vec![para(vec![words("Words—")])],
                        },
                    ],
                }),
            ]
        );
    }

    #[test]
    fn an_image_is_a_block_that_names_its_file() {
        assert_fixture("images");
        assert_eq!(
            blocks_of("<content>Shown:<img src=\" file:///a b.eps \"/>below.<img/></content>"),
            [
                para(vec![words("Shown:")]),
                Block::Image {
                    source: "file:///a b.eps".to_owned(),
                },
                para(vec![words("below.")]),
                Block::Image {
                    source: String::new(),
                },
            ]
        );
    }

    #[test]
    fn table_cells_keep_their_spans_and_empty_cells() {
        assert_fixture("table");
        let blocks = blocks_of(
            "<content><table xmlns=\"http://www.w3.org/1999/xhtml\"><col/><tr><th rowspan=\"02\" \
             colspan=\"3\"/><td><p>A</p>B<table><tr><td>Inner</td></tr></table></td></tr></table>\
             </content>",
        );
        let cell = |header, colspan, rowspan, blocks| Cell {
            header,
            colspan,
            rowspan,
            blocks,
        };
        assert_eq!(
            blocks,
            [Block::Table(Table {
                rows: vec![Row {
                    group: RowGroup::Body,
                    cells: vec![
                        cell(true, 3, 2, Vec::new()),
                        cell(
                            false,
                            1,
                            1,
                            vec![
                                para(vec![words("A")]),
                                para(vec![words("B")]),
                                Block::Table(Table {
                                    rows: vec![Row {
                                        group: RowGroup::Body,
                                        cells: vec![cell(
                                            false,
                                            1,
                                            1,
                                            vec![para(vec![words("Inner")])]
                                        )],
                                    }],
                                }),
                            ]
                        ),
                    ],
                }],
            })]
        );
    }

    #[test]
    fn several_paragraphs_and_a_paragraph_that_holds_a_table() {
        assert_fixture("content");
    }

    #[test]
    fn the_characters_of_the_text_are_normalized_and_kept() {
        assert_fixture("characters");
        // A character that composes across the boundary of a link, a
        // superscript, a subscript or a footnote marker is refused, not
        // divided.
        for inner in [
            "<content>cafe<ref href=\"/us/usc/t99/s2\">\u{301} owner</ref></content>",
            "<content><ref href=\"/us/usc/t99/s2\">cafe</ref>\u{301} owner</content>",
            "<content>cafe<sup>\u{301}</sup> owner</content>",
            "<content>cafe<sub>\u{301}</sub> owner</content>",
            "<content>cafe<ref class=\"footnoteRef\" idref=\"f\">1</ref>\u{301} owner\
             <note type=\"footnote\" id=\"f\"><num>1</num> So in original.</note></content>",
            "<heading>cafe<ref class=\"footnoteRef\" idref=\"f\">1</ref>\u{301}<note \
             type=\"footnote\" id=\"f\"><num>1</num> So in original.</note></heading>",
        ] {
            let mut document = Doc::with_section(inner);
            document.body = document.body.replacen("<heading>Invented</heading>", "", 1);
            let error = refusal(&document);
            assert!(
                error.message().contains("cannot be divided"),
                "{inner}: {error}"
            );
        }
        // A footnote is no boundary: the character composes.
        assert_eq!(
            blocks_of(
                "<content>cafe<note type=\"footnote\" id=\"f\"><num>1</num> X.</note>\u{301} \
                 owner</content>"
            ),
            [para(vec![words("caf\u{e9} owner")])]
        );
    }

    #[test]
    fn a_label_with_no_text_is_absent_unless_it_holds_a_marker() {
        assert_fixture("empty-labels");
        let modules = Doc::with_body(
            "<section identifier=\"/us/usc/t99/s1\"><num/><heading> </heading>\
             <content>Text.</content></section>"
                .to_owned(),
        )
        .read()
        .expect("read");
        let section = provisions(&modules)[0];
        assert_eq!((section.num.clone(), section.heading.clone()), (None, None));
        let modules = Doc::with_body(
            "<section identifier=\"/us/usc/t99/s1\"><num><ref class=\"footnoteRef\" idref=\"f\">1\
             </ref><note type=\"footnote\" id=\"f\"><num>1</num> X.</note></num>\
             <content>Text.</content></section>"
                .to_owned(),
        )
        .read()
        .expect("read");
        assert_eq!(
            provisions(&modules)[0].num,
            Some(Num {
                value: String::new(),
                shown: Text {
                    runs: vec![mark("1", 1)],
                },
            })
        );
    }
}
