"""The check report of the invented outcome workspace matches its schema."""

from __future__ import annotations

import copy
import json
import unittest
from collections.abc import Callable
from pathlib import Path
from typing import Any

import check_json_contracts

ROOT = Path(__file__).resolve().parent.parent
# The check report and the unresolved list of an invented workspace that has
# a reference of every outcome. The Rust test
# `the_committed_check_report_is_what_the_check_writes` holds both files to
# what the check writes.
FIXTURE = Path(__file__).resolve().parent / "fixtures" / "check-report"
SCHEMA = "fidryn.check-report/v0.1"
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
KEYS = (
    "schema",
    "modules",
    "from_cache",
    "errors",
    "warnings",
    "references",
    "citations",
    "outside_by_body",
)
# The keys of a line of the unresolved list, in their order: of a reference,
# and of a citation.
REFERENCE_LINE = ["from", "layer", "target", "outcome"]
CITATION_LINE = ["from", "citation", "target", "outcome"]

Change = Callable[[dict[str, Any]], object]


def load_report() -> dict[str, Any]:
    value = json.loads((FIXTURE / "report.json").read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError("report.json holds no object")
    return value


def changed(document: dict[str, Any], *changes: Change) -> dict[str, Any]:
    """A copy of `document` with each of `changes` made to it."""
    copied = copy.deepcopy(document)
    for change in changes:
        change(copied)
    return copied


class CheckReportSchema(unittest.TestCase):
    def setUp(self) -> None:
        validators, jsonschema_ok = check_json_contracts.load_validators(ROOT)
        self.assertTrue(jsonschema_ok, "uv provides jsonschema")
        self.validators = validators

    def errors(self, document: dict[str, Any]) -> list[str]:
        """What the schema of the check report finds in `document`."""
        validator = self.validators[SCHEMA]
        return [error.message for error in validator.iter_errors(document)]

    def test_the_report_matches_its_schema(self) -> None:
        report = load_report()
        self.assertEqual(report["schema"], SCHEMA)
        self.assertEqual(self.errors(report), [])
        self.assertEqual(list(report), list(KEYS))
        self.assertEqual(list(report["references"]), list(LAYERS))
        for layer in LAYERS:
            self.assertEqual(list(report["references"][layer]), list(OUTCOMES), layer)
        self.assertEqual(list(report["citations"]), list(OUTCOMES))
        refused = {
            "an unknown key": changed(report, lambda d: d.update(extra=1)),
            "a missing count": changed(report, lambda d: d.pop("warnings")),
            "a negative count": changed(report, lambda d: d.update(errors=-1)),
            "a count that is no integer": changed(report, lambda d: d.update(modules="8")),
            "a layer that is none": changed(
                report, lambda d: d["references"].update(quoted=d["references"]["operative"])
            ),
            "a missing layer": changed(report, lambda d: d["references"].pop("enacted")),
            "an outcome that is none": changed(
                report, lambda d: d["references"]["operative"].update(unknown=0)
            ),
            "a missing outcome": changed(
                report, lambda d: d["references"]["editorial"].pop("range-form")
            ),
            "no citations": changed(report, lambda d: d.pop("citations")),
            "citations by layer": changed(report, lambda d: d.update(citations=d["references"])),
            "a missing outcome of citations": changed(
                report, lambda d: d["citations"].pop("no-unit")
            ),
            "a negative count of citations": changed(
                report, lambda d: d["citations"].update(exact=-1)
            ),
            "a body that is no address": changed(
                report, lambda d: d["outside_by_body"].update({"other": 1})
            ),
            "a body of three segments": changed(
                report, lambda d: d["outside_by_body"].update({"/zz/other/x": 1})
            ),
            "a body without a reference": changed(
                report, lambda d: d["outside_by_body"].update({"/zz/none": 0})
            ),
            "the schema of another file": changed(
                report, lambda d: d.update(schema="fidryn.workspace/v0.1")
            ),
        }
        for what, document in refused.items():
            self.assertNotEqual(self.errors(document), [], what)

    def test_the_unresolved_list_is_one_object_to_a_line(self) -> None:
        report = load_report()
        text = (FIXTURE / "unresolved.jsonl").read_text(encoding="utf-8")
        self.assertTrue(text.endswith("\n"))
        lines = text.splitlines()
        rows = [json.loads(line) for line in lines]
        for line, row in zip(lines, rows, strict=True):
            self.assertIn(list(row), [REFERENCE_LINE, CITATION_LINE], line)
            self.assertEqual(json.dumps(row, ensure_ascii=False, separators=(",", ":")), line)
            if "layer" in row:
                self.assertIn(row["layer"], LAYERS, line)
            self.assertIn(row["outcome"], OUTCOMES[1:], line)
        # By `from`, a reference before a citation of the same `from`, then
        # by `target`.
        keys = [(row["from"].encode(), "citation" in row, row["target"].encode()) for row in rows]
        self.assertEqual(keys, sorted(keys))
        not_exact = sum(
            count
            for layer in LAYERS
            for outcome, count in report["references"][layer].items()
            if outcome != "exact"
        ) + sum(count for outcome, count in report["citations"].items() if outcome != "exact")
        self.assertEqual(len(rows), not_exact)


if __name__ == "__main__":
    unittest.main()
