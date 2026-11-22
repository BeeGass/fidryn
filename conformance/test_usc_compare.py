"""Hold the independent comparison of two releases to its fixture pairs and to the command.

Each pair in `fixtures/uslm/pairs/<name>/` is two invented documents of the invented
title 99: `old.xml` of the release 999-1 and `new.xml` of the release 999-2, each read
as the member `usc99.xml`. The sets each pair gives are written below by hand from
the rules of the comparison, not by running the script. The test of agreement
imports both sides with the built `fidryn`, which `FIDRYN_BINARY` names, and holds the
change set it prints to the comparison; without the binary it fails.
"""

from __future__ import annotations

import copy
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from typing import Any

import blake3
import check_json_contracts

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
PAIRS = HERE / "fixtures" / "uslm" / "pairs"
SCRIPT = HERE / "usc_compare.py"
MEMBER = "usc99.xml"
# The two documents of a pair, each with the release it is of.
SIDES = (("old.xml", "999-1"), ("new.xml", "999-2"))

S1 = "/us/usc/t99/s1"
CHAPTER = "/us/usc/t99/ch1"
NOTES = f"{S1}/note"
# The first of the like-named notes of the pairs `inserted-note` and
# `amended-and-inserted-note`.
EFFECTIVE = f"{NOTES}/statutoryNotes/nolaw/effective-date"

# Each pair, and what differs between its two documents: each set by its group and
# its name, `operative.differ`, or `credits`; a set not named is empty.
EXPECTED: dict[str, dict[str, list[str]]] = {
    # A footnote marker and its footnote added in the content of a section.
    "added-marker": {"footnotes": [S1]},
    # A link added to the source credit of a section.
    "changed-credit": {"credits": [S1]},
    # A word of the heading of a chapter.
    "container-heading": {"containers.differ": [CHAPTER]},
    # A word of a subsection.
    "amended-word": {"operative.differ": [f"{S1}/a"]},
    # A subsection with a chapeau and two paragraphs added to a section.
    "added-subsection": {
        "operative.new_only": [f"{S1}/b"],
        "operative.differ": [S1],
    },
    # A section with a subsection and a statutory note renumbered from 2 to 3.
    "renumbered-section": {
        "operative.old_only": ["/us/usc/t99/s2"],
        "operative.new_only": ["/us/usc/t99/s3"],
        "enacted_notes.old_only": ["/us/usc/t99/s2/note/statutoryNotes/pl.999.7/effective-date"],
        "enacted_notes.new_only": ["/us/usc/t99/s3/note/statutoryNotes/pl.999.7/effective-date"],
        "containers.differ": [CHAPTER],
    },
    # A section of a chapter removed.
    "removed-section": {
        "operative.old_only": ["/us/usc/t99/s2"],
        "containers.differ": [CHAPTER],
    },
    # A statutory note amended and one removed, and an editorial note added.
    "notes": {
        "enacted_notes.old_only": [f"{NOTES}/statutoryNotes/nolaw/short-title"],
        "enacted_notes.differ": [f"{NOTES}/statutoryNotes/pl.999.7/effective-date"],
        "editorial_notes.new_only": [f"{NOTES}/editorialNotes/nolaw/amendments"],
    },
    # A note inserted before two like-named notes, which are paired with their
    # unchanged selves at the next ordinal: each is old only at its old address and new
    # only at its new one, and the inserted note is new only at the first.
    "inserted-note": {
        "enacted_notes.old_only": [EFFECTIVE, f"{EFFECTIVE}/2"],
        "enacted_notes.new_only": [EFFECTIVE, f"{EFFECTIVE}/2", f"{EFFECTIVE}/3"],
    },
    # A note inserted before two like-named notes, the first of them amended: the
    # second is paired with its unchanged self, and the notes that remain are compared
    # by address.
    "amended-and-inserted-note": {
        "enacted_notes.old_only": [f"{EFFECTIVE}/2"],
        "enacted_notes.new_only": [f"{EFFECTIVE}/2", f"{EFFECTIVE}/3"],
        "enacted_notes.differ": [EFFECTIVE],
    },
}

GROUPS = ("operative", "enacted_notes", "editorial_notes")
SETS = ("old_only", "new_only", "differ")


def full(found: dict[str, list[str]]) -> dict[str, Any]:
    """The whole output of the script, every set empty but those of `found`."""
    value: dict[str, Any] = {group: {name: [] for name in SETS} for group in GROUPS}
    value["credits"] = []
    value["footnotes"] = []
    value["containers"] = {name: [] for name in SETS}
    for key, addresses in found.items():
        group, _, name = key.partition(".")
        if name:
            value[group][name] = addresses
        else:
            value[group] = addresses
    return value


