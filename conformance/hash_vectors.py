#!/usr/bin/env python3
"""Hash the invented models of the known-answer file by the hash rules of law modules.

A second implementation of the hashes of provisions, notes, containers and layers that
`docs/law-modules.md` gives under "Hashes". It shares no code with the crate
`fidryn-provision`, whose tests hold their own values to the digests in the same file.

Without `--write` the script verifies the digests in the file and exits 1 when one differs;
with `--write` it fills them in.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import blake3

VECTORS = Path(__file__).resolve().parent / "fixtures" / "hash" / "vectors.json"

Json = dict[str, Any]
Mark = tuple[str, str, int]

ROLES = {"chapeau": 0, "content": 1, "continuation": 2, "proviso": 3}
GROUPS = {"head": 0, "body": 1, "foot": 2}
SECTIONS = ("provisions", "notes", "units", "containers")
FIRST_KEY = "invented"


class ModelError(ValueError):
    """A model of the file does not have the shape the hash rules read."""


def as_object(value: Any, what: str) -> Json:
    if not isinstance(value, dict):
        raise ModelError(f"{what} is not an object")
    return value


def as_list(value: Any, what: str) -> list[Any]:
    if not isinstance(value, list):
        raise ModelError(f"{what} is not a list")
    return value


def as_str(value: Any, what: str) -> str:
    if not isinstance(value, str):
        raise ModelError(f"{what} is not a string")
    return value


def as_int(value: Any, what: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ModelError(f"{what} is not an integer")
    return value


def as_bool(value: Any, what: str) -> bool:
    if not isinstance(value, bool):
        raise ModelError(f"{what} is not a boolean")
    return value


def field(value: Json, key: str) -> Any:
    if key not in value:
        raise ModelError(f"the field {key!r} is missing")
    return value[key]


def variant(value: Any, what: str) -> tuple[str, Any]:
    """The name and the content of one variant: `{"name": content}`, or a bare name."""
    if isinstance(value, str):
        return value, None
    entries = list(as_object(value, what).items())
    if len(entries) != 1:
        raise ModelError(f"{what} is not one variant")
    name, content = entries[0]
    return as_str(name, what), content


def frame(tag: int, value: bytes) -> bytes:
    """One frame: the tag byte, the length of the value as eight bytes little-endian, the value."""
    return bytes([tag]) + len(value).to_bytes(8, "little") + value


def u64(number: int) -> bytes:
    return number.to_bytes(8, "little")


def u32(number: int) -> bytes:
    return number.to_bytes(4, "little")


def utf8(text: str) -> bytes:
    return text.encode("utf-8")


def runs_of(text: Any) -> list[tuple[str, Any]]:
    runs = as_list(field(as_object(text, "a text"), "runs"), "the runs of a text")
    return [variant(run, "a run") for run in runs]


def words_of(kind: str, content: Any) -> str:
    """The words a run adds to the plain text of its text."""
    if kind in ("words", "sup", "sub"):
        return as_str(content, f"a {kind} run")
    if kind == "ref":
        return as_str(field(as_object(content, "a link"), "words"), "the words of a link")
    if kind == "mark":
        return ""
    raise ModelError(f"unknown run {kind!r}")


def plain(text: Any) -> str:
    return "".join(words_of(kind, content) for kind, content in runs_of(text))


def encode_text(text: Any) -> bytes:
    """The frames of a text; offsets are bytes of its plain text."""
    out = [frame(0x10, utf8(plain(text)))]
    offset = 0
    for kind, content in runs_of(text):
        start = offset
        offset += len(utf8(words_of(kind, content)))
        if kind == "ref":
            target = as_str(field(content, "target"), "the target of a link")
            out.append(frame(0x11, u64(start) + u64(offset) + utf8(target)))
        elif kind == "sup":
            out.append(frame(0x12, u64(start) + u64(offset)))
        elif kind == "sub":
            out.append(frame(0x13, u64(start) + u64(offset)))
    return b"".join(out)


def encode_optional_text(text: Any) -> bytes:
    return frame(0x1F, b"") if text is None else encode_text(text)


def encode_num(num: Any) -> bytes:
    if num is None:
        return frame(0x04, b"")
    number = as_object(num, "a number")
    value = as_str(field(number, "value"), "the value of a number")
    return frame(0x02, utf8(value)) + frame(0x03, encode_text(field(number, "shown")))


def encode_blocks(blocks: Any) -> bytes:
    return b"".join(encode_block(block) for block in as_list(blocks, "blocks"))


def encode_block(block: Any) -> bytes:
    kind, content = variant(block, "a block")
    if kind == "para":
        return frame(0x20, encode_text(content))
    if kind == "subheading":
        return frame(0x21, encode_text(content))
    if kind == "table":
        return frame(0x22, encode_table(content))
    if kind == "quoted":
        return frame(0x23, encode_quoted(content))
    if kind == "image":
        source = as_str(field(as_object(content, "an image"), "source"), "an image source")
        return frame(0x24, utf8(source))
    raise ModelError(f"unknown block {kind!r}")


def encode_table(table: Any) -> bytes:
    out = []
    for row in as_list(field(as_object(table, "a table"), "rows"), "the rows of a table"):
        row = as_object(row, "a row")
        group = as_str(field(row, "group"), "the group of a row")
        if group not in GROUPS:
            raise ModelError(f"unknown row group {group!r}")
        cells = []
        for cell in as_list(field(row, "cells"), "the cells of a row"):
            cell = as_object(cell, "a cell")
            tag = 0x35 if as_bool(field(cell, "header"), "header") else 0x34
            colspan = as_int(field(cell, "colspan"), "colspan")
            rowspan = as_int(field(cell, "rowspan"), "rowspan")
            blocks = encode_blocks(field(cell, "blocks"))
            cells.append(frame(tag, u32(colspan) + u32(rowspan) + blocks))
        out.append(frame(0x30 + GROUPS[group], b"".join(cells)))
    return b"".join(out)


def encode_part(part: Any) -> bytes:
    part = as_object(part, "a part")
    role = as_str(field(part, "role"), "the role of a part")
    if role not in ROLES:
        raise ModelError(f"unknown part role {role!r}")
    return frame(0x50 + ROLES[role], encode_blocks(field(part, "blocks")))


def encode_quoted(quoted: Any) -> bytes:
    quoted = as_object(quoted, "quoted content")
    origin = field(quoted, "origin")
    origin_bytes = b"" if origin is None else utf8(as_str(origin, "an origin"))
    items = as_list(field(quoted, "items"), "the items of quoted content")
    return frame(0x40, origin_bytes) + b"".join(encode_quoted_item(item) for item in items)


def encode_quoted_item(item: Any) -> bytes:
    kind, content = variant(item, "an item of quoted content")
    if kind == "block":
        return encode_block(content)
    if kind == "part":
        return encode_part(content)
    if kind == "level":
        level = as_object(content, "a level")
        items = as_list(field(level, "items"), "the items of a level")
        value = (
            frame(0x01, utf8(as_str(field(level, "kind"), "the kind of a level")))
            + encode_num(field(level, "num"))
            + encode_optional_text(field(level, "heading"))
            + b"".join(encode_quoted_item(inner) for inner in items)
        )
        return frame(0x41, value)
    raise ModelError(f"unknown item of quoted content {kind!r}")


def encode_status(status: Any) -> bytes:
    return frame(0x05, b"" if status is None else utf8(as_str(status, "a status")))


def provision_hash(provision: Any) -> bytes:
    """The tree hash of a provision."""
    provision = as_object(provision, "a provision")
    data = [
        frame(0x00, b"fidryn.provision/v1"),
        frame(0x01, utf8(as_str(field(provision, "kind"), "the kind of a provision"))),
        encode_num(field(provision, "num")),
        encode_optional_text(field(provision, "heading")),
        encode_status(field(provision, "status")),
    ]
    for item in as_list(field(provision, "items"), "the items of a provision"):
        kind, content = variant(item, "an item")
        if kind == "part":
            data.append(encode_part(content))
        elif kind == "child":
            data.append(frame(0x60, provision_hash(content)))
        else:
            raise ModelError(f"unknown item {kind!r}")
    return blake3.blake3(b"".join(data)).digest()


def note_hash(note: Any) -> bytes:
    note = as_object(note, "a note")
    topics = [as_str(topic, "a topic") for topic in as_list(field(note, "topics"), "topics")]
    data = (
        frame(0x00, b"fidryn.note/v1")
        + frame(0x70, utf8(as_str(field(note, "group"), "the group of a note")))
        + frame(0x71, utf8(" ".join(topics)))
        + encode_optional_text(field(note, "heading"))
        + encode_blocks(field(note, "blocks"))
    )
    return blake3.blake3(data).digest()


def container_hash(container: Json) -> bytes:
    members = as_list(field(container, "members"), "the members of a container")
    data = (
        frame(0x00, b"fidryn.container/v1")
        + frame(0x01, utf8(as_str(field(container, "kind"), "the kind of a container")))
        + encode_num(field(container, "num"))
        + encode_optional_text(field(container, "heading"))
        + encode_status(field(container, "status"))
        + b"".join(frame(0x61, utf8(as_str(member, "a member"))) for member in members)
    )
    return blake3.blake3(data).digest()


def address_of(value: Json) -> str:
    return as_str(field(value, "address"), "an address")


def notes_of_layer(notes: list[Json], layer: str) -> bytes:
    """The frames of the notes of one layer, in the byte order of their addresses."""
    chosen = [note for note in notes if as_str(field(note, "layer"), "a layer") == layer]
    chosen.sort(key=lambda note: utf8(address_of(note)))
    return b"".join(
        frame(0x72, utf8(address_of(note))) + frame(0x73, note_hash(note)) for note in chosen
    )


def in_ordinal_order(footnotes: list[Json]) -> list[Json]:
    return sorted(footnotes, key=lambda footnote: as_int(field(footnote, "ordinal"), "ordinal"))


def enacted_layer(notes: list[Json]) -> bytes:
    data = frame(0x00, b"fidryn.layer.enacted/v1") + notes_of_layer(notes, "enacted")
    return blake3.blake3(data).digest()


def editorial_layer(
    credit: Any, notes: list[Json], footnotes: list[Json], marks: list[Mark]
) -> bytes:
    data = [
        frame(0x00, b"fidryn.layer.editorial/v1"),
        encode_optional_text(credit),
        notes_of_layer(notes, "editorial"),
    ]
    for footnote in in_ordinal_order(footnotes):
        shown = as_str(field(footnote, "shown"), "the shown text of a footnote")
        data.append(frame(0x74, utf8(shown)) + encode_blocks(field(footnote, "blocks")))
    for address, shown, footnote_number in marks:
        data.append(
            frame(0x75, utf8(address))
            + frame(0x76, utf8(shown))
            + frame(0x77, u32(footnote_number))
        )
    return blake3.blake3(b"".join(data)).digest()


def marks_of_text(text: Any, address: str, out: list[Mark]) -> None:
    if text is None:
        return
    for kind, content in runs_of(text):
        if kind == "mark":
            mark = as_object(content, "a footnote marker")
            shown = as_str(field(mark, "shown"), "the shown text of a marker")
            out.append((address, shown, as_int(field(mark, "footnote"), "a footnote number")))


def marks_of_num(num: Any, address: str, out: list[Mark]) -> None:
    if num is not None:
        marks_of_text(field(as_object(num, "a number"), "shown"), address, out)


def marks_of_blocks(blocks: Any, address: str, out: list[Mark]) -> None:
    for block in as_list(blocks, "blocks"):
        kind, content = variant(block, "a block")
        if kind in ("para", "subheading"):
            marks_of_text(content, address, out)
        elif kind == "table":
            for row in as_list(field(as_object(content, "a table"), "rows"), "rows"):
                for cell in as_list(field(as_object(row, "a row"), "cells"), "cells"):
                    marks_of_blocks(field(as_object(cell, "a cell"), "blocks"), address, out)
        elif kind == "quoted":
            items = as_list(field(as_object(content, "quoted content"), "items"), "items")
            for item in items:
                marks_of_quoted_item(item, address, out)


def marks_of_quoted_item(item: Any, address: str, out: list[Mark]) -> None:
    kind, content = variant(item, "an item of quoted content")
    if kind == "block":
        marks_of_blocks([content], address, out)
    elif kind == "part":
        marks_of_blocks(field(as_object(content, "a part"), "blocks"), address, out)
    elif kind == "level":
        level = as_object(content, "a level")
        marks_of_num(field(level, "num"), address, out)
        marks_of_text(field(level, "heading"), address, out)
        for inner in as_list(field(level, "items"), "the items of a level"):
            marks_of_quoted_item(inner, address, out)


def marks_of_provision(provision: Any, out: list[Mark]) -> None:
    """The markers of a provision's tree, depth first: number, heading, then items in order."""
    provision = as_object(provision, "a provision")
    address = address_of(provision)
    marks_of_num(field(provision, "num"), address, out)
    marks_of_text(field(provision, "heading"), address, out)
    for item in as_list(field(provision, "items"), "the items of a provision"):
        kind, content = variant(item, "an item")
        if kind == "part":
            marks_of_blocks(field(as_object(content, "a part"), "blocks"), address, out)
        else:
            marks_of_provision(content, out)


