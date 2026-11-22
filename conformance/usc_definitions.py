#!/usr/bin/env python3
"""Find the definitions of the Code's official XML by a second implementation of the rules.

Reads the official XML release point published by the Office of the Law Revision
Counsel, as `uslm_text.py` reads it (a directory of archives, one archive, or one XML
file), with the independent reading of `uslm_common.py`, and extracts from its units
the definitions that the rules of Definitions in `docs/law-modules.md` find: the second
version of those rules, D1 to D16. This is a second implementation of them, written from
their text, that shares no code with the importer. Each rule below names the rule it
implements by its number, such as D7.3.

The text the rules read is the text of D1.1, made from the reading of a paragraph: the
words of its runs, a footnote marker (a `ref` of the class `footnoteRef`) giving
nothing, and a superscript run (`sup` or `xhtml:sup`) made only of the digits 0 to 9
left out with one space beside it. Every other superscript is read.

`--records FILE` gets one JSON object for each definition, to a line, with the fields
of D15.1 in their order: `at`, `terms`, `kind`, `words` (`null` for `unstated`),
`condition`, `region`, `basis`, `borrowed_from`, `borrowed_words` and `see_note`,
`null` where the definition has none. The lines are sorted by the bytes of `at`, then by
the bytes of the line.

`--against DIR` compares them with the definitions that the `definition` blocks of the
module files of the workspace `DIR` hold, read in their canonical form (the grammar line
of D15.2): a line that begins with the word `definition` must begin a block in that
form. Unless the two are the same, the script names the first definition that differs
and exits 1.

Usage: uv run --locked python -I conformance/usc_definitions.py ARCHIVE_DIR --records FILE
  [--against DIR] [--jobs N]
Exit codes: 0 written, and with `--against` the same as the workspace's; 1 the input is
refused by a reading rule, a module file holds a definition block in another form, or
the definitions differ; 2 an input cannot be read or the records cannot be written.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import Counter
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from uslm_common import (
    PREFIX,
    Container,
    InputError,
    Job,
    MemberResult,
    Para,
    Part,
    Provision,
    RefusedError,
    Text,
    Unit,
    is_address,
    read_member,
    read_release,
)

# D1.2: the quotation marks. Wherever the rules speak of a quotation mark, they mean one
# of these two.
OPEN = "“"
CLOSE = "”"
MARKS = OPEN + CLOSE
# D7.7: the single marks of nested quotation, U+2018 and U+2019.
SINGLE_OPEN = chr(0x2018)
SINGLE_CLOSE = chr(0x2019)
LETTERS = frozenset("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz")
ASCII_LOWER = str.maketrans("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz")

# D1.5: the abbreviations after which a period ends no sentence.
ABBREVIATIONS = frozenset(
    (
        "U.S.C",
        "U.S",
        "App",
        "No",
        "Nos",
        "Stat",
        "Reg",
        "Fed",
        "Pub",
        "Sec",
        "Secs",
        "Co",
        "Inc",
        "Corp",
        "Ltd",
        "Jr",
        "Sr",
        "St",
        "Mt",
        "Ft",
        "Dr",
        "Mr",
        "Mrs",
        "Ms",
        "D.C",
        "seq",
        "etc",
        "e.g",
        "i.e",
        "Res",
        "Cong",
        "Sess",
        "Doc",
        "Rept",
        "H.R",
        "Ch",
        "Art",
        "Par",
        "Cl",
        "Pt",
        "Div",
        "Vol",
        "Supp",
        "Ct",
        "v",
        "Gen",
        "Gov",
        "Rev",
        "Admin",
        "Bros",
        "Assn",
        "Dept",
        "Govt",
        "Natl",
        "Ry",
        "R.R",
        "C.F.R",
        "F.R",
        "L",
    )
)
# The longest run of letters and periods before a period that D1.5 reads.
WORD_BEFORE_WINDOW = 12
# D1.7: the closes.
CLOSES = ("—", "-", ":")
# D1.8: the openings of a reach, in their order, and the lowercase openings.
OPENINGS = (
    "As used in",
    "In",
    "For purposes of",
    "For the purposes of",
    "For the purpose of",
    "For purpose of",
    "When used in",
)
LOWER_OPENINGS = (
    "as used in",
    "in",
    "for purposes of",
    "for the purposes of",
    "for the purpose of",
    "for purpose of",
    "when used in",
)
# D1.9: the six phrases of a reach, looked for in any case.
REACH_PHRASES = (
    "as used in",
    "when used in",
    "for purposes of",
    "for the purposes of",
    "for the purpose of",
    "for purpose of",
)
WHEN_USED_IN = "when used in"
# D1.10: the words of a verb phrase, as written.
VERB_PHRASE_WORDS = (
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
)
# D1.11: the kind words, compared in any case.
KIND_WORDS = (
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
)
# D4.3, D7.8.2, D8.1.3: an Object begins, in any case, with one of these and a space.
OBJECT_BEGINNINGS = tuple(word + " " for word in ("this", "these", "such", *KIND_WORDS))
# D1.12: the region kinds.
REGION_KINDS = (
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
)

# D2.1: each verb form, with the kind of definition it gives.
VERB_FORMS = {
    "means": "means",
    "mean": "means",
    "shall mean": "means",
    "includes": "includes",
    "include": "includes",
    "shall include": "includes",
    "also includes": "includes",
    "also include": "includes",
    "shall also include": "includes",
    "does not include": "does_not_include",
    "do not include": "does_not_include",
    "shall not include": "does_not_include",
    "does not mean": "does_not_include",
    "do not mean": "does_not_include",
    "shall not mean": "does_not_include",
    **dict.fromkeys(
        (
            "has the meaning given",
            "have the meaning given",
            "has the meanings given",
            "have the meanings given",
            "has the same meaning as",
            "have the same meaning as",
            "shall have the meaning given",
            "has the meaning provided",
            "have the meaning provided",
            "has the meanings provided",
            "have the meanings provided",
            "has the meaning set forth",
            "have the meaning set forth",
            "have the meanings set forth",
            "has the meaning prescribed",
            "have the meaning prescribed",
            "have the meanings prescribed",
            "has the meaning stated",
            "have the meanings stated",
            "has the meaning assigned",
            "have the meanings assigned",
            "has the meaning described",
            "has the meaning ascribed",
            "have the meanings ascribed",
            "has the meaning indicated",
            "has the meaning applicable",
            "has the meaning such term has",
            "has the meaning that term has",
            "has the meaning the term has",
            "have the meanings such terms have",
            "have the meanings that such terms have",
            "has the meaning as in",
            "have the meaning as in",
            "has the same meanings as",
            "have the same meanings as",
            "has the same meaning given",
            "have the same meaning given",
            "have the same meanings given",
            "shall have the same meaning as",
            "shall have the meanings given",
        ),
        "borrowed",
    ),
}
# D4.2: the words an Object holds none of, in any case, standing as words.
NOT_IN_AN_OBJECT = (
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
)
# D4.4: the sense words, as written.
SENSE_WORDS = (
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
)
# D5.1 and D5.2: the starts of an exception and of a lowercase exception.
EXCEPTION_STARTS = ("Except ", "Unless ", "Notwithstanding ", "Subject to ")
LOWER_EXCEPTION_STARTS = ("except ", "unless ")
# D6.1: the starts of a condition, besides `When used in ` and a sense word, and the
# lowercase starts.
CONDITION_STARTS = (
    "In the case of ",
    "In any case in which ",
    "In the context of ",
    "With respect to ",
)
LOWER_CONDITION_STARTS = (
    "in the case of ",
    "in any case in which ",
    "in the context of ",
    "with respect to ",
)
WHEN_USED_IN_START = "When used in "
LOWER_WHEN_USED_IN_START = "when used in "
# D6.5: the asides that are a condition as they are, and the starts of the others, in
# the order they are tried.
GEOGRAPHIC_ASIDES = (
    "in a geographical sense",
    "in a geographic sense",
    "in the geographical sense",
    "in the geographic sense",
)
ASIDE_CONDITION_STARTS = (
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
)
# D11.5: the words after `in ` with which text does not speak of a reach. D6.6.2: the
# same list without `addition to `, the words after `in ` with which a condition after
# the verb form begins.
NOT_A_REACH_AFTER_IN = (
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
)
CONDITION_AFTER_IN = tuple(words for words in NOT_A_REACH_AFTER_IN if words != "addition to ")
# D6.6.4: what the rest of the sentence after a condition after the verb form holds none
# of, in any case, standing as words.
FURTHER_CONDITIONS = (
    "in the case of",
    "in any case in which",
    "in the context of",
    "in connection with",
    "in relation to",
    "in reference to",
    "in respect of",
    "with respect to",
    "when used in",
)
# D7.3: the head words, of the first kind and the others.
HEAD_NOUNS = ("term", "terms", "word", "words", "phrase")
FIRST_HEADS = tuple(f"{article} {noun} " for article in ("the", "The") for noun in HEAD_NOUNS)
EACH_HEADS = ("Each of the terms ", "each of the terms ")
ALL_HEADS = FIRST_HEADS + EACH_HEADS
# D7.3.4: what the text before the first quotation mark ends with.
BARE_TERM_ENDINGS = (", ", ", the ", ", a ", ", an ")
# D7.5: the articles before the list form.
ARTICLES = ("The ", "the ", "A ", "a ", "An ", "an ")
# D7.8.2, D7.8.3: the openings of a reach after the terms.
TERM_REACH_OPENINGS = ("as used in ", "when used in ")
# D7.8.4: the starts of an aside with no commas.
BARE_ASIDE_STARTS = (
    "with respect to ",
    "when used with respect to ",
    "used with respect to ",
    "when applied to ",
    "as applied to ",
    "with reference to ",
    "in the case of ",
    "for any ",
    "when referring to ",
)
# D8.1: the first lead-ins and the later lead-ins, in their order.
FIRST_LEAD_INS = (
    "the term",
    "the terms",
    "the following definitions apply",
    "the following definitions shall apply",
)
LATER_LEAD_INS = (
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
)
LEAD_IN_SEPARATORS = (", ", " ")
# D11.3: the words by which an aside speaks of a reach, in any case, standing as words.
REACH_ASIDE_WORDS = (
    "as used by",
    "used herein",
    "where used",
    "wherever used",
    "purpose",
    "purposes",
)
# D12.4: the words of a meaning made under a linked provision, as written.
MADE_WORDS = ("regulation", "regulations", "rule", "rules", "order", "orders")
MADE_UNDER = ("under", "pursuant to")
# D12.5: the pointers, the prepositions and the strings at which the source words end.
POINTERS = (
    "each such term",
    "such terms",
    "such term",
    "the terms",
    "the term",
    "that term",
    "those terms",
    "these terms",
    "this term",
    "it",
    "them",
)
PREPOSITIONS = (
    "for the purposes of",
    "for purposes of",
    "pursuant to",
    "when used in",
    "as used in",
    "as in",
    "in",
    "by",
    "under",
)
SOURCE_STOPS = (
    ";",
    "—",
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
)
AS_IN = " as in"
# D13.3: the heading of the note a restated definition points at, and the words after
# the words that name the region.
REFERENCES_IN_TEXT = "References in Text"
REFERRED_TO_IN = ", referred to in"


def fold(text: str) -> str:
    """D1.3: "in any case", the letters A to Z are compared as a to z, and no other
    character is changed."""
    return text.translate(ASCII_LOWER)


def longest_first(words: Iterable[str]) -> list[str]:
    return sorted(words, key=len, reverse=True)


def stands(text: str, start: int, end: int) -> bool:
    """D1.4: the words at `start` to `end` of `text` stand there: the character before
    them, if any, and the character after them, if any, are not letters A to Z or a to z."""
    return (start == 0 or text[start - 1] not in LETTERS) and (
        end >= len(text) or text[end] not in LETTERS
    )


def standing(text: str, word: str, start: int = 0) -> Iterator[int]:
    """The places at or after `start` where `word` stands in `text`, as written."""
    at = text.find(word, start)
    while at != -1:
        if stands(text, at, at + len(word)):
            yield at
        at = text.find(word, at + 1)


def holds_word(text: str, word: str) -> bool:
    """Whether `word` stands in `text`, as written."""
    return next(standing(text, word), None) is not None


def holds_words(text: str, words: Iterable[str]) -> bool:
    """Whether one of `words` stands in `text`, as written."""
    return any(holds_word(text, word) for word in words)


def holds_any_case(text: str, words: Iterable[str]) -> bool:
    """Whether one of `words` stands in `text`, in any case."""
    folded = fold(text)
    return any(holds_word(folded, fold(word)) for word in words)


def holds_mark(text: str) -> bool:
    return OPEN in text or CLOSE in text


# D2.1, D2.2: a verb form stands where it is looked for, written exactly as in the table,
# and of several forms that stand at one place the longest is taken.
FORMS = "|".join(re.escape(form) for form in longest_first(VERB_FORMS))
VERB_AT = re.compile(f"(?:{FORMS})(?![A-Za-z])")
VERB_IN_TEXT = re.compile(f"(?<![A-Za-z])(?:{FORMS})(?![A-Za-z])")


@dataclass(frozen=True, slots=True)
class Verb:
    """A verb form that stands in a text: where it starts and ends, and its kind."""

    start: int
    end: int
    kind: str


def verb_at(text: str, at: int) -> Verb | None:
    """D2.2: the verb form that stands at `at` in `text`, the longest of several."""
    if at > 0 and text[at - 1] in LETTERS:
        return None
    found = VERB_AT.match(text, at)
    if found is None:
        return None
    return Verb(at, found.end(), VERB_FORMS[found.group()])


def holds_verb_form(text: str) -> bool:
    """D2.4: whether a form of the table stands anywhere in `text`."""
    return VERB_IN_TEXT.search(text) is not None


# D1.1: the text the rules read.


@dataclass(frozen=True, slots=True)
class Read:
    """The text of a paragraph that every rule reads (D1.1), and its links: where the
    words of each link run start and end in that text, and its target, which may be
    empty."""

    plain: str
    links: tuple[tuple[int, int, str], ...]

    def link_on(self, start: int, end: int) -> str | None:
        """The target of the link whose words are exactly those from `start` to `end`;
        `None` when no link has them (D4.3, D4.7)."""
        return next((target for at, stop, target in self.links if (at, stop) == (start, end)), None)


def is_digits(words: str) -> bool:
    return bool(words) and all("0" <= character <= "9" for character in words)


def read_text(text: Text) -> Read:
    """D1.1: the text of a paragraph from its reading. The reading gives the words of
    its words runs, links, subscript and superscript runs, and gives no words for a
    footnote marker, so its plain text is already the text of D1.1.1, D1.1.2 and D1.1.4.
    A superscript run whose words are only the digits 0 to 9 gives nothing, and one
    space goes with it (D1.1.3): the space just before it, when just after it there is
    a space or the end of the text; or, when it begins the text, the space just after
    it. The runs are read in order, so "just before it" is what the text made so far
    ends with."""
    plain = text.plain
    digits = sorted((start, end) for start, end in text.sup if is_digits(plain[start:end]))
    if not digits:
        return Read(plain, text.refs)
    kept = [True] * len(plain)
    for start, end in digits:
        for place in range(start, end):
            kept[place] = False
        before = start - 1
        while before >= 0 and not kept[before]:
            before -= 1
        after = plain[end] if end < len(plain) else None
        if before >= 0:
            if plain[before] == " " and after in (None, " "):
                kept[before] = False
        elif after == " ":
            kept[end] = False
    # The place in the new text of each place of the old one.
    moved = [0] * (len(plain) + 1)
    count = 0
    for place, keep in enumerate(kept):
        moved[place] = count
        count += keep
    moved[len(plain)] = count
    read = "".join(character for character, keep in zip(plain, kept, strict=True) if keep)
    links = tuple((moved[start], moved[end], target) for start, end, target in text.refs)
    return Read(read, links)


# D1.5, D1.6: the end of a sentence, and the sentences of a text.


def word_before(text: str, period: int) -> str | None:
    """D1.5: the word before the period at `period`: the longest run of letters and
    periods that ends just before it, starts with a letter, and lies within the 12
    characters before it; none when the character just before it is neither."""
    if period == 0 or (text[period - 1] not in LETTERS and text[period - 1] != "."):
        return None
    start = period
    while (
        start > 0
        and period - (start - 1) <= WORD_BEFORE_WINDOW
        and (text[start - 1] in LETTERS or text[start - 1] == ".")
    ):
        start -= 1
    while start < period and text[start] not in LETTERS:
        start += 1
    return text[start:period] if start < period else None


def is_sentence_end(text: str, period: int) -> bool:
    """D1.5: whether the period at `period` is an end of a sentence: then optionally `”`
    or `)`, then a space, then a letter A to Z; except where the word before the period
    is an abbreviation or a single capital letter."""
    after = period + 1
    if after < len(text) and text[after] in "”)":
        after += 1
    if not (after + 1 < len(text) and text[after] == " " and "A" <= text[after + 1] <= "Z"):
        return False
    word = word_before(text, period)
    if word is None:
        return True
    return word not in ABBREVIATIONS and not (len(word) == 1 and "A" <= word <= "Z")


def sentence_ends(text: str) -> Iterator[int]:
    """The places of the periods of `text` that end a sentence."""
    period = text.find(".")
    while period != -1:
        if is_sentence_end(text, period):
            yield period
        period = text.find(".", period + 1)


def has_sentence_end(text: str) -> bool:
    return next(sentence_ends(text), None) is not None


def sentences(text: str) -> list[tuple[int, int]]:
    """D1.6: the sentences of `text`, each from where it begins to where it ends (just
    after its period, or the end of the text). The first begins at the start, and each
    later one just after the space of an end of a sentence."""
    found = []
    start = 0
    for period in sentence_ends(text):
        found.append((start, period + 1))
        after = period + 1
        if text[after] in "”)":
            after += 1
        start = after + 1
    found.append((start, len(text)))
    return found


def sentence_of(spans: list[tuple[int, int]], place: int) -> tuple[int, int]:
    """The sentence of `spans` that holds `place`."""
    for start, end in spans:
        if place < end:
            return start, end
    return spans[-1]


def commas(text: str, start: int = 0) -> Iterator[int]:
    """The places of each comma of `text` that a space follows, in order."""
    at = text.find(", ", start)
    while at != -1:
        yield at
        at = text.find(", ", at + 1)


def commas_clean(text: str) -> bool:
    """D5.1, D6.3: each comma of `text` that is followed by a space and a lowercase
    letter is followed by a space and the word `and` or `or`."""
    for at in commas(text):
        after = at + 2
        lowercase = after < len(text) and "a" <= text[after] <= "z"
        if lowercase and not any(
            text.startswith(word, after) and stands(text, after, after + len(word))
            for word in ("and", "or")
        ):
            return False
    return True


def body_end(plain: str) -> int | None:
    """D1.7: where the text before the close ends, when the text ends with a close: the
    close, and one space before it if there is one, are no part of it."""
    if not plain.endswith(CLOSES):
        return None
    end = len(plain) - 1
    if end > 0 and plain[end - 1] == " ":
        end -= 1
    return end


# D4: the reach.


@dataclass(frozen=True, slots=True)
class Reach:
    """A reach: its words (D4.6), its Object, the target of its link (D4.7) when it has
    one, and the condition at the end of its Object (D4.5) or of a line (D8.1.5)."""

    words: str
    object_words: str
    link: str | None
    condition: str | None = None


def begins_with_object_word(object_words: str) -> bool:
    """D4.3, D7.8.2, D8.1.3: the Object begins, in any case, with `this`, `these`,
    `such` or a kind word, and a space."""
    return fold(object_words).startswith(OBJECT_BEGINNINGS)


def object_is_clean(object_words: str) -> bool:
    """D4.2: the Object is not empty, holds no quotation mark, none of the words of
    `NOT_IN_AN_OBJECT` in any case, standing as words, and no end of a sentence."""
    return (
        bool(object_words)
        and not holds_mark(object_words)
        and not holds_any_case(object_words, NOT_IN_AN_OBJECT)
        and not has_sentence_end(object_words)
    )


def read_reach(
    text: Read, start: int, end: int, lowercase: bool, split: bool = True
) -> Reach | None:
    """D4: the reach that the text from `start` to `end` is, read as a reach, with a
    lowercase opening when `lowercase`. With `split`, a condition at the end of the
    Object is split off as D4.5 says."""
    plain = text.plain
    segment = plain[start:end]
    openings = LOWER_OPENINGS if lowercase else OPENINGS
    opening = next((one for one in openings if segment.startswith(one + " ")), None)
    if opening is None:
        return None
    at = start + len(opening) + 1
    object_words = plain[at:end]
    # D4.4: `When used in` and a sense word is no reach.
    if opening in ("When used in", "when used in") and object_words.startswith(SENSE_WORDS):
        return None
    # D4.5: a condition at the end of the Object.
    if split:
        cut = next(
            (
                comma
                for comma in commas(object_words)
                if condition_start(object_words[comma + 2 :], lowercase=True) is not None
            ),
            None,
        )
        if cut is not None and cut > 0:
            condition = object_words[cut + 2 :]
            if is_condition(condition, lowercase=True):
                shorter = read_reach(text, start, at + cut, lowercase, split=False)
                if shorter is not None:
                    return Reach(shorter.words, shorter.object_words, shorter.link, condition)
    if not object_is_clean(object_words):
        return None
    link = text.link_on(at, end)
    # D4.3: after `In`.
    if (
        opening in ("In", "in")
        and not object_words.startswith("this ")
        and link is None
        and not (begins_with_object_word(object_words) and not object_words.startswith("such "))
    ):
        return None
    return Reach(segment, object_words, link)


# D5: the exception.


def is_clean(text: str) -> bool:
    """D5.1: an exception is clean."""
    return (
        not holds_mark(text)
        and not holds_verb_form(text)
        and not holds_any_case(text, REACH_PHRASES)
        and not has_sentence_end(text)
        and not fold(text).startswith("except in ")
        and commas_clean(text)
    )


def is_exception(text: str, lowercase: bool) -> bool:
    """D5.1, D5.2: whether `text` is an exception, or a lowercase exception."""
    starts = LOWER_EXCEPTION_STARTS if lowercase else EXCEPTION_STARTS
    return text.startswith(starts) and is_clean(text)


# D6: the condition.


def condition_start(text: str, lowercase: bool) -> int | None:
    """D6.1: the length of the condition start that `text` begins with, capital or
    lowercase; `None` when it begins with none."""
    for start in LOWER_CONDITION_STARTS if lowercase else CONDITION_STARTS:
        if text.startswith(start):
            return len(start)
    when = LOWER_WHEN_USED_IN_START if lowercase else WHEN_USED_IN_START
    if text.startswith(when) and text[len(when) :].startswith(SENSE_WORDS):
        return len(when)
    return None


def condition_words(words: str) -> bool:
    """D6.3: the text after the start of a condition is not empty, has no space at its
    start or end, holds no quotation mark, no verb form, none of the six phrases of a
    reach in any case, no end of a sentence and no `shall`, and each comma of it followed
    by a space and a lowercase letter is followed by a space and `and` or `or`."""
    return (
        bool(words)
        and not words.startswith(" ")
        and not words.endswith(" ")
        and not holds_mark(words)
        and not holds_verb_form(words)
        and not holds_any_case(words, REACH_PHRASES)
        and not has_sentence_end(words)
        and not holds_word(words, "shall")
        and commas_clean(words)
    )


def is_condition(text: str, lowercase: bool) -> bool:
    """D6.3: whether `text` is a condition, with a capital or a lowercase start."""
    start = condition_start(text, lowercase)
    return start is not None and condition_words(text[start:])


def aside_is_condition(aside: str) -> bool:
    """D6.5: whether an aside is a condition."""
    if aside in GEOGRAPHIC_ASIDES:
        return True
    when = LOWER_WHEN_USED_IN_START
    if aside.startswith(when) and aside[len(when) :].startswith(SENSE_WORDS):
        return condition_words(aside[len(when) :])
    start = next((one for one in ASIDE_CONDITION_STARTS if aside.startswith(one)), None)
    return start is not None and condition_words(aside[len(start) :])


def verb_condition(plain: str, verb_end: int, sentence_end: int) -> str | None:
    """D6.6: the condition after the verb form, when the text after it is a comma, a
    space, C, a comma, a space and R as that rule says; `None` otherwise."""
    if not plain.startswith(", ", verb_end):
        return None
    start = verb_end + 2
    end = plain.find(",", start)
    if end == -1 or end >= sentence_end or not plain.startswith(", ", end):
        return None
    condition = plain[start:end]
    if condition.startswith(LOWER_WHEN_USED_IN_START) and condition[
        len(LOWER_WHEN_USED_IN_START) :
    ].startswith(SENSE_WORDS):
        words = condition[len(LOWER_WHEN_USED_IN_START) :]
    elif condition.startswith("in ") and condition[3:].startswith(CONDITION_AFTER_IN):
        words = condition[3:]
    else:
        return None
    if not condition_words(words) or "—" in condition:
        return None
    rest = plain[end + 2 : sentence_end]
    if rest.startswith(("or ", "and ")) or re.match("[^ ,]+,", rest):
        return None
    if holds_any_case(rest, FURTHER_CONDITIONS):
        return None
    return condition


# D7: the patterns.


@dataclass(frozen=True, slots=True)
class Lead:
    """What a lead gives (D7.2): a reach, a condition, both or neither."""

    reach: Reach | None
    condition: str | None


NO_LEAD = Lead(None, None)


def read_lead(text: Read, start: int, end: int) -> Lead | None:
    """D7.2: the lead from `start` to `end`, read by the first of its forms that applies;
    `None` when none reads it."""
    reach = read_reach(text, start, end, lowercase=False)
    if reach is not None:
        return Lead(reach, reach.condition)
    segment = text.plain[start:end]
    if segment.startswith(EXCEPTION_STARTS):
        for comma in commas(segment):
            after = read_reach(text, start + comma + 2, end, lowercase=True)
            if after is not None and is_exception(segment[:comma], lowercase=False):
                return Lead(after, after.condition)
        if is_exception(segment, lowercase=False):
            return NO_LEAD
    if condition_start(segment, lowercase=False) is not None:
        for comma in commas(segment):
            after = read_reach(text, start + comma + 2, end, lowercase=True)
            if (
                after is not None
                and after.condition is None
                and is_condition(segment[:comma], lowercase=False)
            ):
                return Lead(after, segment[:comma])
        if is_condition(segment, lowercase=False):
            return Lead(None, segment)
    return None


def head_at(plain: str, at: int, heads: tuple[str, ...]) -> int | None:
    """D7.3: where the head words of `heads` that begin at `at` end, if some do."""
    return next((at + len(head) for head in heads if plain.startswith(head, at)), None)


def holds_head_words(plain: str, start: int, end: int) -> bool:
    """D7.3.4: whether head words stand between `start` and `end`."""
    segment = plain[:end]
    for head in ALL_HEADS:
        at = segment.find(head, start)
        while at != -1:
            if at == 0 or plain[at - 1] not in LETTERS:
                return True
            at = segment.find(head, at + 1)
    return False


def sentence_lead(text: Read, start: int) -> tuple[Lead, int] | None:
    """D7.3: in the sentence form, the lead of the reading at `start` and where its
    terms begin, by the first case that applies; `None` when the reading matches no
    pattern."""
    plain = text.plain
    marks = [at for at in (plain.find(OPEN, start), plain.find(CLOSE, start)) if at != -1]
    first_mark = min(marks) if marks else len(plain)
    # D7.3.1: the head words at the start.
    head = head_at(plain, start, ALL_HEADS)
    if head is not None:
        return NO_LEAD, head
    # D7.3.2: a comma and a space, then head words, before the first quotation mark.
    for heads in (FIRST_HEADS, EACH_HEADS):
        for comma in commas(plain, start):
            if comma >= first_mark:
                break
            head = head_at(plain, comma + 2, heads)
            if head is not None and head <= first_mark:
                lead = read_lead(text, start, comma)
                return None if lead is None else (lead, head)
    # D7.3.3: a space, then head words of the first kind.
    space = plain.find(" ", start)
    while space != -1 and space < first_mark:
        head = head_at(plain, space + 1, FIRST_HEADS)
        if head is not None and head <= first_mark:
            reach = read_reach(text, start, space, lowercase=False)
            if reach is None or reach.condition is not None or "," in reach.object_words:
                return None
            return Lead(reach, None), head
        space = plain.find(" ", space + 1)
    # D7.3.4: no head words; the terms begin at the first quotation mark.
    if first_mark == len(plain) or plain[first_mark] != OPEN:
        return None
    if holds_head_words(plain, start, first_mark):
        return None
    before = plain[start:first_mark]
    ending = next((one for one in BARE_TERM_ENDINGS if before.endswith(one)), None)
    if ending is None:
        return None
    lead = read_lead(text, start, first_mark - len(ending))
    if lead is None or lead.reach is None:
        return None
    return lead, first_mark


TERM = f"{OPEN}[^{MARKS}]+{CLOSE}"
ONE_TERM = re.compile(f"{OPEN}([^{MARKS}]+){CLOSE}")
SEPARATED_TERMS = f"{TERM}(?:(?:, and |, or |, | and | or ){TERM})*"
TERMS_FIRST = re.compile(SEPARATED_TERMS)
TERMS_SECOND = re.compile(f"(?:{OPEN}[^{MARKS}]*,{CLOSE} )+(?:and |or )?{TERM}")


def without_comma(term: str) -> str:
    """D7.6: a term longer than one character whose last character is a comma is
    recorded without that comma."""
    return term[:-1] if len(term) > 1 and term.endswith(",") else term


def trimmed(term: str) -> str:
    """D7.7: the term with the stray characters of nested quotation taken off; a term
    that would become empty is kept as it was."""
    trim = term.strip(" ")
    if term.endswith(" ") and trim.endswith(SINGLE_CLOSE) and SINGLE_OPEN not in trim:
        trim = trim[:-1]
    if trim.startswith(SINGLE_OPEN) and SINGLE_CLOSE not in trim:
        trim = trim[1:]
    return trim or term


def recorded_term(term: str) -> str:
    """D7.6, D7.7: a term, its quotation marks taken off, as it is recorded."""
    return trimmed(without_comma(term))


def read_terms(plain: str, at: int) -> tuple[tuple[str, ...], int] | None:
    """D7.6, D7.7: the terms that begin at `at` and where they end."""
    first = TERMS_FIRST.match(plain, at)
    second = TERMS_SECOND.match(plain, at)
    if second is not None and (first is None or second.end() > first.end()):
        words = ONE_TERM.findall(second.group())
        words = [without_comma(term) for term in words[:-1]] + words[-1:]
        end = second.end()
    elif first is not None:
        words = ONE_TERM.findall(first.group())
        end = first.end()
    else:
        return None
    return tuple(recorded_term(term) for term in words), end


@dataclass(frozen=True, slots=True)
class After:
    """What follows the terms (D7.8): the verb form, and an aside or a reach."""

    verb: Verb
    aside: str | None
    reach: Reach | None


def verb_after(
    plain: str, start: int, low: int, separators: tuple[str, ...]
) -> tuple[int, Verb] | None:
    """The shortest text from `start` with no quotation mark, at least to `low`, that is
    followed by one of `separators` and a verb form: where it ends, and the verb form.
    At each place the separators are tried in their order."""
    marks = [at for at in (plain.find(OPEN, start), plain.find(CLOSE, start)) if at != -1]
    limit = min(marks) if marks else len(plain)
    for end in range(low, limit):
        if plain[end] not in ", ":
            continue
        for separator in separators:
            if plain.startswith(separator, end):
                verb = verb_at(plain, end + len(separator))
                if verb is not None:
                    return end, verb
    return None


def term_reach(text: Read, start: int, end: int) -> Reach | None:
    """D7.8.2, D7.8.3: the reach after the terms, when the words from `start` to `end`
    are a lowercase reach with no condition, whose Object holds no word of a verb phrase,
    and that begins with `as used in ` or whose Object begins, in any case, with `this`,
    `these`, `such` or a kind word and a space."""
    reach = read_reach(text, start, end, lowercase=True)
    if reach is None or reach.condition is not None:
        return None
    if holds_words(reach.object_words, VERB_PHRASE_WORDS):
        return None
    if not text.plain.startswith("as used in ", start) and not begins_with_object_word(
        reach.object_words
    ):
        return None
    return reach


def after_terms(text: Read, at: int, lead_gave_reach: bool) -> After | None:
    """D7.8: what follows the terms that end at `at`, by the first case that applies."""
    plain = text.plain
    # D7.8.1: a space and a verb form.
    if plain.startswith(" ", at):
        verb = verb_at(plain, at + 1)
        if verb is not None:
            return After(verb, None, None)
    # D7.8.2: a comma, a space, an aside, a comma, a space and a verb form.
    if plain.startswith(", ", at):
        found = verb_after(plain, at + 2, at + 2, (", ",))
        if found is not None and not holds_verb_form(plain[at + 2 : found[0]]):
            end, verb = found
            aside = plain[at + 2 : end]
            if not lead_gave_reach and fold(aside).startswith(TERM_REACH_OPENINGS):
                reach = term_reach(text, at + 2, end)
                if reach is not None:
                    return After(verb, None, reach)
            return After(verb, aside, None)
    # D7.8.3: a reach after the terms, with no commas around it.
    if not lead_gave_reach:
        start = (
            at + 2 if plain.startswith(", ", at) else at + 1 if plain.startswith(" ", at) else -1
        )
        opening = next((one for one in TERM_REACH_OPENINGS if plain.startswith(one, start)), None)
        if start != -1 and opening is not None:
            found = verb_after(plain, start, start + len(opening), (", ", " "))
            if found is not None:
                reach = term_reach(text, start, found[0])
                if reach is not None:
                    return After(found[1], None, reach)
    # D7.8.4: a space and an aside with no commas.
    if plain.startswith(" ", at):
        opening = next((one for one in BARE_ASIDE_STARTS if plain.startswith(one, at + 1)), None)
        if opening is not None:
            found = verb_after(plain, at + 1, at + 1 + len(opening), (", ", " "))
            if found is not None:
                aside = plain[at + 1 : found[0]]
                if (
                    not holds_verb_form(aside)
                    and not holds_any_case(aside, REACH_PHRASES)
                    and "," not in aside
                    and not holds_any_case(aside, REACH_ASIDE_WORDS)
                    and not holds_words(aside, VERB_PHRASE_WORDS)
                ):
                    return After(found[1], aside, None)
    return None


@dataclass(frozen=True, slots=True)
class Reading:
    """A reading that matches a pattern (D7): where it starts, whether it has the
    sentence form, what its lead gives, its terms, and what follows them."""

    start: int
    sentence: bool
    lead: Lead
    terms: tuple[str, ...]
    after: After


def read_pattern(text: Read, start: int) -> Reading | None:
    """D7, D10.1: the pattern of the reading at `start`, the start of the text or of a
    later sentence; `None` when it matches none."""
    plain = text.plain
    lead = NO_LEAD
    sentence = False
    if plain.startswith(OPEN, start):
        # D7.1, D7.4; D10.1: the list form only at the start of the text.
        if start > 0:
            return None
        at = start
    elif start == 0 and (
        article := next((a for a in ARTICLES if plain.startswith(a + OPEN)), None)
    ):
        # D7.5: an article before the list form.
        at = len(article)
    else:
        sentence = True
        found = sentence_lead(text, start)
        if found is None:
            return None
        lead, at = found
    terms = read_terms(plain, at)
    if terms is None:
        return None
    after = after_terms(text, terms[1], lead.reach is not None)
    if after is None:
        return None
    return Reading(start, sentence, lead, terms[0], after)


# D8: reach lines and condition lines.


def without_first_lead_in(plain: str, start: int, end: int) -> int:
    """D8.1.1: where the text from `start` to `end` ends once the first of the first
    lead-ins that it ends with, after `, ` or a space, is taken off."""
    segment = plain[start:end]
    for lead_in in FIRST_LEAD_INS:
        for separator in LEAD_IN_SEPARATORS:
            if segment.endswith(separator + lead_in):
                return end - len(separator) - len(lead_in)
    return end


def exception_reach(text: Read, start: int, end: int) -> Reach | None:
    """D7.2.2, with a reach: the text from `start` to `end` is `Exception ", "
    LowercaseReach`, for the first comma that makes it so."""
    segment = text.plain[start:end]
    if not segment.startswith(EXCEPTION_STARTS):
        return None
    for comma in commas(segment):
        after = read_reach(text, start + comma + 2, end, lowercase=True)
        if after is not None and is_exception(segment[:comma], lowercase=False):
            return after
    return None


def exception_commas(plain: str, start: int, end: int) -> list[int]:
    """D8.1.2: each place of a comma before `end` that is followed by a space and
    `except ` or `unless ` whose words to `end` are a lowercase exception, in order."""
    return [
        comma
        for comma in commas(plain[:end], start)
        if is_exception(plain[comma + 2 : end], lowercase=True)
    ]


PARENTHESIZED_EXCEPTION = re.compile(r" \(([^()]*)\)\Z")


def line_ends(plain: str, start: int, end: int) -> list[tuple[int, bool]]:
    """D8.1.2: the ends of a line, in their order, each with whether it is marked."""
    base = [end]
    parenthesized = PARENTHESIZED_EXCEPTION.search(plain, start, end)
    if parenthesized is not None and fold(parenthesized[1]).startswith(LOWER_EXCEPTION_STARTS):
        base.append(parenthesized.start())
    made = base + [comma for one in base for comma in exception_commas(plain, start, one)]
    ends: list[tuple[int, bool]] = []
    for one in made:
        ends.append((one, one != end))
        for lead_in in FIRST_LEAD_INS + LATER_LEAD_INS:
            for separator in LEAD_IN_SEPARATORS:
                if plain[start:one].endswith(separator + lead_in):
                    cut = one - len(separator) - len(lead_in)
                    ends.append((cut, not (one == end and lead_in in FIRST_LEAD_INS)))
                    ends += [(comma, True) for comma in exception_commas(plain, start, cut)]
    return ends


def line_form_one(text: Read, start: int, end: int) -> Reach | None:
    """D8.1.1: `Reach [", " | " "] [FirstLeadIn]`."""
    return read_reach(text, start, without_first_lead_in(text.plain, start, end), lowercase=False)


def line_form_two(text: Read, start: int, end: int) -> Reach | None:
    """D8.1.2: the forms tried at each end in turn."""
    for one, marked in line_ends(text.plain, start, end):
        if marked:
            reach = read_reach(text, start, one, lowercase=False)
            if reach is not None:
                return reach
        reach = exception_reach(text, start, one) or read_reach(text, start, one, lowercase=True)
        if reach is not None:
            return reach
    return None


FOLLOWING_DEFINITIONS = re.compile(
    "(?:The following definitions"
    f"|In addition to the definitions [^{MARKS}]*?, the following definitions"
    f"|In addition to the terms [^{MARKS}]*?, the following definitions)"
    f"(?: relating to [^{MARKS}]*?)?(?: shall)? apply "
    "(in|to|for purposes of|for the purposes of) (.*)",
    re.DOTALL,
)


def line_form_three(text: Read, start: int, end: int) -> Reach | None:
    """D8.1.3: `The following definitions ... apply Prep Object`, the whole text."""
    found = FOLLOWING_DEFINITIONS.fullmatch(text.plain, start, end)
    if found is None:
        return None
    preposition, object_words = found[1], found[2]
    if (
        not object_words
        or holds_mark(object_words)
        or holds_any_case(object_words, NOT_IN_AN_OBJECT)
        or has_sentence_end(object_words)
        or object_words.startswith(" ")
        or object_words.endswith(" ")
    ):
        return None
    link = text.link_on(found.start(2), found.end(2))
    if (
        preposition == "in"
        and not object_words.startswith("this ")
        and link is None
        and not begins_with_object_word(object_words)
    ):
        return None
    return Reach(f"{preposition} {object_words}", object_words, link)


def line_form_five(text: Read, start: int, end: int) -> Reach | None:
    """D8.1.5: with a first lead-in taken off, `Condition ", " LowercaseReach` as in
    D7.2.3, with a reach."""
    cut = without_first_lead_in(text.plain, start, end)
    segment = text.plain[start:cut]
    if condition_start(segment, lowercase=False) is None:
        return None
    for comma in commas(segment):
        after = read_reach(text, start + comma + 2, cut, lowercase=True)
        if (
            after is not None
            and after.condition is None
            and is_condition(segment[:comma], lowercase=False)
        ):
            return Reach(after.words, after.object_words, after.link, segment[:comma])
    return None


def line_forms(text: Read, start: int, end: int) -> Reach | None:
    """D8.1 forms 1, 2 and 3 on the text from `start`, whose text before the close ends
    at `end`."""
    return (
        line_form_one(text, start, end)
        or line_form_two(text, start, end)
        or line_form_three(text, start, end)
    )


def reach_line(text: Read) -> Reach | None:
    """D8.1: the reach of `text` when it is a reach line, by the first form that applies."""
    end = body_end(text.plain)
    if end is None:
        return None
    found = line_forms(text, 0, end)
    if found is not None:
        return found
    # D8.1.4: the last sentence of a text of more than one sentence.
    spans = sentences(text.plain)
    if len(spans) > 1:
        last = spans[-1][0]
        found = line_forms(text, last, end) or line_form_five(text, last, end)
        if found is not None:
            return found
    return line_form_five(text, 0, end)


def condition_line(text: Read) -> str | None:
    """D8.2: the condition of `text` when it is a condition line (and, as the caller
    holds, no reach line): the text before the close is a condition with a capital
    start."""
    end = body_end(text.plain)
    if end is None:
        return None
    body = text.plain[:end]
    return body if is_condition(body, lowercase=False) else None


# D11: what is not extracted.


def speaks_of_reach(text: str) -> bool:
    """D11.5: whether `text` speaks of a reach, in any case."""
    folded = fold(text)
    if folded.startswith("in ") and not folded[3:].startswith(NOT_A_REACH_AFTER_IN):
        return True
    for phrase in REACH_PHRASES:
        for at in standing(folded, phrase):
            if phrase != WHEN_USED_IN:
                return True
            after = at + len(WHEN_USED_IN)
            if not (folded.startswith(" ", after) and folded[after + 1 :].startswith(SENSE_WORDS)):
                return True
    return False


def speaks_of_reach_unstated(text: str) -> bool:
    """D11.6: whether `text` holds one of the six phrases of a reach, in any case, other
    than `when used in` followed by a space and a sense word."""
    folded = fold(text)
    for phrase in REACH_PHRASES:
        for at in standing(folded, phrase):
            after = at + len(WHEN_USED_IN)
            if phrase == WHEN_USED_IN and (
                folded.startswith(" ", after) and folded[after + 1 :].startswith(SENSE_WORDS)
            ):
                continue
            return True
    return False


SECOND_TERM_FOLLOWS = re.compile(f"(?: |, [^{MARKS}]*, )(?:{FORMS})(?![A-Za-z])")


def defines_second_term(plain: str, reading: Reading, sentence_end: int) -> bool:
    """D11.1: after the verb form, up to the end of its sentence, a term stands followed
    by a space and a verb form, or by a comma, a space, text with no quotation mark, a
    comma, a space and a verb form, and that term, compared in any case, is not one of
    the reading's terms."""
    known = {fold(term) for term in reading.terms}
    for term in ONE_TERM.finditer(plain, reading.after.verb.end, sentence_end):
        follows = SECOND_TERM_FOLLOWS.match(plain, term.end(), sentence_end)
        if follows is not None and fold(recorded_term(term[1])) not in known:
            return True
    return False


