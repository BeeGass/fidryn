//! Definitions: the terms that the provisions of a unit define in one of a
//! closed list of forms, each with its kind, its reach, its condition when
//! the text states one, and, for a borrowed meaning, its source.
//!
//! These are the rules of the section "Definitions" of `docs/law-modules.md`,
//! in brief:
//!
//! - The text the rules read is a paragraph's runs in order, without its
//!   footnote markers and without its superscript runs made only of digits,
//!   with the one space that goes with each ([`Text::read_text`]). A term is
//!   text in the curly marks `“` and `”`.
//! - Paragraphs are read in the operative provisions only: the first block
//!   of a `chapeau` or `content` part; each later block of such a part, with
//!   the reach of the part's first block when it is a reach line and
//!   otherwise only with a reach of its own; and each block of a
//!   `continuation` or `proviso` part, only with a reach of its own. A
//!   paragraph is read at its start and again at the start of each later
//!   sentence.
//! - The sentence form has a lead (a reach; an exception and a lowercase
//!   reach; or a condition and an optional lowercase reach), head words such
//!   as `the term`, the terms, and a verb form, perhaps after an aside or a
//!   reach that follows the terms. The list form begins with the terms, or
//!   with an article and the terms.
//! - A reach is an opening and an Object. A condition (`In the case of`,
//!   `With respect to`, ...) may end the Object, begin the lead, stand in a
//!   chapeau, be the aside, or follow the verb form.
//! - A reading with no reach of its own takes the reach of its line, or that
//!   of the chapeau of the nearest ancestor that has one, passing the
//!   chapeaus that state a condition; any other chapeau leaves it unread.
//!   With no chapeau above, the sentence form has the words `unstated` and
//!   the list form is not read.
//! - A reading is not extracted when it defines a second term in its
//!   sentence, when its aside or the words after its verb speak of a reach,
//!   when it states a reach after its terms under a chapeau, when it has no
//!   reach and its text speaks of one, when it has two conditions, and when
//!   the source of a borrowed meaning is not read.
//! - A borrowed meaning records the target of the first link after its verb
//!   form, or, when there is none or the meaning is made by regulations
//!   under that link, the source words the text names.
//! - The region is the nearest provision or container of the kind that
//!   `this` and a kind name, and otherwise the target of the link whose
//!   words are the Object, when it lies below the prefix of the Code. The
//!   basis is `unresolved` without a region, `enacted` in a unit that is
//!   positive law and `restated` in any other, and a restated definition
//!   points at the first editorial note headed "References in Text" that
//!   speaks of the words naming its region.
//!
//! [`extract`] reads a unit whose provisions and notes are complete.

use crate::PREFIX;
use fidryn_provision::{
    Address, Basis, Block, Definition, DefinitionKind, Item, Layer, LinkPlace, Note, PartRole,
    Provision, ReadText, Run, Text, UnitModule,
};

/// The verb forms, each with the kind of definition it gives (D2.1).
const VERBS: [(&str, DefinitionKind); 55] = [
    ("means", DefinitionKind::Means),
    ("mean", DefinitionKind::Means),
    ("shall mean", DefinitionKind::Means),
    ("includes", DefinitionKind::Includes),
    ("include", DefinitionKind::Includes),
    ("shall include", DefinitionKind::Includes),
    ("also includes", DefinitionKind::Includes),
    ("also include", DefinitionKind::Includes),
    ("shall also include", DefinitionKind::Includes),
    ("does not include", DefinitionKind::DoesNotInclude),
    ("do not include", DefinitionKind::DoesNotInclude),
    ("shall not include", DefinitionKind::DoesNotInclude),
    ("does not mean", DefinitionKind::DoesNotInclude),
    ("do not mean", DefinitionKind::DoesNotInclude),
    ("shall not mean", DefinitionKind::DoesNotInclude),
    ("has the meaning given", DefinitionKind::Borrowed),
    ("have the meaning given", DefinitionKind::Borrowed),
    ("has the meanings given", DefinitionKind::Borrowed),
    ("have the meanings given", DefinitionKind::Borrowed),
    ("has the same meaning as", DefinitionKind::Borrowed),
    ("have the same meaning as", DefinitionKind::Borrowed),
    ("shall have the meaning given", DefinitionKind::Borrowed),
    ("has the meaning provided", DefinitionKind::Borrowed),
    ("have the meaning provided", DefinitionKind::Borrowed),
    ("has the meanings provided", DefinitionKind::Borrowed),
    ("have the meanings provided", DefinitionKind::Borrowed),
    ("has the meaning set forth", DefinitionKind::Borrowed),
    ("have the meaning set forth", DefinitionKind::Borrowed),
    ("have the meanings set forth", DefinitionKind::Borrowed),
    ("has the meaning prescribed", DefinitionKind::Borrowed),
    ("have the meaning prescribed", DefinitionKind::Borrowed),
    ("have the meanings prescribed", DefinitionKind::Borrowed),
    ("has the meaning stated", DefinitionKind::Borrowed),
    ("have the meanings stated", DefinitionKind::Borrowed),
    ("has the meaning assigned", DefinitionKind::Borrowed),
    ("have the meanings assigned", DefinitionKind::Borrowed),
    ("has the meaning described", DefinitionKind::Borrowed),
    ("has the meaning ascribed", DefinitionKind::Borrowed),
    ("have the meanings ascribed", DefinitionKind::Borrowed),
    ("has the meaning indicated", DefinitionKind::Borrowed),
    ("has the meaning applicable", DefinitionKind::Borrowed),
    ("has the meaning such term has", DefinitionKind::Borrowed),
    ("has the meaning that term has", DefinitionKind::Borrowed),
    ("has the meaning the term has", DefinitionKind::Borrowed),
    (
        "have the meanings such terms have",
        DefinitionKind::Borrowed,
    ),
    (
        "have the meanings that such terms have",
        DefinitionKind::Borrowed,
    ),
    ("has the meaning as in", DefinitionKind::Borrowed),
    ("have the meaning as in", DefinitionKind::Borrowed),
    ("has the same meanings as", DefinitionKind::Borrowed),
    ("have the same meanings as", DefinitionKind::Borrowed),
    ("has the same meaning given", DefinitionKind::Borrowed),
    ("have the same meaning given", DefinitionKind::Borrowed),
    ("have the same meanings given", DefinitionKind::Borrowed),
    ("shall have the same meaning as", DefinitionKind::Borrowed),
    ("shall have the meanings given", DefinitionKind::Borrowed),
];

/// The openings of a reach, in their order (D1.8).
const OPENINGS: [&str; 7] = [
    "As used in",
    "In",
    "For purposes of",
    "For the purposes of",
    "For the purpose of",
    "For purpose of",
    "When used in",
];

/// The lowercase openings, in the same order (D1.8).
const LOWERCASE_OPENINGS: [&str; 7] = [
    "as used in",
    "in",
    "for purposes of",
    "for the purposes of",
    "for the purpose of",
    "for purpose of",
    "when used in",
];

/// The abbreviations after whose period no sentence ends (D1.5).
const ABBREVIATIONS: [&str; 59] = [
    "U.S.C", "U.S", "App", "No", "Nos", "Stat", "Reg", "Fed", "Pub", "Sec", "Secs", "Co", "Inc",
    "Corp", "Ltd", "Jr", "Sr", "St", "Mt", "Ft", "Dr", "Mr", "Mrs", "Ms", "D.C", "seq", "etc",
    "e.g", "i.e", "Res", "Cong", "Sess", "Doc", "Rept", "H.R", "Ch", "Art", "Par", "Cl", "Pt",
    "Div", "Vol", "Supp", "Ct", "v", "Gen", "Gov", "Rev", "Admin", "Bros", "Assn", "Dept", "Govt",
    "Natl", "Ry", "R.R", "C.F.R", "F.R", "L",
];

/// The six phrases of a reach, looked for in any case (D1.9).
const REACH_PHRASES: [&str; 6] = [
    "as used in",
    "when used in",
    "for purposes of",
    "for the purposes of",
    "for the purpose of",
    "for purpose of",
];

/// The words of a verb phrase, as written (D1.10).
const VERB_PHRASE_WORDS: [&str; 16] = [
    "shall",
    "be",
    "is",
    "are",
    "may",
    "deemed",
    "construed",
    "held",
    "considered",
    "treated",
    "interpreted",
    "refer",
    "refers",
    "applies",
    "apply",
    "also",
];

/// The kind words, compared in any case (D1.11).
const KIND_WORDS: [&str; 31] = [
    "these",
    "section",
    "sections",
    "subsection",
    "subsections",
    "paragraph",
    "paragraphs",
    "subparagraph",
    "subparagraphs",
    "clause",
    "clauses",
    "subclause",
    "subclauses",
    "item",
    "items",
    "chapter",
    "chapters",
    "subchapter",
    "subchapters",
    "part",
    "parts",
    "subpart",
    "subparts",
    "title",
    "titles",
    "subtitle",
    "subtitles",
    "division",
    "divisions",
    "rule",
    "rules",
];

/// The kinds that name a region after `this` (D1.12).
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

/// The words the Object of a reach holds none of, in any case (D4.2).
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