def marks_after_the_text(
    address: str, credit: Any, notes: list[Json], footnotes: list[Json], out: list[Mark]
) -> None:
    """The markers of a credit, then of each note in list order, then of each footnote."""
    marks_of_text(credit, address, out)
    for note in notes:
        marks_of_text(field(note, "heading"), address_of(note), out)
        marks_of_blocks(field(note, "blocks"), address_of(note), out)
    for footnote in in_ordinal_order(footnotes):
        marks_of_blocks(field(footnote, "blocks"), address, out)


def objects(value: Any, what: str) -> list[Json]:
    return [as_object(item, what) for item in as_list(value, what)]


def unit_layers(unit: Json) -> dict[str, str]:
    root = field(unit, "root")
    credit = field(unit, "credit")
    notes = objects(field(unit, "notes"), "the notes of a unit")
    footnotes = objects(field(unit, "footnotes"), "the footnotes of a unit")
    marks: list[Mark] = []
    marks_of_provision(root, marks)
    marks_after_the_text(address_of(unit), credit, notes, footnotes, marks)
    return {
        "operative": provision_hash(root).hex(),
        "enacted": enacted_layer(notes).hex(),
        "editorial": editorial_layer(credit, notes, footnotes, marks).hex(),
    }


def container_layers(container: Json) -> dict[str, str]:
    address = address_of(container)
    credit = field(container, "credit")
    notes = objects(field(container, "notes"), "the notes of a container")
    footnotes = objects(field(container, "footnotes"), "the footnotes of a container")
    marks: list[Mark] = []
    marks_of_num(field(container, "num"), address, marks)
    marks_of_text(field(container, "heading"), address, marks)
    marks_after_the_text(address, credit, notes, footnotes, marks)
    return {
        "operative": container_hash(container).hex(),
        "enacted": enacted_layer(notes).hex(),
        "editorial": editorial_layer(credit, notes, footnotes, marks).hex(),
    }


