"""The change set of an invented pair of workspaces matches its schema."""

from __future__ import annotations

import copy
import json
import unittest
from collections.abc import Callable
from pathlib import Path
from typing import Any

import check_json_contracts

ROOT = Path(__file__).resolve().parent.parent
# The change set between two invented generated workspaces in the corpus `/zz/demo`
# in which every list holds more than one entry. The Rust test
# `the_committed_change_set_is_what_the_comparison_writes` holds it to what the
# comparison writes.
FIXTURE = Path(__file__).resolve().parent / "fixtures" / "change-set" / "change-set.json"
SCHEMA = "fidryn.change-set/v0.1"
KEYS = (
    "schema",
    "prefix",
    "old",
    "new",
    "operative",
    "enacted_notes",
    "editorial",
    "containers",
    "stale_bindings",
)
LISTS = {
    "operative": ("added", "removed", "amended_text", "amended_structure", "moved"),
    "enacted_notes": ("added", "removed", "amended", "moved"),
    "editorial": ("added", "removed", "amended", "moved", "credits_changed", "footnotes_changed"),
    "containers": ("added", "removed", "changed"),
}

Change = Callable[[dict[str, Any]], object]


def load_change_set() -> dict[str, Any]:
    value = json.loads(FIXTURE.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError("change-set.json holds no object")
    return value


def changed(document: dict[str, Any], change: Change) -> dict[str, Any]:
    """A copy of `document` with `change` made to it."""
    copied = copy.deepcopy(document)
    change(copied)
    return copied


class ChangeSetSchema(unittest.TestCase):
    def setUp(self) -> None:
        validators, jsonschema_ok = check_json_contracts.load_validators(ROOT)
        self.assertTrue(jsonschema_ok, "uv provides jsonschema")
        self.validators = validators

    def errors(self, document: dict[str, Any]) -> list[str]:
        """What the schema of the change set finds in `document`."""
        return [error.message for error in self.validators[SCHEMA].iter_errors(document)]

    def test_the_change_set_matches_its_schema(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        change_set = load_change_set()
        self.assertEqual(self.errors(change_set), [])
        # Pretty JSON of two spaces, with a final line end, every key in its order.
        self.assertEqual(text, json.dumps(change_set, indent=2, ensure_ascii=False) + "\n")
        self.assertEqual(list(change_set), list(KEYS))
        for group, keys in LISTS.items():
            self.assertEqual(list(change_set[group]), list(keys), group)
            for key in keys:
                entries = change_set[group][key]
                self.assertGreaterEqual(len(entries), 2, f"{group}.{key}")
                order = (
                    [(entry["from"], entry["to"]) for entry in entries]
                    if key == "moved"
                    else entries
                )
                self.assertEqual(order, sorted(order), f"{group}.{key}")
        stale = change_set["stale_bindings"]
        self.assertGreaterEqual(len(stale), 2)
        names = [(entry["workspace"], entry["module"], entry["binding"]) for entry in stale]
        self.assertEqual(names, sorted(names))

    def test_the_schema_refuses_what_the_comparison_never_writes(self) -> None:
        change_set = load_change_set()
        refused = {
            "an unknown key": changed(change_set, lambda d: d.update(extra=[])),
            "a missing group": changed(change_set, lambda d: d.pop("containers")),
            "a missing list": changed(change_set, lambda d: d["operative"].pop("amended_text")),
            "a list of another group": changed(
                change_set, lambda d: d["enacted_notes"].update(credits_changed=[])
            ),
            "an address that is none": changed(
                change_set, lambda d: d["operative"]["added"].append("zz/demo/t1/s99")
            ),
            "an entry twice": changed(
                change_set,
                lambda d: d["containers"]["changed"].append(d["containers"]["changed"][0]),
            ),
            "a moved entry without its end": changed(
                change_set, lambda d: d["operative"]["moved"][0].pop("to")
            ),
            "a digest of capitals": changed(
                change_set, lambda d: d["old"].update(lock_digest="A" * 64)
            ),
            "a release without a digest": changed(
                change_set, lambda d: d["new"].pop("lock_digest")
            ),
            "a reason no binding is stale for": changed(
                change_set, lambda d: d["stale_bindings"][0].update(reason="provision renamed")
            ),
            "a stale binding without a workspace": changed(
                change_set, lambda d: d["stale_bindings"][0].pop("workspace")
            ),
            "the schema of another file": changed(
                change_set, lambda d: d.update(schema="fidryn.check-report/v0.1")
            ),
        }
        for what, document in refused.items():
            self.assertNotEqual(self.errors(document), [], what)


if __name__ == "__main__":
    unittest.main()