/// The sense words, which state a sense in which a term is used (D4.4).
const SENSE_WORDS: [&str; 14] = [
    "a geographical sense",
    "a geographic sense",
    "the geographical sense",
    "the geographic sense",
    "a geographical context",
    "a geographic context",
    "geographical sense",
    "geographic sense",
    "their respective geographical",
    "connection with ",
    "relation to ",
    "reference to ",
    "the context of ",
    "the singular form",
];

/// The starts of an exception (D5.1) and of a lowercase exception (D5.2).
const EXCEPTION_STARTS: [&str; 4] = ["Except ", "Unless ", "Notwithstanding ", "Subject to "];
const LOWERCASE_EXCEPTION_STARTS: [&str; 2] = ["except ", "unless "];

/// The starts of a condition, besides `When used in ` and a sense word, and
/// the same with a lowercase first letter (D6.1).
const CONDITION_STARTS: [&str; 4] = [
    "In the case of ",
    "In any case in which ",
    "In the context of ",
    "With respect to ",
];
const LOWERCASE_CONDITION_STARTS: [&str; 4] = [
    "in the case of ",
    "in any case in which ",
    "in the context of ",
    "with respect to ",
];

/// The asides that are a condition as they are, and the starts of the
/// others, in the order they are tried (D6.5).
const ASIDE_CONDITIONS: [&str; 4] = [
    "in a geographical sense",
    "in a geographic sense",
    "in the geographical sense",
    "in the geographic sense",
];
const ASIDE_CONDITION_STARTS: [&str; 17] = [
    "in the case of ",
    "in any case in which ",
    "in the context of ",
    "in connection with ",
    "in relation to ",
    "in reference to ",
    "in case ",
    "with respect to ",
    "when used with respect to ",
    "used with respect to ",
    "as used with respect to ",
    "with regard to ",
    "with reference to ",
    "as applied to ",
    "when applied to ",
    "when referring to ",
    "for any ",
];

/// What follows `in ` at the start of a condition after the verb form, as
/// written (D6.6.2).
const CONDITIONS_AFTER_IN: [&str; 15] = [
    "the case of ",
    "case ",
    "any case in ",
    "connection with ",
    "relation to ",
    "reference to ",
    "respect of ",
    "the context of ",
    "a geographical sense",
    "a geographic sense",
    "the geographical sense",
    "the geographic sense",
    "interstate cases",
    "an individual capacity",
    "appropriate circumstances",
];

/// What the rest of the sentence after a condition after the verb form
/// holds none of, in any case (D6.6.4).
const AFTER_A_CONDITION: [&str; 9] = [
    "in the case of",
    "in any case in which",
    "in the context of",
    "in connection with",
    "in relation to",
    "in reference to",
    "in respect of",
    "with respect to",
    "when used in",
];

/// The nouns of the head words after `the` or `The` (D7.3).
const HEAD_NOUNS: [&str; 5] = ["term", "terms", "word", "words", "phrase"];
/// The head words of the second kind (D7.3).
const EACH_OF_THE_TERMS: [&str; 2] = ["Each of the terms ", "each of the terms "];

/// What stands between two terms of the first form (D7.6).
const SEPARATORS: [&str; 5] = [", and ", ", or ", ", ", " and ", " or "];

/// The articles skipped before the list form at the start of a text (D7.5).
const ARTICLES: [&str; 6] = ["The ", "the ", "A ", "a ", "An ", "an "];
/// What may stand between the lead's comma and its space and a term that no
/// head words precede, besides nothing (D7.3.4).
const ARTICLES_BEFORE_A_TERM: [&str; 3] = ["the ", "a ", "an "];

/// The starts of an aside with no commas around it (D7.8.4).
const BARE_ASIDE_STARTS: [&str; 9] = [
    "with respect to ",
    "when used with respect to ",
    "used with respect to ",
    "when applied to ",
    "as applied to ",
    "with reference to ",
    "in the case of ",
    "for any ",
    "when referring to ",
];

/// The words by which an aside speaks of a reach, besides those of D11.5,
/// in any case (D11.3).
const ASIDE_REACH_WORDS: [&str; 6] = [
    "as used by",
    "used herein",
    "where used",
    "wherever used",
    "purpose",
    "purposes",
];

/// What, after `in `, makes text that begins `in ` speak of no reach, in
/// any case (D11.5).
const NOT_A_REACH_AFTER_IN: [&str; 16] = [
    "the case of ",
    "case ",
    "any case in ",
    "connection with ",
    "relation to ",
    "reference to ",
    "respect of ",
    "addition to ",
    "the context of ",
    "a geographical sense",
    "a geographic sense",
    "the geographical sense",
    "the geographic sense",
    "interstate cases",
    "an individual capacity",
    "appropriate circumstances",
];

/// The lead-ins between a reach and its close: the first lead-ins, then the
/// later ones (D8.1).
const FIRST_LEAD_INS: [&str; 4] = [
    "the term",
    "the terms",
    "the following definitions apply",
    "the following definitions shall apply",
];
const LATER_LEAD_INS: [&str; 13] = [
    "the following terms have the following meanings",
    "the following terms shall have the following meanings",
    "the following terms mean the following",
    "the following terms apply",
    "the following words and phrases have these meanings",
    "the following definition applies",
    "the following definition shall apply",
    "the following words or phrases shall have the meaning indicated",
    "the following terms shall have the meanings stated below",
    "the following terms shall have the meanings ascribed to them below",
    "the following terms shall have the meanings respectively ascribed to them below",
    "terms have the following meanings",
    "the term or terms",
];

/// The closes of a reach line: an em dash, a hyphen and a colon (D1.7).
const CLOSES: [char; 3] = ['\u{2014}', '-', ':'];

/// The prepositions of a line that says the following definitions apply
/// (D8.1.3).
const APPLY_PREPOSITIONS: [&str; 4] = ["in", "to", "for purposes of", "for the purposes of"];

/// The words of a meaning made by regulations, rules or an order (D12.4).
const MADE_BY: [&str; 6] = [
    "regulation",
    "regulations",
    "rule",
    "rules",
    "order",
    "orders",
];