def digests(section: str, entry: Json) -> Json:
    """The digest fields of one entry of `section`, as the file holds them."""
    if section == "provisions":
        return {"hash": provision_hash(field(entry, "provision")).hex()}
    if section == "notes":
        return {"hash": note_hash(field(entry, "note")).hex()}
    if section == "units":
        return {"layers": unit_layers(as_object(field(entry, "unit"), "a unit"))}
    if section == "containers":
        container = as_object(field(entry, "container"), "a container")
        return {"hash": container_hash(container).hex(), "layers": container_layers(container)}
    raise ModelError(f"unknown section {section!r}")


def entries(vectors: Json) -> list[tuple[str, int, Json]]:
    """Each entry of the file with its section and its index in it."""
    found = []
    for section in SECTIONS:
        for index, entry in enumerate(as_list(field(vectors, section), section)):
            found.append((section, index, as_object(entry, f"{section}[{index}]")))
    return found


def mismatches(vectors: Json) -> list[str]:
    """One line for each digest of the file that differs from the value computed here."""
    problems = []
    for section, index, entry in entries(vectors):
        name = as_str(field(entry, "name"), "a name")
        for key, computed in digests(section, entry).items():
            recorded = entry.get(key)
            if recorded != computed:
                problems.append(
                    f"{section}[{index}] ({name}): {key} is {json.dumps(recorded)}, "
                    f"computed {json.dumps(computed)}"
                )
    return problems


