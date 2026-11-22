#!/usr/bin/env python3
"""Find the definitions of the Code's official XML by a second implementation of the rules.

Reads the official XML release point published by the Office of the Law Revision
Counsel, as `uslm_text.py` reads it (a directory of archives, one archive, or one XML
file), with the independent reading of `uslm_common.py`, and extracts from its units
the definitions that the rules of Definitions in `docs/law-modules.md` find. Those
rules are section S10 of the specification the importer was built from; this is a
second implementation of them, written from their text, that shares no code with the
importer. Each rule below names the part of S10 it implements: the opening paragraph
("Where"), Verbs, Patterns, Reach, A reach chapeau, The reach of a paragraph, What is
not extracted, Region, Basis, Borrowed and The note.

`--records FILE` gets one JSON object for each definition, to a line, with the fields
of a `definition` block: `at`, `terms`, `kind`, `words` (`null` for `unstated`),
`region`, `basis`, `borrowed_from` and `see_note`, `null` where the block has none. The
lines are sorted by the bytes of `at`, then by the bytes of the line.

`--against DIR` compares them with the definitions that the `definition` blocks of the
module files of the workspace `DIR` hold, read in their canonical form: a line that
begins with the word `definition` must begin a block in that form. Unless the two are
the same, the script names the first definition that differs and exits 1.

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

# The quotation marks that enclose a term: wherever the rules speak of a quotation mark,
# they mean one of these two.
OPEN = "“"
CLOSE = "”"
MARKS = OPEN + CLOSE

# Verbs: each verb form, with the kind of definition it gives.
VERB_FORMS = {
    "means": "means",
    "mean": "means",
    "shall mean": "means",
    "includes": "includes",
    "include": "includes",
    "shall include": "includes",
    "does not include": "does_not_include",
    "do not include": "does_not_include",
    "shall not include": "does_not_include",
    "has the meaning given": "borrowed",
    "have the meaning given": "borrowed",
    "has the meanings given": "borrowed",
    "have the meanings given": "borrowed",
    "has the same meaning as": "borrowed",
    "have the same meaning as": "borrowed",
    "shall have the meaning given": "borrowed",
}
# Reach: the openings.
OPENINGS = (
    "As used in",
    "In",
    "For purposes of",
    "For the purposes of",
    "For the purpose of",
    "For purpose of",
    "When used in",
)
# Reach: the words an object holds none of, in any case.
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
# What is not extracted: the six phrases that speak of a reach, in any case.
REACH_PHRASES = (
    "as used in",
    "when used in",
    "for purposes of",
    "for the purposes of",
    "for the purpose of",
    "for purpose of",
)
# A reach chapeau: the lead-ins, and the closes.
LEAD_INS = (
    "the term",
    "the terms",
    "the following definitions apply",
    "the following definitions shall apply",
)
CLOSES = ("—", "-", ":")
# Region: the kinds a region is named by after `this`.
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
# The note: the heading of the note a restated definition points at.
REFERENCES_IN_TEXT = "References in Text"

ASCII_LOWER = str.maketrans("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz")

# Verbs: a verb form stands where its words are followed by the end of the text or by a
# character that is not one of the letters A to Z and a to z; inside other text, the
# character before it is not such a letter either.
FORMS = "|".join(re.escape(form) for form in sorted(VERB_FORMS, key=len, reverse=True))
NO_LETTER_AFTER = "(?![A-Za-z])"
NO_LETTER_BEFORE = "(?<![A-Za-z])"
SPACE_AND_VERB = re.compile(f" (?P<verb>{FORMS}){NO_LETTER_AFTER}")
VERB_IN_TEXT = re.compile(f"{NO_LETTER_BEFORE}(?:{FORMS}){NO_LETTER_AFTER}")
# Where: `Q`, a term in quotation marks, with at least one character and neither mark
# inside it. Patterns: one or more of them, with a separator between two.
TERM = f"{OPEN}[^{MARKS}]+{CLOSE}"
ONE_TERM = re.compile(f"{OPEN}([^{MARKS}]+){CLOSE}")
TERMS = re.compile(f"{TERM}(?:(?:, and |, or |, | and | or ){TERM})*")
# Patterns: what the sentence form names before its terms, and the aside between two
# commas before its verb.
HEAD = "(?:the|The) (?:term|terms|word|words|phrase) "
HEAD_AT_START = re.compile(HEAD)
HEAD_AFTER_COMMA = re.compile(", " + HEAD)
ASIDE_AND_VERB = re.compile(f", (?P<aside>[^{MARKS}]*?), (?P<verb>{FORMS}){NO_LETTER_AFTER}")
# What is not extracted: a term after the verb followed by a space and a verb form, or by
# a comma, a space, text with no quotation mark, a comma, a space and a verb form.
SECOND_TERM = re.compile(f"{TERM}(?:, [^{MARKS}]*,)? (?:{FORMS}){NO_LETTER_AFTER}")
# Region: `this`, a space and a kind, then nothing more or a comma, a space and text that
# begins `unless the context`.
THIS_KIND = re.compile(
    "this (" + "|".join(REGION_KINDS) + ")(?:, unless the context.*)?", re.DOTALL
)


def fold(text: str) -> str:
    """Where: "in any case", the letters A to Z are compared as a to z, and no other
    character is changed."""
    return text.translate(ASCII_LOWER)


def whole_words(words: Iterable[str]) -> re.Pattern[str]:
    """Where: the words that a text holds are whole words, each bounded by the start or
    the end of the text or by a character that is not one of the letters A to Z and a to
    z. The pattern finds any of `words` so in a text that `fold` gave."""
    either = "|".join(re.escape(fold(one)) for one in words)
    return re.compile(f"(?<![a-z])(?:{either})(?![a-z])")


GUARD_WORDS = whole_words(NOT_IN_AN_OBJECT)
PHRASES = whole_words(REACH_PHRASES)


def holds(pattern: re.Pattern[str], text: str) -> bool:
    """Whether `text` holds, in any case and as whole words, what `pattern` of
    `whole_words` finds."""
    return pattern.search(fold(text)) is not None


@dataclass(frozen=True, slots=True)
class Definition:
    """A definition, with the fields of its `definition` block."""

    at: str
    terms: tuple[str, ...]
    kind: str
    words: str | None
    region: str | None
    basis: str
    borrowed_from: str | None
    see_note: str | None

    def record(self) -> dict[str, object]:
        return {
            "at": self.at,
            "terms": list(self.terms),
            "kind": self.kind,
            "words": self.words,
            "region": self.region,
            "basis": self.basis,
            "borrowed_from": self.borrowed_from,
            "see_note": self.see_note,
        }

    def line(self) -> str:
        return json.dumps(self.record(), ensure_ascii=False, separators=(",", ":"))


def order(definition: Definition) -> tuple[bytes, bytes]:
    """The order of the records: the bytes of the address, then of the line."""
    return definition.at.encode(), definition.line().encode()


@dataclass(frozen=True, slots=True)
class Reach:
    """A reach as a text states it: its words, from the opening to the end of the object;
    the object; the target of the link whose words are the whole object, if there is one;
    and the place, in the path from the top provision of the unit, of the provision whose
    text states it."""

    words: str
    object_words: str
    link: str | None
    holder: int


def stated_reach(text: Text, end: int, holder: int) -> Reach | None:
    """Reach: the reach that `text` states from its start to `end`, when that text is
    `Opening Object`. The object is the text after the opening and a space: it is not
    empty, it holds no quotation mark and none of the words of `NOT_IN_AN_OBJECT`, in any
    case. After `In` it begins with `this` and a space, or is a single link, an object
    whose words are exactly the words of one link of the paragraph."""
    words = text.plain[:end]
    opening = next((one for one in OPENINGS if words.startswith(one + " ")), None)
    if opening is None:
        return None
    start = len(opening) + 1
    object_words = words[start:]
    if not object_words or any(mark in object_words for mark in MARKS):
        return None
    if holds(GUARD_WORDS, object_words):
        return None
    link = next((target for at, stop, target in text.refs if (at, stop) == (start, end)), None)
    if opening == "In" and not object_words.startswith("this ") and link is None:
        return None
    return Reach(words, object_words, link, holder)


@dataclass(frozen=True, slots=True)
class Pattern:
    """A paragraph that matches a pattern: whether in the sentence form or the list form,
    the reach it begins with, its terms, the kind of its verb form, where the verb form
    ends, and the aside of the sentence form, when it has one."""

    sentence: bool
    reach: Reach | None
    terms: tuple[str, ...]
    kind: str
    verb_end: int
    aside: str | None


def match_pattern(text: Text, holder: int) -> Pattern | None:
    """Patterns: the pattern the paragraph `text`, of the provision at `holder` in its
    path, matches. The sentence form begins
    `[Reach ","] ("the"|"The") ("term"|"terms"|"word"|"words"|"phrase") Q
    { ("," | "and" | "or" | ", and" | ", or") Q } ["," Aside ","] Verb`; the list form
    begins `Q { ... Q } Verb`. One space stands between two parts, and a comma directly
    after the part before it. `Aside` is text with no quotation mark and no verb form.
    In the sentence form a reach ends at the comma that stands before the words that
    name what it defines, with no quotation mark before that comma; a paragraph whose
    text before those words is anything other than a reach matches neither form."""
    plain = text.plain
    reach: Reach | None = None
    sentence = not plain.startswith(OPEN)
    at = 0
    if sentence:
        head = HEAD_AT_START.match(plain)
        if head is None:
            first_mark = next((place for place, one in enumerate(plain) if one in MARKS), None)
            head = HEAD_AFTER_COMMA.search(
                plain, 0, len(plain) if first_mark is None else first_mark
            )
            if head is None:
                return None
            reach = stated_reach(text, head.start(), holder)
            if reach is None:
                return None
        at = head.end()
    terms = TERMS.match(plain, at)
    if terms is None:
        return None
    aside: str | None = None
    verb = SPACE_AND_VERB.match(plain, terms.end())
    if verb is None and sentence:
        verb = ASIDE_AND_VERB.match(plain, terms.end())
        if verb is None or VERB_IN_TEXT.search(verb["aside"]) is not None:
            return None
        aside = verb["aside"]
    if verb is None:
        return None
    return Pattern(
        sentence,
        reach,
        tuple(ONE_TERM.findall(terms.group())),
        VERB_FORMS[verb["verb"]],
        verb.end(),
        aside,
    )


def not_extracted(plain: str, pattern: Pattern) -> bool:
    """What is not extracted, the three cases a paragraph decides by itself: it defines a
    second term (after the verb, the text holds a term followed by a space and a verb
    form, or by a comma, a space, text with no quotation mark, a comma, a space and a
    verb form); its aside speaks of a reach (it begins with `in` and a space, or holds
    one of `REACH_PHRASES`, in any case); or a reach follows its verb (the verb is
    followed by a comma, a space, one of the openings in any case, and a space)."""
    if SECOND_TERM.search(plain, pattern.verb_end) is not None:
        return True
    aside = pattern.aside
    if aside is not None and (fold(aside).startswith("in ") or holds(PHRASES, aside)):
        return True
    after = fold(plain[pattern.verb_end :])
    return any(after.startswith(", " + fold(opening) + " ") for opening in OPENINGS)


def chapeaus(provision: Provision) -> list[Part]:
    return [item for item in provision.items if isinstance(item, Part) and item.role == "chapeau"]


def chapeau_reach(provision: Provision, holder: int) -> Reach | None:
    """A reach chapeau: the reach of the chapeau of `provision`, at `holder` in the path,
    when that chapeau is one paragraph whose text is `Reach [[","] LeadIn] Close`, and
    nothing more. `Close` is a dash (U+2014 or `-`) or a colon, with at most one space
    before it; the lead-in and the close are no part of the reach."""
    parts = chapeaus(provision)
    if len(parts) != 1 or len(parts[0].blocks) != 1:
        return None
    paragraph = parts[0].blocks[0]
    if not isinstance(paragraph, Para):
        return None
    text = paragraph.text
    plain = text.plain
    if not plain.endswith(CLOSES):
        return None
    end = len(plain) - 1
    if plain[:end].endswith(" "):
        end -= 1
    for lead_in in LEAD_INS:
        before = next((one for one in (", ", " ") if plain[:end].endswith(one + lead_in)), None)
        if before is not None:
            end -= len(before) + len(lead_in)
            break
    return stated_reach(text, end, holder)


def reach_of(path: list[Provision], pattern: Pattern) -> tuple[bool, Reach | None]:
    """The reach of a paragraph: whether it is extracted so far, and its reach. A
    paragraph of the sentence form that begins with a reach has that reach. Any other
    looks to the nearest ancestor of its provision that has a chapeau: a reach chapeau
    gives its reach, and any other chapeau leaves the paragraph unread. With no such
    ancestor, the sentence form has no reach and the list form is not extracted."""
    if pattern.reach is not None:
        return True, pattern.reach
    for holder in range(len(path) - 2, -1, -1):
        if chapeaus(path[holder]):
            reach = chapeau_reach(path[holder], holder)
            return reach is not None, reach
    return pattern.sentence, None


def borrowed_from(text: Text, verb_end: int) -> str | None:
    """Borrowed: the target of the first link that begins at or after the end of the verb
    form; `None` when there is none, or when that link has an empty target, and then the
    paragraph is not extracted."""
    after = [(start, target) for start, _, target in text.refs if start >= verb_end]
    if not after:
        return None
    _, target = min(after)
    return target or None


def region_in_unit(reach: Reach, path: list[Provision]) -> tuple[str | None, str | None]:
    """Region, within the unit: the region of `reach`, and the kind to look for among
    the containers of the unit when the object names one and no provision does. When the
    object is `this` and a kind (`THIS_KIND`), the region is the nearest of that kind,
    counted from the provision whose text holds the reach, that provision included, then
    its ancestors, then the unit's containers from the nearest outward. When the object
    is a single link whose target is an address below the prefix of the corpus, the
    region is that target. Otherwise there is none."""
    this = THIS_KIND.fullmatch(reach.object_words)
    if this is not None:
        kind = this[1]
        for provision in reversed(path[: reach.holder + 1]):
            if provision.kind == kind:
                return provision.address, None
        return None, kind
    link = reach.link
    if link is not None and is_address(link) and link.startswith(PREFIX + "/"):
        return link, None
    return None, None


def region_words(reach: Reach) -> str:
    """The note: the words that name the region, `this` and the kind word, or the words
    of the link."""
    this = THIS_KIND.fullmatch(reach.object_words)
    return f"this {this[1]}" if this is not None else reach.object_words


def see_note(unit: Unit, name: str) -> str | None:
    """The note: the address of the first editorial note of the unit headed "References
    in Text", one of whose own paragraphs (not one inside a table or quoted content)
    holds, in any case, `name` followed by `, referred to in`. "First" is in document
    order, the order in which the reading gives the notes of a unit, those of its
    provisions among them."""
    referred = whole_words([name + ", referred to in"])
    for note in unit.notes:
        if note.layer != "editorial" or note.heading is None:
            continue
        if note.heading.plain != REFERENCES_IN_TEXT:
            continue
        if any(
            isinstance(block, Para) and holds(referred, block.text.plain) for block in note.blocks
        ):
            return note.address
    return None


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
    region: str | None
    container_kind: str | None
    within: str | None
    positive_law: bool | None
    borrowed_from: str | None
    note: str | None


def paragraph_definition(unit: Unit, path: list[Provision], text: Text) -> Found | None:
    """The definition of the first paragraph `text` of a chapeau or content part of the
    provision at the end of `path`, if the rules extract one. What is not extracted, the
    last case: a definition with no reach is not extracted when its text holds one of
    `REACH_PHRASES`, in any case."""
    pattern = match_pattern(text, len(path) - 1)
    if pattern is None or not_extracted(text.plain, pattern):
        return None
    extracted, reach = reach_of(path, pattern)
    if not extracted or (reach is None and holds(PHRASES, text.plain)):
        return None
    borrowed: str | None = None
    if pattern.kind == "borrowed":
        borrowed = borrowed_from(text, pattern.verb_end)
        if borrowed is None:
            return None
    region: str | None = None
    container_kind: str | None = None
    note: str | None = None
    if reach is not None:
        region, container_kind = region_in_unit(reach, path)
        if (region is not None or container_kind is not None) and unit.positive_law is not True:
            note = see_note(unit, region_words(reach))
    return Found(
        path[-1].address,
        pattern.terms,
        pattern.kind,
        reach.words if reach is not None else None,
        region,
        container_kind,
        unit.within,
        unit.positive_law,
        borrowed,
        note,
    )


class DefinitionSink:
    """Where: a definition is extracted from the first block of a `chapeau` or `content`
    part of a provision of a unit, when that block is a paragraph, and from nothing else:
    not from a note, not from quoted content, and not from a container. The text of a
    paragraph is its plain text, the words of its runs without its footnote markers."""

    def __init__(self) -> None:
        self.found: list[Found] = []
        self.containers: dict[str, tuple[str, str | None]] = {}

    def container(self, container: Container) -> None:
        self.containers[container.address] = (container.kind, container.within)

    def unit(self, unit: Unit) -> None:
        self.read(unit, [unit.root])

    def read(self, unit: Unit, path: list[Provision]) -> None:
        for item in path[-1].items:
            if isinstance(item, Provision):
                self.read(unit, [*path, item])
                continue
            first = item.blocks[0] if item.blocks else None
            if item.role in ("chapeau", "content") and isinstance(first, Para):
                found = paragraph_definition(unit, path, first.text)
                if found is not None:
                    self.found.append(found)

    def container_of(self, within: str | None, kind: str) -> str | None:
        """Region: the nearest container of `kind` from the container `within` outward."""
        while within is not None and within in self.containers:
            of, outer = self.containers[within]
            if of == kind:
                return within
            within = outer
        return None

    def definitions(self) -> list[Definition]:
        """Basis: `unresolved` with no region; with one, `enacted` in a unit whose file
        declares its title positive law and `restated` in any other. The note is recorded
        only of a restated definition."""
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
                    region,
                    basis,
                    found.borrowed_from,
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
# The lines of a block, in the order of its canonical form: the name of each, the
# pattern of its value, and whether the block must have it and may have more than one.
FIELDS = (
    ("term", STRING, True, True),
    ("kind", "means|includes|does_not_include|borrowed", True, False),
    ("words", f"{STRING}|unstated", True, False),
    ("region", STRING, False, False),
    ("basis", "enacted|restated|unresolved", True, False),
    ("borrowed_from", STRING, False, False),
    ("see_note", STRING, False, False),
)
FIELD_LINES = {name: re.compile(f"    {name} ({pattern})") for name, pattern, _, _ in FIELDS}


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
    for name, _, required, many in FIELDS:
        while index < len(lines) and (line := FIELD_LINES[name].fullmatch(lines[index])):
            values.setdefault(name, []).append(line[1])
            index += 1
            if not many:
                break
        if required and name not in values:
            shown = lines[index] if index < len(lines) else ""
            raise FormError(
                f"{file}:{index + 1}: `{shown}` stands where the definition block of line "
                f"{at + 1} has its `{name}` in its canonical form"
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
            one("region"),
            values["basis"][0],
            one("borrowed_from"),
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