/// The parts of the form that leads to the source words (D12.5): the
/// alternatives in the order written, the lists longest first.
const SOURCE_PAST: [&str; 3] = [" given", " provided", " set forth"];
const SOURCE_TO: [&str; 2] = [" to", " for"];
const SOURCE_POINTERS: [&str; 11] = [
    "each such term",
    "those terms",
    "these terms",
    "such terms",
    "such term",
    "the terms",
    "that term",
    "this term",
    "the term",
    "them",
    "it",
];
const SOURCE_HAS: [&str; 4] = [" has", " have", " does", " do"];
const SOURCE_PREPOSITIONS: [&str; 9] = [
    "for the purposes of",
    "for purposes of",
    "when used in",
    "pursuant to",
    "as used in",
    "as in",
    "under",
    "in",
    "by",
];
/// The strings at which the source words end, as characters (D12.5).
const SOURCE_STOPS: [&str; 16] = [
    ";",
    "\u{2014}",
    ":",
    ", except",
    " except ",
    ", and includes",
    " and includes",
    ", and does not include",
    " and does not include",
    ", but ",
    " but ",
    ", and shall include",
    " and shall include",
    ", and also includes",
    " and also includes",
    ", subject to",
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
/// order in which the rules read them. `containers` are the unit's
/// containers from the nearest outward, each at its address and with its
/// kind.
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

/// Where a paragraph is read (D3.2).
enum Place<'l> {
    /// The first block of a `chapeau` or `content` part.
    Ordinary,
    /// A later block of a part whose first block is a reach line, with that
    /// line.
    Line(&'l Line),
    /// A later block of any other such part, and a block of a
    /// `continuation` or `proviso` part: only a reach the paragraph states
    /// is read.
    StatedOnly,
}

impl<'u> Reading<'u> {
    fn provision(&mut self, provision: &'u Provision) {
        let order = self.next;
        self.next += 1;
        self.stack.push(provision);
        for item in &provision.items {
            let Item::Part { role, blocks } = item else {
                continue;
            };
            match role {
                PartRole::Chapeau | PartRole::Content => {
                    // The line of the part's first block, read when a later
                    // block needs it.
                    let mut line: Option<Option<Line>> = None;
                    for (index, block) in blocks.iter().enumerate() {
                        let Block::Para(text) = block else {
                            continue;
                        };
                        if index == 0 {
                            self.paragraph(order, text, &Place::Ordinary);
                            continue;
                        }
                        let line = line.get_or_insert_with(|| match blocks.first() {
                            Some(Block::Para(first)) => reach_line(&Paragraph::of(first), 0, true),
                            _ => None,
                        });
                        match line {
                            Some(line) => self.paragraph(order, text, &Place::Line(line)),
                            None => self.paragraph(order, text, &Place::StatedOnly),
                        }
                    }
                }
                PartRole::Continuation | PartRole::Proviso => {
                    for block in blocks {
                        if let Block::Para(text) = block {
                            self.paragraph(order, text, &Place::StatedOnly);
                        }
                    }
                }
            }
        }
        for item in &provision.items {
            if let Item::Child(child) = item {
                self.provision(child);
            }
        }
        self.stack.pop();
    }

    /// Adds the definitions of the paragraph `text` of the provision at the
    /// top of the stack, the `order`th in the document, read at `place`: at
    /// its start, and at the start of each later sentence (D3.3).
    fn paragraph(&mut self, order: usize, text: &Text, place: &Place<'_>) {
        let holds_a_mark = text.runs.iter().any(|run| match run {
            Run::Words(words) | Run::Ref { words, .. } | Run::Sup(words) | Run::Sub(words) => {
                words.contains(OPEN)
            }
            Run::Mark { .. } => false,
        });
        if !holds_a_mark {
            // Every reading begins its terms with an opening mark.
            return;
        }
        let paragraph = Paragraph::of(text);
        for (index, &(start, _)) in paragraph.sentences.iter().enumerate() {
            if let Some(definition) = self.reading(&paragraph, start, index > 0, place) {
                self.found.push((order, definition));
            }
        }
    }

    /// The definition that the reading of `paragraph` at `start` gives at
    /// `place`, when the rules extract one; `later` when the start is that
    /// of a later sentence. The steps are those of D14.
    fn reading(
        &self,
        paragraph: &Paragraph<'_>,
        start: usize,
        later: bool,
        place: &Place<'_>,
    ) -> Option<Definition> {
        let text = paragraph.text();
        let found = find(paragraph, start, later)?;
        // A later reading with no reach of its own (D10.2).
        if later
            && found.reach.is_none()
            && (matches!(place, Place::StatedOnly)
                || holds_a_reach_phrase(&text[..start])
                || text[..start].to_ascii_lowercase().contains("in this "))
        {
            return None;
        }
        // A second term (D11.1), an aside that speaks of a reach (D11.2), a
        // reach after the verb (D11.4).
        let (_, verb_sentence_end) = paragraph.sentence_of(found.verb_start);
        if defines_a_second_term(text, found.verb_end, verb_sentence_end, &found.terms) {
            return None;
        }
        if let Some((from, to)) = found.aside
            && aside_speaks_of_a_reach(&text[from..to])
        {
            return None;
        }
        if reach_after_the_verb(text, found.verb_end) {
            return None;
        }
        let own = self.stack.len() - 1;
        // A reach after the terms, under a chapeau or at a line place
        // (D11.7).
        if found.reach_after_terms
            && (matches!(place, Place::Line(_))
                || self.stack[..own]
                    .iter()
                    .any(|provision| has_chapeau(provision)))
        {
            return None;
        }
        // The reach and the condition (D9).
        let (reach, holder, condition) = self.reach_and_condition(&found, place)?;
        let condition = condition.or_else(|| {
            match found.aside {
                Some((from, to)) if aside_is_a_condition(&text[from..to]) => {
                    Some(text[from..to].to_owned())
                }
                _ => None,
            }
            .or_else(|| condition_after_the_verb(text, found.verb_end, verb_sentence_end))
        });
        // No reach, and the text speaks of one (D11.6).
        if reach.is_none() && speaks_of_a_reach_with_none(text) {
            return None;
        }
        // The source of a borrowed meaning (D12).
        let (borrowed_from, borrowed_words) = match found.kind {
            DefinitionKind::Borrowed => {
                let end = if later {
                    paragraph.sentence_of(start).1
                } else {
                    text.len()
                };
                borrowed(paragraph, &found, end)?
            }
            _ => (None, None),
        };
        // The region, the basis and the note (D13).
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
            terms: found.terms,
            kind: found.kind,
            words: reach.map(|reach| reach.words),
            condition,
            region: region.map(|(address, _)| address),
            basis,
            borrowed_from,
            borrowed_words,
            see_note,
        })
    }

    /// The reach of a reading, the place in the stack of its holder, and its
    /// condition (D9.1 to D9.5); `None` when nothing is extracted.
    fn reach_and_condition(
        &self,
        found: &Found,
        place: &Place<'_>,
    ) -> Option<(Option<Reach>, usize, Option<String>)> {
        let own = self.stack.len() - 1;
        if let Some(reach) = &found.reach {
            return Some((Some(reach.clone()), own, found.condition.clone()));
        }
        match place {
            Place::Line(line) => {
                if line.condition.is_some() && found.condition.is_some() {
                    return None;
                }
                Some((
                    Some(line.reach.clone()),
                    own,
                    line.condition.clone().or_else(|| found.condition.clone()),
                ))
            }
            Place::StatedOnly => None,
            Place::Ordinary => {
                let mut condition = found.condition.clone();
                let mut below = own;
                loop {
                    let Some(at) = (0..below).rev().find(|&at| has_chapeau(self.stack[at])) else {
                        return (!found.list).then_some((None, own, condition));
                    };
                    match chapeau_of(self.stack[at]) {
                        Chapeau::Reach(line) => {
                            if line.condition.is_some() && condition.is_some() {
                                return None;
                            }
                            return Some((Some(line.reach), at, line.condition.or(condition)));
                        }
                        Chapeau::Condition(stated) => {
                            if condition.is_some() {
                                return None;
                            }
                            condition = Some(stated);
                            below = at;
                        }
                        Chapeau::Other => return None,
                    }
                }
            }
        }
    }
}

/// The text of a paragraph that the rules read, the places of its links,
/// and its sentences.
struct Paragraph<'t> {
    read: ReadText<'t>,
    /// Each sentence, from its start to its end: just after its period, or
    /// the end of the text (D1.6).
    sentences: Vec<(usize, usize)>,
}

impl<'t> Paragraph<'t> {
    fn of(text: &'t Text) -> Self {
        let read = text.read_text();
        let sentences = sentences(&read.text);
        Paragraph { read, sentences }
    }

    fn text(&self) -> &str {
        &self.read.text
    }

    fn links(&self) -> &[LinkPlace<'t>] {
        &self.read.links
    }

    /// The start and the end of the sentence that holds the byte `at`.
    fn sentence_of(&self, at: usize) -> (usize, usize) {
        self.sentences
            .iter()
            .rev()
            .find(|(start, _)| *start <= at)
            .copied()
            .unwrap_or((0, self.read.text.len()))
    }
}

/// A reach as a text states it.
#[derive(Clone)]
struct Reach {
    /// The text from the opening to the end of the Object (D4.6).
    words: String,
    object: String,
    /// The target of the link whose place is the Object's (D4.7).
    link: Option<String>,
}

/// A reach line: its reach and the condition it states (D8.1).
struct Line {
    reach: Reach,
    condition: Option<String>,
}

/// What a lead states (D7.2).
struct Lead {
    reach: Option<Reach>,
    condition: Option<String>,
}

impl Lead {
    fn none() -> Self {
        Lead {
            reach: None,
            condition: None,
        }
    }
}

/// A reading that matches a pattern (D7).
struct Found {
    list: bool,
    /// The reach the reading states, by its lead or after its terms.
    reach: Option<Reach>,
    /// Whether that reach stands after the terms (D7.8.2, D7.8.3).
    reach_after_terms: bool,
    /// The condition of the lead.
    condition: Option<String>,
    terms: Vec<String>,
    kind: DefinitionKind,
    verb: &'static str,
    verb_start: usize,
    verb_end: usize,
    /// The place of the aside, when there is one (D7.8.2, D7.8.4).
    aside: Option<(usize, usize)>,
}

/// Whether `byte` is one of the letters `A` to `Z` and `a` to `z`.
fn is_letter(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
}

/// Whether `text` holds `pattern` at the byte `at`, as written.
fn at(text: &str, at: usize, pattern: &str) -> bool {
    text.as_bytes()
        .get(at..)
        .is_some_and(|bytes| bytes.starts_with(pattern.as_bytes()))
}

/// Whether `text` holds `pattern` at the byte `at`, the letters `A` to `Z`
/// compared as `a` to `z` and no other character changed.
fn at_in_any_case(text: &str, at: usize, pattern: &str) -> bool {
    text.as_bytes()
        .get(at..at + pattern.len())
        .is_some_and(|bytes| bytes.eq_ignore_ascii_case(pattern.as_bytes()))
}

/// Whether the byte `at` of `text` is the start of the text or follows a
/// character that is not a letter.
fn no_letter_before(text: &str, at: usize) -> bool {
    at == 0 || !is_letter(text.as_bytes()[at - 1])
}

/// Whether the bytes `start..end` of `text` stand as words: no letter just
/// before them and none just after them (D1.4).
fn stands(text: &str, start: usize, end: usize) -> bool {
    no_letter_before(text, start)
        && text
            .as_bytes()
            .get(end)
            .is_none_or(|&byte| !is_letter(byte))
}

/// The bytes at which `words` stand in `text`, in any case or as written.
fn standing(text: &str, words: &str, any_case: bool) -> impl Iterator<Item = usize> {
    (0..=text.len().saturating_sub(words.len())).filter(move |&start| {
        let found = if any_case {
            at_in_any_case(text, start, words)
        } else {
            at(text, start, words)
        };
        found && stands(text, start, start + words.len())
    })
}

/// Whether `words` stand in `text`, as written.
fn holds(text: &str, words: &str) -> bool {
    standing(text, words, false).next().is_some()
}