def aside_speaks_of_reach(aside: str | None) -> bool:
    """D11.2: an aside that holds one of the words of D11.3, or that begins with `in` and
    a space or holds one of the six phrases, and speaks of a reach by D11.5."""
    if aside is None:
        return False
    if holds_any_case(aside, REACH_ASIDE_WORDS):
        return True
    return (
        fold(aside).startswith("in ") or holds_any_case(aside, REACH_PHRASES)
    ) and speaks_of_reach(aside)


def reach_after_verb(plain: str, verb_end: int) -> bool:
    """D11.4: the text after the verb form begins, in any case, with a comma, a space, an
    opening and a space, and the text after that comma and space, to its next comma or
    the end of the text, with a space added at its end, speaks of a reach."""
    after = fold(plain[verb_end : verb_end + 32])
    if not any(after.startswith(f", {opening} ") for opening in LOWER_OPENINGS):
        return False
    start = verb_end + 2
    comma = plain.find(",", start)
    return speaks_of_reach(plain[start : comma if comma != -1 else len(plain)] + " ")


# D12: borrowed meanings.

SOURCE_LEAD = re.compile(
    "(?: given| provided| set forth)?(?: to| for)?"
    f"(?: (?:{'|'.join(re.escape(one) for one in longest_first(POINTERS))})"
    f"(?: {SEPARATED_TERMS})?(?: has| have| does| do)?)?"
    "(?:, respectively,)?"
    f" (?:{'|'.join(re.escape(one) for one in longest_first(PREPOSITIONS))}) (?=[^ ])"
)


