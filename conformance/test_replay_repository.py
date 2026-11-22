"""Hold the replay of releases to a repository made with `git init`, with a stand-in for
`fidryn`.

Each test makes a repository in a temporary directory and sets the configuration of that
repository only: a name and an address for its commits, no signing, and a directory of
hooks of its own. Every git command, the test's and the tool's, runs without the
variables by which git finds a repository or its configuration (`GIT_DIR`, as inside a
hook of git, among them), with an empty file of the test as the global configuration
and none of the system, so that neither the repository the tests are run from nor the
user's or the machine's configuration is read or written. It replays three invented releases, whose archives are in place with
their retrieval records, so that nothing is fetched; a call that would open a connection
fails the test. Each archive holds one invented document of the title 99, which the
independent reading reads into the two records `TEXT_RECORDS`. The stand-in for `fidryn`
is a shell script that reads no law: `ingest` writes a workspace of the invented title 99
for the release its archive directory is named for, with a ledger that counts no table of
contents, `check` prints a summary that finds nothing, `text` prints `TEXT_RECORDS`, and
`changes` prints a change set from the release of each of the two workspaces it is
given. A replay is stopped at a point by making something fail there, the stand-in, a
hook of the repository or one call of the tool, never by a signal, and is started again.
"""

from __future__ import annotations

import contextlib
import io
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import unittest
import urllib.request
import zipfile
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import fetch_uscode
import replay_uscode

RELEASES = ("42-5not3", "42-5", "42-7")
TEXT_FORM = "/us/usc: {old} to {new}\n\nInvented: the text form of a change set.\n"

# The text records that `conformance/uslm_text.py` prints for the document of each
# invented archive, and that the stand-in prints as `fidryn text`.
TEXT_RECORDS = (
    '{"address":"/us/usc/t99","layer":"operative","kind":"title","status":null,'
    '"num":["99","Title 99"],"heading":{"text":"Invented","refs":[],"sup":[],"sub":[]},'
    '"aliases":[],"within":null,"members":["/us/usc/t99/s1"]}',
    '{"address":"/us/usc/t99/s1","layer":"operative","kind":"section","status":null,'
    '"num":["1","1."],"heading":{"text":"Invented","refs":[],"sup":[],"sub":[]},'
    '"items":[{"part":"content","blocks":[{"p":"Invented text.","refs":[],"sup":[],"sub":[]}]}],'
    '"aliases":[],"within":"/us/usc/t99","source":"usc99.xml","positive_law":true}',
)

# `@SH@`, `@LOG@`, `@FAIL@`, `@FIRST@` and `@SECOND@` are filled in by `Replays.setUp`.
# A command fails when a file named for it and its release stands in the directory
# `@FAIL@`. When `@FAIL@/text-<release>` stands there, `text` prints another word in
# the second record; when `@FAIL@/toc-<release>` does, the ledger of the import counts
# one table of contents.
STAND_IN = """#!@SH@
# Invented: a stand-in for fidryn in the tests of the replay. It reads no law.
set -eu
printf '%s\\n' "$*" >> "@LOG@"
release_of() {
    sed -n 's/.*"release": "\\([^"]*\\)".*/\\1/p' "$1/workspace.json"
}
case $1 in
    ingest)
        release=${3##*/}
        [ ! -e "@FAIL@/ingest-$release" ] || exit 1
        mkdir -p "$5/us/usc"
        printf '// Invented: the module of title 99 at the release %s.\\n' "$release" \\
            > "$5/us/usc/t99.fr"
        printf '{"schema": "fidryn.workspace/v0.1", "name": "invented", "kind": "generated", "prefix": "/us/usc", "release": "%s", "toolchain": "0.3.0", "dependencies": []}\\n' \\
            "$release" > "$5/workspace.json"
        printf '{"schema": "fidryn.law-lock/v0.1", "prefix": "/us/usc", "release": "%s"}\\n' \\
            "$release" > "$5/law.lock.json"
        tables=0
        [ ! -e "@FAIL@/toc-$release" ] || tables=1
        printf '{"schema": "fidryn.coverage/v0.1", "totals": {"not_imported": {"tables_of_contents": %s}}}\\n' \\
            "$tables" > "$5/coverage.json"
        printf 'ingested: 1 unit, 0 containers from 1 archive (release %s)\\n' "$release"
        ;;
    check)
        release=$(release_of "$2")
        [ ! -e "@FAIL@/check-$release" ] || exit 1
        printf 'modules: 1; from cache: 0; errors: 0; warnings: 0\\nok\\n'
        ;;
    text)
        release=$(release_of "$2")
        second='@SECOND@'
        if [ -e "@FAIL@/text-$release" ]; then
            second=$(printf '%s' "$second" | sed 's/Invented text\\./Other text./')
        fi
        printf '%s\\n%s\\n' '@FIRST@' "$second"
        ;;
    changes)
        old=$(release_of "$2")
        new=$(release_of "$3")
        if [ "${8-}" = --json ]; then
            printf '{"prefix": "/us/usc", "old": {"release": "%s"}, "new": {"release": "%s"}}\\n' \\
                "$old" "$new"
        else
            printf '/us/usc: %s to %s\\n\\nInvented: the text form of a change set.\\n' \\
                "$old" "$new"
        fi
        ;;
    *)
        exit 2
        ;;
esac
"""