/// Whether `words` stand in `text`, in any case.
fn holds_in_any_case(text: &str, words: &str) -> bool {
    standing(text, words, true).next().is_some()
}

/// Whether `text` holds one of the six phrases of a reach, in any case.
fn holds_a_reach_phrase(text: &str) -> bool {
    REACH_PHRASES
        .iter()
        .any(|phrase| holds_in_any_case(text, phrase))
}

/// Whether `text` holds a word of a verb phrase, as written.
fn holds_a_verb_phrase_word(text: &str) -> bool {
    VERB_PHRASE_WORDS.iter().any(|word| holds(text, word))
}

/// The verb form that stands at the byte `at` of `text`, the longest when
/// several do: its end, its kind and its words (D2.2).
fn verb_at(text: &str, at: usize) -> Option<(usize, DefinitionKind, &'static str)> {
    if !text.is_char_boundary(at) {
        return None;
    }
    VERBS
        .iter()
        .filter(|(form, _)| self::at(text, at, form) && stands(text, at, at + form.len()))
        .map(|(form, kind)| (at + form.len(), *kind, *form))
        .max_by_key(|(end, _, _)| *end)
}

/// Whether a verb form stands anywhere in `text` (D2.4).
fn holds_a_verb(text: &str) -> bool {
    (0..text.len()).any(|at| verb_at(text, at).is_some())
}

/// The word before the period at the byte `period` of `text`: the longest
/// run of letters and periods that ends just before it, begins with a
/// letter and lies within the 12 characters before it (D1.5).
fn word_before(text: &str, period: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut start = period;
    while start > 0
        && period - (start - 1) <= 12
        && (is_letter(bytes[start - 1]) || bytes[start - 1] == b'.')
    {
        start -= 1;
    }
    while start < period && bytes[start] == b'.' {
        start += 1;
    }
    (start < period).then(|| &text[start..period])
}

/// Where the sentence after an end of a sentence at the period at the byte
/// `period` of `text` begins, when one ends there (D1.5).
fn end_of_a_sentence(text: &str, period: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(period) != Some(&b'.') {
        return None;
    }
    let mut next = period + 1;
    if at(text, next, "\u{201d}") {
        next += CLOSE.len_utf8();
    } else if bytes.get(next) == Some(&b')') {
        next += 1;
    }
    if bytes.get(next) != Some(&b' ') || !bytes.get(next + 1).is_some_and(u8::is_ascii_uppercase) {
        return None;
    }
    if let Some(word) = word_before(text, period)
        && (ABBREVIATIONS.contains(&word)
            || (word.len() == 1 && word.as_bytes()[0].is_ascii_uppercase()))
    {
        return None;
    }
    Some(next + 1)
}

/// Whether `text` holds an end of a sentence, judged on its own characters.
fn holds_an_end_of_a_sentence(text: &str) -> bool {
    (0..text.len()).any(|period| end_of_a_sentence(text, period).is_some())
}

/// The sentences of `text`, each from its start to its end (D1.6).
fn sentences(text: &str) -> Vec<(usize, usize)> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut period = 0;
    while period < text.len() {
        if let Some(next) = end_of_a_sentence(text, period) {
            sentences.push((start, period + 1));
            start = next;
            period = next;
        } else {
            period += 1;
        }
    }
    sentences.push((start, text.len()));
    sentences
}

/// Whether each comma of `text` that is followed by a space and a letter
/// `a` to `z` is followed by a space and the word `and` or `or`.
fn commas_are_clean(text: &str) -> bool {
    let bytes = text.as_bytes();
    (0..bytes.len()).all(|comma| {
        bytes[comma] != b','
            || bytes.get(comma + 1) != Some(&b' ')
            || !bytes.get(comma + 2).is_some_and(u8::is_ascii_lowercase)
            || ["and", "or"].iter().any(|word| {
                at(text, comma + 2, word) && stands(text, comma + 2, comma + 2 + word.len())
            })
    })
}

/// Whether `text` is an exception (D5.1) or, with `lowercase`, a lowercase
/// exception (D5.2).
fn is_exception(text: &str, lowercase: bool) -> bool {
    let starts: &[&str] = if lowercase {
        &LOWERCASE_EXCEPTION_STARTS
    } else {
        &EXCEPTION_STARTS
    };
    starts.iter().any(|start| at(text, 0, start))
        && !text.contains(MARKS)
        && !holds_a_verb(text)
        && !holds_a_reach_phrase(text)
        && !holds_an_end_of_a_sentence(text)
        && !at_in_any_case(text, 0, "Except in ")
        && commas_are_clean(text)
}

/// The length of the condition start that `text` holds at the byte `at`,
/// with a capital or, with `lowercase`, a lowercase first letter (D6.1).
/// For `When used in ` and a sense word, the start is `When used in `.
fn condition_start(text: &str, at: usize, lowercase: bool) -> Option<usize> {
    let (starts, when_used): (&[&str], &str) = if lowercase {
        (&LOWERCASE_CONDITION_STARTS, "when used in ")
    } else {
        (&CONDITION_STARTS, "When used in ")
    };
    if let Some(start) = starts.iter().find(|start| self::at(text, at, start)) {
        return Some(start.len());
    }
    (self::at(text, at, when_used)
        && SENSE_WORDS
            .iter()
            .any(|sense| self::at(text, at + when_used.len(), sense)))
    .then_some(when_used.len())
}

/// Whether `after`, the words after the start of a condition, are as a
/// condition's must be (D6.3).
fn condition_words(after: &str) -> bool {
    !after.is_empty()
        && !after.starts_with(' ')
        && !after.ends_with(' ')
        && !after.contains(MARKS)
        && !holds_a_verb(after)
        && !holds_a_reach_phrase(after)
        && !holds_an_end_of_a_sentence(after)
        && !holds(after, "shall")
        && commas_are_clean(after)
}

/// Whether `text` is a condition with a capital or, with `lowercase`, a
/// lowercase start (D6.3).
fn is_condition(text: &str, lowercase: bool) -> bool {
    condition_start(text, 0, lowercase).is_some_and(|start| condition_words(&text[start..]))
}

/// Whether the aside `aside` is a condition (D6.5).
fn aside_is_a_condition(aside: &str) -> bool {
    if ASIDE_CONDITIONS.contains(&aside) {
        return true;
    }
    let when_used = "when used in ";
    if at(aside, 0, when_used)
        && SENSE_WORDS
            .iter()
            .any(|sense| at(aside, when_used.len(), sense))
    {
        return condition_words(&aside[when_used.len()..]);
    }
    ASIDE_CONDITION_STARTS
        .iter()
        .find(|start| at(aside, 0, start))
        .is_some_and(|start| condition_words(&aside[start.len()..]))
}

/// The condition after the verb form that ends at the byte `verb_end` of
/// `text`, in a sentence that ends at `sentence_end`, when there is one
/// (D6.6).
fn condition_after_the_verb(text: &str, verb_end: usize, sentence_end: usize) -> Option<String> {
    if !at(text, verb_end, ", ") {
        return None;
    }
    let start = verb_end + 2;
    let comma = start + text[start..].find(',')?;
    if comma >= sentence_end || !at(text, comma + 1, " ") {
        return None;
    }
    let condition = &text[start..comma];
    let when_used = "when used in ";
    let after = if at(condition, 0, when_used)
        && SENSE_WORDS
            .iter()
            .any(|sense| at(condition, when_used.len(), sense))
    {
        when_used.len()
    } else if at(condition, 0, "in ")
        && CONDITIONS_AFTER_IN
            .iter()
            .any(|words| at(condition, 3, words))
    {
        3
    } else {
        return None;
    };
    if !condition_words(&condition[after..]) || condition.contains('\u{2014}') {
        return None;
    }
    let rest = &text[comma + 2..sentence_end.max(comma + 2)];
    let word = rest.find([' ', ',']).unwrap_or(rest.len());
    if at(rest, 0, "or ")
        || at(rest, 0, "and ")
        || (word > 0 && at(rest, word, ","))
        || AFTER_A_CONDITION
            .iter()
            .any(|words| holds_in_any_case(rest, words))
    {
        return None;
    }
    Some(condition.to_owned())
}

/// The reach that the bytes `from..to` of `paragraph` state, with a capital
/// opening or, with `lowercase`, a lowercase one, and the condition at the
/// end of its Object (D4).
fn reach(
    paragraph: &Paragraph<'_>,
    from: usize,
    to: usize,
    lowercase: bool,
) -> Option<(Reach, Option<String>)> {
    let text = paragraph.text();
    let openings: &[&str] = if lowercase {
        &LOWERCASE_OPENINGS
    } else {
        &OPENINGS
    };
    let opening = openings
        .iter()
        .find(|opening| at(text, from, opening) && at(text, from + opening.len(), " "))?;
    let object = from + opening.len() + 1;
    if object > to {
        return None;
    }
    // A sense after `When used in` (D4.4).
    if opening.eq_ignore_ascii_case("when used in")
        && SENSE_WORDS.iter().any(|sense| at(text, object, sense))
    {
        return None;
    }
    // A condition at the end of the Object (D4.5).
    if let Some(comma) = (object..to).find(|&comma| {
        at(text, comma, ", ")
            && condition_start(text, comma + 2, true).is_some_and(|start| comma + 2 + start <= to)
    }) && comma > object
        && is_condition(&text[comma + 2..to], true)
        && let Some(reach) = whole_reach(paragraph, from, opening, comma)
    {
        return Some((reach, Some(text[comma + 2..to].to_owned())));
    }
    whole_reach(paragraph, from, opening, to).map(|reach| (reach, None))
}

