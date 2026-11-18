"""Read the publisher's XML of the United States Code into provisions, notes and containers.

This is an independent reading of the official XML release point published by the
Office of the Law Revision Counsel. It shares no code with the Rust ingester. The two
scripts beside it use it: `uslm_text.py` prints the text records it reads, and
`usc_profile.py` counts them.

A release is read from a directory of archives, from one archive, or from one XML
file. Each member is parsed with `defusedxml`, refused when it breaks a reading rule,
and read into units and containers one at a time. Nothing is written anywhere.
"""

from __future__ import annotations

import itertools
import multiprocessing
import os
import re
import struct
import unicodedata
import zipfile
import zlib
from collections import Counter
from collections.abc import Callable, Iterable, Iterator
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass, field, fields
from pathlib import Path
from typing import NoReturn, Protocol
from xml.etree.ElementTree import Element, ParseError

import blake3
import defusedxml
from defusedxml import ElementTree as SafeXml

USLM = "http://xml.house.gov/schemas/uslm/1.0"
XHTML = "http://www.w3.org/1999/xhtml"
XSI_SCHEMA_LOCATION = "{http://www.w3.org/2001/XMLSchema-instance}schemaLocation"
SCHEMA_FILE = "USLM-1.0.15.xsd"
PREFIX = "/us/usc"
MAX_MEMBER_BYTES = 512 * 1024 * 1024
# The local header of a member of a zip archive: its signature, and its length before the
# member's name and extra field.
LOCAL_HEADER = b"PK\x03\x04"
LOCAL_HEADER_BYTES = 30
# The most bytes read or decompressed at once while a member is held to its stream.
CHUNK_BYTES = 1 << 20
# The deepest an element stands in its module, and a container among the containers of
# its file: the limit of nesting of a module in the grammar of edition 2.
MAX_NESTING = 64
MEMBER_NAME = re.compile(r"usc[0-9]{2}[aA]?\.xml")

STATUS_VALUES = frozenset(
    {
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
    }
)

CONTAINER_TAGS = frozenset(
    {
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
    }
)
UNIT_TAGS = frozenset({"section", "courtRule", "reorganizationPlan"})
SMALL_LEVEL_TAGS = frozenset(
    {
        "subsection",
        "paragraph",
        "subparagraph",
        "clause",
        "subclause",
        "item",
        "subitem",
        "subsubitem",
    }
)
LEVEL_TAGS = CONTAINER_TAGS | UNIT_TAGS | SMALL_LEVEL_TAGS | {"level"}
TEXT_PARTS = frozenset({"chapeau", "content", "continuation", "proviso"})
P_TAGS = frozenset({"p", "xhtml:p"})
SUP_TAGS = frozenset({"sup", "xhtml:sup"})
SUB_TAGS = frozenset({"sub", "xhtml:sub"})
DROPPED_TAGS = frozenset(
    {"date", "i", "b", "inline", "xhtml:span", "xhtml:b", "xhtml:i", "xhtml:a", "name", "role"}
)
INLINE_TAGS = DROPPED_TAGS | SUP_TAGS | SUB_TAGS | {"ref", "br"}
ROW_GROUPS = {"xhtml:thead": "head", "xhtml:tbody": "body", "xhtml:tfoot": "foot"}
EDITORIAL_TOPICS = frozenset({"removalDescription", "historicalAndRevision"})
LAW_ORIGINS = ("/us/pl/", "/us/act/", "/us/stat/")
LAW_LINKS = ("/us/pl/", "/us/act/")
RELEASE_NOTE_START = "Current through "
# The en dash, which a publication name may write in the place of the hyphen of its release.
EN_DASH = chr(0x2013)

# The 25 characters with the Unicode property White_Space.
WHITE_SPACE = (
    "\t\n\x0b\x0c\r\x20\x85\xa0\u1680"
    + "".join(chr(code) for code in range(0x2000, 0x200B))
    + "\u2028\u2029\u202f\u205f\u3000"
)
WHITE_SET = frozenset(WHITE_SPACE)
WHITE_RUN = re.compile("[" + re.escape(WHITE_SPACE) + "]+")
SEGMENT = re.compile("[A-Za-z0-9.~\u2013-]+")
GROUP_WORD = re.compile("[A-Za-z][A-Za-z0-9]*")
ASCII_NUMBER = re.compile("[0-9]+")
LABEL_NAMES = {"num": "num", "heading": "heading", "sourceCredit": "source credit"}


class RefusedError(Exception):
    """A document, a member or a release that the reading rules refuse."""

    def __init__(
        self, reason: str, member: str = "", element: str = "", at: str = "", archive: str = ""
    ) -> None:
        super().__init__(reason)
        self.reason = reason
        self.member = member
        self.element = element
        self.at = at
        self.archive = archive

    def __reduce__(self) -> tuple[type[RefusedError], tuple[str, str, str, str, str]]:
        return (RefusedError, (self.reason, self.member, self.element, self.at, self.archive))

    def describe(self) -> str:
        place = ": ".join(part for part in (self.archive, self.member) if part)
        detail = []
        if self.element:
            detail.append(f"element {self.element}")
        if self.at:
            detail.append(f"at {self.at}")
        suffix = f" ({', '.join(detail)})" if detail else ""
        return f"{place + ': ' if place else ''}{self.reason}{suffix}"


class InputError(Exception):
    """An input that cannot be opened or read at all."""


def normalize(raw: str) -> str:
    """Normalization Form C, every white-space character to one space, runs collapsed, trimmed."""
    composed = unicodedata.normalize("NFC", raw)
    return WHITE_RUN.sub(" ", composed).strip(" ")


def is_segment(text: str) -> bool:
    return bool(SEGMENT.fullmatch(text)) and text not in (".", "..")


def is_address(text: str) -> bool:
    if not text.startswith("/"):
        return False
    return all(is_segment(segment) for segment in text[1:].split("/"))


def split_at_spaces(value: str | None) -> list[str]:
    """The members of an identifier or the topics of a note: split at U+0020 only."""
    return [member for member in (value or "").split(" ") if member]


def is_within(target: str, prefix: str) -> bool:
    return target == prefix or target.startswith(prefix + "/")


def tag_of(element: Element) -> str:
    """The element's name: plain for the publisher's namespace, `xhtml:` for XHTML."""
    tag = element.tag
    if tag.startswith("{" + USLM + "}"):
        return tag[len(USLM) + 2 :]
    if tag.startswith("{" + XHTML + "}"):
        return "xhtml:" + tag[len(XHTML) + 2 :]
    return "?" + tag


def is_footnote(element: Element) -> bool:
    return tag_of(element) == "note" and element.get("type") == "footnote"


def is_marker(element: Element) -> bool:
    return "footnoteRef" in (element.get("class") or "").split()


def is_blank(text: str | None) -> bool:
    return text is None or all(character in WHITE_SET for character in text)


# The model that both scripts read.


@dataclass(frozen=True, slots=True)
class Text:
    """A text: its normalized plain words and, by offsets in code points, its runs."""

    plain: str
    refs: tuple[tuple[int, int, str], ...] = ()
    sup: tuple[tuple[int, int], ...] = ()
    sub: tuple[tuple[int, int], ...] = ()
    marks: tuple[tuple[int, str, int], ...] = ()

    def is_words_only(self) -> bool:
        return not (self.refs or self.sup or self.sub or self.marks)


@dataclass(frozen=True, slots=True)
class Num:
    value: str
    shown: Text