def source_words(plain: str, verb: Verb, the_end: int, spans: list[tuple[int, int]]) -> str | None:
    """D12.5: the source words after the verb form; `None` when the text has none."""
    if plain[verb.start : verb.end].endswith(AS_IN):
        if not (
            plain.startswith(" ", verb.end) and plain[verb.end + 1 : verb.end + 2] not in ("", " ")
        ):
            return None
        start = verb.end + 1
    else:
        lead = SOURCE_LEAD.match(plain, verb.end)
        if lead is None:
            return None
        start = lead.end()
    end = min(sentence_of(spans, start)[1], the_end)
    for stop in SOURCE_STOPS:
        at = plain.find(stop, start)
        if at != -1 and at < end:
            end = at
    words = plain[start:end].rstrip(" ,")
    if words.endswith(".") and word_before(words, len(words) - 1) not in ABBREVIATIONS:
        words = words[:-1]
    if not words or words.startswith(" ") or words.endswith(" "):
        return None
    return words


def made_under(between: str) -> bool | None:
    """D12.4: `None` when none of the words of `MADE_WORDS` stands in `between`, as
    written; otherwise whether one of them is followed, somewhere after it, by `under`
    or `pursuant to`, standing as words."""
    places = [at + len(word) for word in MADE_WORDS for at in standing(between, word)]
    if not places:
        return None
    return any(holds_words(between[place:], MADE_UNDER) for place in places)