# The hook fails the commit of the release whose name stands in the file `@FAIL@/commit`.
PRE_COMMIT = """#!@SH@
# Invented: a hook of the tests of the replay.
release=$(sed -n 's/.*"release": "\\([^"]*\\)".*/\\1/p' uscode/workspace.json)
[ ! -e "@FAIL@/commit-$release" ]
"""


# The variables by which git finds a repository, its objects or its configuration; the
# whole `GIT_CONFIG` family is removed too. The tests of `xtask` remove the same ones
# (`xtask/src/test_git.rs`), and the two lists are kept equal.
GIT_LOCATIONS = (
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_SHALLOW_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
)


def isolated(environment: dict[str, str], config: Path) -> dict[str, str]:
    """`environment` without the variables by which git finds a repository or its
    configuration, and with `config`, an empty file, as the only global configuration
    and no configuration of the system, so that git reads and writes neither the
    caller's repository, nor the user's configuration, nor the machine's."""
    kept = {
        name: value
        for name, value in environment.items()
        if name not in GIT_LOCATIONS and not name.startswith("GIT_CONFIG")
    }
    return {**kept, "GIT_CONFIG_GLOBAL": str(config), "GIT_CONFIG_NOSYSTEM": "1"}


def invented_archive(release: str) -> bytes:
    """An archive of one invented document of the title 99, which names `release`."""
    document = (
        f"<!-- Invented for the tests of the replay: {release}. -->\n"
        '<uscDoc xmlns="http://xml.house.gov/schemas/uslm/1.0" '
        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
        'xsi:schemaLocation="http://xml.house.gov/schemas/uslm/1.0 USLM-1.0.15.xsd">\n'
        f"<meta><docPublicationName>Online@{release}</docPublicationName>"
        '<property role="is-positive-law">yes</property></meta>\n'
        '<main><title identifier="/us/usc/t99"><num value="99">Title 99</num>'
        "<heading>Invented</heading>\n"
        '<section identifier="/us/usc/t99/s1"><num value="1">1.</num><heading>Invented</heading>'
        "<content>Invented text.</content></section>\n"
        "</title></main>\n</uscDoc>\n"
    )
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("usc99.xml", document)
    return buffer.getvalue()


def text_form(index: int) -> str:
    if index == 0:
        return f"/us/usc: release {RELEASES[0]}\n"
    return TEXT_FORM.format(old=RELEASES[index - 1], new=RELEASES[index])