def filled(vectors: Json) -> Json:
    """The file's content with every digest computed."""
    for section, _, entry in entries(vectors):
        entry.update(digests(section, entry))
    return vectors


def load(path: Path) -> Json:
    vectors = as_object(json.loads(path.read_text(encoding="utf-8")), "the file")
    if next(iter(vectors), None) != FIRST_KEY:
        raise ModelError(f"the first key of the file is not {FIRST_KEY!r}")
    return vectors


def dump(vectors: Json) -> str:
    """The file's text: two spaces of indentation, with its first key on the first line."""
    text = json.dumps(vectors, ensure_ascii=False, indent=2) + "\n"
    lead = '{\n  "' + FIRST_KEY + '": '
    if not text.startswith(lead):
        raise ModelError(f"the first key of the file is not {FIRST_KEY!r}")
    return '{"' + FIRST_KEY + '": ' + text[len(lead) :]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("vectors", nargs="?", type=Path, default=VECTORS, help="the file to read")
    parser.add_argument(
        "--write", action="store_true", help="fill in the digests instead of verifying them"
    )
    args = parser.parse_args(argv)
    try:
        vectors = load(args.vectors)
        if args.write:
            args.vectors.write_text(dump(filled(vectors)), encoding="utf-8")
            print(f"wrote the digests of {len(entries(vectors))} models")
            return 0
        problems = mismatches(vectors)
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        return 1
    print(f"the digests of {len(entries(vectors))} models verify")
    return 0


if __name__ == "__main__":
    sys.exit(main())