@dataclass(frozen=True, slots=True)
class Para:
    text: Text


@dataclass(frozen=True, slots=True)
class Subheading:
    text: Text


@dataclass(frozen=True, slots=True)
class Image:
    source: str


@dataclass(frozen=True, slots=True)
class Cell:
    header: bool
    colspan: int
    rowspan: int
    blocks: list[Block]


@dataclass(frozen=True, slots=True)
class Row:
    group: str
    cells: list[Cell]


@dataclass(frozen=True, slots=True)
class Table:
    rows: list[Row]


@dataclass(frozen=True, slots=True)
class Part:
    role: str
    blocks: list[Block]


@dataclass(frozen=True, slots=True)
class QuotedLevel:
    kind: str
    num: Num | None
    heading: Text | None
    items: list[QuotedItem]


@dataclass(frozen=True, slots=True)
class Quoted:
    origin: str | None
    items: list[QuotedItem]


Block = Para | Subheading | Table | Quoted | Image
QuotedItem = Para | Subheading | Table | Quoted | Image | Part | QuotedLevel


@dataclass(slots=True)
class Provision:
    address: str
    kind: str
    num: Num | None
    heading: Text | None
    status: str | None
    items: list[Part | Provision]


@dataclass(frozen=True, slots=True)
class Note:
    address: str
    layer: str
    group: str
    topics: list[str]
    heading: Text | None
    blocks: list[Block]


@dataclass(frozen=True, slots=True)
class Footnote:
    ordinal: int
    shown: str
    blocks: list[Block]


@dataclass(slots=True)
class Unit:
    address: str
    aliases: list[str]
    within: str | None
    source: str
    positive_law: bool | None
    root: Provision
    credit: Text | None
    notes: list[Note]
    footnotes: list[Footnote]


@dataclass(slots=True)
class Container:
    address: str
    aliases: list[str]
    within: str | None
    source: str
    positive_law: bool | None
    kind: str
    num: Num | None
    heading: Text | None
    status: str | None
    members: list[str]
    credit: Text | None
    notes: list[Note]
    footnotes: list[Footnote]


def provisions_of(provision: Provision) -> Iterator[Provision]:
    """The provision and every provision below it, in document order."""
    yield provision
    for item in provision.items:
        if isinstance(item, Provision):
            yield from provisions_of(item)


def texts_of_blocks(blocks: Iterable[Block | QuotedItem]) -> Iterator[Text]:
    """Every text inside the blocks, in document order."""
    for block in blocks:
        match block:
            case Para(text) | Subheading(text):
                yield text
            case Table(rows):
                for row in rows:
                    for cell in row.cells:
                        yield from texts_of_blocks(cell.blocks)
            case Quoted(_, items):
                yield from texts_of_blocks(items)
            case Part(_, inner):
                yield from texts_of_blocks(inner)
            case QuotedLevel(_, num, heading, items):
                if num is not None:
                    yield num.shown
                if heading is not None:
                    yield heading
                yield from texts_of_blocks(items)
            case Image():
                pass


def has_image(blocks: Iterable[Block | QuotedItem]) -> bool:
    for block in blocks:
        match block:
            case Image():
                return True
            case Table(rows):
                if any(has_image(cell.blocks) for row in rows for cell in row.cells):
                    return True
            case Quoted(_, items) | QuotedLevel(_, _, _, items):
                if has_image(items):
                    return True
            case Part(_, inner):
                if has_image(inner):
                    return True
            case _:
                pass
    return False


def provision_texts(provision: Provision) -> Iterator[Text]:
    """The texts of one provision itself, its children left out."""
    if provision.num is not None:
        yield provision.num.shown
    if provision.heading is not None:
        yield provision.heading
    for item in provision.items:
        if isinstance(item, Part):
            yield from texts_of_blocks(item.blocks)


def provision_has_image(provision: Provision) -> bool:
    return any(has_image(item.blocks) for item in provision.items if isinstance(item, Part))


def note_texts(note: Note) -> Iterator[Text]:
    if note.heading is not None:
        yield note.heading
    yield from texts_of_blocks(note.blocks)


# The note hash: BLAKE3 over frames of one tag byte, an eight-byte length and a value.


def frame(tag: int, value: bytes) -> bytes:
    return bytes([tag]) + len(value).to_bytes(8, "little") + value


def text_frames(text: Text) -> bytes:
    plain = text.plain
    out = [frame(0x10, plain.encode("utf-8"))]

    def offset(position: int) -> bytes:
        return len(plain[:position].encode("utf-8")).to_bytes(8, "little")

    runs: list[tuple[int, int, int, str]] = [(start, end, 0x11, t) for start, end, t in text.refs]
    runs += [(start, end, 0x12, "") for start, end in text.sup]
    runs += [(start, end, 0x13, "") for start, end in text.sub]
    for start, end, tag, target in sorted(runs):
        value = offset(start) + offset(end) + target.encode("utf-8")
        out.append(frame(tag, value))
    return b"".join(out)


def optional_text_frames(text: Text | None) -> bytes:
    return frame(0x1F, b"") if text is None else text_frames(text)


def num_frames(num: Num | None) -> bytes:
    if num is None:
        return frame(0x04, b"")
    return frame(0x02, num.value.encode("utf-8")) + frame(0x03, text_frames(num.shown))


PART_ROLES = {"chapeau": 0, "content": 1, "continuation": 2, "proviso": 3}
GROUP_CODES = {"head": 0, "body": 1, "foot": 2}


def item_frames(item: Block | QuotedItem) -> bytes:
    match item:
        case Para(text):
            return frame(0x20, text_frames(text))
        case Subheading(text):
            return frame(0x21, text_frames(text))
        case Table(rows):
            encoded = b""
            for row in rows:
                cells = b""
                for cell in row.cells:
                    value = cell.colspan.to_bytes(4, "little") + cell.rowspan.to_bytes(4, "little")
                    value += b"".join(item_frames(block) for block in cell.blocks)
                    cells += frame(0x35 if cell.header else 0x34, value)
                encoded += frame(0x30 + GROUP_CODES[row.group], cells)
            return frame(0x22, encoded)
        case Quoted(origin, items):
            value = frame(0x40, (origin or "").encode("utf-8"))
            value += b"".join(item_frames(inner) for inner in items)
            return frame(0x23, value)
        case Image(source):
            return frame(0x24, source.encode("utf-8"))
        case Part(role, blocks):
            return frame(0x50 + PART_ROLES[role], b"".join(item_frames(b) for b in blocks))
        case QuotedLevel(kind, num, heading, items):
            value = frame(0x01, kind.encode("utf-8")) + num_frames(num)
            value += optional_text_frames(heading)
            value += b"".join(item_frames(inner) for inner in items)
            return frame(0x41, value)
    raise AssertionError(item)


def note_hash(group: str, topics: list[str], heading: Text | None, blocks: list[Block]) -> str:
    value = frame(0x00, b"fidryn.note/v1")
    value += frame(0x70, group.encode("utf-8"))
    value += frame(0x71, " ".join(topics).encode("utf-8"))
    value += optional_text_frames(heading)
    value += b"".join(item_frames(block) for block in blocks)
    return blake3.blake3(value).hexdigest()


# Building a text from the inline content of the XML.


