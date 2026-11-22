//! Definitions: the terms that the provisions of a unit define in one of a
//! closed list of forms, each with its kind and its reach.
//!
//! A definition is read from the first block of a `chapeau` or `content`
//! part of a provision of a unit, when that block is a paragraph, and from
//! nothing else: not from a note, not from quoted content, and not from a
//! container. The text of a paragraph is its plain text. A term is text in
//! the curly quotation marks `“` and `”`, with at least one character and
//! neither mark inside it.
//!
//! - The sentence form begins with an optional reach and a comma, `the` or
//!   `The`, one of `term`, `terms`, `word`, `words`, `phrase`, the terms,
//!   an optional aside between commas, and a verb form. The list form
//!   begins with the terms and a verb form, and has a reach inherited from
//!   a chapeau.
//! - A reach is an opening (`As used in`, `In`, `For purposes of`, ...) and
//!   an object, which holds no quotation mark and none of the words `term`,
//!   `definition`, `following`, `meaning` and the like; after `In` it begins
//!   with `this` or is one link.
//! - A paragraph without a reach of its own looks to the nearest ancestor of
//!   its provision that has a chapeau: a chapeau that is a reach and a close
//!   (with an optional lead-in such as `the term`) gives its reach, and any
//!   other chapeau leaves the paragraph unread.
//! - A paragraph is not read when it defines a second term after its verb,
//!   when its aside speaks of a reach, when a reach follows its verb, or
//!   when it states no reach and speaks of one.
//! - The region of a reach is the nearest provision or container of the
//!   kind its object names after `this`, or the target of its link below
//!   the prefix of the Code; its basis is `unresolved` without a region,
//!   `enacted` in a unit that is positive law, and `restated` in any other.
//!   A restated definition points at the first editorial note headed
//!   "References in Text" that speaks of the words naming its region.
//!
//! [`extract`] reads a unit whose provisions and notes are complete.

use crate::PREFIX;
use fidryn_provision::{
    Address, Basis, Block, Definition, DefinitionKind, Item, Layer, Note, PartRole, Provision, Run,
    Text, UnitModule,
};

/// The verb forms, each with the kind of definition it gives.
const VERBS: [(&str, DefinitionKind); 16] = [
    ("means", DefinitionKind::Means),
    ("mean", DefinitionKind::Means),
    ("shall mean", DefinitionKind::Means),
    ("includes", DefinitionKind::Includes),
    ("include", DefinitionKind::Includes),
    ("shall include", DefinitionKind::Includes),
    ("does not include", DefinitionKind::DoesNotInclude),
    ("do not include", DefinitionKind::DoesNotInclude),
    ("shall not include", DefinitionKind::DoesNotInclude),
    ("has the meaning given", DefinitionKind::Borrowed),
    ("have the meaning given", DefinitionKind::Borrowed),
    ("has the meanings given", DefinitionKind::Borrowed),
    ("have the meanings given", DefinitionKind::Borrowed),
    ("has the same meaning as", DefinitionKind::Borrowed),
    ("have the same meaning as", DefinitionKind::Borrowed),
    ("shall have the meaning given", DefinitionKind::Borrowed),
];

/// The openings of a reach.
const OPENINGS: [&str; 7] = [
    "As used in",
    "In",
    "For purposes of",
    "For the purposes of",
    "For the purpose of",
    "For purpose of",
    "When used in",
];

/// The words that name what the sentence form defines, after `the`.
const NOUNS: [&str; 5] = ["term", "terms", "word", "words", "phrase"];

/// What stands between two terms of a list, each followed by a term.
const SEPARATORS: [&str; 5] = [", and ", ", or ", ", ", " and ", " or "];

/// The words the object of a reach holds none of, in any case.
const NOT_IN_AN_OBJECT: [&str; 11] = [
    "term",
    "terms",
    "word",
    "words",
    "phrase",
    "phrases",
    "definition",
    "definitions",
    "following",
    "meaning",
    "meanings",
];

/// The phrases that speak of a reach, in any case.
const REACH_PHRASES: [&str; 6] = [
    "as used in",
    "when used in",
    "for purposes of",
    "for the purposes of",
    "for the purpose of",
    "for purpose of",
];

/// What may stand between the reach of a reach chapeau and its close.
const LEAD_INS: [&str; 4] = [
    "the term",
    "the terms",
    "the following definitions apply",
    "the following definitions shall apply",
];

/// The closes of a reach chapeau: an em dash, a hyphen and a colon.
const CLOSES: [char; 3] = ['\u{2014}', '-', ':'];

/// The kinds that name a region after `this`.
const REGION_KINDS: [&str; 12] = [
    "title",
    "subtitle",
    "chapter",
    "subchapter",
    "part",
    "subpart",
    "division",
    "section",
    "subsection",
    "paragraph",
    "subparagraph",
    "clause",
];

