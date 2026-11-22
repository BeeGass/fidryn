"""Hold the replay of the releases to the publisher's order, its history and its removal.

Nothing here opens a connection or runs `fidryn`: the list of releases is compared with
the publisher's, the history with the subjects of commits written below, and the
replacement of `uscode/` with directories of invented manifests made in a scratch
directory, whose every file says that it is invented.
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import replay_uscode

# The release points the publisher lists for 2026, oldest first, in its order, which
# the names alone do not give (F9).
PUBLISHED = [
    "119-73not60",
    "119-73",
    "119-83",
    "119-84",
    "119-87",
    "119-88",
    "119-90",
    "119-93",
    "119-94",
    "119-95",
    "119-99",
    "119-100",
    "119-102not101",
    "119-102",
    "119-103",
    "119-108",
    "119-110",
    "119-111",
    "119-117",
    "119-118",
    "119-120",
    "119-121",
    "119-124",
    "119-134",
    "119-136",
    "119-138",
    "119-142",
]


class Order(unittest.TestCase):
    def test_the_releases_are_those_of_the_publisher_in_its_order(self) -> None:
        self.assertEqual(len(replay_uscode.RELEASES), len(PUBLISHED))
        for index, (replayed, published) in enumerate(
            zip(replay_uscode.RELEASES, PUBLISHED, strict=True)
        ):
            with self.subTest(index=index):
                self.assertEqual(replayed, published)

    def test_each_release_has_the_subject_of_its_commit(self) -> None:
        self.assertEqual(replay_uscode.subject(0), "/us/usc: release 119-73not60")
        self.assertEqual(replay_uscode.subject(1), "/us/usc: 119-73not60 to 119-73")
        self.assertEqual(replay_uscode.subject(13), "/us/usc: 119-102not101 to 119-102")
        self.assertEqual(replay_uscode.subject(26), "/us/usc: 119-138 to 119-142")


class History(unittest.TestCase):
    def subjects(self, count: int) -> list[str]:
        return [replay_uscode.subject(index) for index in range(count)]

    def test_the_commits_made_are_counted_and_the_next_release_follows_them(self) -> None:
        for count in (0, 1, 2, 14, 26, 27):
            with self.subTest(count=count):
                self.assertEqual(replay_uscode.replayed(self.subjects(count)), count)

    def test_commits_after_the_last_release_are_left_to_follow_it(self) -> None:
        later = [*self.subjects(27), "encodings: three bindings written against 119-138"]
        self.assertEqual(replay_uscode.replayed(later), 27)

    def test_a_history_that_is_not_the_replay_stops_it(self) -> None:
        histories = {
            "the first import": ["/us/usc: release 119-142"],
            "a release skipped": [*self.subjects(3), replay_uscode.subject(4)],
            "a release twice": [*self.subjects(3), replay_uscode.subject(2)],
            "another commit": [*self.subjects(5), "encodings: three bindings"],
        }
        for name, subjects in histories.items():
            with self.subTest(name), self.assertRaises(replay_uscode.StopReplayError) as caught:
                replay_uscode.replayed(subjects)
            self.assertIn(subjects[-1], str(caught.exception))


def manifest(kind: str, prefix: str, release: str) -> str:
    """An invented manifest; the workspaces of these tests hold no module."""
    return json.dumps(
        {
            "schema": "fidryn.workspace/v0.1",
            "name": "invented",
            "kind": kind,
            "prefix": prefix,
            "release": release,
            "toolchain": "0.3.0",
            "dependencies": [],
        }
    )


def lock(prefix: str, release: str) -> str:
    return json.dumps({"schema": "fidryn.law-lock/v0.1", "prefix": prefix, "release": release})


class Replacement(unittest.TestCase):
    def setUp(self) -> None:
        self.scratch = tempfile.TemporaryDirectory()
        self.root = Path(self.scratch.name)
        self.repository = self.root / "law"
        (self.repository / ".git").mkdir(parents=True)
        self.workspace("uscode", "generated", "/us/usc", "119-73not60")
        self.workspace("uscode.next", "generated", "/us/usc", "119-73")
        (self.repository / "uscode.next" / "law.lock.json").write_text(
            lock("/us/usc", "119-73"), encoding="utf-8"
        )

    def tearDown(self) -> None:
        self.scratch.cleanup()

    def workspace(self, name: str, kind: str, prefix: str, release: str) -> Path:
        directory = self.repository / name
        (directory / "us" / "usc").mkdir(parents=True, exist_ok=True)
        (directory / "workspace.json").write_text(manifest(kind, prefix, release), encoding="utf-8")
        (directory / "us" / "usc" / "t99.fr").write_text(
            f"// Invented: no module of the release {release}.\n", encoding="utf-8"
        )
        return directory

    def replace(self) -> None:
        replay_uscode.replace_workspace(self.repository, "119-73")

    def refused(self, *words: str) -> None:
        with self.assertRaises(replay_uscode.StopReplayError) as caught:
            self.replace()
        for word in words:
            self.assertIn(word, str(caught.exception))
        self.assertTrue((self.repository / "uscode.next" / "workspace.json").is_file())

    def test_the_next_import_takes_the_place_of_the_workspace(self) -> None:
        self.replace()
        self.assertFalse((self.repository / "uscode.next").exists())
        text = (self.repository / "uscode" / "workspace.json").read_text(encoding="utf-8")
        self.assertEqual(json.loads(text)["release"], "119-73")
        self.assertEqual(
            sorted(entry.name for entry in (self.repository / "uscode").iterdir()),
            ["law.lock.json", "us", "workspace.json"],
        )

    def test_a_first_import_needs_no_workspace_to_replace(self) -> None:
        replay_uscode.remove_tree(self.repository / "uscode", "workspace.json")
        self.replace()
        self.assertTrue((self.repository / "uscode" / "law.lock.json").is_file())

    def test_no_link_is_followed_when_the_workspace_is_removed(self) -> None:
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "kept.txt").write_text("Invented, and kept.\n", encoding="utf-8")
        (self.repository / "uscode" / "us" / "link").symlink_to(outside)
        (self.repository / "uscode" / "us" / "file-link").symlink_to(outside / "kept.txt")
        self.replace()
        self.assertEqual(
            (outside / "kept.txt").read_text(encoding="utf-8"), "Invented, and kept.\n"
        )

    def test_a_repository_without_git_is_refused(self) -> None:
        (self.repository / ".git").rmdir()
        self.refused(".git")

    def test_a_workspace_that_is_a_link_is_refused(self) -> None:
        elsewhere = self.root / "elsewhere"
        (self.repository / "uscode").rename(elsewhere)
        (self.repository / "uscode").symlink_to(elsewhere)
        self.refused("symbolic link")
        self.assertTrue((elsewhere / "workspace.json").is_file())

    def test_a_workspace_that_is_not_a_generated_one_of_the_prefix_is_refused(self) -> None:
        cases = {
            "no manifest": None,
            "curated": manifest("curated", "/us/usc", "119-73not60"),
            "another prefix": manifest("generated", "/zz/demo", "119-73not60"),
            "a manifest that does not read": "{",
        }
        for name, text in cases.items():
            with self.subTest(name):
                path = self.repository / "uscode" / "workspace.json"
                if text is None:
                    path.unlink()
                else:
                    path.write_text(text, encoding="utf-8")
                self.refused("uscode")
                self.assertTrue((self.repository / "uscode").is_dir())
                path.write_text(manifest("generated", "/us/usc", "119-73not60"), encoding="utf-8")

    def test_a_next_import_that_is_not_complete_is_refused(self) -> None:
        nxt = self.repository / "uscode.next"
        (nxt / "law.lock.json").unlink()
        self.refused("uscode.next", "law.lock.json")
        (nxt / "law.lock.json").write_text(lock("/us/usc", "119-83"), encoding="utf-8")
        self.refused("uscode.next", "119-83")
        (nxt / "law.lock.json").write_text(lock("/us/usc", "119-73"), encoding="utf-8")
        (nxt / "workspace.json").write_text(
            manifest("generated", "/us/usc", "119-84"), encoding="utf-8"
        )
        self.refused("uscode.next", "119-84")
        self.assertTrue((self.repository / "uscode" / "workspace.json").is_file())

    def test_a_missing_next_import_is_refused(self) -> None:
        replay_uscode.remove_tree(self.repository / "uscode.next", "workspace.json")
        with self.assertRaises(replay_uscode.StopReplayError) as caught:
            self.replace()
        self.assertIn("uscode.next", str(caught.exception))
        self.assertTrue((self.repository / "uscode" / "workspace.json").is_file())

    def test_the_manifest_is_removed_last(self) -> None:
        removed: list[str] = []
        unlink, rmdir = os.unlink, os.rmdir

        def note_unlink(target: str | os.PathLike[str]) -> None:
            removed.append(Path(target).name)
            unlink(target)

        def note_rmdir(target: str | os.PathLike[str]) -> None:
            removed.append(Path(target).name + "/")
            rmdir(target)

        with (
            mock.patch.object(os, "unlink", new=note_unlink),
            mock.patch.object(os, "rmdir", new=note_rmdir),
        ):
            replay_uscode.remove_tree(self.repository / "uscode", "workspace.json")
        self.assertEqual(removed[-2:], ["workspace.json", "uscode/"])
        self.assertIn("t99.fr", removed)
        self.assertFalse((self.repository / "uscode").exists())


if __name__ == "__main__":
    unittest.main()