class PrefixLengths:
    """For a raw text, the length of `normalize(raw[:p])` for any position p."""

    def __init__(self, raw: str) -> None:
        self.raw = raw
        self.lengths: list[int] | None = None
        if unicodedata.is_normalized("NFC", raw):
            # A prefix of a text in Normalization Form C is in that form too, so the
            # length can be counted in one pass.
            lengths = [0]
            count = 0
            started = False
            pending = False
            for character in raw:
                if character in WHITE_SET:
                    pending = started
                else:
                    count += 2 if pending else 1
                    pending = False
                    started = True
                lengths.append(count)
            self.lengths = lengths

    def at(self, position: int) -> int:
        if self.lengths is not None:
            return self.lengths[position]
        return len(normalize(self.raw[:position]))


@dataclass(slots=True)
class Scope:
    """The module a text belongs to: its footnotes, by the `id` of each."""

    address: str
    ordinals: dict[str, int] = field(default_factory=dict)
    footnotes: list[Footnote] = field(default_factory=list)


@dataclass(frozen=True, slots=True)
class Context:
    """Where a flow stands: in a note (where a signature, any subheading and any heading
    other than the note's own are blocks, at any depth, whether or not the note has a
    heading of its own), in quoted content, whether it is the note's own flow (where a
    text part is read as a block), and whether it is quoted content's own flow (where a
    heading or subheading is a subheading)."""

    in_note: bool = False
    in_quoted: bool = False
    note_flow: bool = False
    quoted_top: bool = False

    def inner(self) -> Context:
        """The context of a flow inside this one."""
        return Context(self.in_note, self.in_quoted)


@dataclass(slots=True)
class DocumentCounts:
    tables_of_contents: int = 0
    footnotes_in_tables_of_contents: int = 0
    links_in_tables_of_contents: int = 0
    release_notes: int = 0
    links_without_words: int = 0
    note_group_headings: int = 0
    disambiguated: int = 0
    assigned: int = 0

    def add(self, other: DocumentCounts) -> None:
        for name in (f.name for f in fields(self)):
            setattr(self, name, getattr(self, name) + getattr(other, name))


class TextBuilder:
    """Gathers the text nodes of a paragraph, a heading or a number, and its runs."""

    def __init__(self, reader: MemberReader, scope: Scope) -> None:
        self.reader = reader
        self.scope = scope
        self.pieces: list[str] = []
        self.length = 0
        self.spans: list[tuple[str, int, int, str]] = []
        self.marks: list[tuple[int, str, int]] = []
        self.in_run = False

    def text(self, text: str | None) -> None:
        if text:
            self.pieces.append(text)
            self.length += len(text)

    def children(self, element: Element) -> None:
        self.text(element.text)
        for child in element:
            self.element(child)
            self.text(child.tail)

    def element(self, element: Element) -> None:
        tag = tag_of(element)
        if is_footnote(element):
            return
        if tag == "ref":
            link = element.get("href")
            if link is not None and is_marker(element):
                self.reader.refuse(
                    "a ref that is both a link, with an href, and a footnote marker, of the "
                    "class footnoteRef",
                    element,
                )
            elif is_marker(element):
                self.marker(element)
            elif link is None:
                self.reader.refuse("a ref with neither an href nor the class footnoteRef", element)
            else:
                self.run("ref", element, link)
        elif tag in SUP_TAGS:
            self.run("sup", element, "")
        elif tag in SUB_TAGS:
            self.run("sub", element, "")
        elif tag in DROPPED_TAGS:
            self.children(element)
        elif tag == "br":
            if len(element) or not is_blank(element.text):
                self.reader.refuse("a br that holds text or an element", element)
            self.text(" ")
        else:
            self.reader.refuse("an element that does not stand in text", element)

    def run(self, kind: str, element: Element, target: str) -> None:
        if self.in_run:
            self.reader.refuse("a link, superscript or subscript inside another", element)
        start = self.length
        self.in_run = True
        self.children(element)
        self.in_run = False
        self.spans.append((kind, start, self.length, target))

    def marker(self, element: Element) -> None:
        if self.in_run:
            self.reader.refuse("a footnote marker inside a link, superscript or subscript", element)
        if len(element):
            self.reader.refuse("a footnote marker that holds an element", element)
        ordinal = self.scope.ordinals.get(element.get("idref") or "")
        if ordinal is None:
            self.reader.refuse(
                "a footnote marker whose idref names no footnote of its module", element
            )
        self.marks.append((self.length, normalize(element.text or ""), ordinal))

    def finish(self) -> Text:
        raw = "".join(self.pieces)
        plain = normalize(raw)
        if not self.spans and not self.marks:
            return Text(plain)
        self.check_divided(raw)
        lengths = PrefixLengths(raw)
        refs: list[tuple[int, int, str]] = []
        sup: list[tuple[int, int]] = []
        sub: list[tuple[int, int]] = []
        for kind, start, end, target in self.spans:
            while start < end and raw[start] in WHITE_SET:
                start += 1
            while end > start and raw[end - 1] in WHITE_SET:
                end -= 1
            if start == end:
                if kind == "ref":
                    self.reader.counts.links_without_words += 1
                continue
            extent = (lengths.at(start + 1) - 1, lengths.at(end))
            if kind == "ref":
                refs.append((*extent, target))
            elif kind == "sup":
                sup.append(extent)
            else:
                sub.append(extent)
        marks = tuple((lengths.at(position), shown, n) for position, shown, n in self.marks)
        return Text(plain, tuple(refs), tuple(sup), tuple(sub), marks)

    def check_divided(self, raw: str) -> None:
        """Refuse a text in which normalization acts across the boundary of a link, a
        superscript, a subscript or a footnote marker, so that the extent of a run could
        not be told: its pieces between those boundaries, each in Normalization Form C,
        must give the whole text in that form."""
        cuts = {0, len(raw)}
        for _, start, end, _ in self.spans:
            cuts.update((start, end))
        cuts.update(position for position, _, _ in self.marks)
        pieces = (raw[start:end] for start, end in itertools.pairwise(sorted(cuts)))
        divided = "".join(unicodedata.normalize("NFC", piece) for piece in pieces)
        if divided != unicodedata.normalize("NFC", raw):
            self.reader.refuse(
                "a character that normalization composes across the boundary of a link, "
                "a superscript, a subscript or a footnote marker"
            )


class Flow:
    """The blocks of a flow: text and inline elements gather into paragraphs."""

    def __init__(
        self, reader: MemberReader, scope: Scope, context: Context, out: list[QuotedItem]
    ) -> None:
        self.reader = reader
        self.scope = scope
        self.context = context
        self.out = out
        self.builder = TextBuilder(reader, scope)

    def text(self, text: str | None) -> None:
        self.builder.text(text)

    def close(self) -> None:
        if self.builder.pieces or self.builder.spans or self.builder.marks:
            text = self.builder.finish()
            if text.plain:
                self.out.append(Para(text))
            self.builder = TextBuilder(self.reader, self.scope)

    def add(self, blocks: list[Block]) -> None:
        self.close()
        self.out.extend(blocks)

    def child(self, child: Element) -> None:
        reader, context, scope = self.reader, self.context, self.scope
        tag = tag_of(child)
        if is_footnote(child):
            return
        if tag in P_TAGS or (tag in TEXT_PARTS and context.note_flow):
            self.add(reader.read_flow(child, context.inner(), scope))
        elif tag == "xhtml:table":
            self.add([reader.read_table(child, context.inner(), scope)])
        elif tag == "quotedContent":
            self.add([reader.read_quoted(child, context.inner(), scope)])
        elif tag == "img":
            self.add([reader.read_image(child)])
        elif tag == "signature" and context.in_note:
            self.add(reader.read_signature(child, scope))
        elif tag in ("heading", "subheading") and (context.in_note or context.quoted_top):
            self.add([Subheading(reader.read_text(child, scope))])
        elif tag == "note" and context.in_quoted:
            self.add(reader.read_quoted_note(child, context.inner(), scope))
        elif tag in INLINE_TAGS:
            self.builder.element(child)
        else:
            reader.refuse("an element that does not stand in a flow", child)