/// The heading of the editorial note a restated definition points at.
const REFERENCES_IN_TEXT: &str = "References in Text";

/// The quotation marks that enclose a term.
const OPEN: char = '\u{201c}';
const CLOSE: char = '\u{201d}';
const MARKS: [char; 2] = [OPEN, CLOSE];

/// The definitions of `unit`, whose provisions and notes are complete, in
/// the order of their provisions in the document and then of their first
/// term; two definitions of one provision with one first term keep the
/// order of their parts. `containers` are the unit's containers from the
/// nearest outward, each at its address and with its kind.
pub fn extract(unit: &UnitModule, containers: &[(Address, String)]) -> Vec<Definition> {
    let mut reading = Reading {
        unit,
        containers,
        stack: Vec::new(),
        next: 0,
        found: Vec::new(),
    };
    reading.provision(&unit.root);
    let mut found = reading.found;
    found.sort_by(|(a, first), (b, second)| {
        a.cmp(b)
            .then_with(|| first.terms.first().cmp(&second.terms.first()))
    });
    found
        .into_iter()
        .map(|(_, definition)| definition)
        .collect()
}

/// The reading of the provisions of one unit.
struct Reading<'u> {
    unit: &'u UnitModule,
    containers: &'u [(Address, String)],
    /// The provision being read and its ancestors, the top provision first.
    stack: Vec<&'u Provision>,
    /// The place in the document of the next provision.
    next: usize,
    /// Each definition found, with the place of its provision.
    found: Vec<(usize, Definition)>,
}

impl<'u> Reading<'u> {
    fn provision(&mut self, provision: &'u Provision) {
        let place = self.next;
        self.next += 1;
        self.stack.push(provision);
        for item in &provision.items {
            if let Item::Part {
                role: PartRole::Chapeau | PartRole::Content,
                blocks,
            } = item
                && let Some(Block::Para(text)) = blocks.first()
                && let Some(definition) = self.paragraph(text)
            {
                self.found.push((place, definition));
            }
        }
        for item in &provision.items {
            if let Item::Child(child) = item {
                self.provision(child);
            }
        }
        self.stack.pop();
    }

    /// The definition of the paragraph `text` of the provision at the top of
    /// the stack, when the rules read one.
    fn paragraph(&self, text: &Text) -> Option<Definition> {
        let paragraph = Paragraph::of(text);
        let matched = matched(&paragraph)?;
        let own = self.stack.len() - 1;
        // The reach and the place in the stack of the provision whose text
        // holds it.
        let (reach, holder) = match matched.reach {
            Some(reach) => (Some(reach), own),
            None => match nearest_chapeau(&self.stack) {
                Some(at) => (Some(reach_chapeau(self.stack[at])?), at),
                None if matched.list => return None,
                None => (None, own),
            },
        };
        if reach.is_none()
            && REACH_PHRASES
                .iter()
                .any(|phrase| holds_in_any_case(&paragraph.plain, phrase))
        {
            return None;
        }
        let borrowed_from = match matched.kind {
            DefinitionKind::Borrowed => {
                let link = paragraph
                    .links
                    .iter()
                    .find(|link| link.start >= matched.verb_end)?;
                // A definition records no empty target, which the canonical
                // form refuses.
                if link.target.is_empty() {
                    return None;
                }
                Some(link.target.to_owned())
            }
            _ => None,
        };
        let chain = self.stack[..=holder]
            .iter()
            .rev()
            .map(|provision| (&provision.address, provision.kind.as_str()))
            .chain(
                self.containers
                    .iter()
                    .map(|(address, kind)| (address, kind.as_str())),
            );
        let region = reach.as_ref().and_then(|reach| region(reach, chain));
        let basis = match (&region, self.unit.positive_law) {
            (None, _) => Basis::Unresolved,
            (Some(_), Some(true)) => Basis::Enacted,
            (Some(_), _) => Basis::Restated,
        };
        let see_note = match &region {
            Some((_, name)) if basis == Basis::Restated => see_note(&self.unit.notes, name),
            _ => None,
        };
        Some(Definition {
            at: self.stack[own].address.clone(),
            terms: matched.terms,
            kind: matched.kind,
            words: reach.map(|reach| reach.words),
            region: region.map(|(address, _)| address),
            basis,
            borrowed_from,
            see_note,
        })
    }
}

/// The plain text of a paragraph, and where each link stands in it.
struct Paragraph<'t> {
    plain: String,
    links: Vec<Link<'t>>,
}

/// A link of a paragraph: the bytes of its words in the plain text, and its
/// target.
struct Link<'t> {
    start: usize,
    end: usize,
    target: &'t str,
}