def run(*args: str | Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(arg) for arg in args],
        capture_output=True,
        check=False,
        encoding="utf-8",
        timeout=600,
    )


def documents(name: str, directory: Path) -> tuple[Path, Path]:
    """The two documents of the pair `name`, each copied as the member `usc99.xml`
    into a directory of its own under `directory`."""
    copied = []
    for document, release in SIDES:
        target = directory / release / MEMBER
        target.parent.mkdir(parents=True)
        shutil.copyfile(PAIRS / name / document, target)
        copied.append(target)
    return copied[0], copied[1]


def change_set(**lists: list[Any]) -> dict[str, Any]:
    """A change set of the prefix of the Code in which nothing changed but the lists
    named, each by its group and its key joined by `__`."""
    value: dict[str, Any] = {
        "schema": "fidryn.change-set/v0.1",
        "prefix": "/us/usc",
        "old": {"release": "999-1", "lock_digest": "0" * 64},
        "new": {"release": "999-2", "lock_digest": "1" * 64},
        "operative": {
            key: [] for key in ("added", "removed", "amended_text", "amended_structure", "moved")
        },
        "enacted_notes": {key: [] for key in ("added", "removed", "amended", "moved")},
        "editorial": {
            key: []
            for key in (
                "added",
                "removed",
                "amended",
                "moved",
                "credits_changed",
                "footnotes_changed",
            )
        },
        "containers": {key: [] for key in ("added", "removed", "changed")},
        "stale_bindings": [],
    }
    for name, entries in lists.items():
        group, key = name.split("__")
        value[group][key] = entries
    return value


class Pairs(unittest.TestCase):
    def compared(self, name: str, *args: str | Path) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-compare-") as directory:
            old, new = documents(name, Path(directory))
            return run(sys.executable, SCRIPT, old, new, "--jobs", "1", *args)

    def test_each_pair_gives_its_expected_sets(self) -> None:
        for name, found in EXPECTED.items():
            with self.subTest(name):
                result = self.compared(name)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stderr, "")
                self.assertEqual(json.loads(result.stdout), full(found))
                self.assertTrue(result.stdout.endswith("}\n"))

    def test_every_pair_is_named(self) -> None:
        self.assertEqual({path.name for path in PAIRS.iterdir()}, set(EXPECTED))
        for name in EXPECTED:
            self.assertEqual(
                sorted(path.name for path in (PAIRS / name).iterdir()),
                ["new.xml", "old.xml"],
                name,
            )
            for document, release in SIDES:
                text = (PAIRS / name / document).read_text(encoding="utf-8")
                self.assertIn("Invented for tests", text.split("\n", 1)[0], name)
                self.assertIn(f"<docPublicationName>Online@{release}<", text, name)

    def test_against_names_the_first_address_that_differs(self) -> None:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-compare-against-") as directory:
            path = Path(directory) / "changes.json"
            word = f"{S1}/a"

            def against(name: str, value: dict[str, Any]) -> subprocess.CompletedProcess[str]:
                path.write_text(json.dumps(value), encoding="utf-8")
                return self.compared(name, "--against", path)

            agreed = against("amended-word", change_set(operative__amended_text=[word]))
            self.assertEqual((agreed.returncode, agreed.stdout, agreed.stderr), (0, "", ""))
            # Either list of amended provisions will do.
            agreed = against("amended-word", change_set(operative__amended_structure=[word]))
            self.assertEqual(agreed.returncode, 0, agreed.stderr)
            # A provision removed and one added are a moved provision.
            moved = change_set(
                operative__moved=[{"from": "/us/usc/t99/s2", "to": "/us/usc/t99/s3"}],
                enacted_notes__removed=EXPECTED["renumbered-section"]["enacted_notes.old_only"],
                enacted_notes__added=EXPECTED["renumbered-section"]["enacted_notes.new_only"],
                containers__changed=[CHAPTER],
            )
            agreed = against("renumbered-section", moved)
            self.assertEqual(agreed.returncode, 0, agreed.stderr)

            missing = against("amended-word", change_set())
            self.assertEqual(missing.returncode, 1)
            self.assertEqual(missing.stdout, "")
            self.assertEqual(
                missing.stderr,
                f"operative that differ: the text records hold `{word}`, and the change set "
                "does not\n",
            )
            extra = against(
                "amended-word",
                change_set(operative__amended_text=[word, f"{S1}/b"]),
            )
            self.assertEqual(extra.returncode, 1)
            self.assertEqual(
                extra.stderr,
                f"operative that differ: the change set lists `{S1}/b`, and the text records "
                "do not\n",
            )
            # The first set that differs is named, in the order of the change set.
            unpaired = copy.deepcopy(moved)
            unpaired["operative"]["moved"] = []
            refused = against("renumbered-section", unpaired)
            self.assertEqual(refused.returncode, 1)
            self.assertEqual(
                refused.stderr,
                "operative only in the old: the text records hold `/us/usc/t99/s2`, and the "
                "change set does not\n",
            )

    def test_against_holds_like_named_notes_to_their_pairing(self) -> None:
        second, third = f"{EFFECTIVE}/2", f"{EFFECTIVE}/3"
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-compare-like-named-") as directory:
            path = Path(directory) / "changes.json"

            def against(name: str, value: dict[str, Any]) -> subprocess.CompletedProcess[str]:
                path.write_text(json.dumps(value), encoding="utf-8")
                return self.compared(name, "--against", path)

            paired = change_set(
                enacted_notes__added=[EFFECTIVE],
                enacted_notes__moved=[
                    {"from": EFFECTIVE, "to": second},
                    {"from": second, "to": third},
                ],
            )
            agreed = against("inserted-note", paired)
            self.assertEqual((agreed.returncode, agreed.stderr), (0, ""))
            # Compared by address alone, the two unchanged notes would be amended and
            # the last one added: that is not what the pairing gives.
            by_address = change_set(
                enacted_notes__added=[third], enacted_notes__amended=[EFFECTIVE, second]
            )
            refused = against("inserted-note", by_address)
            self.assertEqual(refused.returncode, 1)
            self.assertEqual(
                refused.stderr,
                f"enacted_notes only in the old: the text records hold `{EFFECTIVE}`, and the "
                "change set does not\n",
            )
            amended = change_set(
                enacted_notes__amended=[EFFECTIVE],
                enacted_notes__added=[second],
                enacted_notes__moved=[{"from": second, "to": third}],
            )
            agreed = against("amended-and-inserted-note", amended)
            self.assertEqual((agreed.returncode, agreed.stderr), (0, ""))