def blocks_only(items: list[QuotedItem]) -> list[Block]:
    out: list[Block] = []
    for item in items:
        if isinstance(item, Part | QuotedLevel):
            raise AssertionError(item)
        out.append(item)
    return out


class ModuleSink(Protocol):
    def unit(self, unit: Unit) -> None: ...

    def container(self, container: Container) -> None: ...


@dataclass(frozen=True, slots=True)
class DocumentMeta:
    """The release a document names, and its positive-law flag (None when it has none)."""

    release: str
    positive_law: bool | None


def parse_document(member: str, data: bytes) -> tuple[Element, DocumentMeta]:
    """Parse one member and check what its root and metadata must say.

    The name and the flag are the whole text of their elements, as written: markup
    inside them gives its words, and white space around them is part of them. The
    release is what the one publication name gives after `Online@`, each en dash in it
    read as a hyphen and no other character as another. The flag is the one property
    is-positive-law, `yes` or `no`; a document without it has none.
    """
    try:
        root = SafeXml.fromstring(data, forbid_dtd=True, forbid_entities=True)
    except defusedxml.DTDForbidden:
        raise RefusedError("a document type declaration", member) from None
    except defusedxml.EntitiesForbidden:
        raise RefusedError("an entity declaration", member) from None
    except defusedxml.ExternalReferenceForbidden:
        raise RefusedError("an external reference", member) from None
    except ParseError as error:
        raise RefusedError(f"not well-formed XML ({error})", member) from None
    if root.tag != "{" + USLM + "}uscDoc":
        raise RefusedError(f"the root is {root.tag}, not uscDoc in the namespace {USLM}", member)
    tokens = (root.get(XSI_SCHEMA_LOCATION) or "").split()
    pairs = dict(zip(tokens[0::2], tokens[1::2], strict=False))
    location = pairs.get(USLM, "")
    if len(tokens) % 2 or location.rsplit("/", 1)[-1] != SCHEMA_FILE:
        raise RefusedError(f"the schema location does not name {SCHEMA_FILE}", member)
    meta = next((child for child in root if tag_of(child) == "meta"), None)
    names: list[str] = []
    flags: list[str] = []
    if meta is not None:
        for child in meta:
            if tag_of(child) == "docPublicationName":
                names.append("".join(child.itertext()))
            elif tag_of(child) == "property" and child.get("role") == "is-positive-law":
                flags.append("".join(child.itertext()))
    if len(names) > 1:
        raise RefusedError("the metadata holds more than one docPublicationName", member)
    name = names[0] if names else ""
    if not name.startswith("Online@") or name == "Online@":
        raise RefusedError(f"the publication name {name!r} does not begin Online@", member)
    if len(flags) > 1:
        raise RefusedError("the metadata holds more than one property is-positive-law", member)
    if flags and flags[0] not in ("yes", "no"):
        raise RefusedError(
            f"the property is-positive-law is {flags[0]!r}, neither yes nor no", member
        )
    release = name[len("Online@") :].replace(EN_DASH, "-")
    return root, DocumentMeta(release, flags[0] == "yes" if flags else None)


def leading_letters(segment: str) -> str:
    match = re.match("[a-z]*", segment)
    return match.group() if match else ""


def range_aliases(identifier: str) -> list[str]:
    """The aliases of an identifier whose last segment holds three full stops."""
    last = identifier.rsplit("/", 1)[-1]
    head = identifier[: len(identifier) - len(last)]
    cut = last.index("...")
    first = head + last[:cut]
    second = head + leading_letters(last) + last[cut + 3 :]
    first_match = re.fullmatch("(.*?)([0-9]+)", first)
    second_match = re.fullmatch("(.*?)([0-9]+)", second)
    if first_match and second_match and first_match.group(1) == second_match.group(1):
        low, high = int(first_match.group(2)), int(second_match.group(2))
        if low <= high <= low + 999:
            stem = first_match.group(1)
            between = [stem + str(number) for number in range(low + 1, high)]
            return unique([first, *between, second])
    return unique([first, second])


def unique[T](values: list[T]) -> list[T]:
    seen: list[T] = []
    for value in values:
        if value not in seen:
            seen.append(value)
    return seen


def heading_segment(text: str) -> str:
    lowered = "".join(chr(ord(c) + 32) if "A" <= c <= "Z" else c for c in text)
    dashed = re.sub("[^A-Za-z0-9]+", "-", lowered).strip("-")
    return dashed[:60].rstrip("-")