def borrowed_source(
    text: Read, reading: Reading, the_end: int, spans: list[tuple[int, int]]
) -> tuple[str | None, str | None] | None:
    """D12: `borrowed_from` and `borrowed_words` of a borrowed definition, D12.4 before
    D12.2 and D12.3; `None` when nothing is extracted (D11.9)."""
    plain = text.plain
    verb = reading.after.verb
    after = [link for link in text.links if verb.end <= link[0] < the_end]
    first = min(after) if after else None
    if first is not None and first[2]:
        between = ONE_TERM.sub("", plain[verb.end : first[0]])
        made = made_under(between)
        if made is None:
            return first[2], None
        words = source_words(plain, verb, the_end, spans) if made else None
        return None if words is None else (None, words)
    words = source_words(plain, verb, the_end, spans)
    return None if words is None else (None, words)


# D13: the region and the note.

THIS_KIND = re.compile(
    "this (" + "|".join(REGION_KINDS) + ")(?:, unless the context.*)?", re.DOTALL
)


def region_in_unit(
    reach: Reach, path: list[Provision], holder: int
) -> tuple[str | None, str | None]:
    """D13.1, within the unit: the region of `reach`, and the kind to look for among the
    unit's containers when the Object names `this` and a kind that neither the holder
    nor its ancestors have. An Object that is `this` and a region kind is resolved by
    that kind alone; otherwise a link below the prefix of the corpus gives its target."""
    this = THIS_KIND.fullmatch(reach.object_words)
    if this is not None:
        kind = this[1]
        for provision in reversed(path[: holder + 1]):
            if provision.kind == kind:
                return provision.address, None
        return None, kind
    link = reach.link
    if link is not None and is_address(link) and link.startswith(PREFIX + "/"):
        return link, None
    return None, None