def release(directory: Path, version: str, document: Path) -> Path:
    """A directory of the archive of the release `version` that holds `document` as
    the member `usc99.xml`, with its retrieval record; its path."""
    archives = directory / version / "archives"
    archives.mkdir(parents=True)
    archive = archives / f"xml_usc99@{version}.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zipped:
        zipped.writestr(MEMBER, document.read_bytes())
    data = archive.read_bytes()
    record = {
        "schema": "fidryn.retrieval/v0.1",
        "release": version,
        "archives": [
            {
                "file": archive.name,
                "url": f"https://example.invalid/releases/{version}/{archive.name}",
                "retrieved_at": "2099-03-04T05:06:07Z",
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
                "blake3": blake3.blake3(data).hexdigest(),
            }
        ],
    }
    (archives / "retrieval.json").write_text(json.dumps(record, indent=2) + "\n", "utf-8")
    return archives


class Agreement(unittest.TestCase):
    def test_the_command_and_the_independent_comparison_agree(self) -> None:
        binary = os.environ.get("FIDRYN_BINARY")
        if not binary or not Path(binary).is_file():
            self.fail("FIDRYN_BINARY names no built fidryn; cargo xtask ci sets it")
        validators, jsonschema_ok = check_json_contracts.load_validators(ROOT)
        self.assertTrue(jsonschema_ok, "uv provides jsonschema")
        for name in EXPECTED:
            with (
                self.subTest(name),
                tempfile.TemporaryDirectory(prefix="fidryn-usc-compare-agree-") as directory,
            ):
                sides = []
                for document, version in SIDES:
                    archives = release(Path(directory), version, PAIRS / name / document)
                    out = Path(directory) / version / "workspace"
                    ingested = run(binary, "ingest", "uscode", archives, "--out", out)
                    self.assertEqual(ingested.returncode, 0, ingested.stderr)
                    sides.append((archives, out))
                (old_archives, old), (new_archives, new) = sides
                printed = run(binary, "changes", old, new, "--json", "--no-cache")
                self.assertEqual(printed.returncode, 0, printed.stderr)
                value = json.loads(printed.stdout)
                errors = [e.message for e in validators[value["schema"]].iter_errors(value)]
                self.assertEqual(errors, [])
                changes = Path(directory) / "changes.json"
                changes.write_text(printed.stdout, encoding="utf-8")
                compared = run(
                    sys.executable,
                    SCRIPT,
                    old_archives,
                    new_archives,
                    "--jobs",
                    "1",
                    "--against",
                    changes,
                )
                self.assertEqual(compared.returncode, 0, compared.stderr)
                self.assertEqual(compared.stderr, "")


if __name__ == "__main__":
    unittest.main()
