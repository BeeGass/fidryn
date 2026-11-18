"""The coverage ledgers of the sample and of the invented demo workspace match their schema."""

from __future__ import annotations

import copy
import json
import unittest
from collections.abc import Callable
from pathlib import Path
from typing import Any

import check_json_contracts

ROOT = Path(__file__).resolve().parent.parent
# The ledger of the workspace of the sample of the Code, and that of the invented
# demo workspace. The Rust tests hold each to what the writer writes and to what
# `fidryn coverage` prints.
LEDGERS = (
    ROOT / "samples" / "uscode" / "workspace" / "coverage.json",
    ROOT / "conformance" / "fixtures" / "workspace" / "demo" / "coverage.json",
)
SCHEMA = "fidryn.coverage/v0.1"
KEYS = ("schema", "prefix", "release", "totals", "by_source")
TOTALS = (
    "units",
    "containers",
    "provisions",
    "units_by_kind",
    "units_by_status",
    "containers_by_kind",
    "addresses",
    "notes",
    "notes_by_group",
    "footnotes",
    "with_images",
    "references",
    "outside_by_body",
    "definitions",
    "definitions_by_kind",
    "not_imported",
)
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
# The maps whose keys are words of the workspace, written sorted by their bytes.
OPEN_MAPS = (
    "units_by_kind",
    "units_by_status",
    "containers_by_kind",
    "notes_by_group",
    "outside_by_body",
    "definitions_by_kind",
)

Change = Callable[[dict[str, Any]], object]


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"{path} holds no object")
    return value


def changed(document: dict[str, Any], *changes: Change) -> dict[str, Any]:
    """A copy of `document` with each of `changes` made to it."""
    copied = copy.deepcopy(document)
    for change in changes:
        change(copied)
    return copied


class CoverageSchema(unittest.TestCase):
    def setUp(self) -> None:
        validators, jsonschema_ok = check_json_contracts.load_validators(ROOT)
        self.assertTrue(jsonschema_ok, "uv provides jsonschema")
        self.validators = validators

    def errors(self, document: dict[str, Any]) -> list[str]:
        """What the schema of the ledger finds in `document`."""
        validator = self.validators[SCHEMA]
        return [error.message for error in validator.iter_errors(document)]

    def test_each_ledger_matches_its_schema_and_its_layout(self) -> None:
        for path in LEDGERS:
            text = path.read_text(encoding="utf-8")
            ledger = load(path)
            self.assertEqual(ledger["schema"], SCHEMA, path)
            self.assertEqual(self.errors(ledger), [], path)
            self.assertEqual(json.dumps(ledger, indent=2, ensure_ascii=False) + "\n", text, path)
            self.assertEqual(list(ledger), list(KEYS), path)
            totals = ledger["totals"]
            self.assertEqual(list(totals), list(TOTALS), path)
            self.assertEqual(list(totals["references"]), list(LAYERS), path)
            for layer in LAYERS:
                self.assertEqual(list(totals["references"][layer]), list(OUTCOMES), path)
            for key in OPEN_MAPS:
                words = [word.encode() for word in totals[key]]
                self.assertEqual(words, sorted(words), f"{path}: {key}")
            sources = [row["source"].encode() for row in ledger["by_source"]]
            self.assertEqual(sources, sorted(sources), path)
            self.assertEqual(len(sources), len(set(sources)), path)
            self.assertEqual(sum(row["units"] for row in ledger["by_source"]), totals["units"])
            self.assertEqual(
                sum(row["provisions"] for row in ledger["by_source"]), totals["provisions"]
            )
            self.assertEqual(
                sum(row["definitions"] for row in ledger["by_source"]),
                sum(totals["definitions"].values()),
            )
            self.assertEqual(
                sum(totals["definitions_by_kind"].values()),
                sum(totals["definitions"].values()),
            )
            self.assertEqual(sum(totals["notes_by_group"].values()), sum(totals["notes"].values()))

    def test_the_schema_refuses_what_is_no_ledger(self) -> None:
        ledger = load(LEDGERS[0])
        refused = {
            "an unknown key": changed(ledger, lambda d: d.update(extra=1)),
            "an unknown count": changed(ledger, lambda d: d["totals"].update(extra=1)),
            "a missing count": changed(ledger, lambda d: d["totals"].pop("footnotes")),
            "a negative count": changed(ledger, lambda d: d["totals"].update(units=-1)),
            "a count that is no integer": changed(
                ledger, lambda d: d["totals"].update(provisions="4857")
            ),
            "a word counted zero times": changed(
                ledger, lambda d: d["totals"]["units_by_kind"].update(rule=0)
            ),
            "a missing layer": changed(ledger, lambda d: d["totals"]["references"].pop("enacted")),
            "a missing outcome": changed(
                ledger, lambda d: d["totals"]["references"]["operative"].pop("malformed")
            ),
            "a basis that is none": changed(
                ledger, lambda d: d["totals"]["definitions"].update(inferred=0)
            ),
            "a kind of definition that is none": changed(
                ledger, lambda d: d["totals"]["definitions_by_kind"].update(defines=1)
            ),
            "a body of three segments": changed(
                ledger, lambda d: d["totals"]["outside_by_body"].update({"/zz/other/x": 1})
            ),
            "a count of what is not imported that is none": changed(
                ledger, lambda d: d["totals"]["not_imported"].update(images=1)
            ),
            "a flag that is no boolean": changed(
                ledger, lambda d: d["by_source"][0].update(positive_law="yes")
            ),
            "a row without its definitions": changed(
                ledger, lambda d: d["by_source"][0].pop("definitions")
            ),
            "the schema of another file": changed(
                ledger, lambda d: d.update(schema="fidryn.law-lock/v0.1")
            ),
        }
        for what, document in refused.items():
            self.assertNotEqual(self.errors(document), [], what)
        # A file without a flag is a row like any other.
        self.assertEqual(
            self.errors(changed(ledger, lambda d: d["by_source"][0].update(positive_law=None))),
            [],
        )


if __name__ == "__main__":
    unittest.main()
