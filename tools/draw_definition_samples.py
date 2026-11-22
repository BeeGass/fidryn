#!/usr/bin/env python3
"""Draw the two samples of definitions that a person reads to judge their extraction.

Reads the definitions of a generated workspace from the `definition` blocks of its
module files, in the canonical form that a generated workspace holds them in, one
declaration to a line, and stops at a line of a block that is not in that form, a
line that begins with the word `definition` at another indentation among them. Holds
the number of the definitions it read, by basis, to `totals.definitions` of the
workspace's coverage ledger, `coverage.json`, which is to be of the release of the
workspace. Reads the text of its provisions from the text records that
`fidryn text DIR --json` printed for the same workspace, given as `--records FILE`,
and holds them to the workspace: they count the units, containers, provisions, notes
by layer and footnotes that the ledger counts, and every definition's provision has a
record. It starts no program. It writes into `--out DIR`, which must not hold either
file yet:

- `extracted.jsonl`: 500 definitions drawn without replacement from every definition
  of the workspace. Each line holds the address of the provision, the terms, the kind,
  the reach words (`unstated` when there are none), the region (`null` when there is
  none), the basis, and the text of the paragraph it was extracted from: of the first
  blocks of the provision's chapeau and content parts that are paragraphs, the one
  whose first term in curly quotation marks is the definition's first term (the second
  such for a second definition of that term, and so on).
- `near-misses.jsonl`: 500 paragraphs of operative text drawn without replacement from
  those that hold a term in curly quotation marks, `“` and `”` around at least one
  character that is neither, followed within 60 characters, from the end of the term
  to the start of the verb, by one of the verb forms `mean`, `means`, `include`,
  `includes`, `shall mean` and `has the meaning`, standing as words (no letter `A` to
  `Z` or `a` to `z` just before or after them, and in the case written here), and from
  which no definition was extracted. The paragraphs are those of every text part of
  every provision, in its blocks and in the cells of its tables, and not in quoted
  content. Each line holds the address of the provision, the role of the part, the
  place of the paragraph among the paragraphs of the provision (from 1, in the order of
  the text), its text, and the first term and its verb that put it into the population.

Every line also holds the seed, the release of the workspace, the size of the
population it was drawn from, the place of the line in the draw (from 1), the address
and the text of the chapeau of the nearest provision above the provision that has one
(`null` when none has), where an inherited reach stands, and an empty `verdict` for
the reader. The lines are written in the order of the draw, as compact JSON.

The draw is that of `draw`: the population in the order of its addresses (the bytes of
the address, then the place in the module or in the provision), shuffled by the first
steps of a Fisher-Yates shuffle driven by SplitMix64 (`SplitMix64`), so that one seed
draws the same lines with any version of Python.

Usage: uv run --locked python -I tools/draw_definition_samples.py --workspace DIR
  --records FILE --seed N --out DIR
Exit codes: 0 drawn; 1 an input is not in the form the tool reads, or a sample is there
already; 2 an input cannot be read or the output written.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SIZE = 500
MASK = (1 << 64) - 1
WINDOW = 60
KINDS = ("means", "includes", "does_not_include", "borrowed")
BASES = ("enacted", "restated", "unresolved")
STRING = r'"(?:[^"\\]|\\.)*"'
HEADER = re.compile(rf"  definition ({STRING}) \{{")
FIELDS = {
    name: re.compile(rf"    {name} ({pattern})")
    for name, pattern in (
        ("term", STRING),
        ("kind", r"[a-z_]+"),
        ("words", rf"{STRING}|unstated"),
        ("region", STRING),
        ("basis", r"[a-z_]+"),
        ("borrowed_from", STRING),
        ("see_note", STRING),
    )
}
# A line that begins a definition block in any form: the word `definition` after white
# space, or none.
DECLARATION = re.compile(r"[ \t]*definition(?![A-Za-z0-9_])")
TERM = re.compile(r"“([^“”]+)”")
VERB = re.compile(
    r"(?<![A-Za-z])(?:shall mean|has the meaning|includes|include|means|mean)(?![A-Za-z])"
)
EXTRACTED = "extracted.jsonl"
NEAR_MISSES = "near-misses.jsonl"
LEDGER = "coverage.json"


class FormError(Exception):
    """An input is not in the form the tool reads, or a sample is there already."""


class SplitMix64:
    """The generator SplitMix64 of Steele, Lea and Flood, on 64-bit words: each step adds
    0x9E3779B97F4A7C15 to the state, and mixes the new state by two multiplications."""

    def __init__(self, seed: int) -> None:
        self.state = seed & MASK

    def next(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        mixed = self.state
        mixed = ((mixed ^ (mixed >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        mixed = ((mixed ^ (mixed >> 27)) * 0x94D049BB133111EB) & MASK
        return mixed ^ (mixed >> 31)

    def below(self, bound: int) -> int:
        """A number from 0 to `bound - 1`, each as likely as the others: a word below the
        largest multiple of `bound` that 64 bits hold, modulo `bound`; a word at or above
        it is drawn again."""
        limit = (1 << 64) - (1 << 64) % bound
        while True:
            word = self.next()
            if word < limit:
                return word % bound


def draw(count: int, size: int, seed: int) -> list[int]:
    """The places, in a population of `count`, of `min(size, count)` members drawn
    without replacement, in the order drawn: for each step `i`, the member at a place
    from `i` to `count - 1` that `below(count - i)` gives, added to `i`, is swapped with
    the member at `i` and drawn."""
    generator = SplitMix64(seed)
    places = list(range(count))
    taken = min(size, count)
    for step in range(taken):
        other = step + generator.below(count - step)
        places[step], places[other] = places[other], places[step]
    return places[:taken]


def loose_match(words: str) -> tuple[str, str] | None:
    """The first term in curly quotation marks of `words` that a verb form follows
    within `WINDOW` characters, with that verb form; `None` when there is none."""
    for term in TERM.finditer(words):
        verb = VERB.search(words, term.end())
        if verb is not None and verb.start() - term.end() <= WINDOW:
            return term[1], verb[0]
    return None


@dataclass
class Definition:
    address: str
    terms: list[str]
    kind: str
    words: str
    region: str | None
    basis: str
    place: tuple[str, int]
    text: str = ""
    item: int = -1


@dataclass
class Paragraph:
    address: str
    role: str
    ordinal: int
    text: str
    # The place of its part among the items of the provision, when it is the part's
    # first block; such a paragraph is the one a definition is extracted from.
    first_of: int | None
    term: str
    verb: str


def decoded(literal: str, where: str) -> str:
    try:
        value = json.loads(literal)
    except json.JSONDecodeError as error:
        raise FormError(f"{where}: {literal} is no string: {error}") from error
    if not isinstance(value, str):
        raise FormError(f"{where}: {literal} is no string")
    return value


def read_block(lines: list[str], at: int, file: str, number: int) -> tuple[Definition, int]:
    """The definition block that begins at `lines[at]`, the line `number` of `file`, and
    the place of the line after it."""
    header = HEADER.fullmatch(lines[at])
    if header is None:
        raise FormError(f"{file}:{number}: `{lines[at]}` is no header of a definition block")
    address = decoded(header[1], f"{file}:{number}")
    values: dict[str, list[str]] = {}
    index = at + 1

    def field(name: str, required: bool, many: bool = False) -> None:
        nonlocal index
        while index < len(lines):
            found = FIELDS[name].fullmatch(lines[index])
            if found is None:
                break
            values.setdefault(name, []).append(found[1])
            index += 1
            if not many:
                break
        if required and name not in values:
            line = lines[index] if index < len(lines) else ""
            raise FormError(
                f"{file}:{number + index - at}: `{line}` stands where the definition block "
                f"of `{address}` has its `{name}`"
            )

    field("term", True, many=True)
    field("kind", True)
    field("words", True)
    field("region", False)
    field("basis", True)
    field("borrowed_from", False)
    field("see_note", False)
    if index >= len(lines) or lines[index] != "  }":
        line = lines[index] if index < len(lines) else ""
        raise FormError(
            f"{file}:{number + index - at}: `{line}` stands where the definition block of "
            f"`{address}` ends"
        )
    where = f"{file}:{number}"
    kind, basis = values["kind"][0], values["basis"][0]
    if kind not in KINDS or basis not in BASES:
        raise FormError(f"{where}: the kind `{kind}` or the basis `{basis}` is not of its list")
    words = values["words"][0]
    region = values.get("region")
    return (
        Definition(
            address,
            [decoded(term, where) for term in values["term"]],
            kind,
            "unstated" if words == "unstated" else decoded(words, where),
            decoded(region[0], where) if region else None,
            basis,
            (file, number),
        ),
        index + 1,
    )


def module_files(top: Path, relative: str = "") -> Iterator[tuple[Path, str]]:
    """The module files below `top`, in the order of their paths; an entry whose name
    begins with a dot is left out, and a link stops the reading."""
    with os.scandir(top / relative if relative else top) as found:
        entries = sorted(found, key=lambda entry: entry.name.encode())
    for entry in entries:
        if entry.name.startswith("."):
            continue
        name = f"{relative}/{entry.name}" if relative else entry.name
        if entry.is_symlink():
            raise FormError(f"{name} is a symbolic link, and the tool follows no link")
        if entry.is_dir():
            yield from module_files(top, name)
        elif entry.name.endswith(".fr"):
            yield Path(entry.path), name


def read_definitions(workspace: Path) -> tuple[str, list[Definition]]:
    """The release of the generated workspace `workspace` and its definitions, in the
    order of their addresses."""
    try:
        manifest = json.loads((workspace / "workspace.json").read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise FormError(f"{workspace / 'workspace.json'} does not read: {error}") from error
    if not isinstance(manifest, dict) or manifest.get("kind") != "generated":
        raise FormError(f"{workspace} holds no generated workspace")
    release = manifest.get("release")
    if not isinstance(release, str):
        raise FormError(f"{workspace / 'workspace.json'} names no release")
    definitions = []
    for path, name in module_files(workspace):
        source = path.read_text(encoding="utf-8")
        if "definition" not in source:
            continue
        lines = source.split("\n")
        index = 0
        while index < len(lines):
            if DECLARATION.match(lines[index]):
                found, index = read_block(lines, index, name, index + 1)
                definitions.append(found)
            else:
                index += 1
    definitions.sort(key=lambda found: (found.address.encode(), found.place))
    return release, definitions


def listed(counts: dict[str, Any]) -> str:
    return ", ".join(f"{name}: {count}" for name, count in counts.items())


def read_ledger(workspace: Path, release: str) -> dict[str, Any]:
    """The totals of the coverage ledger of the workspace, which must be of `release`."""
    path = workspace / LEDGER
    try:
        ledger = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise FormError(f"{path} does not read: {error}") from error
    if (
        not isinstance(ledger, dict)
        or ledger.get("schema") != "fidryn.coverage/v0.1"
        or ledger.get("release") != release
        or not isinstance(ledger.get("totals"), dict)
    ):
        raise FormError(f"{path} is no coverage ledger of the release {release} of the workspace")
    totals: dict[str, Any] = ledger["totals"]
    return totals


def hold_definitions(definitions: list[Definition], totals: dict[str, Any], ledger: Path) -> None:
    """Stops unless the ledger counts, by basis, the definitions read."""
    counted = dict.fromkeys(BASES, 0)
    for definition in definitions:
        counted[definition.basis] += 1
    if totals.get("definitions") != counted:
        stated = totals.get("definitions")
        raise FormError(
            f"the module files hold the definitions {listed(counted)}, and {ledger} counts "
            f"{listed(stated) if isinstance(stated, dict) else stated}: a definition is in a "
            "form the tool does not read, or the ledger is of other modules"
        )


def paragraphs(blocks: list[Any]) -> Iterator[tuple[bool, str]]:
    """The paragraphs of `blocks`, and of the cells of their tables, in order, each with
    whether it is the first of `blocks`; quoted content is not read."""
    for index, block in enumerate(blocks):
        if "p" in block:
            yield index == 0, str(block["p"])
        elif "table" in block:
            for _, cells in block["table"]:
                for _, _, _, inner in cells:
                    for _, words in paragraphs(inner):
                        yield False, words


@dataclass
class Provisions:
    parent: dict[str, str]
    chapeau: dict[str, str]
    # The first blocks of the chapeau and content parts of each provision that holds a
    # definition, that are paragraphs: the place of the part and the text.
    firsts: dict[str, list[tuple[int, str]]]
    near: list[Paragraph]
    # What the records count, by the names of the coverage ledger.
    counted: dict[str, int]

    def nearest(self, address: str) -> dict[str, str] | None:
        above = self.parent.get(address)
        while above is not None:
            if above in self.chapeau:
                return {"address": above, "text": self.chapeau[above]}
            above = self.parent.get(above)
        return None


COUNTED = ("units", "containers", "provisions", "enacted notes", "editorial notes", "footnotes")


def read_records(records: Path, defined: set[str]) -> Provisions:
    found = Provisions({}, {}, {}, [], dict.fromkeys(COUNTED, 0))
    with records.open(encoding="utf-8") as stream:
        for number, line in enumerate(stream, start=1):
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise FormError(f"{records}:{number}: no text record: {error}") from error
            if record.get("record") == "footnote":
                found.counted["footnotes"] += 1
            if "group" in record:
                layer = record.get("layer")
                if layer not in ("enacted", "editorial"):
                    raise FormError(f"{records}:{number}: a note of no layer: {layer!r}")
                found.counted[f"{layer} notes"] += 1
            if "members" in record:
                found.counted["containers"] += 1
            if "record" in record or "group" in record or "members" in record:
                continue
            if record.get("layer") != "operative" or "items" not in record:
                raise FormError(f"{records}:{number}: no record of a provision")
            found.counted["provisions"] += 1
            if "source" in record:
                found.counted["units"] += 1
            address = str(record["address"])
            if address in defined:
                found.firsts[address] = []
            chapeau: list[str] = []
            ordinal = 0
            for place, item in enumerate(record["items"]):
                if "child" in item:
                    found.parent[str(item["child"])] = address
                    continue
                role, blocks = str(item["part"]), item["blocks"]
                if role == "chapeau":
                    chapeau += [str(block["p"]) for block in blocks if "p" in block]
                if (
                    address in defined
                    and role in ("chapeau", "content")
                    and blocks
                    and "p" in blocks[0]
                ):
                    found.firsts[address].append((place, str(blocks[0]["p"])))
                for first, words in paragraphs(blocks):
                    ordinal += 1
                    match = loose_match(words)
                    if match is not None:
                        found.near.append(
                            Paragraph(
                                address, role, ordinal, words, place if first else None, *match
                            )
                        )
            if chapeau:
                found.chapeau[address] = "\n".join(chapeau)
    return found


def hold_records(provisions: Provisions, totals: dict[str, Any], ledger: Path) -> None:
    """Stops unless the records count the units, containers, provisions, notes and
    footnotes that the ledger of the workspace counts."""
    notes = totals.get("notes")
    stated = {
        "units": totals.get("units"),
        "containers": totals.get("containers"),
        "provisions": totals.get("provisions"),
        "enacted notes": notes.get("enacted") if isinstance(notes, dict) else None,
        "editorial notes": notes.get("editorial") if isinstance(notes, dict) else None,
        "footnotes": totals.get("footnotes"),
    }
    if provisions.counted != stated:
        raise FormError(
            f"the records count {listed(provisions.counted)}, and {ledger} counts "
            f"{listed(stated)}: they are to be the records that `fidryn text DIR --json` "
            "printed for the workspace"
        )


def place_definitions(definitions: list[Definition], provisions: Provisions) -> None:
    """Gives each definition the paragraph it was extracted from."""
    taken: dict[tuple[str, str], int] = {}
    for definition in definitions:
        if definition.address not in provisions.firsts:
            raise FormError(
                f"the records hold no provision at `{definition.address}`, which a definition "
                "names; they are to be the records of the workspace"
            )
        term = definition.terms[0]
        candidates = [
            (place, words)
            for place, words in provisions.firsts[definition.address]
            if (first := TERM.search(words)) is not None and first[1] == term
        ]
        key = (definition.address, term)
        nth = taken.get(key, 0)
        if nth >= len(candidates):
            raise FormError(
                f"the records hold no paragraph of `{definition.address}` whose first term is "
                f"“{term}”, from which its definition is extracted"
            )
        definition.item, definition.text = candidates[nth]
        taken[key] = nth + 1


def compact(value: dict[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n"


@dataclass(frozen=True)
class Drawn:
    release: str
    extracted: tuple[int, int]
    near_misses: tuple[int, int]


def draw_samples(workspace: Path, records: Path, seed: int, out: Path, size: int = SIZE) -> Drawn:
    """Draws both samples into `out`, and returns how many lines of how large a
    population each holds."""
    for name in (EXTRACTED, NEAR_MISSES):
        if (out / name).exists():
            raise FormError(f"{out / name} is there already, and a sample drawn is kept")
    release, definitions = read_definitions(workspace)
    totals = read_ledger(workspace, release)
    hold_definitions(definitions, totals, workspace / LEDGER)
    provisions = read_records(records, {definition.address for definition in definitions})
    hold_records(provisions, totals, workspace / LEDGER)
    place_definitions(definitions, provisions)
    extracted = {(definition.address, definition.item) for definition in definitions}
    near = sorted(
        (
            paragraph
            for paragraph in provisions.near
            if (paragraph.address, paragraph.first_of) not in extracted
        ),
        key=lambda paragraph: (paragraph.address.encode(), paragraph.ordinal),
    )
    common = {"seed": seed, "release": release}
    lines = []
    for number, place in enumerate(draw(len(definitions), size, seed), start=1):
        definition = definitions[place]
        lines.append(
            compact(
                {
                    **common,
                    "population": len(definitions),
                    "draw": number,
                    "address": definition.address,
                    "terms": definition.terms,
                    "kind": definition.kind,
                    "words": definition.words,
                    "region": definition.region,
                    "basis": definition.basis,
                    "text": definition.text,
                    "chapeau": provisions.nearest(definition.address),
                    "verdict": "",
                }
            )
        )
    near_lines = []
    for number, place in enumerate(draw(len(near), size, seed), start=1):
        paragraph = near[place]
        near_lines.append(
            compact(
                {
                    **common,
                    "population": len(near),
                    "draw": number,
                    "address": paragraph.address,
                    "part": paragraph.role,
                    "paragraph": paragraph.ordinal,
                    "text": paragraph.text,
                    "chapeau": provisions.nearest(paragraph.address),
                    "term": paragraph.term,
                    "verb": paragraph.verb,
                    "verdict": "",
                }
            )
        )
    out.mkdir(parents=True, exist_ok=True)
    for name, written in ((EXTRACTED, lines), (NEAR_MISSES, near_lines)):
        with (out / name).open("x", encoding="utf-8", newline="\n") as stream:
            stream.writelines(written)
    return Drawn(release, (len(lines), len(definitions)), (len(near_lines), len(near)))


def seed_of(text: str) -> int:
    value = int(text)
    if not 0 <= value <= MASK:
        raise argparse.ArgumentTypeError(f"{text} is not a seed from 0 to {MASK}")
    return value


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n", 1)[0])
    parser.add_argument("--workspace", type=Path, required=True, help="a generated workspace")
    parser.add_argument(
        "--records", type=Path, required=True, help="what `fidryn text DIR --json` printed"
    )
    parser.add_argument("--seed", type=seed_of, required=True, help="the seed of the draw")
    parser.add_argument("--out", type=Path, required=True, help="the directory of the samples")
    args = parser.parse_args(argv)
    try:
        drawn = draw_samples(args.workspace, args.records, args.seed, args.out)
    except FormError as error:
        print(f"refused: {error}", file=sys.stderr)
        return 1
    except (OSError, UnicodeDecodeError) as error:
        print(f"cannot read or write: {error}", file=sys.stderr)
        return 2
    print(
        f"drew {drawn.extracted[0]} of {drawn.extracted[1]} definitions and "
        f"{drawn.near_misses[0]} of {drawn.near_misses[1]} near misses of the release "
        f"{drawn.release} with the seed {args.seed}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