/// The reach of the bytes `from..to` of `paragraph`, which begin with
/// `opening` and a space, when its Object, read whole, is one (D4.2, D4.3).
fn whole_reach(paragraph: &Paragraph<'_>, from: usize, opening: &str, to: usize) -> Option<Reach> {
    let text = paragraph.text();
    let start = from + opening.len() + 1;
    let object = &text[start..to];
    if object.is_empty()
        || object.contains(MARKS)
        || NOT_IN_AN_OBJECT
            .iter()
            .any(|word| holds_in_any_case(object, word))
        || holds_an_end_of_a_sentence(object)
    {
        return None;
    }
    let link = paragraph
        .links()
        .iter()
        .find(|link| link.start == start && link.end == to)
        .map(|link| link.target.to_owned());
    if opening.eq_ignore_ascii_case("in")
        && !at(object, 0, "this ")
        && link.is_none()
        && !(begins_with_this_or_a_kind_word(object) && !at(object, 0, "such "))
    {
        return None;
    }
    Some(Reach {
        words: text[from..to].to_owned(),
        object: object.to_owned(),
        link,
    })
}

/// Whether `object` begins, in any case, with `this`, `these`, `such` or a
/// kind word, and a space.
fn begins_with_this_or_a_kind_word(object: &str) -> bool {
    ["this", "such"]
        .iter()
        .chain(KIND_WORDS.iter())
        .any(|word| at_in_any_case(object, 0, word) && at(object, word.len(), " "))
}

/// The bytes at which `text` holds a comma and a space that both lie from
/// `from` to `to`, in order.
fn commas(text: &str, from: usize, to: usize) -> impl Iterator<Item = usize> {
    (from..to.saturating_sub(1)).filter(move |&comma| at(text, comma, ", "))
}

/// What the lead at the bytes `from..to` of `paragraph` states, when it is
/// read (D7.2).
fn lead(paragraph: &Paragraph<'_>, from: usize, to: usize) -> Option<Lead> {
    let text = paragraph.text();
    if let Some((reach, condition)) = reach(paragraph, from, to, false) {
        return Some(Lead {
            reach: Some(reach),
            condition,
        });
    }
    if EXCEPTION_STARTS.iter().any(|start| at(text, from, start)) {
        if let Some(lead) = exception_and_reach(paragraph, from, to) {
            return Some(lead);
        }
        return is_exception(&text[from..to], false).then(Lead::none);
    }
    if condition_start(text, from, false).is_some() {
        if let Some(lead) = condition_and_reach(paragraph, from, to) {
            return Some(lead);
        }
        return is_condition(&text[from..to], false).then(|| Lead {
            reach: None,
            condition: Some(text[from..to].to_owned()),
        });
    }
    None
}

/// The lead at the bytes `from..to` of `paragraph` read as an exception, a
/// comma, a space and a lowercase reach, at the first comma that gives one
/// (D7.2.2).
fn exception_and_reach(paragraph: &Paragraph<'_>, from: usize, to: usize) -> Option<Lead> {
    let text = paragraph.text();
    commas(text, from, to).find_map(|comma| {
        let (reach, condition) = reach(paragraph, comma + 2, to, true)?;
        is_exception(&text[from..comma], false).then_some(Lead {
            reach: Some(reach),
            condition,
        })
    })
}

/// The lead at the bytes `from..to` of `paragraph` read as a condition, a
/// comma, a space and a lowercase reach with no condition of its own, at
/// the first comma that gives one (D7.2.3).
fn condition_and_reach(paragraph: &Paragraph<'_>, from: usize, to: usize) -> Option<Lead> {
    let text = paragraph.text();
    commas(text, from, to).find_map(|comma| match reach(paragraph, comma + 2, to, true) {
        Some((reach, None)) if is_condition(&text[from..comma], false) => Some(Lead {
            reach: Some(reach),
            condition: Some(text[from..comma].to_owned()),
        }),
        _ => None,
    })
}

/// The length of the head words of the first kind at the byte `at` of
/// `text`: `the` or `The`, a space, a noun and a space (D7.3).
fn head_of_the_first_kind(text: &str, at: usize) -> Option<usize> {
    if !self::at(text, at, "the ") && !self::at(text, at, "The ") {
        return None;
    }
    HEAD_NOUNS
        .iter()
        .find(|noun| self::at(text, at + 4, noun) && self::at(text, at + 4 + noun.len(), " "))
        .map(|noun| 4 + noun.len() + 1)
}

/// The length of the head words of the second kind at the byte `at` of
/// `text` (D7.3).
fn head_of_the_second_kind(text: &str, at: usize) -> Option<usize> {
    EACH_OF_THE_TERMS
        .iter()
        .find(|words| self::at(text, at, words))
        .map(|words| words.len())
}

/// The length of the head words of either kind at the byte `at` of `text`.
fn head_words(text: &str, at: usize) -> Option<usize> {
    head_of_the_first_kind(text, at).or_else(|| head_of_the_second_kind(text, at))
}

/// The lead of the sentence form read at `start` of `paragraph`, and where
/// its terms begin (D7.3).
fn sentence_lead(paragraph: &Paragraph<'_>, start: usize) -> Option<(Lead, usize)> {
    let text = paragraph.text();
    let mark = text[start..]
        .find(MARKS)
        .map_or(text.len(), |offset| start + offset);
    // Head words at the start (D7.3.1).
    if let Some(head) = head_words(text, start) {
        return Some((Lead::none(), start + head));
    }
    // A comma and a space, then head words (D7.3.2).
    let head_after_comma = |kind: fn(&str, usize) -> Option<usize>| {
        commas(text, start, mark).find_map(|comma| {
            kind(text, comma + 2)
                .filter(|head| comma + 2 + head <= mark)
                .map(|head| (comma, head))
        })
    };
    if let Some((comma, head)) = head_after_comma(head_of_the_first_kind)
        .or_else(|| head_after_comma(head_of_the_second_kind))
    {
        return Some((lead(paragraph, start, comma)?, comma + 2 + head));
    }
    // A space, then head words of the first kind (D7.3.3).
    if let Some((space, head)) = (start..mark).find_map(|space| {
        if text.as_bytes()[space] != b' ' {
            return None;
        }
        head_of_the_first_kind(text, space + 1)
            .filter(|head| space + 1 + head <= mark)
            .map(|head| (space, head))
    }) {
        let (reach, condition) = reach(paragraph, start, space, false)?;
        if condition.is_some() || reach.object.contains(',') {
            return None;
        }
        return Some((
            Lead {
                reach: Some(reach),
                condition: None,
            },
            space + 1 + head,
        ));
    }
    // No head words before the first quotation mark (D7.3.4).
    if (start..mark).any(|at| head_words(text, at).is_some() && no_letter_before(text, at)) {
        return None;
    }
    if !at(text, mark, "\u{201c}") {
        return None;
    }
    let before = &text[start..mark];
    let comma = ARTICLES_BEFORE_A_TERM
        .iter()
        .map(|article| format!(", {article}"))
        .chain(std::iter::once(", ".to_owned()))
        .find(|end| before.ends_with(end.as_str()))
        .map(|end| mark - end.len())?;
    let lead = lead(paragraph, start, comma)?;
    lead.reach.is_some().then_some((lead, mark))
}

/// The term in quotation marks that begins at the byte `at` of `text`,
/// without the marks, and where its closing mark ends (D1.2).
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

/// The terms of the first form at the byte `at` of `text`, a term and then
/// as many separators each followed by a term as follow, and where the last
/// term ends (D7.6).
fn first_form(text: &str, at: usize) -> Option<(Vec<&str>, usize)> {
    let (first, mut end) = quoted(text, at)?;
    let mut terms = vec![first];
    'list: loop {
        for separator in SEPARATORS {
            if self::at(text, end, separator)
                && let Some((term, after)) = quoted(text, end + separator.len())
            {
                terms.push(term);
                end = after;
                continue 'list;
            }
        }
        return Some((terms, end));
    }
}

/// The terms of the second form at the byte `at` of `text`: one or more
/// terms that end with a comma, each followed by a space, then optionally
/// `and ` or `or `, then a term (D7.6).
fn second_form(text: &str, at: usize) -> Option<(Vec<&str>, usize)> {
    let mut terms: Vec<(&str, usize)> = Vec::new();
    let mut next = at;
    while let Some((term, end)) = quoted(text, next)
        && term.ends_with(',')
        && self::at(text, end, " ")
    {
        terms.push((term, end));
        next = end + 1;
    }
    if terms.is_empty() {
        return None;
    }
    for word in ["and ", "or ", ""] {
        if self::at(text, next, word)
            && let Some((term, end)) = quoted(text, next + word.len())
        {
            terms.push((term, end));
            return Some((terms.iter().map(|(term, _)| *term).collect(), end));
        }
    }
    // The last term that ends with a comma is the last term.
    (terms.len() >= 2).then(|| {
        let end = terms[terms.len() - 1].1;
        (terms.iter().map(|(term, _)| *term).collect(), end)
    })
}