impl<'t> Paragraph<'t> {
    /// The words of every run of `text` but the footnote markers, in order.
    fn of(text: &'t Text) -> Self {
        let mut plain = String::new();
        let mut links = Vec::new();
        for run in &text.runs {
            match run {
                Run::Words(words) | Run::Sup(words) | Run::Sub(words) => plain.push_str(words),
                Run::Ref { words, target } => {
                    let start = plain.len();
                    plain.push_str(words);
                    links.push(Link {
                        start,
                        end: plain.len(),
                        target,
                    });
                }
                Run::Mark { .. } => {}
            }
        }
        Paragraph { plain, links }
    }
}

/// A paragraph that matches a pattern and that none of the rules of what is
/// not extracted leaves unread so far.
struct Matched {
    /// The reach of the sentence form, when it begins with one.
    reach: Option<Reach>,
    /// Whether the paragraph has the list form.
    list: bool,
    terms: Vec<String>,
    kind: DefinitionKind,
    /// Where the verb form ends.
    verb_end: usize,
}

/// A reach as a text states it.
struct Reach {
    /// The text from the opening to the end of the object.
    words: String,
    object: String,
    /// The target of the link whose words are the whole object, when there
    /// is one.
    link: Option<String>,
}

/// Whether `byte` is one of the letters `A` to `Z` and `a` to `z`.
fn is_letter(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
}

/// Whether the bytes `start..end` of `text` are bounded by the start or the
/// end of the text or by a character that is not a letter.
fn bounded(text: &str, start: usize, end: usize) -> bool {
    let bytes = text.as_bytes();
    (start == 0 || !is_letter(bytes[start - 1])) && (end == bytes.len() || !is_letter(bytes[end]))
}

/// Whether `text` holds `words` at the byte `at`, the letters `A` to `Z`
/// compared as `a` to `z` and no other character changed.
fn at_in_any_case(text: &str, at: usize, words: &str) -> bool {
    text.as_bytes()
        .get(at..at + words.len())
        .is_some_and(|bytes| bytes.eq_ignore_ascii_case(words.as_bytes()))
}

/// Whether `text` holds `words` as whole words, in any case. A match of the
/// bytes of `words` begins and ends at characters, since `words` is text.
fn holds_in_any_case(text: &str, words: &str) -> bool {
    (0..=text.len().saturating_sub(words.len()))
        .any(|at| at_in_any_case(text, at, words) && bounded(text, at, at + words.len()))
}

/// The verb form that stands at the byte `at` of `text`, followed by the end
/// of the text or by a character that is not a letter: its end and its kind.
fn verb_at(text: &str, at: usize) -> Option<(usize, DefinitionKind)> {
    let rest = text.get(at..)?;
    VERBS
        .iter()
        .filter(|(form, _)| rest.starts_with(form) && bounded(text, at, at + form.len()))
        .map(|(form, kind)| (at + form.len(), *kind))
        .max_by_key(|(end, _)| *end)
}

/// The first verb form of `text` from the byte `from` on that stands with a
/// character before it that is not a letter: its start, its end and its
/// kind.
fn first_verb(text: &str, from: usize) -> Option<(usize, usize, DefinitionKind)> {
    (from..text.len())
        .filter(|&at| text.is_char_boundary(at))
        .find_map(|at| verb_at(text, at).map(|(end, kind)| (at, end, kind)))
}

/// The term in quotation marks that begins at the byte `at` of `text`,
/// without the marks, and where its closing mark ends.
fn quoted(text: &str, at: usize) -> Option<(&str, usize)> {
    let inner = text.get(at..)?.strip_prefix(OPEN)?;
    let length = inner.find(MARKS)?;
    if length == 0 || !inner[length..].starts_with(CLOSE) {
        return None;
    }
    let start = at + OPEN.len_utf8();
    Some((
        &text[start..start + length],
        start + length + CLOSE.len_utf8(),
    ))
}

/// The terms of the list that begins at the byte `at` of `text`, a term and
/// then any number of separators each followed by a term, and where the
/// last term ends.
fn terms(text: &str, at: usize) -> Option<(Vec<String>, usize)> {
    let (first, mut end) = quoted(text, at)?;
    let mut terms = vec![first.to_owned()];
    'list: loop {
        for separator in SEPARATORS {
            if text[end..].starts_with(separator)
                && let Some((term, after)) = quoted(text, end + separator.len())
            {
                terms.push(term.to_owned());
                end = after;
                continue 'list;
            }
        }
        return Some((terms, end));
    }
}