def region_words(reach: Reach) -> str:
    """D13.3: the words that name the region: `this` and the kind word, or the Object."""
    this = THIS_KIND.fullmatch(reach.object_words)
    return f"this {this[1]}" if this is not None else reach.object_words


def see_note(unit: Unit, name: str) -> str | None:
    """D13.3: the address of the first editorial note of the unit, in the order the notes
    stand in the document, whose heading is "References in Text" and one of whose own
    paragraphs (not one inside a table or quoted content) holds, in any case and standing
    as words, `name` followed by `, referred to in`."""
    words = fold(name + REFERRED_TO_IN)
    for note in unit.notes:
        if note.layer != "editorial" or note.heading is None:
            continue
        if read_text(note.heading).plain != REFERENCES_IN_TEXT:
            continue
        for block in note.blocks:
            if isinstance(block, Para) and holds_word(fold(read_text(block.text).plain), words):
                return note.address
    return None


# D15.1: the record.


@dataclass(frozen=True, slots=True)
class Definition:
    """A definition, with the fields of D15.1."""

    at: str
    terms: tuple[str, ...]
    kind: str
    words: str | None
    condition: str | None
    region: str | None
    basis: str
    borrowed_from: str | None
    borrowed_words: str | None
    see_note: str | None

    def record(self) -> dict[str, object]:
        return {
            "at": self.at,
            "terms": list(self.terms),
            "kind": self.kind,
            "words": self.words,
            "condition": self.condition,
            "region": self.region,
            "basis": self.basis,
            "borrowed_from": self.borrowed_from,
            "borrowed_words": self.borrowed_words,
            "see_note": self.see_note,
        }

    def line(self) -> str:
        return json.dumps(self.record(), ensure_ascii=False, separators=(",", ":"))


