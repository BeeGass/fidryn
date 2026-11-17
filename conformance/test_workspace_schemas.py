"""The three files of a workspace, as the writer wrote them, match their schemas."""

from __future__ import annotations

import copy
import json
import unittest
from collections.abc import Callable
from pathlib import Path
from typing import Any

import check_json_contracts

ROOT = Path(__file__).resolve().parent.parent
# The invented demo workspace and the retrieval record of its archive. The
# tests of `fidryn-driver` hold every file here to what the writer writes.
FIXTURE = Path(__file__).resolve().parent / "fixtures" / "workspace"
FILES = {
    "demo/workspace.json": "fidryn.workspace/v0.1",
    "demo/law.lock.json": "fidryn.law-lock/v0.1",
    "archives/retrieval.json": "fidryn.retrieval/v0.1",
}

Change = Callable[[dict[str, Any]], object]


def load(rel: str) -> dict[str, Any]:
    value = json.loads((FIXTURE / rel).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"{rel} holds no object")
    return value


def changed(document: dict[str, Any], *changes: Change) -> dict[str, Any]:
    """A copy of `document` with each of `changes` made to it."""
    copied = copy.deepcopy(document)
    for change in changes:
        change(copied)
    return copied


def rename_module(lock: dict[str, Any]) -> object:
    modules: dict[str, str] = lock["modules"]
    digest = modules.pop("zz/demo/t1.fr")
    modules["zz/demo/T1.fr"] = digest
    return None


class WorkspaceSchemas(unittest.TestCase):
    def setUp(self) -> None:
        validators, jsonschema_ok = check_json_contracts.load_validators(ROOT)
        self.assertTrue(jsonschema_ok, "uv provides jsonschema")
        self.validators = validators

    def errors(self, document: dict[str, Any]) -> list[str]:
        validator = self.validators[document["schema"]]
        return [error.message for error in validator.iter_errors(document)]

    def test_the_three_files_match_their_schemas(self) -> None:
        for rel, schema in FILES.items():
            document = load(rel)
            self.assertEqual(document["schema"], schema, rel)
            self.assertEqual(self.errors(document), [], rel)

    def test_the_schemas_refuse_what_a_workspace_check_refuses(self) -> None:
        manifest = load("demo/workspace.json")
        lock = load("demo/law.lock.json")
        record = load("archives/retrieval.json")
        refused = {
            "an unknown field": changed(manifest, lambda d: d.update(extra=1)),
            "an unknown field of an archive member": changed(
                lock, lambda d: d["artifacts"][0]["members"][0].update(extra=1)
            ),
            "a missing field": changed(lock, lambda d: d.pop("not_imported")),
            "an object written as an array": changed(
                lock, lambda d: d.update(generator=list(d["generator"].values()))
            ),
            "a digest in upper case": changed(
                record,
                lambda d: d["archives"][0].update(blake3=d["archives"][0]["blake3"].upper()),
            ),
            "a negative count": changed(lock, lambda d: d["addresses"].update(aliases=-1)),
            "a path that no address has": changed(lock, rename_module),
            "a prefix that is no address": changed(manifest, lambda d: d.update(prefix="zz/demo")),
            "a kind of no workspace": changed(manifest, lambda d: d.update(kind="library")),
            "a generated workspace without a release": changed(
                manifest, lambda d: d.pop("release")
            ),
            "a curated workspace with a release": changed(
                manifest, lambda d: d.update(kind="curated")
            ),
            "a retrieval record that names the schema of a lock": changed(
                record, lambda d: d.update(schema="fidryn.law-lock/v0.1")
            ),
        }
        for what, document in refused.items():
            self.assertNotEqual(self.errors(document), [], what)
        curated = changed(manifest, lambda d: d.update(kind="curated"), lambda d: d.pop("release"))
        self.assertEqual(self.errors(curated), [])


if __name__ == "__main__":
    unittest.main()