/// A term as it is recorded: without the comma that ends a term longer
/// than one character, taken off once (D7.6), and then without the spaces
/// and stray marks of nested quotation at its ends (D7.7).
fn recorded(term: &str) -> String {
    let term = match term.strip_suffix(',') {
        Some(rest) if term.chars().count() > 1 => rest,
        _ => term,
    };
    let ended_with_a_space = term.ends_with(' ');
    let mut trimmed = term.trim_matches(' ');
    if ended_with_a_space
        && !trimmed.contains('\u{2018}')
        && let Some(rest) = trimmed.strip_suffix('\u{2019}')
    {
        trimmed = rest;
    }
    if !trimmed.contains('\u{2019}')
        && let Some(rest) = trimmed.strip_prefix('\u{2018}')
    {
        trimmed = rest;
    }
    if trimmed.is_empty() { term } else { trimmed }.to_owned()
}

/// The terms at the byte `at` of `text`, as they are recorded, and where
/// the last ends (D7.6, D7.7).
fn terms(text: &str, at: usize) -> Option<(Vec<String>, usize)> {
    let first = first_form(text, at)?;
    let (terms, end) = match second_form(text, at) {
        Some(second) if second.1 > first.1 => second,
        _ => first,
    };
    Some((terms.into_iter().map(recorded).collect(), end))
}

/// What follows the terms of a reading (D7.8).
struct After {
    verb: &'static str,
    kind: DefinitionKind,
    verb_start: usize,
    verb_end: usize,
    reach: Option<Reach>,
    aside: Option<(usize, usize)>,
}

/// The first byte from `from` on, before the first quotation mark at or
/// after it, at which `text` holds an optional comma, a space and a verb
/// form: that byte, and where the verb form begins.
fn shortest_before_a_verb(text: &str, from: usize) -> Option<(usize, usize)> {
    let mark = text[from..]
        .find(MARKS)
        .map_or(text.len(), |offset| from + offset);
    (from..mark).find_map(|at| {
        if self::at(text, at, ", ") && verb_at(text, at + 2).is_some() {
            Some((at, at + 2))
        } else if self::at(text, at, " ") && verb_at(text, at + 1).is_some() {
            Some((at, at + 1))
        } else {
            None
        }
    })
}

/// Whether the lowercase reach `reach`, which has no condition, may stand
/// after the terms: its Object holds no word of a verb phrase, and it
/// begins with `as used in ` or its Object with `this`, `these`, `such` or
/// a kind word and a space (D7.8.2).
fn may_stand_after_the_terms(reach: &Reach) -> bool {
    !holds_a_verb_phrase_word(&reach.object)
        && (reach.words.starts_with("as used in ")
            || begins_with_this_or_a_kind_word(&reach.object))
}

/// What follows the terms that end at the byte `end` of `paragraph`, when
/// one of the cases of D7.8 applies; `no_reach` when the lead gave no
/// reach.
fn after_terms(paragraph: &Paragraph<'_>, end: usize, no_reach: bool) -> Option<After> {
    let text = paragraph.text();
    let with_verb = |verb_start: usize, reach: Option<Reach>, aside: Option<(usize, usize)>| {
        let (verb_end, kind, verb) = verb_at(text, verb_start)?;
        Some(After {
            verb,
            kind,
            verb_start,
            verb_end,
            reach,
            aside,
        })
    };
    // A space and a verb form (D7.8.1).
    if at(text, end, " ") && verb_at(text, end + 1).is_some() {
        return with_verb(end + 1, None, None);
    }
    // An aside between commas (D7.8.2).
    if at(text, end, ", ") {
        let from = end + 2;
        let mark = text[from..]
            .find(MARKS)
            .map_or(text.len(), |offset| from + offset);
        if let Some(to) =
            (from..mark).find(|&to| at(text, to, ", ") && verb_at(text, to + 2).is_some())
            && !holds_a_verb(&text[from..to])
        {
            let aside = &text[from..to];
            if no_reach
                && (at_in_any_case(aside, 0, "as used in ")
                    || at_in_any_case(aside, 0, "when used in "))
                && let Some((reach, None)) = reach(paragraph, from, to, true)
                && may_stand_after_the_terms(&reach)
            {
                return with_verb(to + 2, Some(reach), None);
            }
            return with_verb(to + 2, None, Some((from, to)));
        }
    }
    // A reach after the terms (D7.8.3).
    if no_reach {
        let from = if at(text, end, ", ") {
            Some(end + 2)
        } else if at(text, end, " ") {
            Some(end + 1)
        } else {
            None
        };
        if let Some(from) = from
            && let Some(opening) = ["as used in ", "when used in "]
                .iter()
                .find(|opening| at(text, from, opening))
            && let Some((to, verb_start)) = shortest_before_a_verb(text, from + opening.len())
            && let Some((reach, None)) = reach(paragraph, from, to, true)
            && may_stand_after_the_terms(&reach)
        {
            return with_verb(verb_start, Some(reach), None);
        }
    }
    // An aside with no commas around it (D7.8.4).
    if at(text, end, " ")
        && let Some(start) = BARE_ASIDE_STARTS
            .iter()
            .find(|start| at(text, end + 1, start))
        && let Some((to, verb_start)) = shortest_before_a_verb(text, end + 1 + start.len())
    {
        let aside = &text[end + 1..to];
        if !holds_a_verb(aside)
            && !holds_a_reach_phrase(aside)
            && !aside.contains(',')
            && !ASIDE_REACH_WORDS
                .iter()
                .any(|words| holds_in_any_case(aside, words))
            && !holds_a_verb_phrase_word(aside)
        {
            return with_verb(verb_start, None, Some((end + 1, to)));
        }
    }
    None
}

/// The reading of `paragraph` at `start`, when it matches a pattern (D7);
/// `later` when the start is that of a later sentence (D10.1).
fn find(paragraph: &Paragraph<'_>, start: usize, later: bool) -> Option<Found> {
    let text = paragraph.text();
    let article = if later {
        None
    } else {
        ARTICLES
            .iter()
            .find(|article| at(text, start, article) && at(text, start + article.len(), "\u{201c}"))
    };
    let (list, lead, terms_at) = if at(text, start, "\u{201c}") {
        (true, Lead::none(), start)
    } else if let Some(article) = article {
        (true, Lead::none(), start + article.len())
    } else {
        let (lead, terms_at) = sentence_lead(paragraph, start)?;
        (false, lead, terms_at)
    };
    if list && later {
        return None;
    }
    let (terms, end) = terms(text, terms_at)?;
    let after = after_terms(paragraph, end, lead.reach.is_none())?;
    let reach_after_terms = after.reach.is_some();
    Some(Found {
        list,
        reach: lead.reach.or(after.reach),
        reach_after_terms,
        condition: lead.condition,
        terms,
        kind: after.kind,
        verb: after.verb,
        verb_start: after.verb_start,
        verb_end: after.verb_end,
        aside: after.aside,
    })
}

/// Whether, after the verb form that ends at the byte `verb_end` of `text`
/// and before `sentence_end`, there stands a term followed by a space and a
/// verb form, or by a comma, a space, text with no quotation mark, a comma,
/// a space and a verb form, all within the sentence, that is not one of
/// `terms`, compared in any case as it would be recorded (D11.1).
fn defines_a_second_term(
    text: &str,
    verb_end: usize,
    sentence_end: usize,
    terms: &[String],
) -> bool {
    let verb_within = |at: usize| verb_at(text, at).is_some_and(|(end, _, _)| end <= sentence_end);
    text[verb_end..sentence_end]
        .match_indices(OPEN)
        .any(|(offset, _)| {
            let Some((term, end)) = quoted(text, verb_end + offset) else {
                return false;
            };
            if end > sentence_end {
                return false;
            }
            let follows = (at(text, end, " ") && verb_within(end + 1))
                || (at(text, end, ", ") && {
                    let from = end + 2;
                    let mark = text[from..]
                        .find(MARKS)
                        .map_or(text.len(), |offset| from + offset);
                    commas(text, from, mark.min(sentence_end)).any(|comma| verb_within(comma + 2))
                });
            let term = recorded(term);
            follows && !terms.iter().any(|own| own.eq_ignore_ascii_case(&term))
        })
}

/// Whether `text` speaks of a reach, in any case (D11.5).
fn speaks_of_a_reach(text: &str) -> bool {
    (at_in_any_case(text, 0, "in ")
        && !NOT_A_REACH_AFTER_IN
            .iter()
            .any(|words| at_in_any_case(text, 3, words)))
        || REACH_PHRASES
            .iter()
            .filter(|phrase| **phrase != "when used in")
            .any(|phrase| holds_in_any_case(text, phrase))
        || when_used_in_without_a_sense(text)
}

/// Whether `text` holds `when used in`, in any case, not followed by a space
/// and a sense word.
fn when_used_in_without_a_sense(text: &str) -> bool {
    let phrase = "when used in";
    standing(text, phrase, true).any(|start| {
        let after = start + phrase.len();
        !(at(text, after, " ")
            && SENSE_WORDS
                .iter()
                .any(|sense| at_in_any_case(text, after + 1, sense)))
    })
}