/// Where the terms of the sentence form begin when `the` or `The` and one of
/// [`NOUNS`] and a space stand at the byte `at` of `text`.
fn after_noun(text: &str, at: usize) -> Option<usize> {
    let rest = text.get(at..)?;
    let rest = rest
        .strip_prefix("the ")
        .or_else(|| rest.strip_prefix("The "))?;
    NOUNS.iter().find_map(|noun| {
        rest.strip_prefix(noun)?
            .strip_prefix(' ')
            .map(|_| at + "the ".len() + noun.len() + 1)
    })
}

/// The reach that the text of `paragraph` states from its start to the
/// byte `end`, when that text is an opening, a space and an object.
fn reach(paragraph: &Paragraph<'_>, end: usize) -> Option<Reach> {
    let text = paragraph.plain.get(..end)?;
    let opening = OPENINGS.iter().find(|opening| {
        text.strip_prefix(**opening)
            .is_some_and(|rest| rest.starts_with(' '))
    })?;
    let start = opening.len() + 1;
    let object = &text[start..];
    if object.is_empty()
        || object.contains(MARKS)
        || NOT_IN_AN_OBJECT
            .iter()
            .any(|word| holds_in_any_case(object, word))
    {
        return None;
    }
    let link = paragraph
        .links
        .iter()
        .find(|link| link.start == start && link.end == end)
        .map(|link| link.target.to_owned());
    if *opening == "In" && !object.starts_with("this ") && link.is_none() {
        return None;
    }
    Some(Reach {
        words: text.to_owned(),
        object: object.to_owned(),
        link,
    })
}

/// The pattern `paragraph` matches, its terms, kind and verb, and its reach
/// when it states one, unless one of the rules of what is not extracted
/// that do not depend on an inherited reach leaves it unread.
fn matched(paragraph: &Paragraph<'_>) -> Option<Matched> {
    let text = paragraph.plain.as_str();
    let found = if text.starts_with(OPEN) {
        let (terms, end) = terms(text, 0)?;
        if !text[end..].starts_with(' ') {
            return None;
        }
        let (verb_end, kind) = verb_at(text, end + 1)?;
        Matched {
            reach: None,
            list: true,
            terms,
            kind,
            verb_end,
        }
    } else {
        sentence(paragraph)?
    };
    let rest = &text[found.verb_end..];
    // A reach follows its verb.
    if let Some(after) = rest.strip_prefix(", ")
        && OPENINGS.iter().any(|opening| {
            at_in_any_case(after, 0, opening) && after.as_bytes().get(opening.len()) == Some(&b' ')
        })
    {
        return None;
    }
    if defines_a_second_term(text, found.verb_end) {
        return None;
    }
    Some(found)
}

/// The sentence form of `paragraph`, unless its reach is no reach or its
/// aside speaks of a reach.
fn sentence(paragraph: &Paragraph<'_>) -> Option<Matched> {
    let text = paragraph.plain.as_str();
    let (reach, first) = match after_noun(text, 0) {
        Some(first) => (None, first),
        None => {
            // The reach ends at the comma before `the term`, with no
            // quotation mark before it.
            let marks = text.find(MARKS).unwrap_or(text.len());
            let comma = text[..marks]
                .match_indices(", ")
                .map(|(at, _)| at)
                .find(|&at| after_noun(text, at + 2).is_some())?;
            (Some(reach(paragraph, comma)?), after_noun(text, comma + 2)?)
        }
    };
    let (terms, end) = terms(text, first)?;
    let (verb_end, kind) = if text[end..].starts_with(' ') {
        verb_at(text, end + 1)?
    } else if text[end..].starts_with(", ") {
        // An aside between two commas, with no quotation mark and no verb
        // form in it.
        let from = end + 2;
        let (start, verb_end, kind) = first_verb(text, from)?;
        let aside_end = start
            .checked_sub(2)
            .filter(|&at| at >= from && &text.as_bytes()[at..start] == b", ")?;
        let aside = &text[from..aside_end];
        if aside.contains(MARKS)
            || at_in_any_case(aside, 0, "in ")
            || REACH_PHRASES
                .iter()
                .any(|phrase| holds_in_any_case(aside, phrase))
        {
            return None;
        }
        (verb_end, kind)
    } else {
        return None;
    };
    Some(Matched {
        reach,
        list: false,
        terms,
        kind,
        verb_end,
    })
}

/// Whether `text`, after the byte `from`, holds a term followed by a space
/// and a verb form, or by a comma, a space, text with no quotation mark, a
/// comma, a space and a verb form.
fn defines_a_second_term(text: &str, from: usize) -> bool {
    text[from..].match_indices(OPEN).any(|(offset, _)| {
        let Some((_, end)) = quoted(text, from + offset) else {
            return false;
        };
        let rest = &text[end..];
        if rest.starts_with(' ') && verb_at(text, end + 1).is_some() {
            return true;
        }
        let Some(inner) = rest.strip_prefix(", ") else {
            return false;
        };
        let inner_start = end + 2;
        let stop = inner.find(MARKS).unwrap_or(inner.len());
        inner[..stop]
            .match_indices(", ")
            .any(|(at, _)| verb_at(text, inner_start + at + 2).is_some())
    })
}