def order(definition: Definition) -> tuple[bytes, bytes]:
    """The order of the records: the bytes of the address, then of the line."""
    return definition.at.encode(), definition.line().encode()


@dataclass(frozen=True, slots=True)
class Found:
    """A definition found in a unit. The containers of the unit are read after it, so a
    region that is a container is looked for once they are: `container_kind` is the kind
    to look for from `within`, and `note` is what `see_note` gives should the basis be
    restated."""

    at: str
    terms: tuple[str, ...]
    kind: str
    words: str | None
    condition: str | None
    region: str | None
    container_kind: str | None
    within: str | None
    positive_law: bool | None
    borrowed_from: str | None
    borrowed_words: str | None
    note: str | None


# D3, D9, D10, D14: the definitions of a unit.

ORDINARY = "ordinary"
LINE = "line"
STATED_ONLY = "stated-only"


@dataclass(frozen=True, slots=True)
class Place:
    """D3.2: the place a paragraph is read at, and at a line place the line's reach."""

    kind: str
    line: Reach | None = None


ORDINARY_PLACE = Place(ORDINARY)
STATED_ONLY_PLACE = Place(STATED_ONLY)


@dataclass(frozen=True, slots=True)
class Chapeau:
    """D8.3, D9.4: what the chapeau of an ancestor decides: a reach chapeau gives its
    reach, a condition chapeau its condition, and any other chapeau neither."""

    reach: Reach | None
    condition: str | None