class MemberReader:
    """Reads the units and containers of one member, one module at a time."""

    def __init__(
        self,
        member: str,
        positive_law: bool | None,
        offsets: dict[str, int],
        sink: ModuleSink,
        top_offsets: dict[str, int] | None = None,
    ) -> None:
        self.member = member
        self.positive_law = positive_law
        self.offsets = offsets
        self.top_offsets = top_offsets or {}
        self.held: Counter[str] = Counter()
        self.identifiers: list[str] = []
        self.unnamed_tops: list[str] = []
        self.sink = sink
        self.counts = DocumentCounts()
        self.at: list[str] = []

    def refuse(self, reason: str, element: Element | None = None) -> NoReturn:
        tag = tag_of(element).lstrip("?") if element is not None else ""
        raise RefusedError(reason, self.member, tag, self.at[-1] if self.at else "")

    def check_blank(self, element: Element) -> None:
        if not is_blank(element.text) or not all(is_blank(c.tail) for c in element):
            self.refuse("text where only elements may stand", element)

    def check_status(self, element: Element) -> str | None:
        status = element.get("status")
        if status is not None and status not in STATUS_VALUES:
            self.refuse(f"the status {status!r} is not one of the schema's values", element)
        return status

    def check_address(self, address: str, element: Element) -> str:
        if not is_address(address):
            self.refuse(f"the identifier gives {address!r}, which is not an address", element)
        return address

    def identifier_of(self, element: Element) -> list[str]:
        """The members of an element's identifier; one that holds `~`, the character the
        reading alone may add to an address, is refused."""
        raw = element.get("identifier")
        if raw is not None and "~" in raw:
            self.refuse(
                f"the identifier {raw!r} holds ~, the character the reading alone may add "
                "to an address",
                element,
            )
        return split_at_spaces(raw)

    # The document and its containers.

    def read(self, root: Element) -> None:
        """The top container of a file: the title inside main, or an appendix under uscDoc."""
        self.check_blank(root)
        place: Counter[str] = Counter(self.top_offsets)
        for top in root:
            tag = tag_of(top)
            if tag == "meta":
                continue
            if tag == "main":
                self.check_blank(top)
                for child in top:
                    if tag_of(child) != "title":
                        self.refuse("an element of main that is not a title", child)
                    self.check_nesting(child)
                    self.read_module(child, None, place)
            elif tag == "appendix":
                self.check_nesting(top)
                self.read_module(top, None, place)
            else:
                self.refuse("an element of uscDoc that is neither meta, main nor appendix", top)

    def check_nesting(self, top: Element) -> None:
        """Refuse, before the top container `top` is read, the first element in document
        order that stands more than 64 deep in its module, and the first container that
        stands more than 64 deep among the containers of its file.

        The element of a unit or a container stands at depth 1 of its module and each
        element inside it one deeper, but a unit or container that is a member of a
        container begins a module of its own; the top container stands at depth 1 among
        the containers, and a container that is a member of another one deeper."""
        waiting: list[tuple[Element, int, int, bool]] = [(top, 1, 1, True)]
        while waiting:
            element, depth, containers, is_container = waiting.pop()
            shown = tag_of(element).lstrip("?")
            if is_container and containers > MAX_NESTING:
                self.refuse(
                    f"<{shown}> stands {containers} containers deep in its file, deeper than "
                    f"the {MAX_NESTING} the reading reads",
                    element,
                )
            if depth > MAX_NESTING:
                self.refuse(
                    f"<{shown}> stands {depth} deep in its unit or container, deeper than the "
                    f"{MAX_NESTING} the reading reads",
                    element,
                )
            for child in reversed(list(element)):
                kind = self.classify(child) if is_container else None
                if kind == "container":
                    waiting.append((child, 1, containers + 1, True))
                elif kind == "unit":
                    waiting.append((child, 1, containers, False))
                else:
                    waiting.append((child, depth + 1, containers, False))

    def classify(self, element: Element) -> str | None:
        tag = tag_of(element)
        if tag in CONTAINER_TAGS:
            return "container"
        if tag in UNIT_TAGS:
            return "unit"
        if tag == "level":
            text_bearing = any(tag_of(c) in TEXT_PARTS or tag_of(c) in P_TAGS for c in element)
            return "unit" if text_bearing else "container"
        return None

    def module_address(
        self, element: Element, within: str | None, place: Counter[str]
    ) -> tuple[str, list[str]]:
        identifier = self.identifier_of(element)
        if not identifier:
            tag = tag_of(element)
            place[tag] += 1
            self.counts.assigned += 1
            if within is None:
                self.unnamed_tops.append(tag)
            address = f"{within or PREFIX}/~{tag}.{place[tag]}"
            return self.check_address(address, element), []
        used = identifier[0]
        aliases = identifier[1:]
        if "..." in used.rsplit("/", 1)[-1]:
            aliases += range_aliases(used)
        self.identifiers.append(used)
        self.held[used] += 1
        holder = self.offsets.get(used, 0) + self.held[used]
        address = used
        if holder > 1:
            address = f"{used}~{holder}"
            self.counts.disambiguated += 1
        for alias in aliases:
            self.check_address(alias, element)
        return self.check_address(address, element), unique(aliases)

    def read_module(self, element: Element, within: str | None, place: Counter[str]) -> str:
        kind = self.classify(element)
        if kind is None:
            self.refuse("an element that is neither a container nor a unit", element)
        address, aliases = self.module_address(element, within, place)
        self.at.append(address)
        if kind == "container":
            self.read_container(element, address, aliases, within)
        else:
            self.read_unit(element, address, aliases, within)
        self.at.pop()
        return address

    def is_release_note(self, element: Element, holder: Element) -> bool:
        if tag_of(holder) not in ("title", "appendix") or tag_of(element) != "note":
            return False
        if is_footnote(element) or element.get("role") == "crossHeading":
            return False
        return normalize("".join(element.itertext())).startswith(RELEASE_NOTE_START)

    def prepare_footnotes(self, scope: Scope, module: Element, container: bool) -> None:
        found: list[Element] = []

        def walk(node: Element) -> None:
            for child in node:
                tag = tag_of(child)
                if tag == "toc":
                    continue
                if (
                    container
                    and node is module
                    and (self.classify(child) is not None or self.is_release_note(child, module))
                ):
                    continue
                if is_footnote(child):
                    found.append(child)
                    continue
                walk(child)

        walk(module)
        for ordinal, footnote in enumerate(found, start=1):
            identifier = footnote.get("id")
            if identifier is None:
                continue
            if identifier in scope.ordinals:
                self.refuse(
                    f"two footnotes of the module have the id {identifier!r}, so a marker "
                    "cannot name one",
                    footnote,
                )
            scope.ordinals[identifier] = ordinal
        for ordinal, footnote in enumerate(found, start=1):
            num = next((c for c in footnote if tag_of(c) == "num"), None)
            shown = self.read_text(num, scope).plain if num is not None else ""
            blocks = self.read_flow(footnote, Context(), scope, skip=num)
            scope.footnotes.append(Footnote(ordinal, shown, blocks))

    def read_container(
        self, element: Element, address: str, aliases: list[str], within: str | None
    ) -> None:
        kind = tag_of(element)
        status = self.check_status(element)
        scope = Scope(address)
        self.prepare_footnotes(scope, element, container=True)
        self.check_blank(element)
        num: Num | None = None
        heading: Text | None = None
        members: list[str] = []
        credit: Text | None = None
        notes: list[tuple[Element, str, bool]] = []
        place: Counter[str] = Counter()
        seen: set[str] = set()
        for child in element:
            tag = tag_of(child)
            if tag in LABEL_NAMES:
                self.check_once(child, seen)
            if tag == "num":
                num = self.read_num(child, scope)
            elif tag == "heading":
                heading = self.read_optional_text(child, scope)
            elif tag == "toc":
                self.count_toc(child)
            elif tag == "notes":
                notes += self.notes_of(child)
            elif tag == "note":
                if is_footnote(child):
                    continue
                if self.is_release_note(child, element):
                    self.counts.release_notes += 1
                    continue
                notes.append((child, "ungrouped", False))
            elif tag == "sourceCredit":
                credit = self.read_optional_text(child, scope)
            elif self.classify(child) is not None:
                members.append(self.read_module(child, address, place))
            else:
                self.refuse("an element that does not stand in a container", child)
        read = self.read_notes(notes, address, scope)
        self.sink.container(
            Container(
                address,
                aliases,
                within,
                self.member,
                self.positive_law,
                kind,
                num,
                heading,
                status,
                members,
                credit,
                read,
                scope.footnotes,
            )
        )

    def count_toc(self, element: Element) -> None:
        self.counts.tables_of_contents += 1
        for inner in element.iter():
            if is_footnote(inner):
                self.counts.footnotes_in_tables_of_contents += 1
            elif tag_of(inner) == "ref" and inner.get("href") is not None:
                self.counts.links_in_tables_of_contents += 1

    # Units and the provisions inside them.

    def read_unit(
        self, element: Element, address: str, aliases: list[str], within: str | None
    ) -> None:
        scope = Scope(address)
        self.prepare_footnotes(scope, element, container=False)
        unit = UnitState(scope)
        root = self.read_provision(element, address, unit)
        self.sink.unit(
            Unit(
                address,
                aliases,
                within,
                self.member,
                self.positive_law,
                root,
                unit.credit,
                unit.notes,
                scope.footnotes,
            )
        )

    def check_once(self, element: Element, seen: set[str]) -> None:
        """A number, a heading and a source credit each stand once in one level."""
        tag = tag_of(element)
        if tag in seen:
            self.refuse(f"a second {LABEL_NAMES[tag]} of one level", element)
        seen.add(tag)

    def read_num(self, element: Element, scope: Scope) -> Num | None:
        """A number; absent when it has neither a value, nor text, nor a footnote marker."""
        value = element.get("value") or ""
        shown = self.read_text(element, scope)
        return Num(value, shown) if value or shown.plain or shown.marks else None

    def read_optional_text(self, element: Element, scope: Scope) -> Text | None:
        """A heading or a source credit; absent when it has no text and no footnote marker."""
        text = self.read_text(element, scope)
        return text if text.plain or text.marks else None

    def read_provision(self, element: Element, address: str, unit: UnitState) -> Provision:
        scope = unit.scope
        status = self.check_status(element)
        self.check_blank(element)
        num: Num | None = None
        heading: Text | None = None
        items: list[Part | Provision] = []
        notes: list[tuple[Element, str, bool]] = []
        siblings: Counter[str] = Counter()
        place: Counter[str] = Counter()
        seen: set[str] = set()
        operative = Context()
        for child in element:
            tag = tag_of(child)
            if tag in LABEL_NAMES:
                self.check_once(child, seen)
            if tag == "num":
                num = self.read_num(child, scope)
            elif tag == "heading":
                heading = self.read_optional_text(child, scope)
            elif tag in TEXT_PARTS:
                items += self.read_parts(child, operative, scope)
            elif tag in P_TAGS:
                blocks = self.read_flow(child, operative, scope)
                if blocks:
                    items.append(Part("content", blocks))
            elif tag in LEVEL_TAGS:
                child_address = self.provision_address(child, address, siblings, place)
                self.at.append(child_address)
                items.append(self.read_provision(child, child_address, unit))
                self.at.pop()
            elif tag == "notes":
                notes += self.notes_of(child)
            elif tag == "note":
                if not is_footnote(child):
                    notes.append((child, "ungrouped", False))
            elif tag == "sourceCredit":
                if address != unit.scope.address:
                    self.refuse("a source credit that is not the direct child of its unit", child)
                unit.credit = self.read_optional_text(child, scope)
            elif tag == "toc":
                self.count_toc(child)
            else:
                self.refuse("an element that does not stand in a level", child)
        unit.notes += self.read_notes(notes, address, scope)
        return Provision(address, tag_of(element), num, heading, status, items)

    def provision_address(
        self, element: Element, parent: str, siblings: Counter[str], place: Counter[str]
    ) -> str:
        identifier = self.identifier_of(element)
        if not identifier:
            tag = tag_of(element)
            place[tag] += 1
            self.counts.assigned += 1
            return self.check_address(f"{parent}/~{tag}.{place[tag]}", element)
        base = f"{parent}/{identifier[0].rsplit('/', 1)[-1]}"
        siblings[base] += 1
        if siblings[base] == 1:
            return self.check_address(base, element)
        self.counts.disambiguated += 1
        return self.check_address(f"{base}~{siblings[base]}", element)

    def read_parts(self, element: Element, context: Context, scope: Scope) -> list[Part]:
        """The parts a text part gives: one, or several for a content split by others."""
        role = tag_of(element)
        if role != "content":
            blocks = self.read_flow(element, context, scope)
            return [Part(role, blocks)] if blocks else []
        parts: list[Part] = []
        current: list[QuotedItem] = []
        flow = Flow(self, scope, context, current)
        flow.text(element.text)
        for child in element:
            tag = tag_of(child)
            if tag in ("continuation", "proviso"):
                flow.close()
                if current:
                    parts.append(Part("content", blocks_only(current)))
                inner = self.read_flow(child, context, scope)
                if inner:
                    parts.append(Part(tag, inner))
                current = []
                flow = Flow(self, scope, context, current)
            else:
                flow.child(child)
            flow.text(child.tail)
        flow.close()
        if current:
            parts.append(Part("content", blocks_only(current)))
        return parts

    # Flows, texts and the blocks inside them.

    def read_flow(
        self, element: Element, context: Context, scope: Scope, skip: Element | None = None
    ) -> list[Block]:
        out: list[QuotedItem] = []
        flow = Flow(self, scope, context, out)
        flow.text(element.text)
        for child in element:
            if child is not skip:
                flow.child(child)
            flow.text(child.tail)
        flow.close()
        return blocks_only(out)

    def read_text(self, element: Element, scope: Scope) -> Text:
        builder = TextBuilder(self, scope)
        builder.children(element)
        return builder.finish()

    def read_image(self, element: Element) -> Image:
        if set(element.attrib) - {"src"}:
            self.refuse("an img with an attribute other than src", element)
        if len(element) or not is_blank(element.text):
            self.refuse("an img that holds text or an element", element)
        return Image(normalize(element.get("src") or ""))

    def read_signature(self, element: Element, scope: Scope) -> list[Block]:
        self.check_blank(element)
        blocks: list[Block] = []
        for child in element:
            if tag_of(child) not in ("name", "role"):
                self.refuse("an element of a signature that is neither a name nor a role", child)
            text = self.read_text(child, scope)
            if text.plain:
                blocks.append(Para(text))
        return blocks

    def read_table(self, element: Element, context: Context, scope: Scope) -> Table:
        rows: list[Row] = []
        self.check_blank(element)
        for child in element:
            tag = tag_of(child)
            if tag == "xhtml:colgroup":
                self.check_blank(child)
                for col in child:
                    self.check_column(col)
            elif tag == "xhtml:col":
                self.check_column(child)
            elif tag in ROW_GROUPS:
                self.check_blank(child)
                for row in child:
                    rows.append(self.read_row(row, ROW_GROUPS[tag], context, scope))
            else:
                rows.append(self.read_row(child, "body", context, scope))
        return Table(rows)

    def check_column(self, element: Element) -> None:
        if tag_of(element) != "xhtml:col" or len(element) or not is_blank(element.text):
            self.refuse("a column group that holds more than empty columns", element)

    def read_row(self, element: Element, group: str, context: Context, scope: Scope) -> Row:
        if tag_of(element) != "xhtml:tr":
            self.refuse("an element that does not stand in a table", element)
        self.check_blank(element)
        cells: list[Cell] = []
        for cell in element:
            tag = tag_of(cell)
            if tag not in ("xhtml:th", "xhtml:td"):
                self.refuse("an element that does not stand in a table row", cell)
            blocks = self.read_flow(cell, context, scope)
            cells.append(
                Cell(
                    tag == "xhtml:th",
                    self.span(cell, "colspan"),
                    self.span(cell, "rowspan"),
                    blocks,
                )
            )
        return Row(group, cells)

    def span(self, element: Element, name: str) -> int:
        value = element.get(name)
        if value is None:
            return 1
        if not ASCII_NUMBER.fullmatch(value):
            self.refuse(f"a {name} that is not a whole number written in ASCII digits", element)
        return int(value)

    def read_quoted(self, element: Element, context: Context, scope: Scope) -> Quoted:
        """Quoted content: its origin as written, none when it has none or an empty one,
        and its items."""
        quoted = Context(context.in_note, True, quoted_top=True)
        origin = element.get("origin") or None
        return Quoted(origin, self.read_items(element, quoted, scope, None))

    def read_items(
        self,
        element: Element,
        context: Context,
        scope: Scope,
        labels: list[Num | Text | None] | None,
    ) -> list[QuotedItem]:
        """Items of quoted content or of a level in it; `labels` collects num and heading."""
        items: list[QuotedItem] = []
        flow = Flow(self, scope, context, items)
        flow.text(element.text)
        seen: set[str] = set()
        for child in element:
            tag = tag_of(child)
            if labels is not None and tag == "num":
                flow.close()
                self.check_once(child, seen)
                labels[0] = self.read_num(child, scope)
            elif labels is not None and tag == "heading":
                flow.close()
                self.check_once(child, seen)
                labels[1] = self.read_optional_text(child, scope)
            elif tag in TEXT_PARTS:
                flow.close()
                items += self.read_parts(child, context.inner(), scope)
            elif tag in LEVEL_TAGS:
                flow.close()
                items.append(self.read_quoted_level(child, context.inner(), scope))
            else:
                flow.child(child)
            flow.text(child.tail)
        flow.close()
        return items

    def read_quoted_level(self, element: Element, context: Context, scope: Scope) -> QuotedLevel:
        self.check_status(element)
        labels: list[Num | Text | None] = [None, None]
        items = self.read_items(element, context, scope, labels)
        num, heading = labels
        if not isinstance(num, Num | None) or not isinstance(heading, Text | None):
            raise AssertionError(labels)
        return QuotedLevel(tag_of(element), num, heading, items)

    def read_quoted_note(self, element: Element, context: Context, scope: Scope) -> list[Block]:
        """A note inside quoted content, wherever the quotation stands: its heading first
        as a subheading, then the flow of the rest, read as the flow of a note, where a
        text part that stands directly in it is read as a block."""
        heading = next((c for c in element if tag_of(c) == "heading"), None)
        blocks: list[Block] = []
        if heading is not None:
            text = self.read_optional_text(heading, scope)
            if text is not None:
                blocks.append(Subheading(text))
        inner = Context(True, context.in_quoted, note_flow=True)
        return blocks + self.read_flow(element, inner, scope, skip=heading)

    # Notes.

    def notes_of(self, element: Element) -> list[tuple[Element, str, bool]]:
        """The notes of a `notes` element, each with the group of the cross heading before it.

        A cross heading opens a group and is no note; its label is not imported and is
        counted. What it holds after its label is a note of the group it opens, marked
        True here.
        """
        self.check_blank(element)
        group = "ungrouped"
        found: list[tuple[Element, str, bool]] = []
        for child in element:
            if tag_of(child) != "note":
                self.refuse("an element of notes that is not a note", child)
            if child.get("role") == "crossHeading":
                group = child.get("topic") or ""
                if not GROUP_WORD.fullmatch(group):
                    self.refuse(
                        f"a cross heading whose topic {group!r} is not one word of ASCII letters "
                        "and digits that begins with a letter",
                        child,
                    )
                self.counts.note_group_headings += 1
                found.append((child, group, True))
            elif not is_footnote(child):
                found.append((child, group, False))
        return found

    def after_label(self, element: Element, scope: Scope) -> list[Block]:
        """The blocks a cross heading holds after its first heading child, its label."""
        label = next((c for c in element if tag_of(c) == "heading"), None)
        context = Context(True, False, note_flow=True)
        if label is None:
            return self.read_flow(element, context, scope)
        out: list[QuotedItem] = []
        flow = Flow(self, scope, context, out)
        if not is_blank(element.text):
            self.refuse("text of a cross heading before its heading", element)
        before = True
        for child in element:
            if before:
                if child is not label:
                    self.refuse("an element of a cross heading before its heading", child)
                before = False
            else:
                flow.child(child)
            flow.text(child.tail)
        flow.close()
        return blocks_only(out)

    def read_notes(
        self, notes: list[tuple[Element, str, bool]], anchor: str, scope: Scope
    ) -> list[Note]:
        """The notes of one anchor, each named by its group, law and heading."""
        read: list[Note] = []
        named: Counter[str] = Counter()
        for element, group, labeled in notes:
            heading: Text | None = None
            if labeled:
                topics: list[str] = []
                blocks = self.after_label(element, scope)
                if not blocks:
                    continue
            else:
                topics = self.topics_of(element)
                heading_element = next((c for c in element if tag_of(c) == "heading"), None)
                if heading_element is not None:
                    heading = self.read_optional_text(heading_element, scope)
                context = Context(True, False, note_flow=True)
                blocks = self.read_flow(element, context, scope, skip=heading_element)
            law = self.law_of(heading, blocks, element, anchor)
            segment = heading_segment(heading.plain) if heading is not None else ""
            if not segment:
                segment = (
                    "noheading" if law else "h." + note_hash(group, topics, heading, blocks)[:16]
                )
            base = f"{anchor}/note/{group}/{law or 'nolaw'}/{segment}"
            named[base] += 1
            address = base if named[base] == 1 else f"{base}/{named[base]}"
            editorial = group == "editorialNotes" or (
                group == "ungrouped" and any(topic in EDITORIAL_TOPICS for topic in topics)
            )
            layer = "editorial" if editorial else "enacted"
            read.append(Note(address, layer, group, topics, heading, blocks))
        return read

    def topics_of(self, note: Element) -> list[str]:
        """The topics of a note: its `topic` attribute split at U+0020, with no empty
        member, so that an empty attribute gives none. A topic that holds any other
        white-space character is refused."""
        topics = split_at_spaces(note.get("topic"))
        for topic in topics:
            if any(character in WHITE_SET for character in topic):
                self.refuse(
                    f"a note whose topic {topic!r} holds a white-space character other than "
                    "U+0020, at which its topics are split",
                    note,
                )
        return topics

    def law_of(self, heading: Text | None, blocks: list[Block], note: Element, anchor: str) -> str:
        """The law a note cites first: its first quoted origin, else its first link to a law."""
        origin = first_quoted_origin(blocks)
        if origin is not None and origin.startswith(LAW_ORIGINS):
            return self.law_segment(origin, note, anchor)
        texts = [heading] if heading is not None else []
        for text in [*texts, *texts_of_blocks(blocks)]:
            for _, _, target in text.refs:
                if target.startswith(LAW_LINKS):
                    return self.law_segment(target, note, anchor)
        return ""

    def law_segment(self, reference: str, note: Element, anchor: str) -> str:
        segments = reference.split("/")[1:]
        if len(segments) < 4:
            why = "a reference of fewer than four segments"
        else:
            bad = next((s for s in segments[1:4] if not is_segment(s)), None)
            if bad is None:
                return ".".join(segments[1:4])
            why = f"whose segment {bad!r} is empty or holds a character an address segment "
            why += "cannot hold"
        heading = next((c for c in note if tag_of(c) == "heading"), None)
        shown = normalize("".join(heading.itertext())) if heading is not None else ""
        raise RefusedError(
            f"the note under {anchor} headed {shown!r} cites {reference!r}, {why}",
            self.member,
            "note",
            anchor,
        )


