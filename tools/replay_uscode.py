#!/usr/bin/env python3
"""Replay the release points of the Code as the commits of a local law repository.

Takes the release points of the official XML release point published by the Office of
the Law Revision Counsel for 2026, `RELEASES`, oldest first and in the publisher's order,
and for each one in turn:

1. fetches its archive of every title into `ARCHIVES/<release>/` with
   `tools/fetch_uscode.py` when it is absent, and otherwise holds it to its record;
2. imports it with `fidryn ingest uscode` into `uscode.next/` of the repository, and
   checks that with `fidryn check`, which must find no error and no warning;
3. for every release after the first, runs `fidryn changes uscode uscode.next`, writes
   the change set as JSON to `changes/<release>.json`, and keeps its text form;
4. replaces `uscode/` with `uscode.next/`;
5. commits `.gitattributes`, `.gitignore`, `uscode/` and, after the first, the change
   set, with the text form of the change set as the message (`/us/usc: release
   <release>` for the first), given to git as it is (`--cleanup=verbatim`); reads the
   message back from the commit object, and stops unless it is the text form byte for
   byte.

The commits are made on the branch that is checked out, which is unborn before the
first. The history of the branch tells where the replay stands: its commits, from its
root, are those of the releases in order, and the next release is the one after the
last of them, so a replay that is stopped and started again finishes the release it
was in and goes on, and never makes a commit twice or skips a release. Before
`uscode/` is replaced, the text form is kept in `.git/fidryn-replay.json` with its
release, so that a replay stopped between the replacement and the commit commits that
release with it; the file is removed once the commit holds the message.

Before it asks git anything or writes anything, the tool holds the repository to be a
repository of its own: `.git` is a directory in it, and git takes the directory to be
the top of the repository of that `.git`, not a directory inside another repository.
Before an import, `uscode.next` is absent or a directory, never a symbolic link or a
file. The only directory the tool removes is `uscode/`, and only when the repository
holds `.git`; `uscode` is a directory, no symbolic link, with the manifest of a
generated workspace of the prefix, or an empty directory when the run found the text
form of the release kept (a replay stopped between the removal of the manifest and of
the directory leaves one); and `uscode.next/` holds the complete import of the next
release: its manifest and its lock read, and `fidryn check` of it found nothing. The
manifest is removed last, and no symbolic link is followed. The tool writes each of its
files through a new file beside it, which it makes and renames; an entry that has the
name of that file already stops the replay and is left as it is. Anything else stops
the replay with a sentence that says what it found.

Each run of `fidryn` has a limit of its own on its address space, set in the child
before the program starts, and on its time; a run that exceeds one stops the replay.
Git's configuration decides whether a commit is signed; no option is passed either
way. Git's automatic housekeeping is turned off for these commits (`gc.auto=0`), so
that no packing runs in the background while the replay works. The cache of the checks
is neither the repository nor inside it, the two paths compared with every link in the
part of each that exists resolved.

Usage: uv run --locked python -I tools/replay_uscode.py --archives DIR --repository DIR
  --fidryn PATH --cache-dir DIR [--jobs N] [--memory-limit BYTES] [--time-limit SECONDS]
Exit codes: 0 every release is committed; 1 the replay stopped at what it found; 2 a
command or a file could not be run, read or written.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import resource
import signal
import stat
import subprocess
import sys
import time
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import fetch_uscode

PREFIX = "/us/usc"
# The release points the publisher lists for 2026, from the oldest to the newest, in
# the publisher's order, which the names alone do not give.
RELEASES = (
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
)
WORKSPACE = "uscode"
NEXT = "uscode.next"
CHANGES = "changes"
MANIFEST = "workspace.json"
LOCK = "law.lock.json"
STATE = "fidryn-replay.json"
GITATTRIBUTES = "* -text\n"
GITIGNORE = ".fidryn-cache/\nuscode.next/\n"
MEMORY_LIMIT = 16 << 30
TIME_LIMIT = 30 * 60
SUMMARY = re.compile(
    r"modules: (?P<modules>\d+); from cache: \d+; errors: (?P<errors>\d+); "
    r"warnings: (?P<warnings>\d+)"
)


class StopReplayError(Exception):
    """The replay stops: what it found is not what the next step needs."""


class CannotRunError(Exception):
    """A command or a file could not be run, read or written."""


def subject(index: int) -> str:
    """The first line of the message of the commit of `RELEASES[index]`."""
    if index == 0:
        return f"{PREFIX}: release {RELEASES[0]}"
    return f"{PREFIX}: {RELEASES[index - 1]} to {RELEASES[index]}"


def replayed(subjects: Sequence[str]) -> int:
    """How many releases the commits of a branch, from its root, with these first lines
    of their messages, have committed. Commits after the last release are left alone;
    a commit before it that is not the next release's stops the replay."""
    for index, found in enumerate(subjects):
        if index == len(RELEASES):
            return index
        if found != subject(index):
            raise StopReplayError(
                f"commit {index + 1} of the branch is not the commit of the release "
                f"{RELEASES[index]}: its message begins `{found}`, and the replay "
                f"expects `{subject(index)}`"
            )
    return len(subjects)