/// Whether the aside `aside` speaks of a reach (D11.2, D11.3).
fn aside_speaks_of_a_reach(aside: &str) -> bool {
    ASIDE_REACH_WORDS
        .iter()
        .any(|words| holds_in_any_case(aside, words))
        || ((at_in_any_case(aside, 0, "in ") || holds_a_reach_phrase(aside))
            && speaks_of_a_reach(aside))
}

/// Whether a reach follows the verb form that ends at the byte `verb_end`
/// of `text` (D11.4).
fn reach_after_the_verb(text: &str, verb_end: usize) -> bool {
    if !at(text, verb_end, ", ") {
        return false;
    }
    let after = verb_end + 2;
    if !OPENINGS
        .iter()
        .any(|opening| at_in_any_case(text, after, opening) && at(text, after + opening.len(), " "))
    {
        return false;
    }
    let end = text[after..]
        .find(',')
        .map_or(text.len(), |offset| after + offset);
    speaks_of_a_reach(&format!("{} ", &text[after..end]))
}

/// Whether a paragraph whose text is `text`, read with no reach, speaks of
/// one: it holds one of the six phrases of a reach, in any case, other than
/// `when used in` followed by a space and a sense word (D11.6).
fn speaks_of_a_reach_with_none(text: &str) -> bool {
    REACH_PHRASES
        .iter()
        .filter(|phrase| **phrase != "when used in")
        .any(|phrase| holds_in_any_case(text, phrase))
        || when_used_in_without_a_sense(text)
}

/// `text` with every term in quotation marks taken out of it.
fn without_terms(text: &str) -> String {
    let mut out = String::new();
    let mut rest = 0;
    while let Some(offset) = text[rest..].find(OPEN) {
        let start = rest + offset;
        match quoted(text, start) {
            Some((_, end)) => {
                out.push_str(&text[rest..start]);
                rest = end;
            }
            None => {
                let next = start + OPEN.len_utf8();
                out.push_str(&text[rest..next]);
                rest = next;
            }
        }
    }
    out.push_str(&text[rest..]);
    out
}

/// The source of the borrowed meaning of `found`, a reading of `paragraph`
/// whose end is the byte `end`: its `borrowed_from` or its
/// `borrowed_words`; `None` when nothing is extracted (D12).
fn borrowed(
    paragraph: &Paragraph<'_>,
    found: &Found,
    end: usize,
) -> Option<(Option<String>, Option<String>)> {
    let text = paragraph.text();
    let link = paragraph
        .links()
        .iter()
        .filter(|link| link.start >= found.verb_end && link.start < end)
        .min_by_key(|link| link.start);
    let source = || source_words(paragraph, found.verb, found.verb_end, end);
    match link {
        Some(link) if !link.target.is_empty() => {
            // What is made under a linked provision (D12.4).
            let between = without_terms(&text[found.verb_end..link.start]);
            let named: Vec<usize> = MADE_BY
                .iter()
                .flat_map(|word| standing(&between, word, false).map(|start| start + word.len()))
                .collect();
            if named.is_empty() {
                return Some((Some(link.target.to_owned()), None));
            }
            let under = named.iter().any(|&after| {
                ["under", "pursuant to"]
                    .iter()
                    .any(|words| standing(&between, words, false).any(|start| start >= after))
            });
            if !under {
                return None;
            }
            source().map(|words| (None, Some(words)))
        }
        _ => source().map(|words| (None, Some(words))),
    }
}

/// Where the source words begin after the verb form that ends at the byte
/// `verb_end` of `text`: the form of D12.5 read there as a regular
/// expression, its alternatives tried in their order and its optional parts
/// first with, then without, and then a character that is not a space.
fn source_start(text: &str, verb_end: usize) -> Option<usize> {
    let optional =
        |list: &'static [&'static str]| list.iter().map(|part| Some(*part)).chain([None]);
    let then = |at: usize, part: Option<&str>| match part {
        Some(part) => self::at(text, at, part).then_some(at + part.len()),
        None => Some(at),
    };
    // `, respectively,`, a space, a preposition, a space and a character
    // that is not a space.
    let rest = |at: usize| {
        optional(&[", respectively,"]).find_map(|respectively| {
            let at = then(then(at, respectively)?, Some(" "))?;
            SOURCE_PREPOSITIONS.iter().find_map(|preposition| {
                let at = then(then(at, Some(*preposition))?, Some(" "))?;
                text[at..]
                    .chars()
                    .next()
                    .is_some_and(|next| next != ' ')
                    .then_some(at)
            })
        })
    };
    optional(&SOURCE_PAST).find_map(|past| {
        let at = then(verb_end, past)?;
        optional(&SOURCE_TO).find_map(|to| {
            let at = then(at, to)?;
            // A space and a pointer, then the terms and a verb, each
            // optional; or none of them.
            let with_pointer = SOURCE_POINTERS.iter().find_map(|pointer| {
                let at = then(then(at, Some(" "))?, Some(*pointer))?;
                let with_terms = self::at(text, at, " ")
                    .then(|| first_form(text, at + 1).map(|(_, end)| end))
                    .flatten();
                with_terms
                    .into_iter()
                    .chain([at])
                    .find_map(|at| optional(&SOURCE_HAS).find_map(|has| rest(then(at, has)?)))
            });
            with_pointer.or_else(|| rest(at))
        })
    })
}

/// The source words of a borrowed meaning whose verb form `verb` ends at
/// the byte `verb_end` of `paragraph`, in a reading whose end is the byte
/// `end`, when the text has them (D12.5).
fn source_words(
    paragraph: &Paragraph<'_>,
    verb: &str,
    verb_end: usize,
    end: usize,
) -> Option<String> {
    let text = paragraph.text();
    let start = if verb.ends_with(" as in") {
        (at(text, verb_end, " ")
            && text[verb_end + 1..]
                .chars()
                .next()
                .is_some_and(|next| next != ' '))
        .then_some(verb_end + 1)?
    } else {
        source_start(text, verb_end)?
    };
    let (_, sentence_end) = paragraph.sentence_of(start);
    let mut stop = sentence_end.min(end);
    if start >= stop {
        return None;
    }
    for words in SOURCE_STOPS {
        if let Some(offset) = text[start..stop].find(words) {
            stop = start + offset;
        }
    }
    let mut words = text[start..stop].trim_end_matches([' ', ',']);
    if let Some(rest) = words.strip_suffix('.')
        && !word_before(words, rest.len()).is_some_and(|word| ABBREVIATIONS.contains(&word))
    {
        words = rest;
    }
    (!words.is_empty() && !words.starts_with(' ') && !words.ends_with(' '))
        .then(|| words.to_owned())
}

/// Where the text of `paragraph` from `from` to its end stands before its
/// close and the one space before the close (D1.7), when it ends with a
/// close.
fn before_the_close(paragraph: &Paragraph<'_>, from: usize) -> Option<usize> {
    let text = paragraph.text();
    let close = text[from..]
        .chars()
        .next_back()
        .filter(|last| CLOSES.contains(last))?;
    let mut end = text.len() - close.len_utf8();
    if end > from && text.as_bytes()[end - 1] == b' ' {
        end -= 1;
    }
    Some(end)
}

/// Where the text of `text` from `from` to `end` ends with `lead_in` taken
/// off, with `separator` before it, when it ends with them.
fn taken_off(text: &str, from: usize, end: usize, separator: &str, lead_in: &str) -> Option<usize> {
    let length = separator.len() + lead_in.len();
    (end >= from + length
        && at(text, end - length, separator)
        && at(text, end - lead_in.len(), lead_in))
    .then(|| end - length)
}

/// Where the text of `text` from `from` to `end` ends with the first of the
/// first lead-ins that it ends with, after `, ` or a space, taken off; `end`
/// when it ends with none (D8.1.1).
fn first_lead_in_off(text: &str, from: usize, end: usize) -> usize {
    FIRST_LEAD_INS
        .iter()
        .find_map(|lead_in| {
            taken_off(text, from, end, ", ", lead_in)
                .or_else(|| taken_off(text, from, end, " ", lead_in))
        })
        .unwrap_or(end)
}

/// The line that the text of `paragraph` from `from` to its end is, when it
/// is a reach line (D8.1); with `last_sentence`, the form of its last
/// sentence is tried too.
fn reach_line(paragraph: &Paragraph<'_>, from: usize, last_sentence: bool) -> Option<Line> {
    let end = before_the_close(paragraph, from)?;
    let text = paragraph.text();
    let line = |(reach, condition): (Reach, Option<String>)| Line { reach, condition };
    // A reach and a first lead-in (D8.1.1).
    reach(paragraph, from, first_lead_in_off(text, from, end), false)
        .map(line)
        // The marked ends (D8.1.2).
        .or_else(|| marked_ends(paragraph, from, end).map(line))
        // The following definitions apply (D8.1.3).
        .or_else(|| following_definitions_apply(paragraph, from, end))
        // The last sentence (D8.1.4).
        .or_else(|| {
            if !last_sentence {
                return None;
            }
            let sentences = sentences(&text[from..]);
            let &(start, _) = sentences.last()?;
            (sentences.len() > 1)
                .then(|| reach_line(paragraph, from + start, false))
                .flatten()
        })
        // A condition and a lowercase reach (D8.1.5).
        .or_else(|| {
            let lead = condition_and_reach(paragraph, from, first_lead_in_off(text, from, end))?;
            Some(Line {
                reach: lead.reach?,
                condition: lead.condition,
            })
        })
}

