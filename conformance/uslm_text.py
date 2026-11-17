#!/usr/bin/env python3
"""Print the text records of the Code's official XML, one JSON object to a line.

Reads the official XML release point published by the Office of the Law Revision
Counsel: a directory of archives, one archive, or one XML file. Prints a record for
every provision, container, note, source credit and footnote, sorted by the bytes
of their addresses; for one address, the provision or container comes first, then
the credit, then the footnotes by ordinal.

Usage: uv run --locked python conformance/uslm_text.py ARCHIVE_DIR_OR_XML [--jobs N]
Exit codes: 0 printed; 1 the input is refused by a reading rule; 2 the input cannot be read.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from uslm_common import (
    Block,
    Container,
    Image,
    InputError,
    Job,
    MemberResult,
    Num,
    Para,
    Part,
    Provision,
    Quoted,
    QuotedItem,
    QuotedLevel,
    RefusedError,
    Subheading,
    Table,
    Text,
    Unit,
    provisions_of,
    read_member,
    read_release,
)

Json = dict[str, object]


def dumps(record: Json) -> str:
    return json.dumps(record, ensure_ascii=False, separators=(",", ":"))


def text_object(text: Text, key: str = "text") -> Json:
    record: Json = {
        key: text.plain,
        "refs": [[start, end, target] for start, end, target in text.refs],
        "sup": [[start, end] for start, end in text.sup],
        "sub": [[start, end] for start, end in text.sub],
    }
    if text.marks:
        record["marks"] = [[offset, shown, number] for offset, shown, number in text.marks]
    return record


def optional_text(text: Text | None) -> Json | None:
    return None if text is None else text_object(text)


def num_value(num: Num | None) -> list[object] | None:
    if num is None:
        return None
    shown: object = num.shown.plain if num.shown.is_words_only() else text_object(num.shown)
    return [num.value, shown]


def item_value(item: Block | QuotedItem) -> Json:
    match item:
        case Para(text):
            return text_object(text, "p")
        case Subheading(text):
            return text_object(text, "subheading")
        case Table(rows):
            return {
                "table": [
                    [
                        row.group,
                        [
                            [
                                cell.header,
                                cell.colspan,
                                cell.rowspan,
                                [item_value(b) for b in cell.blocks],
                            ]
                            for cell in row.cells
                        ],
                    ]
                    for row in rows
                ]
            }
        case Image(source):
            return {"image": source}
        case Quoted(origin, items):
            return {"quoted": {"origin": origin, "items": [item_value(i) for i in items]}}
        case Part(role, blocks):
            return {"part": role, "blocks": [item_value(b) for b in blocks]}
        case QuotedLevel(kind, num, heading, items):
            return {
                "level": kind,
                "num": num_value(num),
                "heading": optional_text(heading),
                "items": [item_value(i) for i in items],
            }
    raise AssertionError(item)


def provision_record(provision: Provision) -> Json:
    items: list[Json] = []
    for item in provision.items:
        if isinstance(item, Provision):
            items.append({"child": item.address})
        else:
            items.append(item_value(item))
    return {
        "address": provision.address,
        "layer": "operative",
        "kind": provision.kind,
        "status": provision.status,
        "num": num_value(provision.num),
        "heading": optional_text(provision.heading),
        "items": items,
    }


Entry = tuple[str, int, int, str | Json]


class TextSink:
    """Collects the records of one member; the aliases are filtered once the release is read."""

    def __init__(self) -> None:
        self.records: list[Entry] = []
        self.addresses: list[str] = []

    def unit(self, unit: Unit) -> None:
        for provision in provisions_of(unit.root):
            self.addresses.append(provision.address)
            record = provision_record(provision)
            if provision is unit.root:
                record["aliases"] = unit.aliases
                record["within"] = unit.within
                record["source"] = unit.source
                record["positive_law"] = unit.positive_law
                self.records.append((provision.address, 0, 0, record))
            else:
                self.records.append((provision.address, 0, 0, dumps(record)))
        self.credit(unit)
        self.footnotes(unit)
        self.notes(unit)

    def container(self, container: Container) -> None:
        self.addresses.append(container.address)
        record: Json = {
            "address": container.address,
            "layer": "operative",
            "kind": container.kind,
            "status": container.status,
            "num": num_value(container.num),
            "heading": optional_text(container.heading),
            "aliases": container.aliases,
            "within": container.within,
            "members": container.members,
        }
        self.records.append((container.address, 0, 0, record))
        self.credit(container)
        self.footnotes(container)
        self.notes(container)

    def credit(self, module: Unit | Container) -> None:
        if module.credit is not None:
            record: Json = {"address": module.address, "record": "credit"}
            record.update(text_object(module.credit, "p"))
            self.records.append((module.address, 1, 0, dumps(record)))

    def footnotes(self, module: Unit | Container) -> None:
        address = module.address
        for footnote in module.footnotes:
            record: Json = {
                "address": address,
                "record": "footnote",
                "ordinal": footnote.ordinal,
                "shown": footnote.shown,
                "blocks": [item_value(b) for b in footnote.blocks],
            }
            self.records.append((address, 2, footnote.ordinal, dumps(record)))

    def notes(self, module: Unit | Container) -> None:
        for note in module.notes:
            record: Json = {
                "address": note.address,
                "layer": note.layer,
                "group": note.group,
                "topics": note.topics,
                "heading": optional_text(note.heading),
                "blocks": [item_value(b) for b in note.blocks],
            }
            self.records.append((note.address, 0, 0, dumps(record)))


def work(job: Job) -> MemberResult | RefusedError:
    sink = TextSink()
    try:
        _, reader, meta = read_member(job, sink)
    except RefusedError as error:
        return error
    return MemberResult(job.source.member, meta.release, reader.identifiers, reader.counts, sink)


def text_lines(results: list[MemberResult]) -> list[str]:
    """Every record of the release, sorted, with each alias that is an address dropped."""
    addresses: set[str] = set()
    entries: list[Entry] = []
    for result in results:
        sink = result.output
        if not isinstance(sink, TextSink):
            raise AssertionError(sink)
        addresses.update(sink.addresses)
        entries += sink.records
    entries.sort(key=lambda entry: entry[:3])
    lines: list[str] = []
    for _, _, _, payload in entries:
        if isinstance(payload, dict):
            aliases = payload["aliases"]
            if not isinstance(aliases, list):
                raise AssertionError(aliases)
            payload["aliases"] = [alias for alias in aliases if alias not in addresses]
            payload = dumps(payload)
        lines.append(payload)
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    parser.add_argument(
        "input", type=Path, help="a directory of archives, an archive or an XML file"
    )
    parser.add_argument("--jobs", type=int, default=4, help="worker processes (default 4)")
    args = parser.parse_args()
    try:
        _, results = read_release(args.input, work, args.jobs)
    except RefusedError as error:
        print(f"refused: {error.describe()}", file=sys.stderr)
        return 1
    except (InputError, OSError) as error:
        print(f"cannot read: {error}", file=sys.stderr)
        return 2
    out = sys.stdout.buffer
    for line in text_lines(results):
        out.write(line.encode("utf-8") + b"\n")
    out.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
