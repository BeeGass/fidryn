"""The text records of the fixtures, and those `fidryn text --json` prints, match their schema."""

from __future__ import annotations

import copy
import json
import unittest
from collections.abc import Callable
from pathlib import Path
from typing import Any

import check_json_contracts

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = Path(__file__).resolve().parent / "fixtures"
SCHEMA = "fidryn.text-record/v0.1"
# What `fidryn text --json` prints of the invented demo workspace. The Rust test
# `text_prints_one_record_for_each_provision_and_note_in_address_order` holds the
# file to what the command prints.
DEMO_RECORDS = FIXTURES / "text" / "demo.records.jsonl"

Change = Callable[[dict[str, Any]], object]


def records(path: Path) -> list[dict[str, Any]]:
    values = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    for value in values:
        if not isinstance(value, dict):
            raise TypeError(f"{path.name} holds a line that is no object")
    return values


def changed(record: dict[str, Any], *changes: Change) -> dict[str, Any]:
    """A copy of `record` with each of `changes` made to it."""
    copied = copy.deepcopy(record)
    for change in changes:
        change(copied)
    return copied


def first(kind: Callable[[dict[str, Any]], bool]) -> dict[str, Any]:
    """The first record of the fixtures of the publisher's XML of the kind given."""
    for path in sorted((FIXTURES / "uslm").glob("*.records.jsonl")):
        for record in records(path):
            if kind(record):
                return record
    raise AssertionError("no record of the kind")


class TextRecordSchema(unittest.TestCase):
    def setUp(self) -> None:
        validators, jsonschema_ok = check_json_contracts.load_validators(ROOT)
        self.assertTrue(jsonschema_ok, "uv provides jsonschema")
        self.validator = validators[SCHEMA]

    def errors(self, record: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(record)]

    def test_every_record_of_the_fixtures_matches_the_schema(self) -> None:
        paths = sorted(FIXTURES.rglob("*.records.jsonl"))
        self.assertIn(DEMO_RECORDS, paths)
        self.assertGreater(len(paths), 30)
        for path in paths:
            for number, record in enumerate(records(path), start=1):
                self.assertEqual(self.errors(record), [], f"{path.name}:{number}")

    def test_the_schema_refuses_what_is_no_text_record(self) -> None:
        unit = records(DEMO_RECORDS)[2]
        self.assertEqual(self.errors(unit), [])
        provision = first(lambda r: "items" in r and "source" not in r)
        container = records(DEMO_RECORDS)[0]
        note = first(lambda r: "group" in r)
        footnote = first(lambda r: r.get("record") == "footnote")
        marked = first(lambda r: "marks" in json.dumps(r))
        for record in (provision, container, note, footnote, marked):
            self.assertEqual(self.errors(record), [], record["address"])
        refused = {
            "an unknown key": changed(unit, lambda r: r.update(extra=1)),
            "a missing key": changed(unit, lambda r: r.pop("positive_law")),
            "a provision with a key of a unit": changed(provision, lambda r: r.update(source="x")),
            "a layer of no note": changed(note, lambda r: r.update(layer="operative")),
            "a layer of no provision": changed(provision, lambda r: r.update(layer="enacted")),
            "an address that is no address": changed(unit, lambda r: r.update(address="zz/demo")),
            "a number of three members": changed(unit, lambda r: r["num"].append("x")),
            "a part with no block": changed(unit, lambda r: r["items"][0].update(blocks=[])),
            "a part of no role": changed(unit, lambda r: r["items"][0].update(part="body")),
            "a negative offset": changed(
                container, lambda r: r["heading"]["refs"].append([-1, 2, "/zz/demo"])
            ),
            "an empty list of markers": changed(container, lambda r: r["heading"].update(marks=[])),
            "a footnote of ordinal 0": changed(footnote, lambda r: r.update(ordinal=0)),
            "a credit of another record": changed(footnote, lambda r: r.update(record="credit")),
            "a member that is no address": changed(container, lambda r: r["members"].append("t1")),
        }
        for what, record in refused.items():
            self.assertNotEqual(self.errors(record), [], what)


if __name__ == "__main__":
    unittest.main()