/// The reach that the text of `paragraph` from `from` to `end` gives at the
/// first of its ends at which one of the forms of D8.1.2 gives one, and the
/// condition at the end of its Object.
fn marked_ends(
    paragraph: &Paragraph<'_>,
    from: usize,
    end: usize,
) -> Option<(Reach, Option<String>)> {
    let text = paragraph.text();
    // The commas before `to`, in the order they stand, each followed by a
    // space and a lowercase exception that runs to `to`.
    let exception_commas = |to: usize| -> Vec<usize> {
        commas(text, from, to)
            .filter(|&comma| is_exception(&text[comma + 2..to], true))
            .collect()
    };
    let mut ends = vec![end];
    if text[from..end].ends_with(')')
        && let Some(open) = text[from..end].rfind(" (").map(|offset| from + offset)
    {
        let words = &text[open + 2..end - 1];
        if !words.contains(['(', ')'])
            && (at_in_any_case(words, 0, "except ") || at_in_any_case(words, 0, "unless "))
        {
            ends.push(open);
        }
    }
    for to in ends.clone() {
        ends.extend(exception_commas(to));
    }
    // Each end, followed by the ends made from it by taking off a lead-in,
    // each of them followed by its exception commas; and whether each end
    // is marked.
    let mut all: Vec<(usize, bool)> = Vec::new();
    for &to in &ends {
        all.push((to, to != end));
        for (index, lead_in) in FIRST_LEAD_INS.iter().chain(&LATER_LEAD_INS).enumerate() {
            for separator in [", ", " "] {
                if let Some(off) = taken_off(text, from, to, separator, lead_in) {
                    let first = index < FIRST_LEAD_INS.len();
                    all.push((off, !(to == end && first)));
                    all.extend(exception_commas(off).into_iter().map(|comma| (comma, true)));
                }
            }
        }
    }
    all.into_iter().find_map(|(to, marked)| {
        if marked && let Some(found) = reach(paragraph, from, to, false) {
            return Some(found);
        }
        if EXCEPTION_STARTS.iter().any(|start| at(text, from, start))
            && let Some(lead) = exception_and_reach(paragraph, from, to)
            && let Some(reach) = lead.reach
        {
            return Some((reach, lead.condition));
        }
        reach(paragraph, from, to, true)
    })
}

/// The line that the text of `paragraph` from `from` to `end` is when it
/// says that the following definitions apply in, to or for purposes of an
/// Object (D8.1.3).
fn following_definitions_apply(paragraph: &Paragraph<'_>, from: usize, end: usize) -> Option<Line> {
    let text = paragraph.text();
    let head = "The following definitions";
    let named = ", the following definitions";
    // Where each way of naming the definitions may end, in the order they
    // are tried: the shortest texts first.
    let mut heads: Vec<usize> = Vec::new();
    if at(text, from, head) {
        heads.push(from + head.len());
    }
    for prefix in [
        "In addition to the definitions ",
        "In addition to the terms ",
    ] {
        if at(text, from, prefix) {
            let start = from + prefix.len();
            let mark = text[start..end]
                .find(MARKS)
                .map_or(end, |offset| start + offset);
            heads.extend(
                (start..=mark)
                    .filter(|&place| at(text, place, named) && place + named.len() <= end)
                    .map(|place| place + named.len()),
            );
        }
    }
    // `[" shall"] " apply " Prep " " Object` from `place` to the end.
    let apply = |place: usize| -> Option<Line> {
        [" shall", ""].iter().find_map(|shall| {
            if !at(text, place, shall) || !at(text, place + shall.len(), " apply ") {
                return None;
            }
            let preposition_at = place + shall.len() + " apply ".len();
            APPLY_PREPOSITIONS.iter().find_map(|preposition| {
                if !at(text, preposition_at, preposition)
                    || !at(text, preposition_at + preposition.len(), " ")
                {
                    return None;
                }
                let start = preposition_at + preposition.len() + 1;
                if start > end {
                    return None;
                }
                let object = &text[start..end];
                let link = paragraph
                    .links()
                    .iter()
                    .find(|link| link.start == start && link.end == end)
                    .map(|link| link.target.to_owned());
                let fits = !object.is_empty()
                    && !object.contains(MARKS)
                    && !NOT_IN_AN_OBJECT
                        .iter()
                        .any(|word| holds_in_any_case(object, word))
                    && !holds_an_end_of_a_sentence(object)
                    && !object.starts_with(' ')
                    && !object.ends_with(' ')
                    && (*preposition != "in"
                        || at(object, 0, "this ")
                        || link.is_some()
                        || begins_with_this_or_a_kind_word(object));
                fits.then(|| Line {
                    reach: Reach {
                        words: text[preposition_at..end].to_owned(),
                        object: object.to_owned(),
                        link,
                    },
                    condition: None,
                })
            })
        })
    };
    let relating = " relating to ";
    heads.into_iter().find_map(|after_head| {
        let with_relating = if at(text, after_head, relating) {
            let start = after_head + relating.len();
            let mark = text[start..end]
                .find(MARKS)
                .map_or(end, |offset| start + offset);
            (start..=mark).find_map(&apply)
        } else {
            None
        };
        with_relating.or_else(|| apply(after_head))
    })
}

/// Whether `provision` has a chapeau part.
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

/// What the chapeau of a provision that has one gives the paragraphs below
/// (D8.3, D9.4).
enum Chapeau {
    /// A reach chapeau: its line.
    Reach(Line),
    /// A condition chapeau: its condition.
    Condition(String),
    /// Any other chapeau.
    Other,
}

/// The chapeau of `provision` (D8.3): its one chapeau part, of one block, a
/// paragraph whose whole text is a reach line or a condition line; any
/// other chapeau otherwise.
fn chapeau_of(provision: &Provision) -> Chapeau {
    let mut chapeaus = provision.items.iter().filter_map(|item| match item {
        Item::Part {
            role: PartRole::Chapeau,
            blocks,
        } => Some(blocks),
        _ => None,
    });
    let (Some(blocks), None) = (chapeaus.next(), chapeaus.next()) else {
        return Chapeau::Other;
    };
    let [Block::Para(text)] = blocks.as_slice() else {
        return Chapeau::Other;
    };
    let paragraph = Paragraph::of(text);
    if let Some(line) = reach_line(&paragraph, 0, true) {
        return Chapeau::Reach(line);
    }
    // A condition line (D8.2).
    match before_the_close(&paragraph, 0) {
        Some(end) if is_condition(&paragraph.text()[..end], false) => {
            Chapeau::Condition(paragraph.text()[..end].to_owned())
        }
        _ => Chapeau::Other,
    }
}

/// The region `reach` names and the words that name it, counted along
/// `chain`: the holder of the reach, then its ancestors and the containers
/// of its unit, each at its address and with its kind, the nearest first
/// (D13.1). An Object that is `this` and a kind of region, with nothing
/// more or a comma, a space and `unless the context` after it, names the
/// nearest of that kind and nothing else; otherwise the link whose words
/// are the Object names its target when it lies below the prefix.
fn region<'c>(
    reach: &Reach,
    mut chain: impl Iterator<Item = (&'c Address, &'c str)>,
) -> Option<(Address, String)> {
    let kind = reach.object.strip_prefix("this ").and_then(|rest| {
        REGION_KINDS.iter().find(|kind| {
            rest.strip_prefix(**kind)
                .is_some_and(|after| after.is_empty() || after.starts_with(", unless the context"))
        })
    });
    if let Some(kind) = kind {
        let (address, _) = chain.find(|(_, of)| of == kind)?;
        return Some((address.clone(), format!("this {kind}")));
    }
    let target = Address::parse(reach.link.as_deref()?).ok()?;
    let prefix = Address::parse(PREFIX).ok()?;
    (target != prefix && target.is_within(&prefix)).then(|| (target, reach.object.clone()))
}

/// The address of the first editorial note of `notes`, in their order,
/// headed "References in Text", one of whose own paragraphs holds, in any
/// case and standing as words, `name` followed by `, referred to in`; the
/// heading and the paragraphs are read as the definitions are (D13.3).
fn see_note(notes: &[Note], name: &str) -> Option<Address> {
    let words = format!("{name}, referred to in");
    notes
        .iter()
        .find(|note| {
            note.layer == Layer::Editorial
                && note
                    .heading
                    .as_ref()
                    .is_some_and(|heading| heading.read_text().text == REFERENCES_IN_TEXT)
                && note.blocks.iter().any(|block| match block {
                    Block::Para(text) => holds_in_any_case(&text.read_text().text, &words),
                    _ => false,
                })
        })
        .map(|note| note.address.clone())
}

#[cfg(test)]
mod tests;