/// Whether `provision` has a chapeau.
fn has_chapeau(provision: &Provision) -> bool {
    provision.items.iter().any(|item| {
        matches!(
            item,
            Item::Part {
                role: PartRole::Chapeau,
                ..
            }
        )
    })
}

/// The place in `stack` of the nearest ancestor of the provision at its top
/// that has a chapeau.
fn nearest_chapeau(stack: &[&Provision]) -> Option<usize> {
    (0..stack.len().saturating_sub(1))
        .rev()
        .find(|&at| has_chapeau(stack[at]))
}

/// The reach of the chapeau of `provision` when it is a reach chapeau: one
/// paragraph whose text is a reach, optionally a comma and a lead-in, and a
/// close with at most one space before it.
fn reach_chapeau(provision: &Provision) -> Option<Reach> {
    let mut chapeaus = provision.items.iter().filter_map(|item| match item {
        Item::Part {
            role: PartRole::Chapeau,
            blocks,
        } => Some(blocks),
        _ => None,
    });
    let blocks = chapeaus.next()?;
    if chapeaus.next().is_some() {
        return None;
    }
    let [Block::Para(text)] = blocks.as_slice() else {
        return None;
    };
    let paragraph = Paragraph::of(text);
    let before = paragraph.plain.strip_suffix(CLOSES)?;
    let before = before.strip_suffix(' ').unwrap_or(before);
    let end = LEAD_INS
        .iter()
        .find_map(|lead_in| {
            let rest = before.strip_suffix(lead_in)?;
            rest.strip_suffix(", ")
                .or_else(|| rest.strip_suffix(' '))
                .map(str::len)
        })
        .unwrap_or(before.len());
    reach(&paragraph, end)
}

/// The region `reach` names and the words that name it, counted along
/// `chain`: the provision whose text holds the reach, then its ancestors
/// and the containers of its unit, each at its address and with its kind,
/// the nearest first.
fn region<'c>(
    reach: &Reach,
    mut chain: impl Iterator<Item = (&'c Address, &'c str)>,
) -> Option<(Address, String)> {
    if let Some(rest) = reach.object.strip_prefix("this ") {
        let kind = REGION_KINDS.iter().find(|kind| {
            rest.strip_prefix(**kind)
                .is_some_and(|after| after.is_empty() || after.starts_with(", unless the context"))
        })?;
        let (address, _) = chain.find(|(_, of)| of == kind)?;
        return Some((address.clone(), format!("this {kind}")));
    }
    let target = Address::parse(reach.link.as_deref()?).ok()?;
    let prefix = Address::parse(PREFIX).ok()?;
    (target != prefix && target.is_within(&prefix)).then(|| (target, reach.object.clone()))
}

/// The address of the first editorial note of `notes` headed "References in
/// Text" one of whose own paragraphs holds, in any case, `name` followed by
/// `, referred to in`.
fn see_note(notes: &[Note], name: &str) -> Option<Address> {
    let words = format!("{name}, referred to in");
    notes
        .iter()
        .find(|note| {
            note.layer == Layer::Editorial
                && note
                    .heading
                    .as_ref()
                    .is_some_and(|heading| heading.plain() == REFERENCES_IN_TEXT)
                && note.blocks.iter().any(|block| {
                    matches!(block, Block::Para(text) if holds_in_any_case(&text.plain(), &words))
                })
        })
        .map(|note| note.address.clone())
}

#[cfg(test)]
mod tests {
    use crate::testing::{Document, MEMBER, RELEASE, fixture, read_fixture};
    use fidryn_provision::{Address, Basis, Definition, DefinitionKind, Module};

    use Basis::{Enacted, Restated, Unresolved};
    use DefinitionKind::{Borrowed, DoesNotInclude, Includes, Means};