@dataclass(slots=True)
class UnitState:
    scope: Scope
    credit: Text | None = None
    notes: list[Note] = field(default_factory=list)


def first_quoted_origin(blocks: Iterable[Block | QuotedItem]) -> str | None:
    """The origin of the first quoted content in the blocks, or None when there is none."""
    for block in blocks:
        match block:
            case Quoted(origin, _):
                return origin or ""
            case Table(rows):
                for row in rows:
                    for cell in row.cells:
                        found = first_quoted_origin(cell.blocks)
                        if found is not None:
                            return found
            case _:
                pass
    return None


# Members, archives and the release.


@dataclass(frozen=True, slots=True)
class Source:
    """One member: the archive it is read from (or None for a plain file) and its name."""

    archive: str | None
    path: str
    member: str


def list_sources(path: Path) -> list[Source]:
    """Every member of the input, sorted by name, after the checks on names and sizes."""
    if path.is_dir():
        archives = sorted(p for p in path.iterdir() if p.is_file() and p.suffix == ".zip")
        if not archives:
            raise InputError(f"{path.name}: no archive in the directory")
    elif path.is_file() and path.suffix == ".zip":
        archives = [path]
    elif path.is_file():
        name = path.name
        if not MEMBER_NAME.fullmatch(name):
            raise RefusedError(f"the file name {name!r} is not that of a member", name)
        if path.stat().st_size > MAX_MEMBER_BYTES:
            raise RefusedError("larger than 512 MiB", name)
        return [Source(None, str(path), name)]
    else:
        raise InputError(f"{path.name}: neither a directory, an archive nor an XML file")
    sources: dict[str, Source] = {}
    for archive in archives:
        try:
            with zipfile.ZipFile(archive) as opened:
                infos = opened.infolist()
        except (OSError, zipfile.BadZipFile) as error:
            raise InputError(f"{archive.name}: cannot read the archive ({error})") from None
        for info in infos:
            name = info.filename
            if not MEMBER_NAME.fullmatch(name):
                raise RefusedError(
                    f"the member {name!r} is not named usc, two digits, "
                    "an optional a or A, and .xml",
                    archive=archive.name,
                )
            if info.file_size > MAX_MEMBER_BYTES:
                raise RefusedError("larger than 512 MiB", name, archive=archive.name)
            if name in sources:
                other = sources[name].archive or ""
                raise RefusedError(
                    f"held by two archives, {other} and {archive.name}", name, archive=archive.name
                )
            sources[name] = Source(archive.name, str(archive), name)
    return [sources[name] for name in sorted(sources)]


