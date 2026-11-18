#!/usr/bin/env python3
"""Compare the text records of two releases of the Code's official XML.

Reads the official XML release point published by the Office of the Law Revision
Counsel, two releases of it (each a directory of archives, one archive, or one XML
file), into the text records that `uslm_text.py` prints, and from those records
alone prints, as one JSON object, which addresses differ between the two:

- `operative`: of the provisions, with their footnote markers left out, those only
  in the old (`old_only`) and only in the new (`new_only`), each reduced to the
  highest provision, the one whose parent is not on that side only; and those on
  both sides whose records differ (`differ`). Of the top provision of a unit, its
  aliases, the container it is within, its source and its positive-law flag are
  not compared.
- `enacted_notes` and `editorial_notes`: the same three sets from the records of
  the notes of each layer, footnote markers left out.
- `credits`: the units and containers on both sides whose credit record, footnote
  markers left out, is on one side only or differs.
- `footnotes`: the units and containers on both sides whose footnote records,
  footnote markers left out, differ, or whose footnote markers differ: each marker
  as the address of the provision or note it stands in (the module's for one in a
  credit or a footnote), its shown text and its footnote, in document order.
- `containers`: the same three sets from the records of the containers, footnote
  markers and the container each is within left out.

With `--against CHANGES.json`, a change set that `fidryn changes --json` printed
for the same two releases, it prints nothing and exits 1, naming the first address
that differs, unless each set equals what the change set lists: the provisions
only in the old equal `operative.removed` with the `from` of `operative.moved`,
those only in the new `operative.added` with the `to` of `operative.moved`, and
those that differ `amended_text` with `amended_structure`; the notes of each layer
likewise against `added`, `removed`, `moved` and `amended`; the credits
`editorial.credits_changed`; the footnotes `editorial.footnotes_changed`; and the
containers `containers.removed`, `added` and `changed`.

Each record is kept as its line, and read again only where the lines of the two
releases differ, so that two whole releases fit in memory.

Usage: uv run --locked python -I conformance/usc_compare.py OLD NEW [--against FILE]
Exit codes: 0 printed, or the change set agrees; 1 an input is refused by a reading
rule, or the change set differs; 2 an input cannot be read.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

from uslm_common import InputError, RefusedError, read_release
from uslm_text import text_lines, work

Record = dict[str, Any]
Mark = tuple[str, str, int]

# The keys of the record of a unit's top provision that are not compared.
UNIT_KEYS = ("aliases", "within", "source", "positive_law")


def unmarked(value: object) -> object:
    """`value` without the footnote markers of its texts, and with the shown text of
    each number that is words only written as its plain string, as a record writes a
    number that holds no marker."""
    if isinstance(value, list):
        return [unmarked(item) for item in value]
    if not isinstance(value, dict):
        return value
    out = {key: unmarked(item) for key, item in value.items() if key != "marks"}
    num = out.get("num")
    if isinstance(num, list) and len(num) == 2 and isinstance(num[1], dict):
        shown = num[1]
        if not shown["refs"] and not shown["sup"] and not shown["sub"]:
            out["num"] = [num[0], shown["text"]]
    return out


def marks_in(value: object, address: str, out: list[Mark]) -> None:
    """Adds the footnote markers of every text of `value`, in document order, each
    with `address`."""
    if isinstance(value, list):
        for item in value:
            marks_in(item, address, out)
    elif isinstance(value, dict):
        for key, item in value.items():
            if key == "marks":
                out.extend((address, str(shown), int(footnote)) for _, shown, footnote in item)
            else:
                marks_in(item, address, out)


def comparable_provision(record: Record) -> object:
    return unmarked({key: item for key, item in record.items() if key not in UNIT_KEYS})


def comparable_container(record: Record) -> object:
    return unmarked({key: item for key, item in record.items() if key != "within"})


def same(old: str | None, new: str | None, comparable: Callable[[Record], object]) -> bool:
    """Whether the records of the lines `old` and `new` are the same as `comparable`
    gives them; a line that is not there is the same only as another that is not."""
    if old == new:
        return True
    if old is None or new is None:
        return False
    return bool(comparable(json.loads(old)) == comparable(json.loads(new)))


@dataclass
class Side:
    """The text records of one release, each kept as its line, by what they are."""

    provisions: dict[str, str] = field(default_factory=dict)
    # The children of each provision that has any, in order, and the parent of each.
    children: dict[str, list[str]] = field(default_factory=dict)
    parent: dict[str, str] = field(default_factory=dict)
    # The top provisions of the units, and the unit of each provision.
    units: set[str] = field(default_factory=set)
    unit_of: dict[str, str] = field(default_factory=dict)
    containers: dict[str, str] = field(default_factory=dict)
    notes: dict[str, str] = field(default_factory=dict)
    note_layers: dict[str, str] = field(default_factory=dict)
    # The notes of each unit and container, in the order of their addresses.
    notes_of: dict[str, list[str]] = field(default_factory=dict)
    credits: dict[str, str] = field(default_factory=dict)
    # In ordinal order.
    footnotes: dict[str, list[str]] = field(default_factory=dict)

    def module_of_note(self, address: str) -> str:
        """The unit or container of the note at `address`: of the provision or the
        container whose address, followed by `/note/`, begins it, the longest."""
        anchor = None
        at = address.find("/note/")
        while at != -1:
            if address[:at] in self.provisions or address[:at] in self.containers:
                anchor = address[:at]
            at = address.find("/note/", at + 1)
        if anchor is None:
            raise InputError(f"the note {address} stands in no provision or container")
        return self.unit_of.get(anchor, anchor)

    def tree(self, unit: str) -> list[str]:
        """The provisions of `unit`, in document order."""
        out = [unit]
        for child in self.children.get(unit, []):
            out += self.tree(child)
        return out

    def lines_of(self, module: str) -> list[str]:
        """Every record line of the unit or container `module`."""
        if module in self.units:
            lines = [self.provisions[address] for address in self.tree(module)]
        else:
            lines = [self.containers[module]]
        if module in self.credits:
            lines.append(self.credits[module])
        lines += [self.notes[address] for address in self.notes_of.get(module, [])]
        return lines + self.footnotes.get(module, [])

    def marks(self, module: str) -> list[Mark]:
        """Every footnote marker of the unit or container `module`, in document
        order: the provisions of a unit depth first, each its number, its heading
        and then its items, or the number and the heading of a container; then the
        credit; the notes in the order of their addresses, each its heading and then
        its blocks; and the footnotes in ordinal order."""
        out: list[Mark] = []

        def provision(address: str) -> None:
            record = json.loads(self.provisions[address])
            marks_in(record["num"], address, out)
            marks_in(record["heading"], address, out)
            for item in record["items"]:
                if "child" in item:
                    provision(str(item["child"]))
                else:
                    marks_in(item, address, out)

        if module in self.units:
            provision(module)
        else:
            container = json.loads(self.containers[module])
            marks_in(container["num"], module, out)
            marks_in(container["heading"], module, out)
        if module in self.credits:
            marks_in(json.loads(self.credits[module]), module, out)
        for address in self.notes_of.get(module, []):
            note = json.loads(self.notes[address])
            marks_in(note["heading"], address, out)
            marks_in(note["blocks"], address, out)
        for line in self.footnotes.get(module, []):
            marks_in(json.loads(line)["blocks"], module, out)
        return out


def side_of(lines: list[str]) -> Side:
    """The records of `lines`, one to a line, as `uslm_text.py` prints them."""
    side = Side()
    footnotes: dict[str, list[tuple[int, str]]] = {}
    for line in lines:
        record: Record = json.loads(line)
        address = str(record["address"])
        if record.get("record") == "credit":
            side.credits[address] = line
        elif record.get("record") == "footnote":
            footnotes.setdefault(address, []).append((int(record["ordinal"]), line))
        elif "group" in record:
            side.notes[address] = line
            side.note_layers[address] = str(record["layer"])
        elif "members" in record:
            side.containers[address] = line
        else:
            side.provisions[address] = line
            children = [str(item["child"]) for item in record["items"] if "child" in item]
            if children:
                side.children[address] = children
            if "source" in record:
                side.units.add(address)
    side.footnotes = {
        module: [line for _, line in sorted(found)] for module, found in footnotes.items()
    }
    for unit in side.units:
        for address in side.tree(unit):
            side.unit_of[address] = unit
            for child in side.children.get(address, []):
                side.parent[child] = address
    for address in sorted(side.notes):
        side.notes_of.setdefault(side.module_of_note(address), []).append(address)
    return side


def read_side(path: Path, jobs: int) -> Side:
    _, results = read_release(path, work, jobs)
    return side_of(text_lines(results))


def three_sets(
    old: dict[str, str], new: dict[str, str], comparable: Callable[[Record], object]
) -> dict[str, set[str]]:
    """The addresses only in `old`, only in `new`, and on both sides whose records,
    as `comparable` gives them, differ."""
    both = old.keys() & new.keys()
    return {
        "old_only": set(old) - set(new),
        "new_only": set(new) - set(old),
        "differ": {a for a in both if not same(old[a], new[a], comparable)},
    }


def highest(addresses: set[str], parent: dict[str, str]) -> set[str]:
    """The addresses of `addresses` whose parent is not one of them."""
    return {address for address in addresses if parent.get(address) not in addresses}


@dataclass
class Sets:
    """What differs between two releases."""

    groups: dict[str, dict[str, set[str]]]
    credits: set[str]
    footnotes: set[str]

    def to_json(self) -> str:
        value: dict[str, object] = {
            name: {key: sorted(found) for key, found in sets.items()}
            for name, sets in self.groups.items()
            if name != "containers"
        }
        value["credits"] = sorted(self.credits)
        value["footnotes"] = sorted(self.footnotes)
        value["containers"] = {
            key: sorted(found) for key, found in self.groups["containers"].items()
        }
        return json.dumps(value, indent=2, ensure_ascii=False)


def footnotes_differ(old: Side, new: Side, module: str) -> bool:
    """Whether the footnotes of `module`, footnote markers left out, or its footnote
    markers differ between `old` and `new`."""
    before, after = old.footnotes.get(module, []), new.footnotes.get(module, [])
    if before != after and unmarked([json.loads(line) for line in before]) != unmarked(
        [json.loads(line) for line in after]
    ):
        return True
    return old.lines_of(module) != new.lines_of(module) and old.marks(module) != new.marks(module)


def compare(old: Side, new: Side) -> Sets:
    operative = three_sets(old.provisions, new.provisions, comparable_provision)
    operative["old_only"] = highest(operative["old_only"], old.parent)
    operative["new_only"] = highest(operative["new_only"], new.parent)
    groups = {"operative": operative}
    for layer in ("enacted", "editorial"):
        old_notes = {a: line for a, line in old.notes.items() if old.note_layers[a] == layer}
        new_notes = {a: line for a, line in new.notes.items() if new.note_layers[a] == layer}
        groups[f"{layer}_notes"] = three_sets(old_notes, new_notes, unmarked)
    groups["containers"] = three_sets(old.containers, new.containers, comparable_container)
    both = (old.units & new.units) | (old.containers.keys() & new.containers.keys())
    credited = {m for m in both if not same(old.credits.get(m), new.credits.get(m), unmarked)}
    footnotes = {m for m in both if footnotes_differ(old, new, m)}
    return Sets(groups, credited, footnotes)


def listed(change_set: Record, group: str, key: str) -> set[str]:
    """The addresses that the list `key` of the group `group` of `change_set` holds;
    of `moved`, `from` and `to` name the end."""
    if key in ("from", "to"):
        return {str(entry[key]) for entry in change_set[group]["moved"]}
    return {str(address) for address in change_set[group][key]}


def disagreement(sets: Sets, change_set: Record) -> str | None:
    """The first address that differs between `sets` and what `change_set` lists,
    named in a sentence; `None` when every set equals what it lists."""
    checks: list[tuple[str, set[str], set[str]]] = []
    for name, group, amended in (
        ("operative", "operative", ("amended_text", "amended_structure")),
        ("enacted_notes", "enacted_notes", ("amended",)),
        ("editorial_notes", "editorial", ("amended",)),
    ):
        found = sets.groups[name]
        checks += [
            (
                f"{name} only in the old",
                found["old_only"],
                listed(change_set, group, "removed") | listed(change_set, group, "from"),
            ),
            (
                f"{name} only in the new",
                found["new_only"],
                listed(change_set, group, "added") | listed(change_set, group, "to"),
            ),
            (
                f"{name} that differ",
                found["differ"],
                set[str]().union(*(listed(change_set, group, key) for key in amended)),
            ),
        ]
    checks.append(("credits", sets.credits, listed(change_set, "editorial", "credits_changed")))
    checks.append(
        ("footnotes", sets.footnotes, listed(change_set, "editorial", "footnotes_changed"))
    )
    for name, key in (("old_only", "removed"), ("new_only", "added"), ("differ", "changed")):
        checks.append(
            (
                f"containers {name}",
                sets.groups["containers"][name],
                listed(change_set, "containers", key),
            )
        )
    for what, found_set, listed_set in checks:
        if found_set == listed_set:
            continue
        first = min(found_set ^ listed_set)
        if first in found_set:
            return f"{what}: the text records hold `{first}`, and the change set does not"
        return f"{what}: the change set lists `{first}`, and the text records do not"
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    parser.add_argument("old", type=Path, help="the older release: archives, an archive or XML")
    parser.add_argument("new", type=Path, help="the newer release: archives, an archive or XML")
    parser.add_argument(
        "--against", type=Path, help="a change set that `fidryn changes --json` printed"
    )
    parser.add_argument("--jobs", type=int, default=4, help="worker processes (default 4)")
    args = parser.parse_args()
    try:
        change_set: Record | None = None
        if args.against is not None:
            change_set = json.loads(args.against.read_text(encoding="utf-8"))
        sets = compare(read_side(args.old, args.jobs), read_side(args.new, args.jobs))
        if change_set is None:
            print(sets.to_json())
            return 0
        found = disagreement(sets, change_set)
    except RefusedError as error:
        print(f"refused: {error.describe()}", file=sys.stderr)
        return 1
    except (InputError, OSError, KeyError, TypeError, ValueError) as error:
        print(f"cannot read: {error}", file=sys.stderr)
        return 2
    if found is not None:
        print(found, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