class Replays(unittest.TestCase):
    def setUp(self) -> None:
        sh, git = shutil.which("sh"), shutil.which("git")
        if sh is None or git is None:
            raise unittest.SkipTest("the replay runs git, and its stand-in is a shell script")
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        # Every git command of the test, and every one the tool runs, inherits this
        # environment.
        (self.root / "gitconfig").write_bytes(b"")
        self.enterContext(
            mock.patch.dict(
                os.environ, isolated(dict(os.environ), self.root / "gitconfig"), clear=True
            )
        )
        self.archives = self.root / "archives"
        self.cache = self.root / "cache"
        self.failing = self.root / "fail"
        self.failing.mkdir()
        self.log = self.root / "fidryn.log"
        self.hooks = self.root / "hooks"
        self.hooks.mkdir()
        self.stand_in = self.root / "fidryn"
        for path, text in ((self.stand_in, STAND_IN), (self.hooks / "pre-commit", PRE_COMMIT)):
            path.write_text(
                text.replace("@SH@", sh)
                .replace("@LOG@", str(self.log))
                .replace("@FAIL@", str(self.failing))
                .replace("@FIRST@", TEXT_RECORDS[0])
                .replace("@SECOND@", TEXT_RECORDS[1]),
                encoding="utf-8",
            )
            path.chmod(0o755)
        for release in RELEASES:
            self.write_archive(release)
        self.repository = self.root / "law"
        self.init(self.repository)

    def write_archive(self, release: str) -> None:
        directory = self.archives / release
        directory.mkdir(parents=True)
        file = fetch_uscode.archive_file(release, None)
        (directory / file).write_bytes(invented_archive(release))
        digests = fetch_uscode.digests_of(directory / file)
        record = fetch_uscode.Archive(
            file,
            fetch_uscode.archive_url(release, None),
            "2026-11-16T22:14:03Z",
            digests.bytes,
            digests.sha256,
            digests.blake3,
        )
        (directory / fetch_uscode.RECORD).write_text(
            fetch_uscode.record_text(release, {file: record}), encoding="utf-8"
        )

    def git(self, *arguments: str, at: Path | None = None) -> str:
        ran = subprocess.run(
            ["git", "-C", str(at or self.repository), *arguments],
            capture_output=True,
            check=False,
            text=True,
        )
        self.assertEqual(ran.returncode, 0, ran.stderr)
        return ran.stdout

    def init(self, directory: Path) -> None:
        """A repository at `directory` whose own configuration needs no signing key and
        runs only the hooks of the test."""
        directory.mkdir(parents=True, exist_ok=True)
        self.git("init", "--quiet", "--initial-branch=main", at=directory)
        for key, value in (
            ("user.name", "Invented Replayer"),
            ("user.email", "replayer@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", str(self.hooks)),
        ):
            self.git("config", "--local", key, value, at=directory)

    def replay(
        self, repository: Path | None = None, cache: Path | None = None, *options: str
    ) -> tuple[int, str, str]:
        """Runs the tool on the invented releases, with `options` after its other
        arguments, and gives its exit status, that of a refusal of its arguments too, and
        what it printed; the opening of a connection fails the test."""
        stdout, stderr = io.StringIO(), io.StringIO()
        given = [
            "--archives",
            str(self.archives),
            "--repository",
            str(repository or self.repository),
            "--fidryn",
            str(self.stand_in),
            "--cache-dir",
            str(cache or self.cache),
            "--jobs",
            "1",
            "--memory-limit",
            str(2 << 30),
            "--time-limit",
            "60",
            *options,
        ]
        connection = AssertionError("the replay opened a connection")
        with (
            mock.patch.object(replay_uscode, "RELEASES", RELEASES),
            mock.patch.object(urllib.request.OpenerDirector, "open", side_effect=connection),
            mock.patch.object(socket.socket, "connect", side_effect=connection),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            try:
                status = replay_uscode.main(given)
            except SystemExit as refused:
                status = refused.code if isinstance(refused.code, int) else 1
        return status, stdout.getvalue(), stderr.getvalue()

    def history(self) -> list[tuple[str, str, str]]:
        """The tree, the message and the files of each commit of the branch, from its root."""
        listed = self.git("rev-list", "--first-parent", "--reverse", "HEAD").split()
        return [
            (
                self.git("rev-parse", f"{commit}^{{tree}}").strip(),
                self.git("cat-file", "commit", commit).split("\n\n", 1)[1],
                self.git("ls-tree", "-r", "--name-only", commit),
            )
            for commit in listed
        ]

    def commands(self) -> list[str]:
        if not self.log.exists():
            return []
        return self.log.read_text(encoding="utf-8").splitlines()

    def state(self) -> Path:
        return self.repository / ".git" / "fidryn-replay.json"

    def assert_complete(self) -> list[tuple[str, str, str]]:
        """Holds the repository to a complete replay, and gives its history."""
        found = self.history()
        self.assertEqual([message for _, message, _ in found], [text_form(i) for i in range(3)])
        for index, (_, _, files) in enumerate(found):
            changes = [f"changes/{release}.json" for release in RELEASES[1 : index + 1]]
            self.assertEqual(
                files.splitlines(),
                [
                    ".gitattributes",
                    ".gitignore",
                    *changes,
                    "uscode/coverage.json",
                    "uscode/law.lock.json",
                    "uscode/us/usc/t99.fr",
                    "uscode/workspace.json",
                ],
            )
        self.assertFalse(self.state().exists())
        self.assertFalse((self.repository / "uscode.next").exists())
        self.assertEqual(self.git("status", "--porcelain", "--untracked-files=all"), "")
        manifest = json.loads(
            (self.repository / "uscode" / "workspace.json").read_text(encoding="utf-8")
        )
        self.assertEqual(manifest["release"], RELEASES[-1])
        return found

    def uninterrupted(self) -> list[tuple[str, str, str]]:
        """The history of a replay that nothing stopped, made in a repository of its own."""
        other = self.root / "uninterrupted"
        self.init(other)
        status, _, stderr = self.replay(repository=other)
        self.assertEqual(status, 0, stderr)
        kept, self.repository = self.repository, other
        try:
            return self.assert_complete()
        finally:
            self.repository = kept


class Complete(Replays):
    def test_a_replay_commits_each_release_with_its_text_form_and_its_change_set(self) -> None:
        status, stdout, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assertTrue(
            stdout.endswith("the replay is complete: 3 releases are committed\n"), stdout
        )
        found = self.assert_complete()
        for index, release in enumerate(RELEASES[1:], start=1):
            change_set = json.loads(self.git("show", f"HEAD:changes/{release}.json"))
            self.assertEqual(
                change_set,
                {
                    "prefix": "/us/usc",
                    "old": {"release": RELEASES[index - 1]},
                    "new": {"release": release},
                },
            )
        self.assertEqual(
            self.git("show", "HEAD:.gitattributes") + self.git("show", "HEAD:.gitignore"),
            "* -text\n.fidryn-cache/\nuscode.next/\n",
        )
        # Each release is imported, checked, read back and compared once.
        self.assertEqual(
            [line.split()[0] for line in self.commands()],
            ["ingest", "check", "text"] + ["ingest", "check", "text", "changes", "changes"] * 2,
        )
        # Run again, it finds every release committed and makes no commit.
        status, stdout, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assertTrue(stdout.startswith("3 of 3 releases are committed\n"), stdout)
        self.assertEqual(self.history(), found)


class Resumed(Replays):
    def stopped(self, status: int, *words: str) -> None:
        found, _, stderr = self.replay()
        self.assertEqual(found, status, stderr)
        for word in words:
            self.assertIn(word, stderr)

    def resumed(self) -> str:
        status, stdout, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assertTrue(stdout.startswith("1 of 3 releases are committed\n"), stdout)
        self.assertEqual(self.assert_complete(), self.uninterrupted())
        return stdout

    def test_a_replay_stopped_before_its_replacement_imports_the_release_again(self) -> None:
        (self.failing / "check-42-5").touch()
        self.stopped(1, "`fidryn check uscode.next")
        self.assertEqual(len(self.history()), 1)
        self.assertFalse(self.state().exists())
        self.assertTrue((self.repository / "uscode.next" / "workspace.json").is_file())
        (self.failing / "check-42-5").unlink()
        self.resumed()

    def test_a_replay_stopped_before_its_commit_commits_the_text_form_it_kept(self) -> None:
        (self.failing / "commit-42-5").touch()
        self.stopped(1, "commit")
        self.assertEqual(len(self.history()), 1)
        kept = json.loads(self.state().read_text(encoding="utf-8"))
        self.assertEqual(kept, {"release": "42-5", "message": text_form(1)})
        self.assertFalse((self.repository / "uscode.next").exists())
        manifest = (self.repository / "uscode" / "workspace.json").read_text(encoding="utf-8")
        self.assertEqual(json.loads(manifest)["release"], "42-5")
        before = len(self.commands())
        (self.failing / "commit-42-5").unlink()
        stdout = self.resumed()
        self.assertIn("42-5: the text form was kept: the replacement is finished\n", stdout)
        # The release is checked again and committed, and not imported again.
        self.assertEqual(
            [line.split()[0] for line in self.commands()[before:]][:2], ["check", "ingest"]
        )

    def test_a_replay_stopped_after_the_manifest_of_the_workspace_was_removed_goes_on(
        self,
    ) -> None:
        workspace = str(self.repository / "uscode")
        rmdir = os.rmdir

        def stopping(target: str | os.PathLike[str]) -> None:
            if os.fspath(target) == workspace:
                raise OSError("Invented: the replay stops here")
            rmdir(target)

        with mock.patch.object(os, "rmdir", new=stopping):
            self.stopped(2, "Invented: the replay stops here")
        self.assertEqual(len(self.history()), 1)
        self.assertEqual(json.loads(self.state().read_text(encoding="utf-8"))["release"], "42-5")
        self.assertEqual(list((self.repository / "uscode").iterdir()), [])
        self.assertTrue((self.repository / "uscode.next" / "workspace.json").is_file())
        stdout = self.resumed()
        self.assertIn("42-5: the text form was kept: the replacement is finished\n", stdout)

    def test_a_replay_stopped_after_its_commit_finishes_the_commit_and_goes_on(self) -> None:
        remove_state = replay_uscode.remove_state
        stops: list[Path] = []

        def stopping(repository: Path) -> None:
            kept = json.loads(self.state().read_text(encoding="utf-8"))
            if not stops and kept["release"] == "42-5":
                stops.append(repository)
                raise replay_uscode.CannotRunError("Invented: the replay stops here")
            remove_state(repository)

        with mock.patch.object(replay_uscode, "remove_state", new=stopping):
            self.stopped(2, "Invented: the replay stops here")
        self.assertEqual(len(self.history()), 2)
        self.assertTrue(self.state().exists())
        status, stdout, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assertTrue(stdout.startswith("2 of 3 releases are committed\n"), stdout)
        self.assertEqual(self.assert_complete(), self.uninterrupted())

    def test_a_workspace_changed_since_its_commit_stops_the_next_release(self) -> None:
        (self.failing / "ingest-42-7").touch()
        self.stopped(1, "`fidryn ingest")
        (self.failing / "ingest-42-7").unlink()
        module = self.repository / "uscode" / "us" / "usc" / "t99.fr"
        module.write_text("// Invented: an edit made by hand.\n", encoding="utf-8")
        before = len(self.commands())
        self.stopped(1, "the files of the commit of 42-5 have changed")
        self.assertEqual(self.commands()[before:], [])
        self.assertEqual(len(self.history()), 2)

    def test_a_path_staged_beyond_the_commit_stops_it_and_the_replay_goes_on_after(
        self,
    ) -> None:
        (self.failing / "ingest-42-7").touch()
        self.stopped(1, "`fidryn ingest")
        (self.failing / "ingest-42-7").unlink()
        (self.repository / "extra.txt").write_text("Invented.\n", encoding="utf-8")
        self.git("add", "extra.txt")
        self.stopped(1, "would hold ['extra.txt']")
        self.assertEqual(len(self.history()), 2)
        self.git("rm", "--quiet", "--cached", "extra.txt")
        (self.repository / "extra.txt").unlink()
        status, stdout, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assertIn("42-7: the text form was kept: the replacement is finished\n", stdout)
        self.assertEqual(self.assert_complete(), self.uninterrupted())


class Readings(Replays):
    def test_a_release_whose_two_readings_agree_is_committed(self) -> None:
        status, stdout, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assert_complete()
        for release in RELEASES:
            self.assertIn(
                f"{release}: the two readings agree: 2 records, 0 tables of contents\n", stdout
            )
        # Each import is read back once, before the replacement and the commit.
        self.assertEqual(
            [line for line in self.commands() if line.startswith("text ")],
            ["text uscode.next --json"] * len(RELEASES),
        )

    def stopped_by_a_difference(self, failing: str, *words: str) -> None:
        """Makes the readings of 42-5 differ with the file `failing`, and holds the
        replay to stop before the commit of 42-5, with `words` in what it says, and to
        leave the repository as a stop before the replacement leaves it; then starts it
        again with readings that agree."""
        (self.failing / failing).touch()
        status, _, stderr = self.replay()
        self.assertEqual(status, 1, stderr)
        self.assertTrue(stderr.startswith("stopped: "), stderr)
        for word in words:
            self.assertIn(word, stderr)
        # As after the stop of a check: the release before is committed, the import of
        # 42-5 stands in `uscode.next`, and neither a text form nor a change set of it is
        # kept.
        self.assertEqual(len(self.history()), 1)
        self.assertFalse(self.state().exists())
        self.assertFalse((self.repository / "changes").exists())
        manifest = json.loads((self.repository / "uscode" / "workspace.json").read_text("utf-8"))
        self.assertEqual(manifest["release"], RELEASES[0])
        manifest = json.loads(
            (self.repository / "uscode.next" / "workspace.json").read_text("utf-8")
        )
        self.assertEqual(manifest["release"], "42-5")
        (self.failing / failing).unlink()
        status, stdout, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assertTrue(stdout.startswith("1 of 3 releases are committed\n"), stdout)
        self.assertEqual(self.assert_complete(), self.uninterrupted())

    def test_a_release_whose_two_texts_differ_is_not_committed(self) -> None:
        self.stopped_by_a_difference(
            "text-42-5",
            "the two readings of 42-5 differ at line 2, the record of /us/usc/t99/s1",
            '`fidryn text uscode.next --json` prints `{"address":"/us/usc/t99/s1",',
            '`uslm_text.py` prints `{"address":"/us/usc/t99/s1",',
            "Other text.",
            "Invented text.",
        )

    def test_a_release_whose_counts_of_tables_of_contents_differ_is_not_committed(self) -> None:
        self.stopped_by_a_difference(
            "toc-42-5",
            "the tables of contents of 42-5 are counted 0 by `usc_profile.py` and 1 in the "
            "ledger of the import",
        )

    def test_no_readings_commits_without_the_comparison_and_says_so(self) -> None:
        # Readings that differ are not compared, and do not stop the replay.
        (self.failing / "text-42-5").touch()
        (self.failing / "toc-42-7").touch()
        status, stdout, stderr = self.replay(None, None, "--no-readings")
        self.assertEqual((status, stderr), (0, ""))
        self.assert_complete()
        line = "the two readings of the releases are not compared (--no-readings)\n"
        self.assertEqual(stdout.count(line), 1, stdout)
        self.assertNotIn("the two readings agree", stdout)
        self.assertNotIn("text", [command.split()[0] for command in self.commands()])


class Refusals(Replays):
    def test_a_next_import_that_is_a_link_or_a_file_is_refused_before_the_import(self) -> None:
        elsewhere = self.root / "elsewhere"
        elsewhere.mkdir()
        nxt = self.repository / "uscode.next"
        for name, make in (
            ("a symbolic link", lambda: nxt.symlink_to(elsewhere)),
            ("a file", lambda: nxt.write_text("Invented.\n", encoding="utf-8")),
        ):
            with self.subTest(name):
                make()
                try:
                    status, _, stderr = self.replay()
                    self.assertEqual(status, 1, stderr)
                    self.assertIn(f"{nxt} is {name}", stderr)
                    self.assertEqual(list(elsewhere.iterdir()), [])
                    self.assertEqual(self.commands(), [])
                    self.assertEqual(
                        sorted(entry.name for entry in self.repository.iterdir()),
                        [".git", "uscode.next"],
                    )
                finally:
                    nxt.unlink()

    def test_a_directory_that_is_no_repository_of_its_own_is_refused_and_left_alone(
        self,
    ) -> None:
        enclosing = self.root / "enclosing"
        self.init(enclosing)
        law = enclosing / "law"
        law.mkdir()
        for name, git_directory in (("no .git", False), ("an empty .git", True)):
            with self.subTest(name):
                if git_directory:
                    (law / ".git").mkdir()
                status, _, stderr = self.replay(repository=law)
                self.assertEqual(status, 1, stderr)
                self.assertIn(".git", stderr)
                self.assertEqual(
                    [entry.name for entry in law.iterdir()], [".git"] if git_directory else []
                )
                if git_directory:
                    self.assertEqual(list((law / ".git").iterdir()), [])
                self.assertEqual(self.commands(), [])
                self.assertEqual(self.git("status", "--porcelain", at=enclosing), "")
                ran = subprocess.run(
                    ["git", "-C", str(enclosing), "rev-parse", "--verify", "--quiet", "HEAD"],
                    capture_output=True,
                    check=False,
                )
                self.assertNotEqual(ran.returncode, 0, "a commit was made in the enclosing one")

    def test_a_cache_inside_the_repository_through_a_link_is_refused(self) -> None:
        link = self.root / "link"
        link.symlink_to(self.repository)
        for name, repository, cache in (
            ("the cache through a link", self.repository, link / "cache"),
            ("the repository through a link", link, self.repository / "cache"),
            ("the cache is the repository", link, self.repository),
        ):
            with self.subTest(name):
                status, _, stderr = self.replay(repository=repository, cache=cache)
                self.assertEqual(status, 2, stderr)
                self.assertIn("the cache directory lies outside the repository", stderr)
                self.assertEqual([entry.name for entry in self.repository.iterdir()], [".git"])
                self.assertEqual(self.commands(), [])

    def test_a_link_or_a_directory_with_the_name_of_a_temporary_file_is_refused_and_left_alone(
        self,
    ) -> None:
        elsewhere = self.root / "elsewhere.txt"
        elsewhere.write_text("Invented, and kept.\n", encoding="utf-8")
        partial = self.repository / ".git" / ".fidryn-replay.json.part"
        for name in ("a symbolic link", "a directory"):
            with self.subTest(name):
                if name == "a directory":
                    partial.mkdir()
                else:
                    partial.symlink_to(elsewhere)
                try:
                    status, _, stderr = self.replay()
                    self.assertEqual(status, 1, stderr)
                    self.assertIn(f"{partial} is {name}, and the replay writes", stderr)
                    self.assertEqual(elsewhere.read_text(encoding="utf-8"), "Invented, and kept.\n")
                    self.assertTrue(partial.is_symlink() or partial.is_dir())
                    self.assertFalse(self.state().exists())
                finally:
                    if partial.is_symlink():
                        partial.unlink()
                    else:
                        partial.rmdir()
        status, _, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assert_complete()

    def test_a_temporary_file_that_a_stopped_run_left_is_removed_and_the_replay_goes_on(
        self,
    ) -> None:
        for partial in (
            self.repository / ".git" / ".fidryn-replay.json.part",
            self.repository / "..gitattributes.part",
        ):
            partial.write_text("Invented: what a run that was killed left.\n", encoding="utf-8")
        status, _, stderr = self.replay()
        self.assertEqual((status, stderr), (0, ""))
        self.assert_complete()
        self.assertEqual(self.git("show", "HEAD:.gitattributes"), "* -text\n")
        self.assertFalse((self.repository / ".git" / ".fidryn-replay.json.part").exists())
        self.assertFalse((self.repository / "..gitattributes.part").exists())


class Guard(unittest.TestCase):
    def test_the_tests_write_nothing_into_the_repository_that_git_dir_names(self) -> None:
        """Inside a hook of git, `GIT_DIR` names the repository the hook runs for. The
        setup of these tests, and a replay they start, write neither its configuration
        nor a commit."""
        if shutil.which("git") is None or shutil.which("sh") is None:
            raise unittest.SkipTest("the replay runs git, and its stand-in is a shell script")
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        caller = root / "caller"
        (root / "gitconfig").write_bytes(b"")
        without = isolated(dict(os.environ), root / "gitconfig")
        subprocess.run(
            ["git", "init", "--quiet", str(caller)], env=without, capture_output=True, check=True
        )
        config = caller / ".git" / "config"
        before = config.read_bytes()
        with mock.patch.dict(os.environ, {"GIT_DIR": str(caller / ".git")}):
            case = Complete(
                "test_a_replay_commits_each_release_with_its_text_form_and_its_change_set"
            )
            try:
                case.setUp()
                status, _, stderr = case.replay()
            finally:
                case.doCleanups()
        self.assertEqual(config.read_bytes(), before)
        head = subprocess.run(
            ["git", "-C", str(caller), "rev-parse", "--verify", "--quiet", "HEAD"],
            env=without,
            capture_output=True,
            check=False,
        )
        self.assertNotEqual(head.returncode, 0, "a commit was made in the calling repository")
        self.assertEqual((status, stderr), (0, ""))


if __name__ == "__main__":
    unittest.main()