def chapeau_parts(provision: Provision) -> list[Part]:
    return [item for item in provision.items if isinstance(item, Part) and item.role == "chapeau"]


class UnitDefinitions:
    """The definitions that the rules extract from one unit."""

    def __init__(self, unit: Unit) -> None:
        self.unit = unit
        self.found: list[Found] = []
        self.texts: dict[int, Read] = {}
        self.chapeaus: dict[int, Chapeau | None] = {}
        self.walk([unit.root])

    def read_of(self, text: Text) -> Read:
        read = self.texts.get(id(text))
        if read is None:
            read = self.texts[id(text)] = read_text(text)
        return read

    def walk(self, path: list[Provision]) -> None:
        """D3.1, D3.2: the paragraphs of the operative provisions, each at its place."""
        for item in path[-1].items:
            if isinstance(item, Provision):
                self.walk([*path, item])
            elif item.role in ("chapeau", "content"):
                first = item.blocks[0] if item.blocks else None
                first_read = self.read_of(first.text) if isinstance(first, Para) else None
                if first_read is not None:
                    self.paragraph(path, ORDINARY_PLACE, first_read)
                later: Place | None = None
                for block in item.blocks[1:]:
                    if not isinstance(block, Para):
                        continue
                    read = self.read_of(block.text)
                    if OPEN not in read.plain:
                        continue
                    if later is None:
                        line = reach_line(first_read) if first_read is not None else None
                        later = STATED_ONLY_PLACE if line is None else Place(LINE, line)
                    self.paragraph(path, later, read)
            elif item.role in ("continuation", "proviso"):
                for block in item.blocks:
                    if isinstance(block, Para):
                        self.paragraph(path, STATED_ONLY_PLACE, self.read_of(block.text))

    def chapeau_of(self, provision: Provision) -> Chapeau | None:
        """D8.3: what the chapeau of `provision` decides; `None` when it has no chapeau
        part. A provision with more than one chapeau part has no reach chapeau and no
        condition chapeau."""
        key = id(provision)
        if key in self.chapeaus:
            return self.chapeaus[key]
        parts = chapeau_parts(provision)
        decided: Chapeau | None = None
        if parts:
            decided = Chapeau(None, None)
            if len(parts) == 1 and len(parts[0].blocks) == 1:
                block = parts[0].blocks[0]
                if isinstance(block, Para):
                    read = self.read_of(block.text)
                    line = reach_line(read)
                    if line is not None:
                        decided = Chapeau(line, None)
                    else:
                        decided = Chapeau(None, condition_line(read))
        self.chapeaus[key] = decided
        return decided

    def paragraph(self, path: list[Provision], place: Place, text: Read) -> None:
        """D3.3, D14: the paragraph read at the start of its text and again at the start
        of each later sentence."""
        plain = text.plain
        if OPEN not in plain:
            return
        spans = sentences(plain)
        for index, (start, end) in enumerate(spans):
            if plain.find(OPEN, start) == -1:
                break
            reading = read_pattern(text, start)
            if reading is not None:
                # D10.3, D12: "the end" is the end of the text for the first reading,
                # and the end of its sentence for a later one.
                self.extract(path, place, text, reading, spans, len(plain) if index == 0 else end)

    def reach_and_condition(
        self, path: list[Provision], place: Place, reading: Reading
    ) -> tuple[Reach | None, int, str | None] | None:
        """D9.1 to D9.5: the reach of the reading, the place in `path` of its holder, and
        its condition; `None` when nothing is extracted."""
        depth = len(path) - 1
        condition = reading.lead.condition
        if reading.lead.reach is not None:
            return reading.lead.reach, depth, condition
        if reading.after.reach is not None:
            return reading.after.reach, depth, condition
        if place.kind == LINE and place.line is not None:
            line = place.line
            if line.condition is not None and condition is not None:
                return None
            return line, depth, condition if condition is not None else line.condition
        if place.kind == STATED_ONLY:
            return None
        for holder in range(depth - 1, -1, -1):
            chapeau = self.chapeau_of(path[holder])
            if chapeau is None:
                continue
            if chapeau.reach is not None:
                if chapeau.reach.condition is not None and condition is not None:
                    return None
                inherited = condition if condition is not None else chapeau.reach.condition
                return chapeau.reach, holder, inherited
            if chapeau.condition is None or condition is not None:
                return None
            condition = chapeau.condition
        return (None, depth, condition) if reading.sentence else None

    def extract(
        self,
        path: list[Provision],
        place: Place,
        text: Read,
        reading: Reading,
        spans: list[tuple[int, int]],
        the_end: int,
    ) -> None:
        """D14: the steps after the pattern is found, in their order."""
        plain = text.plain
        after = reading.after
        # D10.2: a later reading with no reach of its own.
        if reading.start > 0 and reading.lead.reach is None and after.reach is None:
            if place.kind == STATED_ONLY:
                return
            before = plain[: reading.start]
            if holds_any_case(before, REACH_PHRASES) or "in this " in fold(before):
                return
        # D11.1, D11.2, D11.4.
        verb_sentence_end = sentence_of(spans, after.verb.start)[1]
        if defines_second_term(plain, reading, verb_sentence_end):
            return
        if aside_speaks_of_reach(after.aside) or reach_after_verb(plain, after.verb.end):
            return
        # D11.7: a reach after the term, under a chapeau or at a line place.
        if after.reach is not None and (
            place.kind == LINE or any(chapeau_parts(provision) for provision in path[:-1])
        ):
            return
        # D9.1 to D9.5, then D9.6.
        decided = self.reach_and_condition(path, place, reading)
        if decided is None:
            return
        reach, holder, condition = decided
        if condition is None:
            if after.aside is not None and aside_is_condition(after.aside):
                condition = after.aside
            else:
                condition = verb_condition(plain, after.verb.end, verb_sentence_end)
        # D11.6: no reach, and the text speaks of one.
        if reach is None and speaks_of_reach_unstated(plain):
            return
        # D12.
        borrowed_from: str | None = None
        borrowed_words: str | None = None
        if after.verb.kind == "borrowed":
            source = borrowed_source(text, reading, the_end, spans)
            if source is None:
                return
            borrowed_from, borrowed_words = source
        # D13.
        region: str | None = None
        container_kind: str | None = None
        note: str | None = None
        if reach is not None:
            region, container_kind = region_in_unit(reach, path, holder)
            if (
                region is not None or container_kind is not None
            ) and self.unit.positive_law is not True:
                note = see_note(self.unit, region_words(reach))
        self.found.append(
            Found(
                path[-1].address,
                reading.terms,
                after.verb.kind,
                reach.words if reach is not None else None,
                condition,
                region,
                container_kind,
                self.unit.within,
                self.unit.positive_law,
                borrowed_from,
                borrowed_words,
                note,
            )
        )


class DefinitionSink:
    """D3.1: definitions are read from the operative provisions of each unit, and from
    nothing else: not from a note, not from quoted content, and not from a container."""

    def __init__(self) -> None:
        self.found: list[Found] = []
        self.containers: dict[str, tuple[str, str | None]] = {}

    def container(self, container: Container) -> None:
        self.containers[container.address] = (container.kind, container.within)

    def unit(self, unit: Unit) -> None:
        self.found += UnitDefinitions(unit).found

    def container_of(self, within: str | None, kind: str) -> str | None:
        """D13.1: the nearest container of `kind` from the container `within` outward."""
        while within is not None and within in self.containers:
            of, outer = self.containers[within]
            if of == kind:
                return within
            within = outer
        return None

    def definitions(self) -> list[Definition]:
        """D13.2: the basis is `unresolved` with no region; with one, `enacted` in a unit
        whose `positive_law` is true and `restated` in any other. D13.3: the note is
        recorded only of a restated definition."""
        definitions = []
        for found in self.found:
            region = found.region
            if region is None and found.container_kind is not None:
                region = self.container_of(found.within, found.container_kind)
            if region is None:
                basis = "unresolved"
            elif found.positive_law is True:
                basis = "enacted"
            else:
                basis = "restated"
            definitions.append(
                Definition(
                    found.at,
                    found.terms,
                    found.kind,
                    found.words,
                    found.condition,
                    region,
                    basis,
                    found.borrowed_from,
                    found.borrowed_words,
                    found.note if basis == "restated" else None,
                )
            )
        return definitions