# Reading what the repository holds.


def kind_of(path: Path) -> str:
    """What is at `path`, without following a link: `missing`, `link`, `directory`,
    `file` or `other`."""
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return "missing"
    except OSError as error:
        raise CannotRunError(f"{path}: {error}") from error
    if stat.S_ISLNK(mode):
        return "link"
    if stat.S_ISDIR(mode):
        return "directory"
    if stat.S_ISREG(mode):
        return "file"
    return "other"


def read_json(path: Path, what: str) -> dict[str, object]:
    """The JSON object of the file at `path`, which must be a file and no link."""
    found = kind_of(path)
    if found != "file":
        raise StopReplayError(f"{path} is to be {what}, and it is {describe(found)}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise StopReplayError(f"{path} is to be {what}, and it does not read: {error}") from error
    except OSError as error:
        raise CannotRunError(f"{path}: {error}") from error
    if not isinstance(value, dict):
        raise StopReplayError(f"{path} is to be {what}, and it is no JSON object")
    return value


def describe(found: str) -> str:
    return {
        "missing": "not there",
        "link": "a symbolic link",
        "directory": "a directory",
        "file": "a file",
        "other": "neither a file nor a directory",
    }[found]


def generated_release(directory: Path) -> str:
    """The release of the generated workspace of the prefix in `directory`, by its
    manifest; anything else stops the replay."""
    found = kind_of(directory)
    if found != "directory":
        raise StopReplayError(
            f"{directory} is to be a generated workspace of {PREFIX}, and it is {describe(found)}"
        )
    document = read_json(directory / MANIFEST, "the manifest of a generated workspace")
    if document.get("schema") != "fidryn.workspace/v0.1" or document.get("kind") != "generated":
        raise StopReplayError(
            f"{directory / MANIFEST} is not the manifest of a generated workspace: its "
            f"kind is {document.get('kind')!r}"
        )
    if document.get("prefix") != PREFIX:
        raise StopReplayError(
            f"{directory / MANIFEST} is the manifest of a workspace of the prefix "
            f"{document.get('prefix')!r}, not of {PREFIX}"
        )
    release = document.get("release")
    if not isinstance(release, str):
        raise StopReplayError(f"{directory / MANIFEST} names no release")
    return release


def complete_import(directory: Path, release: str) -> None:
    """Stops the replay unless `directory` holds the manifest and the lock of an import
    of `release`. That `fidryn check` of it finds nothing is checked before."""
    found = generated_release(directory)
    if found != release:
        raise StopReplayError(
            f"{directory / MANIFEST} is the manifest of the release {found}, and the "
            f"release imported there is {release}"
        )
    document = read_json(directory / LOCK, "the lock of the import")
    if document.get("schema") != "fidryn.law-lock/v0.1":
        raise StopReplayError(f"{directory / LOCK} is not a lock: {document.get('schema')!r}")
    if document.get("prefix") != PREFIX or document.get("release") != release:
        raise StopReplayError(
            f"{directory / LOCK} is the lock of {document.get('prefix')!r} at the release "
            f"{document.get('release')!r}, and the release imported there is {release}"
        )


def remove_tree(top: Path, last: str) -> None:
    """Removes the directory `top` and what it holds without following a link: a link
    is removed as a link. The file `last` at the top of it is removed last."""

    def contents(directory: Path, keep: str | None) -> None:
        try:
            with os.scandir(directory) as found:
                entries = sorted(found, key=lambda entry: entry.name)
        except OSError as error:
            raise CannotRunError(f"{directory}: {error}") from error
        for entry in entries:
            if entry.name == keep:
                continue
            if entry.is_dir(follow_symlinks=False):
                contents(Path(entry.path), None)
                os.rmdir(entry.path)
            else:
                os.unlink(entry.path)

    try:
        contents(top, last)
        if kind_of(top / last) != "missing":
            os.unlink(top / last)
        os.rmdir(top)
    except OSError as error:
        raise CannotRunError(f"{top}: {error}") from error


def empty(directory: Path) -> bool:
    try:
        with os.scandir(directory) as found:
            return next(found, None) is None
    except OSError as error:
        raise CannotRunError(f"{directory}: {error}") from error


def replace_workspace(repository: Path, release: str, kept: bool = False) -> None:
    """Replaces `uscode/` of `repository` with `uscode.next/`, the import of `release`,
    under the conditions the module says; `kept` says that the run found the text form
    of `release` kept."""
    if kind_of(repository / ".git") != "directory":
        raise StopReplayError(
            f"{repository} holds no directory `.git`, and the replay removes nothing "
            "outside a repository"
        )
    nxt, workspace = repository / NEXT, repository / WORKSPACE
    if kind_of(nxt) != "directory":
        raise StopReplayError(
            f"{nxt} is to hold the import of {release}, and it is {describe(kind_of(nxt))}"
        )
    complete_import(nxt, release)
    found = kind_of(workspace)
    if found == "link":
        raise StopReplayError(
            f"{workspace} is a symbolic link, and the replay removes only a directory"
        )
    if kept and found == "directory" and empty(workspace):
        # What a replay stopped between the removal of the manifest and of the
        # directory leaves.
        try:
            os.rmdir(workspace)
        except OSError as error:
            raise CannotRunError(f"{workspace}: {error}") from error
    elif found != "missing":
        generated_release(workspace)
        remove_tree(workspace, MANIFEST)
    try:
        os.rename(nxt, workspace)
    except OSError as error:
        raise CannotRunError(f"{nxt}: {error}") from error


# Running the programs.


@dataclass(frozen=True)
class Limits:
    memory: int
    seconds: int


def address_space(limit: int) -> Callable[[], None]:
    def apply() -> None:
        resource.setrlimit(resource.RLIMIT_AS, (limit, limit))

    return apply


def shown(command: Sequence[str]) -> str:
    return " ".join(["fidryn", *command[1:]])


def run_fidryn(command: Sequence[str], cwd: Path, limits: Limits) -> str:
    """Runs `fidryn` under its limits, and returns what it printed on stdout."""
    started = time.monotonic()
    try:
        ran = subprocess.run(
            list(command),
            cwd=cwd,
            capture_output=True,
            timeout=limits.seconds,
            preexec_fn=address_space(limits.memory),
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise StopReplayError(
            f"`{shown(command)}` ran longer than {limits.seconds} seconds, the limit of a "
            "run, and was stopped"
        ) from error
    except OSError as error:
        raise CannotRunError(f"{command[0]}: {error}") from error
    stderr = ran.stderr.decode("utf-8", "replace")
    if ran.returncode < 0:
        try:
            name = signal.Signals(-ran.returncode).name
        except ValueError:
            name = f"the signal {-ran.returncode}"
        raise StopReplayError(
            f"`{shown(command)}` was ended by {name}; a run that needs more than its "
            f"limit of {limits.memory} bytes of address space ends so\n{stderr}"
        )
    if ran.returncode != 0:
        raise StopReplayError(f"`{shown(command)}` exited with status {ran.returncode}\n{stderr}")
    if stderr:
        print(stderr, end="", file=sys.stderr, flush=True)
    try:
        out = ran.stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise StopReplayError(f"`{shown(command)}` printed what is not UTF-8") from error
    print(f"  ({time.monotonic() - started:.1f} s) {shown(command)}", flush=True)
    return out


def git(repository: Path, *arguments: str, given: bytes | None = None) -> bytes:
    """Runs git in `repository`, and returns what it printed on stdout."""
    try:
        ran = subprocess.run(
            ["git", "-C", str(repository), *arguments],
            input=given,
            capture_output=True,
            check=False,
        )
    except OSError as error:
        raise CannotRunError(f"git: {error}") from error
    if ran.returncode != 0:
        raise StopReplayError(
            f"`git {' '.join(arguments)}` exited with status {ran.returncode}\n"
            + ran.stderr.decode("utf-8", "replace")
        )
    return ran.stdout


def text_of(found: bytes, what: str) -> str:
    try:
        return found.decode("utf-8")
    except UnicodeDecodeError as error:
        raise StopReplayError(f"{what} is not UTF-8") from error


def own_repository(repository: Path) -> None:
    """Stops the replay unless `repository` holds a directory `.git` and git takes
    `repository` to be the top of the repository of that `.git`. Git is asked only
    after `.git` is found to be a directory."""
    found = kind_of(repository / ".git")
    if found != "directory":
        raise StopReplayError(
            f"{repository / '.git'} is {describe(found)}, and the replay commits only to a "
            "repository whose `.git` is a directory of its own"
        )
    shown = git(repository, "rev-parse", "--show-toplevel", "--absolute-git-dir")
    lines = text_of(shown, "what git names as the repository").split("\n")
    if len(lines) != 3 or lines[2]:
        raise StopReplayError(f"git does not name the top and the `.git` of {repository}")
    top, git_directory = Path(lines[0]), Path(lines[1])
    if (top.resolve(), git_directory.resolve()) != (
        repository.resolve(),
        (repository / ".git").resolve(),
    ):
        raise StopReplayError(
            f"git takes {repository} to be in the repository {git_directory}, whose top is "
            f"{top}; {repository / '.git'} is no repository of its own"
        )


def commits_of(repository: Path) -> list[str]:
    """The commits of the branch that is checked out, from its root, along first
    parents; none when the branch is unborn."""
    head = git(repository, "symbolic-ref", "--quiet", "HEAD").decode().strip()
    if not head.startswith("refs/heads/"):
        raise StopReplayError(f"HEAD is {head}, and the replay commits to a branch")
    try:
        ran = subprocess.run(
            ["git", "-C", str(repository), "rev-parse", "--verify", "--quiet", "HEAD"],
            capture_output=True,
            check=False,
        )
    except OSError as error:
        raise CannotRunError(f"git: {error}") from error
    if ran.returncode != 0:
        return []
    listed = git(repository, "rev-list", "--first-parent", "--reverse", "HEAD")
    return listed.decode().split()


def message_of(repository: Path, commit: str) -> bytes:
    """The message of `commit`, byte for byte, as its object holds it."""
    raw = git(repository, "cat-file", "commit", commit)
    end = raw.find(b"\n\n")
    if end == -1:
        return b""
    return raw[end + 2 :]


def read_state(repository: Path) -> tuple[str, str] | None:
    path = repository / ".git" / STATE
    if kind_of(path) == "missing":
        return None
    document = read_json(path, "the text form kept for a commit")
    release, message = document.get("release"), document.get("message")
    if not isinstance(release, str) or not isinstance(message, str):
        raise StopReplayError(f"{path} holds no release and message")
    return release, message


def write_file(path: Path, text: str) -> None:
    """Writes `text` to `path` through a new file beside it that is renamed. An entry
    that has the name of that file already, a symbolic link among them, stops the
    replay and is left as it is."""
    partial = path.with_name(f".{path.name}.part")
    try:
        stream = partial.open("x", encoding="utf-8", newline="\n")
    except FileExistsError as error:
        raise StopReplayError(
            f"{partial} is there already, and the replay writes {path.name} through a new "
            "file of that name; it is left as it is"
        ) from error
    except OSError as error:
        raise CannotRunError(f"{partial}: {error}") from error
    try:
        with stream:
            stream.write(text)
        partial.replace(path)
    except OSError as error:
        raise CannotRunError(f"{path}: {error}") from error
    finally:
        partial.unlink(missing_ok=True)


def write_state(repository: Path, release: str, message: str) -> None:
    write_file(
        repository / ".git" / STATE,
        json.dumps({"release": release, "message": message}, ensure_ascii=False) + "\n",
    )


def remove_state(repository: Path) -> None:
    try:
        (repository / ".git" / STATE).unlink(missing_ok=True)
    except OSError as error:
        raise CannotRunError(f"{repository / '.git' / STATE}: {error}") from error


# The steps of one release.


@dataclass(frozen=True)
class Replay:
    archives: Path
    repository: Path
    fidryn: Path
    cache: Path
    jobs: int
    limits: Limits

    def say(self, release: str, line: str) -> None:
        print(f"{release}: {line}", flush=True)

    def fetch(self, release: str) -> Path:
        directory = self.archives / release
        try:
            lines = fetch_uscode.fetch(release, None, directory)
        except fetch_uscode.RefusedError as error:
            raise StopReplayError(f"the archive of {release} is refused: {error}") from error
        except (fetch_uscode.FetchError, OSError) as error:
            raise CannotRunError(f"the archive of {release} cannot be fetched: {error}") from error
        for line in lines:
            self.say(release, line)
        return directory

    def check(self, release: str, name: str) -> None:
        """`fidryn check` of the workspace `name` of the repository, which must find
        no error and no warning."""
        out = run_fidryn(
            [
                str(self.fidryn),
                "check",
                name,
                "--jobs",
                str(self.jobs),
                "--cache-dir",
                str(self.cache),
            ],
            self.repository,
            self.limits,
        )
        summary = SUMMARY.search(out)
        if summary is None or out.split("\n")[-2:] != ["ok", ""]:
            raise StopReplayError(f"`fidryn check {name}` printed no summary and `ok`:\n{out}")
        if summary["errors"] != "0" or summary["warnings"] != "0":
            raise StopReplayError(
                f"`fidryn check {name}` found {summary['errors']} errors and "
                f"{summary['warnings']} warnings in the release {release}"
            )
        self.say(release, summary[0])

    def ingest(self, release: str, archives: Path) -> None:
        out = run_fidryn(
            [
                str(self.fidryn),
                "ingest",
                "uscode",
                str(archives),
                "--out",
                NEXT,
                "--jobs",
                str(self.jobs),
            ],
            self.repository,
            self.limits,
        )
        if not out.endswith(f"(release {release})\n"):
            raise StopReplayError(f"`fidryn ingest` printed {out!r} for the release {release}")
        self.say(release, out.strip())

    def change_set(self, index: int) -> str:
        """Writes `changes/<release>.json`, and returns the text form."""
        release = RELEASES[index]
        command = [
            str(self.fidryn),
            "changes",
            WORKSPACE,
            NEXT,
            "--jobs",
            str(self.jobs),
            "--cache-dir",
            str(self.cache),
        ]
        document = run_fidryn([*command, "--json"], self.repository, self.limits)
        text = run_fidryn(command, self.repository, self.limits)
        try:
            value = json.loads(document)
        except json.JSONDecodeError as error:
            raise StopReplayError(f"the change set of {release} does not read: {error}") from error
        sides = (value.get("old", {}).get("release"), value.get("new", {}).get("release"))
        if value.get("prefix") != PREFIX or sides != (RELEASES[index - 1], release):
            raise StopReplayError(
                f"the change set is of {value.get('prefix')!r} from {sides[0]!r} to "
                f"{sides[1]!r}, and the replay is at {RELEASES[index - 1]} to {release}"
            )
        if text.split("\n", 1)[0] != subject(index):
            raise StopReplayError(
                f"the text form of the change set begins {text.split(chr(10), 1)[0]!r}, not "
                f"{subject(index)!r}"
            )
        changes = self.repository / CHANGES
        try:
            changes.mkdir(exist_ok=True)
        except OSError as error:
            raise CannotRunError(f"{changes}: {error}") from error
        if kind_of(changes) != "directory":
            raise StopReplayError(f"{changes} is {describe(kind_of(changes))}, not a directory")
        write_file(changes / f"{release}.json", document)
        for line in text.split("\n")[2:6]:
            self.say(release, line)
        return text

    def top_files(self, release: str) -> None:
        """Writes `.gitattributes` and `.gitignore` when absent, and holds them to their
        text when present."""
        for name, text in ((".gitattributes", GITATTRIBUTES), (".gitignore", GITIGNORE)):
            path = self.repository / name
            found = kind_of(path)
            if found == "missing":
                write_file(path, text)
                self.say(release, f"wrote {name}")
            elif found != "file" or path.read_bytes() != text.encode():
                raise StopReplayError(f"{path} is not the file the replay writes: {text!r}")

    def staged(self, index: int) -> None:
        """Stops the replay unless the index holds exactly what the commit of
        `RELEASES[index]` holds."""
        listed = git(self.repository, "ls-files", "-z").split(b"\0")
        paths = {text_of(item, "a path of the index") for item in listed if item}
        tops = {item.split("/", 1)[0] for item in paths}
        expected = {".gitattributes", ".gitignore", WORKSPACE} | ({CHANGES} if index else set())
        changes = {item for item in paths if item.startswith(f"{CHANGES}/")}
        wanted = {f"{CHANGES}/{RELEASES[number]}.json" for number in range(1, index + 1)}
        if tops != expected or changes != wanted:
            extra = sorted((tops - expected) | (changes - wanted))
            missing = sorted((expected - tops) | (wanted - changes))
            raise StopReplayError(
                f"the commit of {RELEASES[index]} would hold {extra} beyond what it is to "
                f"hold, and lack {missing}"
            )

    def commit(self, index: int, message: str) -> None:
        release = RELEASES[index]
        before = commits_of(self.repository)
        paths = [".gitattributes", ".gitignore", WORKSPACE]
        if index:
            paths.append(f"{CHANGES}/{release}.json")
        started = time.monotonic()
        git(self.repository, "add", "--all", "--", *paths)
        self.staged(index)
        git(
            self.repository,
            "-c",
            "gc.auto=0",
            "commit",
            "--quiet",
            "--cleanup=verbatim",
            "--file=-",
            given=message.encode("utf-8"),
        )
        after = commits_of(self.repository)
        if after[:-1] != before or len(after) != index + 1:
            raise StopReplayError(f"the commit of {release} did not follow the commit before it")
        self.verify(index, after[-1], message)
        self.say(release, f"committed {after[-1]} ({time.monotonic() - started:.1f} s)")

    def verify(self, index: int, commit: str, message: str) -> None:
        """Stops the replay unless `commit` holds `message` byte for byte."""
        held = message_of(self.repository, commit)
        if held != message.encode("utf-8"):
            raise StopReplayError(
                f"the message of {commit}, the commit of {RELEASES[index]}, is not the text "
                f"form of its change set: it holds {len(held)} bytes, the text form "
                f"{len(message.encode('utf-8'))}"
            )
        remove_state(self.repository)

    def unchanged(self, index: int) -> None:
        """Stops the replay unless the files of the commit before are as it holds them,
        so that the change set is made from the release it committed. A file of
        `changes/` that no commit holds, such as the change set of a replay stopped
        before its commit, is no part of the commit before, and is written again."""
        if index == 0:
            return
        found = git(
            self.repository,
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".gitattributes",
            ".gitignore",
            WORKSPACE,
        ) + git(
            self.repository,
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=no",
            "--",
            CHANGES,
        )
        if found:
            first = text_of(found.split(b"\0")[0], "the status of git")
            raise StopReplayError(
                f"the files of the commit of {RELEASES[index - 1]} have changed since it "
                f"was made, as `{first}`, and the change set is made from what it holds"
            )
        release = generated_release(self.repository / WORKSPACE)
        if release != RELEASES[index - 1]:
            raise StopReplayError(
                f"{self.repository / WORKSPACE} holds the release {release}, and the last "
                f"commit is of {RELEASES[index - 1]}"
            )

    def release(self, index: int, kept: str | None) -> None:
        """Commits `RELEASES[index]`; `kept` is the text form kept before a replacement
        that a replay stopped before its commit."""
        release = RELEASES[index]
        found = kind_of(self.repository / NEXT)
        if found not in ("missing", "directory"):
            raise StopReplayError(
                f"{self.repository / NEXT} is {describe(found)}, and the import of {release} "
                "is made only into a directory, or where nothing is"
            )
        archives = self.fetch(release)
        if kept is None:
            self.unchanged(index)
            self.ingest(release, archives)
            self.check(release, NEXT)
            complete_import(self.repository / NEXT, release)
            if index == 0:
                self.top_files(release)
                message = subject(0) + "\n"
            else:
                message = self.change_set(index)
            write_state(self.repository, release, message)
            replace_workspace(self.repository, release)
        else:
            message = kept
            self.say(release, "the text form was kept: the replacement is finished")
            if found != "missing":
                self.check(release, NEXT)
                replace_workspace(self.repository, release, kept=True)
            else:
                self.check(release, WORKSPACE)
                complete_import(self.repository / WORKSPACE, release)
        self.commit(index, message)

    def run(self) -> None:
        own_repository(self.repository)
        commits = commits_of(self.repository)
        subjects = [
            text_of(message_of(self.repository, commit), f"the message of {commit}").split("\n", 1)[
                0
            ]
            for commit in commits[: len(RELEASES) + 1]
        ]
        done = replayed(subjects)
        for index in range(done):
            release = generated_release_at(self.repository, commits[index])
            if release != RELEASES[index]:
                raise StopReplayError(
                    f"{commits[index]} has the message of the release {RELEASES[index]}, and "
                    f"its {WORKSPACE}/{MANIFEST} is of the release {release}"
                )
        kept: str | None = None
        state = read_state(self.repository)
        if state is not None:
            release, message = state
            if done and release == RELEASES[done - 1]:
                self.verify(done - 1, commits[done - 1], message)
            elif done < len(RELEASES) and release == RELEASES[done]:
                kept = message
            else:
                raise StopReplayError(
                    f"{self.repository / '.git' / STATE} keeps the text form of {release}, "
                    "which is neither the last release committed nor the next"
                )
        if done:
            print(f"{done} of {len(RELEASES)} releases are committed", flush=True)
        for index in range(done, len(RELEASES)):
            self.release(index, kept if index == done else None)
        print(f"the replay is complete: {len(RELEASES)} releases are committed", flush=True)


def generated_release_at(repository: Path, commit: str) -> str:
    """The release of the manifest of `uscode/` that `commit` holds."""
    text = text_of(git(repository, "show", f"{commit}:{WORKSPACE}/{MANIFEST}"), "a manifest")
    try:
        document = json.loads(text)
    except json.JSONDecodeError as error:
        raise StopReplayError(f"{commit} holds a manifest that does not read: {error}") from error
    if not isinstance(document, dict) or not isinstance(document.get("release"), str):
        raise StopReplayError(f"{commit} holds a manifest that names no release")
    return str(document["release"])


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n", 1)[0])
    parser.add_argument(
        "--archives", type=Path, required=True, help="the directory of a directory per release"
    )
    parser.add_argument(
        "--repository", type=Path, required=True, help="the law repository, with `.git`"
    )
    parser.add_argument("--fidryn", type=Path, required=True, help="the binary that is run")
    parser.add_argument(
        "--cache-dir", type=Path, required=True, help="the cache of every check, outside it"
    )
    parser.add_argument("--jobs", type=int, default=16, help="the workers of each run")
    parser.add_argument(
        "--memory-limit",
        type=int,
        default=MEMORY_LIMIT,
        help=f"the address space of each run, in bytes (default {MEMORY_LIMIT})",
    )
    parser.add_argument(
        "--time-limit",
        type=int,
        default=TIME_LIMIT,
        help=f"the time of each run, in seconds (default {TIME_LIMIT})",
    )
    args = parser.parse_args(argv)
    if args.jobs < 1 or args.memory_limit < 1 or args.time_limit < 1:
        parser.error("--jobs, --memory-limit and --time-limit are at least 1")
    repository = args.repository.absolute()
    cache = args.cache_dir.absolute()
    # Compared with every link resolved in the part of each path that exists.
    held, inside = repository.resolve(), cache.resolve()
    if inside == held or held in inside.parents:
        parser.error("the cache directory lies outside the repository")
    replay = Replay(
        args.archives.absolute(),
        repository,
        args.fidryn.absolute(),
        cache,
        args.jobs,
        Limits(args.memory_limit, args.time_limit),
    )
    try:
        replay.run()
    except StopReplayError as error:
        print(f"stopped: {error}", file=sys.stderr, flush=True)
        return 1
    except CannotRunError as error:
        print(f"cannot run: {error}", file=sys.stderr, flush=True)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
