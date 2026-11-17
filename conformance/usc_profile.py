#!/usr/bin/env python3
"""Count the Code's official XML by two sets of counting rules, and print both as JSON.

Reads the official XML release point published by the Office of the Law Revision
Counsel: a directory of archives, one archive, or one XML file. Prints one JSON
object with sorted keys and two members: `baseline`, the counts by the rules of
the first profile of the Code (2026-11-13), and `import`, the counts of what the
reading rules import.
`conformance/usc-counting-rules.md` states every rule and every key.

Usage: uv run --locked python conformance/usc_profile.py ARCHIVE_DIR_OR_XML [--out FILE] [--jobs N]
Exit codes: 0 printed; 1 the input is refused by a reading rule; 2 the input cannot be read.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from collections.abc import Iterable
from dataclasses import dataclass, field
from pathlib import Path
from xml.etree.ElementTree import Element

sys.path.insert(0, str(Path(__file__).resolve().parent))

from uslm_common import (
    PREFIX,
    USLM,
    Container,
    DocumentCounts,
    InputError,
    Job,
    MemberResult,
    RefusedError,
    Text,
    Unit,
    has_image,
    is_address,
    is_within,
    note_texts,
    provision_has_image,
    provision_texts,
    provisions_of,
    read_member,
    read_release,
    texts_of_blocks,
)

# The baseline: the counting rules of the first profile of the Code (2026-11-13).

BASELINE_LEVELS = frozenset(
    {
        "section",
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
BASELINE_EXCLUDED = frozenset({"note", "notes", "quotedContent", "toc", "sourceCredit"})
SECTION_OF = re.compile(r"^(/us/usc/t[0-9A-Za-z]+/s[^/]+)")


@dataclass(slots=True)
class BaselinePart:
    provisions: int = 0
    sections_by_status: Counter[str] = field(default_factory=Counter)
    identifiers: list[str] = field(default_factory=list)
    targets: list[str] = field(default_factory=list)


def baseline_part(root: Element) -> BaselinePart:
    """Counted elements, identifiers and links to the Code outside the excluded elements."""
    part = BaselinePart()
    namespace = "{" + USLM + "}"
    stack = [root]
    while stack:
        for child in stack.pop():
            name = child.tag[len(namespace) :] if child.tag.startswith(namespace) else child.tag
            if name in BASELINE_EXCLUDED:
                continue
            stack.append(child)
            if name in BASELINE_LEVELS:
                identifier = child.get("identifier")
                if identifier:
                    part.provisions += 1
                    part.identifiers.append(identifier)
                    if name == "section":
                        part.sections_by_status[child.get("status") or "operative"] += 1
            elif name == "ref":
                target = child.get("href")
                if target and target.startswith("/us/usc/"):
                    part.targets.append(target)
    return part


def baseline_profile(parts: Iterable[BaselinePart]) -> dict[str, object]:
    provisions = 0
    statuses: Counter[str] = Counter()
    identifiers: set[str] = set()
    targets: list[str] = []
    for part in parts:
        provisions += part.provisions
        statuses.update(part.sections_by_status)
        identifiers.update(part.identifiers)
        targets += part.targets
    sections = {m.group(1) for i in identifiers if (m := SECTION_OF.match(i))}
    exact = section_only = no_section = 0
    for target in targets:
        if target in identifiers:
            exact += 1
        elif (m := SECTION_OF.match(target)) and m.group(1) in sections:
            section_only += 1
        else:
            no_section += 1
    return {
        "provisions": provisions,
        "references": {
            "to_the_code": len(targets),
            "exact": exact,
            "section_only": section_only,
            "no_section": no_section,
        },
        "sections": sum(statuses.values()),
        "sections_by_status": dict(statuses),
    }


# The import: the counts of what the reading rules import.

LAYERS = ("operative", "enacted", "editorial")
OUTCOMES = (
    "exact",
    "target-status",
    "ambiguous",
    "unit-only",
    "range-form",
    "no-unit",
    "outside",
    "malformed",
)
IN_UNIT = 1
HAS_STATUS = 2


@dataclass(slots=True)
class ProfileSink:
    """The counts of one member, and what its references are resolved against."""

    counts: Counter[str] = field(default_factory=Counter)
    units_by_kind: Counter[str] = field(default_factory=Counter)
    units_by_status: Counter[str] = field(default_factory=Counter)
    containers_by_kind: Counter[str] = field(default_factory=Counter)
    notes_by_layer: Counter[str] = field(default_factory=Counter)
    notes_by_group: Counter[str] = field(default_factory=Counter)
    references: Counter[tuple[str, str]] = field(default_factory=Counter)
    index: dict[str, int] = field(default_factory=dict)
    aliases: list[tuple[list[str], int]] = field(default_factory=list)
    addresses: list[str] = field(default_factory=list)

    def refer(self, layer: str, texts: Iterable[Text]) -> None:
        for text in texts:
            for _, _, target in text.refs:
                self.references[(layer, target)] += 1

    def unit(self, unit: Unit) -> None:
        flags = IN_UNIT | (HAS_STATUS if unit.root.status is not None else 0)
        self.counts["units"] += 1
        self.units_by_kind[unit.root.kind] += 1
        self.units_by_status[unit.root.status or "in-force"] += 1
        self.aliases.append((unit.aliases, flags))
        for provision in provisions_of(unit.root):
            self.counts["provisions"] += 1
            self.addresses.append(provision.address)
            self.index[provision.address] = flags | (HAS_STATUS if provision.status else 0)
            self.counts["with_images"] += provision_has_image(provision)
            self.refer("operative", provision_texts(provision))
        self.module_matter(unit, flags)

    def container(self, container: Container) -> None:
        flags = HAS_STATUS if container.status is not None else 0
        self.counts["containers"] += 1
        self.containers_by_kind[container.kind] += 1
        self.aliases.append((container.aliases, flags))
        self.addresses.append(container.address)
        self.index[container.address] = flags
        if container.num is not None:
            self.refer("operative", [container.num.shown])
        if container.heading is not None:
            self.refer("operative", [container.heading])
        self.module_matter(container, 0)

    def module_matter(self, module: Unit | Container, flags: int) -> None:
        if module.credit is not None:
            self.refer("editorial", [module.credit])
        for footnote in module.footnotes:
            self.counts["footnotes"] += 1
            self.refer("editorial", texts_of_blocks(footnote.blocks))
        for note in module.notes:
            self.notes_by_layer[note.layer] += 1
            self.notes_by_group[note.group] += 1
            self.index[note.address] = flags
            self.counts["with_images"] += has_image(note.blocks)
            self.refer(note.layer, note_texts(note))


def outcome(target: str, index: dict[str, int], aliases: dict[str, int]) -> str:
    """The first outcome of the closed list that applies to a reference."""
    if not is_address(target):
        return "malformed"
    segments = target.split("/")
    beginnings = ["/".join(segments[:count]) for count in range(2, len(segments) + 1)]
    if any(b in index and b + "~2" in index for b in beginnings):
        return "ambiguous"

    def known(address: str) -> int | None:
        return index[address] if address in index else aliases.get(address)

    flags = known(target)
    if flags is not None:
        return "target-status" if flags & HAS_STATUS else "exact"
    longest = next((b for b in reversed(beginnings[:-1]) if known(b) is not None), None)
    if longest is not None and (known(longest) or 0) & IN_UNIT:
        return "unit-only"
    base = longest if longest is not None else (PREFIX if is_within(target, PREFIX) else None)
    if base is not None:
        segment = target[len(base) + 1 :].split("/", 1)[0]
        cut = min((segment.find(d) for d in "\u2013-" if d in segment), default=-1)
        if cut > 0 and known(f"{base}/{segment[:cut]}") is not None:
            return "range-form"
    if is_within(target, PREFIX):
        return "no-unit"
    return "outside"


def import_profile(release: str, results: list[MemberResult]) -> dict[str, object]:
    sinks = [result.output for result in results]
    if not all(isinstance(sink, ProfileSink) for sink in sinks):
        raise AssertionError(sinks)
    profile_sinks = [sink for sink in sinks if isinstance(sink, ProfileSink)]
    totals = ProfileSink()
    documents = DocumentCounts()
    for result, sink in zip(results, profile_sinks, strict=True):
        documents.add(result.counts)
        for name in (
            "counts",
            "units_by_kind",
            "units_by_status",
            "containers_by_kind",
            "notes_by_layer",
            "notes_by_group",
            "references",
        ):
            getattr(totals, name).update(getattr(sink, name))
        totals.index.update(sink.index)
        totals.addresses += sink.addresses
        totals.aliases += sink.aliases
    addresses = set(totals.addresses)
    alias_index: dict[str, int] = {}
    alias_count = 0
    for aliases, flags in totals.aliases:
        for alias in aliases:
            if alias not in addresses:
                alias_count += 1
                alias_index.setdefault(alias, flags)
    references = {layer: dict.fromkeys(OUTCOMES, 0) for layer in LAYERS}
    outside: Counter[str] = Counter()
    for (layer, target), count in totals.references.items():
        found = outcome(target, totals.index, alias_index)
        references[layer][found] += count
        if found == "outside":
            outside["/".join(target.split("/")[:3])] += count
    counts = totals.counts
    return {
        "prefix": PREFIX,
        "release": release,
        "units": counts["units"],
        "units_by_kind": dict(totals.units_by_kind),
        "units_by_status": dict(totals.units_by_status),
        "containers": counts["containers"],
        "containers_by_kind": dict(totals.containers_by_kind),
        "provisions": counts["provisions"],
        "addresses": {
            "aliases": alias_count,
            "disambiguated": documents.disambiguated,
            "assigned": documents.assigned,
        },
        "notes": {layer: totals.notes_by_layer[layer] for layer in ("enacted", "editorial")},
        "notes_by_group": dict(totals.notes_by_group),
        "footnotes": counts["footnotes"],
        "with_images": counts["with_images"],
        "references": references,
        "outside_by_body": dict(outside),
        "not_imported": {
            "tables_of_contents": documents.tables_of_contents,
            "footnotes_in_tables_of_contents": documents.footnotes_in_tables_of_contents,
            "links_in_tables_of_contents": documents.links_in_tables_of_contents,
            "release_notes": documents.release_notes,
            "links_without_words": documents.links_without_words,
            "note_group_headings": documents.note_group_headings,
        },
    }


def work(job: Job) -> MemberResult | RefusedError:
    sink = ProfileSink()
    try:
        root, reader, meta = read_member(job, sink)
    except RefusedError as error:
        return error
    baseline = baseline_part(root) if job.first_pass else None
    return MemberResult(
        job.source.member, meta.release, reader.identifiers, reader.counts, sink, baseline
    )


def profile(path: Path, jobs: int) -> dict[str, object]:
    release, results = read_release(path, work, jobs)
    parts = [result.baseline for result in results]
    if not all(isinstance(part, BaselinePart) for part in parts):
        raise AssertionError(parts)
    baseline = baseline_profile(part for part in parts if isinstance(part, BaselinePart))
    return {"baseline": baseline, "import": import_profile(release, results)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    parser.add_argument(
        "input", type=Path, help="a directory of archives, an archive or an XML file"
    )
    parser.add_argument("--out", type=Path, help="also write the profile to this file")
    parser.add_argument("--jobs", type=int, default=4, help="worker processes (default 4)")
    args = parser.parse_args()
    try:
        result = profile(args.input, args.jobs)
    except RefusedError as error:
        print(f"refused: {error.describe()}", file=sys.stderr)
        return 1
    except (InputError, OSError) as error:
        print(f"cannot read: {error}", file=sys.stderr)
        return 2
    text = json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    if args.out:
        args.out.write_text(text, encoding="utf-8", newline="\n")
    sys.stdout.buffer.write(text.encode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