def deflated_size(path: str, info: zipfile.ZipInfo) -> int | None:
    """The number of bytes that the compressed data of the deflated member `info` of the
    archive at `path` gives when it is decompressed to the end of its own stream, which
    no header declares; None when the data ends before the stream does. The count stops
    once it passes the limit of a member."""
    decompressor = zlib.decompressobj(-zlib.MAX_WBITS)
    total = 0
    with open(path, "rb") as stream:
        stream.seek(info.header_offset)
        header = stream.read(LOCAL_HEADER_BYTES)
        if len(header) < LOCAL_HEADER_BYTES or header[:4] != LOCAL_HEADER:
            return None
        name_bytes, extra_bytes = struct.unpack("<HH", header[26:30])
        stream.seek(name_bytes + extra_bytes, os.SEEK_CUR)
        left = info.compress_size
        while left > 0 and not decompressor.eof:
            chunk = stream.read(min(left, CHUNK_BYTES))
            if not chunk:
                break
            left -= len(chunk)
            while chunk and not decompressor.eof:
                total += len(decompressor.decompress(chunk, CHUNK_BYTES))
                if total > MAX_MEMBER_BYTES:
                    return total
                chunk = decompressor.unconsumed_tail
    return total if decompressor.eof else None


def load(source: Source) -> bytes:
    """The bytes of one member. The size of a deflated member of an archive is held to
    the end of its compressed stream as well as to its header, so that a header that
    understates it, with the CRC-32 of what it declares, is refused rather than read
    short; a member stored without compression is held to its header alone."""
    try:
        if source.archive is None:
            with open(source.path, "rb") as stream:
                data = stream.read(MAX_MEMBER_BYTES + 1)
        else:
            with zipfile.ZipFile(source.path) as archive:
                info = archive.getinfo(source.member)
                if info.compress_type == zipfile.ZIP_DEFLATED:
                    held = deflated_size(source.path, info)
                    if held is None:
                        raise RefusedError(
                            "the compressed data of the member ends before its stream does",
                            source.member,
                            archive=source.archive,
                        )
                    if held > MAX_MEMBER_BYTES:
                        raise RefusedError(
                            "larger than 512 MiB", source.member, archive=source.archive
                        )
                    if held != info.file_size:
                        raise RefusedError(
                            f"the member holds {held} bytes, and its header declares "
                            f"{info.file_size}",
                            source.member,
                            archive=source.archive,
                        )
                with archive.open(info) as stream:
                    data = stream.read(MAX_MEMBER_BYTES + 1)
    except (OSError, zipfile.BadZipFile, zlib.error) as error:
        raise InputError(f"{source.member}: cannot read the member ({error})") from None
    if len(data) > MAX_MEMBER_BYTES:
        raise RefusedError("larger than 512 MiB", source.member, archive=source.archive or "")
    return data