    fn address(text: &str) -> Address {
        Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    /// A definition with no `borrowed_from` and no `see_note`.
    fn definition(
        at: &str,
        terms: &[&str],
        kind: DefinitionKind,
        words: Option<&str>,
        region: Option<&str>,
        basis: Basis,
    ) -> Definition {
        Definition {
            at: address(at),
            terms: terms.iter().map(|term| (*term).to_owned()).collect(),
            kind,
            words: words.map(str::to_owned),
            region: region.map(address),
            basis,
            borrowed_from: None,
            see_note: None,
        }
    }

    /// The definitions of the units of `modules`, unit after unit.
    fn definitions_of(modules: &[Module]) -> Vec<Definition> {
        modules
            .iter()
            .flat_map(|module| match module {
                Module::Unit(unit) => unit.definitions.clone(),
                _ => Vec::new(),
            })
            .collect()
    }

    /// The definitions of the document whose section `/us/usc/t99/s1`
    /// holds `inner`.
    fn defined(inner: &str) -> Vec<Definition> {
        definitions_of(
            &Document::with_section(inner)
                .read()
                .unwrap_or_else(|error| panic!("{inner}: {error}")),
        )
    }

    /// The definitions of the text of a document of title 99.
    fn definitions_of_text(text: &str) -> Vec<Definition> {
        definitions_of(
            &crate::ingest_xml(MEMBER, text, RELEASE)
                .unwrap_or_else(|error| panic!("{error}"))
                .0,
        )
    }

    const CHAPTER: &str = "/us/usc/t99/ch1";

    #[test]
    fn the_sentence_form_has_a_reach_of_its_own_or_none() {
        let s1 = "/us/usc/t99/s1";
        let s2 = "/us/usc/t99/s2";
        assert_eq!(
            definitions_of(&read_fixture("definitions-sentence")),
            [
                definition(
                    "/us/usc/t99/s1/a",
                    &["widget"],
                    Means,
                    Some("As used in this section"),
                    Some(s1),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s1/b",
                    &["widget", "gadget"],
                    Means,
                    None,
                    None,
                    Unresolved
                ),
                definition(
                    "/us/usc/t99/s1/c",
                    &["widget"],
                    Includes,
                    Some("For purposes of this chapter"),
                    Some(CHAPTER),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s1/d",
                    &["widget"],
                    DoesNotInclude,
                    None,
                    None,
                    Unresolved
                ),
                Definition {
                    borrowed_from: Some(s2.to_owned()),
                    ..definition(
                        "/us/usc/t99/s1/e",
                        &["widget"],
                        Borrowed,
                        None,
                        None,
                        Unresolved,
                    )
                },
                definition(
                    "/us/usc/t99/s1/f",
                    &["widget"],
                    Means,
                    Some("For purposes of this Act"),
                    None,
                    Unresolved
                ),
                definition(
                    "/us/usc/t99/s1/g",
                    &["widget"],
                    Means,
                    Some("As used in section 2 of this title"),
                    Some(s2),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s1/h",
                    &["widget"],
                    Means,
                    Some("As used in Public Law 99\u{2013}1"),
                    None,
                    Unresolved
                ),
                definition(
                    "/us/usc/t99/s1/i",
                    &["widget"],
                    Means,
                    Some("For the purpose of this section"),
                    Some(s1),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s1/j",
                    &["widget"],
                    Means,
                    None,
                    None,
                    Unresolved
                ),
            ]
        );
    }

    #[test]
    fn paragraphs_outside_the_patterns_define_nothing() {
        // Section 3 of the fixture holds every one of them, a note and
        // quoted content among them, and nothing else.
        let modules = read_fixture("definitions-sentence");
        let Some(Module::Unit(s3)) = modules
            .iter()
            .find(|module| crate::testing::address_of(module) == "/us/usc/t99/s3")
        else {
            panic!("section 3");
        };
        assert_eq!(s3.root.items.len(), 11);
        assert_eq!(s3.notes.len(), 1);
        assert_eq!(s3.definitions, []);
    }

    #[test]
    fn a_chapeau_that_states_a_reach_gives_it_to_the_paragraphs_below() {
        let s4 = "/us/usc/t99/s4";
        let s6 = "/us/usc/t99/s6";
        assert_eq!(
            definitions_of(&read_fixture("definitions-chapeau")),
            [
                definition(
                    "/us/usc/t99/s1/a/1",
                    &["widget"],
                    Means,
                    Some("In this chapter"),
                    Some(CHAPTER),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s1/a/2",
                    &["gadget"],
                    Includes,
                    Some("In this chapter"),
                    Some(CHAPTER),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s1/c/1",
                    &["widget"],
                    Means,
                    Some("As used in this chapter"),
                    Some(CHAPTER),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s2/1",
                    &["widget"],
                    Means,
                    Some("In this section"),
                    Some("/us/usc/t99/s2"),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s3/a/1/A",
                    &["widget"],
                    Means,
                    Some("For the purpose of this chapter"),
                    Some(CHAPTER),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s4/a/1",
                    &["widget"],
                    Means,
                    Some("In this section"),
                    Some(s4),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s4/b/1",
                    &["widget"],
                    Means,
                    Some("In this section"),
                    Some(s4),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s4/c/1",
                    &["widget"],
                    Means,
                    Some("In this chapter, unless the context otherwise requires"),
                    Some(CHAPTER),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s4/d/1",
                    &["widget"],
                    Means,
                    Some("In this chapter and chapter 5"),
                    None,
                    Unresolved
                ),
                definition(
                    "/us/usc/t99/s4/e/1",
                    &["widget"],
                    Means,
                    Some("In this subsection"),
                    Some("/us/usc/t99/s4/e"),
                    Enacted
                ),
                // One provision: by the first term, then in the order of the
                // parts.
                definition(
                    "/us/usc/t99/s6/a",
                    &["apple"],
                    Means,
                    Some("For purposes of this section"),
                    Some(s6),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s6/a",
                    &["zebra"],
                    Means,
                    Some("For purposes of this section"),
                    Some(s6),
                    Enacted
                ),
                definition(
                    "/us/usc/t99/s6/b",
                    &["widget"],
                    Means,
                    None,
                    None,
                    Unresolved
                ),
                definition(
                    "/us/usc/t99/s6/b",
                    &["widget"],
                    Includes,
                    None,
                    None,
                    Unresolved
                ),
            ]
        );
    }

    #[test]
    fn the_words_of_the_rules_are_read_as_they_say() {
        let s1 = "/us/usc/t99/s1";
        let s2 = "/us/usc/t99/s2";
        let widget = |at: &str, words: Option<&str>, region: Option<&str>, basis| {
            vec![definition(at, &["widget"], Means, words, region, basis)]
        };
        let in_content = |content: &str| format!("<content>{content}</content>");
        let under = |chapeau: &str, child: &str| {
            format!(
                "<chapeau>{chapeau}</chapeau><paragraph identifier=\"/us/usc/t99/s1/1\">\
                 <num value=\"1\">(1)</num><content>{child}</content></paragraph>"
            )
        };
        let sentence = "The term “widget” means a small device.";
        let none = Vec::new();
        for (inner, expected) in [
            // A verb form stands before the end or a character that is not a
            // letter, and a term has at least one character.
            (
                in_content("The term “widget” meanest a small device."),
                none.clone(),
            ),
            (
                in_content("The term “” means a small device."),
                none.clone(),
            ),
            // Where the rules say "in any case", in any case, and words are
            // whole words.
            (
                in_content("The term “widget” means a device AS USED IN commerce."),
                none.clone(),
            ),
            (
                in_content("The term “widget”, In the case of a gadget, means a small device."),
                none.clone(),
            ),
            (
                in_content("The term “widget” means, For Purposes Of this section, a device."),
                none.clone(),
            ),
            (
                in_content("For purposes of the Terms below, the term “widget” means a device."),
                none.clone(),
            ),
            (
                in_content("For purposes of this termination, the term “widget” means a device."),
                widget(
                    s1,
                    Some("For purposes of this termination"),
                    None,
                    Unresolved,
                ),
            ),
            // Elsewhere, exactly.
            (
                in_content("for purposes of this section, the term “widget” means a device."),
                none.clone(),
            ),
            (
                in_content("The Term “widget” means a small device."),
                none.clone(),
            ),
            (
                in_content("The term “widget” Means a small device."),
                none.clone(),
            ),
            // After In, the object is `this` and a space, or a single link.
            (
                in_content(
                    "In <ref href=\"/us/usc/t99/s2\">section 2</ref>, the term “widget” means a \
                     device.",
                ),
                widget(s1, Some("In section 2"), Some(s2), Enacted),
            ),
            (
                in_content(
                    "In <ref href=\"/us/usc/t99/s2\">section 2</ref> and section 3, the term \
                     “widget” means a device.",
                ),
                none.clone(),
            ),
            (
                in_content(
                    "As used in <ref href=\"/us/usc/t99/s2\">section 2</ref> and section 3, the \
                     term “widget” means a device.",
                ),
                widget(
                    s1,
                    Some("As used in section 2 and section 3"),
                    None,
                    Unresolved,
                ),
            ),
            (
                in_content(
                    "In <ref href=\"/us/pl/99/1\">Public Law 99\u{2013}1</ref>, the term \
                     “widget” means a device.",
                ),
                widget(s1, Some("In Public Law 99\u{2013}1"), None, Unresolved),
            ),
            // The word of a region is followed by nothing more, or by a comma,
            // a space and `unless the context`.
            (
                in_content(
                    "In this section, unless the context requires otherwise, the term “widget” \
                     means a device.",
                ),
                widget(
                    s1,
                    Some("In this section, unless the context requires otherwise"),
                    Some(s1),
                    Enacted,
                ),
            ),
            (
                in_content(
                    "For purposes of this section, except as provided, the term “widget” means a \
                     device.",
                ),
                widget(
                    s1,
                    Some("For purposes of this section, except as provided"),
                    None,
                    Unresolved,
                ),
            ),
            // A close is a dash, a hyphen or a colon, with at most one space
            // before it; an en dash is none.
            (
                under("In this section :", sentence),
                widget(
                    "/us/usc/t99/s1/1",
                    Some("In this section"),
                    Some(s1),
                    Enacted,
                ),
            ),
            (
                under("In this section-", sentence),
                widget(
                    "/us/usc/t99/s1/1",
                    Some("In this section"),
                    Some(s1),
                    Enacted,
                ),
            ),
            (under("In this section\u{2013}", sentence), none.clone()),
            (
                under(
                    "In this section, the terms\u{2014}",
                    "“widget” means a small device.",
                ),
                widget(
                    "/us/usc/t99/s1/1",
                    Some("In this section"),
                    Some(s1),
                    Enacted,
                ),
            ),
            // A paragraph of the list form under no chapeau is not extracted.
            (in_content("“widget” means a small device."), none.clone()),
        ] {
            assert_eq!(defined(&inner), expected, "{inner}");
        }
    }

    /// The definitions of `definitions-restated`, with each basis and note
    /// that `bases` and `notes` give.
    fn restated(bases: [Basis; 5], notes: [Option<&str>; 5]) -> Vec<Definition> {
        let mut definitions = vec![
            definition(
                "/us/usc/t99/s1",
                &["widget"],
                Means,
                Some("In this chapter"),
                Some(CHAPTER),
                bases[0],
            ),
            definition(
                "/us/usc/t99/s2",
                &["gadget"],
                Means,
                Some("In this section"),
                Some("/us/usc/t99/s2"),
                bases[1],
            ),
            definition("/us/usc/t99/s3", &["sprocket"], Means, None, None, bases[2]),
            definition(
                "/us/usc/t99/s4",
                &["cog"],
                Means,
                Some("As used in section 2 of this title"),
                Some("/us/usc/t99/s2"),
                bases[3],
            ),
            definition(
                "/us/usc/t99/s5",
                &["gear"],
                Means,
                Some("In this chapter"),
                Some(CHAPTER),
                bases[4],
            ),
        ];
        for (definition, note) in definitions.iter_mut().zip(notes) {
            definition.see_note = note.map(address);
        }
        definitions
    }

    const FLAG_NO: &str = "<property role=\"is-positive-law\">no</property>";

    const NOTE_OF_S1: &str = "/us/usc/t99/s1/note/editorialNotes/nolaw/references-in-text";
    const NOTE_OF_S4: &str = "/us/usc/t99/s4/note/editorialNotes/nolaw/references-in-text";

    #[test]
    fn basis_follows_the_files_flag() {
        let text = fixture("definitions-restated");
        assert!(text.contains(FLAG_NO));
        let restated_bases = [Restated, Restated, Unresolved, Restated, Restated];
        let notes = [Some(NOTE_OF_S1), None, None, Some(NOTE_OF_S4), None];
        assert_eq!(definitions_of_text(&text), restated(restated_bases, notes));
        // A file without the flag declares no positive law either.
        assert_eq!(
            definitions_of_text(&text.replace(FLAG_NO, "")),
            restated(restated_bases, notes)
        );
        // With the flag on, a definition with a region is enacted, and it
        // points at no note.
        assert_eq!(
            definitions_of_text(
                &text.replace(FLAG_NO, "<property role=\"is-positive-law\">yes</property>")
            ),
            restated([Enacted, Enacted, Unresolved, Enacted, Enacted], [None; 5])
        );
    }

    #[test]
    fn the_references_in_text_note_is_linked() {
        let text = fixture("definitions-restated");
        let see_notes = |text: &str| -> Vec<Option<String>> {
            definitions_of_text(text)
                .into_iter()
                .map(|definition| definition.see_note.map(|note| note.as_str().to_owned()))
                .collect()
        };
        let some = |note: &str| Some(note.to_owned());
        // Section 2's note names the chapter, not the section; section 5's
        // words stand in quoted content and in a note of the statutory
        // group.
        assert_eq!(
            see_notes(&text),
            [some(NOTE_OF_S1), None, None, some(NOTE_OF_S4), None]
        );
        let first = "<p>This chapter, referred to in text, was";
        assert!(text.contains(first));
        // The words are compared in any case.
        assert_eq!(
            see_notes(&text.replace(first, "<p>THIS CHAPTER, REFERRED TO IN text, was"))[0],
            some(NOTE_OF_S1)
        );
        // The first note that holds them: here the second one.
        assert_eq!(
            see_notes(&text.replace(first, "<p>This chapter was"))[0],
            some(&format!("{NOTE_OF_S1}/2"))
        );
        // A note with another heading is not read.
        let heading = "<heading>References in Text</heading><p>Section 2 of this title";
        assert!(text.contains(heading));
        let changed = text.replace(
            heading,
            "<heading>Codification</heading><p>Section 2 of this title",
        );
        assert_eq!(see_notes(&changed)[3], None);
    }
}