def work(job: Job) -> MemberResult | RefusedError:
    sink = DefinitionSink()
    try:
        _, reader, meta = read_member(job, sink)
    except RefusedError as error:
        return error
    return MemberResult(
        job.source.member,
        meta.release,
        reader.identifiers,
        reader.counts,
        sink.definitions(),
        unnamed_tops=reader.unnamed_tops,
    )


def find_definitions(path: Path, jobs: int) -> tuple[str, list[Definition]]:
    """The release of the input at `path` and its definitions, in the order of the
    records."""
    release, results = read_release(path, work, jobs)
    definitions: list[Definition] = []
    for result in results:
        output = result.output
        if not isinstance(output, list) or not all(isinstance(d, Definition) for d in output):
            raise AssertionError(output)
        definitions += output
    return release, sorted(definitions, key=order)


# The definitions of a workspace, from the `definition` blocks of its module files.


class FormError(Exception):
    """A module file holds a definition block that is not in its canonical form."""


STRING = r'"(?:[^"\\]|\\.)*"'
# A line that begins a definition block in any form.
DECLARATION = re.compile(r"[ \t]*definition(?![A-Za-z0-9_])")
HEADER = re.compile(rf"  definition ({STRING}) \{{")
# D15.2: the slots of a block, in the order of its canonical form: the names a slot takes
# (one of them, in one line), the pattern of its value, whether the block must fill it,
# and whether it takes more than one line. `borrowed_from` and `borrowed_words` share one
# slot, so that a block holds at most one of the two.
SLOTS = (
    (("term",), STRING, True, True),
    (("kind",), "means|includes|does_not_include|borrowed", True, False),
    (("words",), f"{STRING}|unstated", True, False),
    (("condition",), STRING, False, False),
    (("region",), STRING, False, False),
    (("basis",), "enacted|restated|unresolved", True, False),
    (("borrowed_from", "borrowed_words"), STRING, False, False),
    (("see_note",), STRING, False, False),
)
FIELD_LINES = {
    name: re.compile(f"    {name} ({pattern})") for names, pattern, _, _ in SLOTS for name in names
}


def decoded(literal: str, where: str) -> str:
    try:
        value = json.loads(literal)
    except json.JSONDecodeError as error:
        raise FormError(f"{where}: {literal} is no string: {error}") from None
    if not isinstance(value, str):
        raise FormError(f"{where}: {literal} is no string")
    return value


def read_block(lines: list[str], at: int, file: str) -> tuple[Definition, int]:
    """The definition block that begins at `lines[at]` of `file`, and the place of the
    line after it."""
    header = HEADER.fullmatch(lines[at])
    if header is None:
        raise FormError(
            f"{file}:{at + 1}: `{lines[at]}` begins a definition block that is not in its "
            "canonical form"
        )
    values: dict[str, list[str]] = {}
    index = at + 1
    for names, _, required, many in SLOTS:
        filled = False
        while index < len(lines):
            name = next((one for one in names if FIELD_LINES[one].fullmatch(lines[index])), None)
            if name is None:
                break
            line = FIELD_LINES[name].fullmatch(lines[index])
            assert line is not None
            values.setdefault(name, []).append(line[1])
            filled = True
            index += 1
            if not many:
                break
        if required and not filled:
            shown = lines[index] if index < len(lines) else ""
            raise FormError(
                f"{file}:{index + 1}: `{shown}` stands where the definition block of line "
                f"{at + 1} has its `{names[0]}` in its canonical form"
            )
    if index >= len(lines) or lines[index] != "  }":
        shown = lines[index] if index < len(lines) else ""
        raise FormError(
            f"{file}:{index + 1}: `{shown}` stands where the definition block of line "
            f"{at + 1} ends in its canonical form"
        )
    where = f"{file}:{at + 1}"

    def one(name: str) -> str | None:
        found = values.get(name)
        return decoded(found[0], where) if found else None

    words = values["words"][0]
    return (
        Definition(
            decoded(header[1], where),
            tuple(decoded(term, where) for term in values["term"]),
            values["kind"][0],
            None if words == "unstated" else decoded(words, where),
            one("condition"),
            one("region"),
            values["basis"][0],
            one("borrowed_from"),
            one("borrowed_words"),
            one("see_note"),
        ),
        index + 1,
    )


def module_files(top: Path, relative: str = "") -> Iterator[tuple[Path, str]]:
    """The module files below `top`, in the byte order of their paths, each with its path
    relative to `top`; an entry whose name begins with a dot is left out, and a symbolic
    link stops the reading."""
    with os.scandir(top / relative if relative else top) as listed:
        entries = sorted(listed, key=lambda entry: entry.name.encode())
    for entry in entries:
        if entry.name.startswith("."):
            continue
        name = f"{relative}/{entry.name}" if relative else entry.name
        if entry.is_symlink():
            raise FormError(f"{name} is a symbolic link, and the script follows none")
        if entry.is_dir():
            yield from module_files(top, name)
        elif entry.name.endswith(".fr"):
            yield Path(entry.path), name


def workspace_definitions(workspace: Path) -> list[Definition]:
    """The definitions that the module files of `workspace` hold, in the order of the
    records."""
    definitions: list[Definition] = []
    for path, name in module_files(workspace):
        lines = path.read_text(encoding="utf-8").split("\n")
        index = 0
        while index < len(lines):
            if DECLARATION.match(lines[index]):
                found, index = read_block(lines, index, str(workspace / name))
                definitions.append(found)
            else:
                index += 1
    return sorted(definitions, key=order)


def first_difference(found: list[Definition], held: list[Definition], workspace: str) -> str | None:
    """`None` when the release gives the definitions `found` and the workspace holds the
    same, `held`, as many times each; otherwise a sentence that gives both counts and
    names the first definition, in the order of the records, that one side has and the
    other does not. When the other side has a definition of the same provision with the
    same terms that differs from it, the sentence names the first field that differs."""
    only_found = sorted((Counter(found) - Counter(held)).elements(), key=order)
    only_held = sorted((Counter(held) - Counter(found)).elements(), key=order)
    if not only_found and not only_held:
        return None
    counts = f"the release gives {len(found)} definitions and {workspace} holds {len(held)}"
    in_release = bool(only_found) and (not only_held or order(only_found[0]) <= order(only_held[0]))
    first = only_found[0] if in_release else only_held[0]
    others = only_held if in_release else only_found
    partner = next((o for o in others if (o.at, o.terms) == (first.at, first.terms)), None)
    if partner is None:
        if in_release:
            return (
                f"{counts}; the first that differs: the release gives {first.line()}, and "
                f"{workspace} holds no such definition"
            )
        return (
            f"{counts}; the first that differs: {workspace} holds {first.line()}, and the "
            "release gives no such definition"
        )
    given, kept = (first, partner) if in_release else (partner, first)
    given_record, kept_record = given.record(), kept.record()
    name = next(key for key in given_record if given_record[key] != kept_record[key])
    return (
        f"{counts}; the first that differs: the definition of `{first.at}` with the terms "
        f"{json.dumps(list(first.terms), ensure_ascii=False)} has the {name} "
        f"{json.dumps(given_record[name], ensure_ascii=False)} in the release and "
        f"{json.dumps(kept_record[name], ensure_ascii=False)} in {workspace}"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n", 1)[0])
    parser.add_argument(
        "input", type=Path, help="a directory of archives, an archive or an XML file"
    )
    parser.add_argument(
        "--records", type=Path, required=True, help="the file to write the definitions to"
    )
    parser.add_argument(
        "--against", type=Path, help="a workspace whose definitions are compared with them"
    )
    parser.add_argument("--jobs", type=int, default=4, help="worker processes (default 4)")
    args = parser.parse_args(argv)
    try:
        release, found = find_definitions(args.input, args.jobs)
        with args.records.open("w", encoding="utf-8", newline="\n") as out:
            out.writelines(definition.line() + "\n" for definition in found)
        held = None if args.against is None else workspace_definitions(args.against)
    except (RefusedError, FormError) as error:
        shown = error.describe() if isinstance(error, RefusedError) else str(error)
        print(f"refused: {shown}", file=sys.stderr)
        return 1
    except (InputError, OSError, UnicodeDecodeError) as error:
        print(f"cannot read or write: {error}", file=sys.stderr)
        return 2
    summary = f"{len(found)} definitions in the release {release}"
    if held is None:
        print(summary)
        return 0
    difference = first_difference(found, held, str(args.against))
    if difference is not None:
        print(difference, file=sys.stderr)
        return 1
    print(f"{summary}, the same as {args.against} holds")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