@dataclass(slots=True)
class MemberResult:
    member: str
    release: str
    identifiers: list[str]
    counts: DocumentCounts
    output: object
    baseline: object = None
    unnamed_tops: list[str] = field(default_factory=list)


@dataclass(frozen=True, slots=True)
class Job:
    """One member to read, with what earlier members hold: the holders of each
    identifier, and the top elements without an identifier of each tag."""

    source: Source
    offsets: dict[str, int]
    first_pass: bool
    top_offsets: dict[str, int] = field(default_factory=dict)


Worker = Callable[[Job], "MemberResult | RefusedError"]


def read_member(job: Job, sink: ModuleSink) -> tuple[Element, MemberReader, DocumentMeta]:
    """Read one member into the sink; what the workers of both scripts call."""
    member = job.source.member
    data = load(job.source)
    root, meta = parse_document(member, data)
    del data
    reader = MemberReader(member, meta.positive_law, job.offsets, sink, job.top_offsets)
    reader.read(root)
    return root, reader, meta


def run_jobs(worker: Worker, jobs: list[Job], workers: int) -> list[MemberResult | RefusedError]:
    if workers <= 1 or len(jobs) <= 1:
        return [worker(job) for job in jobs]
    context = multiprocessing.get_context("spawn")
    with ProcessPoolExecutor(
        max_workers=min(workers, len(jobs)), mp_context=context, max_tasks_per_child=1
    ) as pool:
        return list(pool.map(worker, jobs))


def read_release(path: Path, worker: Worker, workers: int) -> tuple[str, list[MemberResult]]:
    """Read every member of the input; repeated identifiers are numbered over the release.

    Every member is read once on its own. A unit or container whose identifier an
    earlier member already holds needs the count of those holders, and a top element
    without an identifier the count of such top elements of its tag in earlier
    members, so a member that holds one is read again with the counts. The result is
    the same for any number of workers.
    """
    sources = list_sources(path)
    first = run_jobs(worker, [Job(s, {}, True) for s in sources], workers)
    results: list[MemberResult] = []
    release = ""
    for source, result in zip(sources, first, strict=True):
        if isinstance(result, RefusedError):
            result.archive = source.archive or ""
            raise result
        if not results:
            release = result.release
        elif result.release != release:
            raise RefusedError(
                f"the release is {result.release}, and the first member says {release}",
                source.member,
                archive=source.archive or "",
            )
        results.append(result)
    held: Counter[str] = Counter()
    unnamed: Counter[str] = Counter()
    again: list[tuple[int, Job]] = []
    for index, (source, result) in enumerate(zip(sources, results, strict=True)):
        offsets = {name: held[name] for name in set(result.identifiers) if held[name]}
        tops = {tag: unnamed[tag] for tag in set(result.unnamed_tops) if unnamed[tag]}
        if offsets or tops:
            again.append((index, Job(source, offsets, False, tops)))
        held.update(result.identifiers)
        unnamed.update(result.unnamed_tops)
    for (index, job), result in zip(
        again, run_jobs(worker, [job for _, job in again], workers), strict=True
    ):
        if isinstance(result, RefusedError):
            result.archive = job.source.archive or ""
            raise result
        result.baseline = results[index].baseline
        results[index] = result
    return release, results
