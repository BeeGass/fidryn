#!/usr/bin/env python3
"""Download archives of a release point of the Code's official XML, and record them.

Fetches the archives of one release point published by the Office of the Law Revision
Counsel, from https://uscode.house.gov/download/releasepoints/, one request at a time,
into a directory, and writes or extends the directory's retrieval record,
`retrieval.json`: for each archive its file name, its address, the time of its download
in UTC, its size, its SHA-256 and its BLAKE3. An archive that is present and equal to
its record is not fetched again, and its record keeps its time. An archive whose bytes
differ from its record is refused, never replaced.

This is the only program in the repository that opens a connection.

Usage: uv run --locked python -I tools/fetch_uscode.py RELEASE --out DIR (--all | --title T ...)
Exit codes: 0 every archive fetched or kept; 1 refused; 2 cannot fetch, read or write.
"""

from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import re
import sys
import urllib.request
import zipfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Protocol

import blake3

VERSION = "0.1.0"
USER_AGENT = f"fidryn-fetch/{VERSION}"
SCHEMA = "fidryn.retrieval/v0.1"
RECORD = "retrieval.json"
BASE = "https://uscode.house.gov/download/releasepoints/us/pl"
RELEASE = re.compile(r"(?P<congress>[1-9][0-9]*)-(?P<law>[1-9][0-9]*(?:not[1-9][0-9]*)?)")
TITLE = re.compile(r"(?P<number>[0-9]{1,2})(?P<appendix>[aA]?)")
DIGEST = re.compile(r"[0-9a-f]{64}")
FIELDS = ("file", "url", "retrieved_at", "bytes", "sha256", "blake3")
TIMEOUT_SECONDS = 60
CHUNK = 1 << 16


class RefusedError(Exception):
    """The request is refused: it would replace or contradict what the record holds."""


class FetchError(Exception):
    """An archive cannot be fetched, or the directory cannot be read or written."""


class Response(Protocol):
    def read(self, amt: int, /) -> bytes: ...

    def close(self) -> None: ...


Opener = Callable[[urllib.request.Request], Response]
Clock = Callable[[], datetime]


def open_url(request: urllib.request.Request) -> Response:
    response: Response = urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS)
    return response


def utc_now() -> datetime:
    return datetime.now(UTC)


@dataclass(frozen=True, slots=True)
class Archive:
    """An archive as the retrieval record holds it."""

    file: str
    url: str
    retrieved_at: str
    bytes: int
    sha256: str
    blake3: str

    def to_json(self) -> dict[str, object]:
        return {
            "file": self.file,
            "url": self.url,
            "retrieved_at": self.retrieved_at,
            "bytes": self.bytes,
            "sha256": self.sha256,
            "blake3": self.blake3,
        }


@dataclass(frozen=True, slots=True)
class Digests:
    bytes: int
    sha256: str
    blake3: str

    def differences(self, archive: Archive) -> list[str]:
        """The fields in which these digests differ from the record of `archive`."""
        found = []
        for name in ("bytes", "sha256", "blake3"):
            have, recorded = getattr(self, name), getattr(archive, name)
            if have != recorded:
                found.append(f"{name} is {have}, the record gives {recorded}")
        return found


def release_parts(release: str) -> tuple[str, str]:
    """The Congress and the law part of a release point such as `119-142` or `119-73not60`."""
    match = RELEASE.fullmatch(release)
    if not match:
        raise RefusedError(f"{release!r} is not a release point such as 119-142 or 119-73not60")
    return match["congress"], match["law"]


def archive_file(release: str, title: str | None) -> str:
    """The file name of the archive of `title`, or of every title when it is None."""
    release_parts(release)
    if title is None:
        return f"xml_uscAll@{release}.zip"
    match = TITLE.fullmatch(title)
    if not match or int(match["number"]) == 0:
        raise RefusedError(f"{title!r} is not a title such as 1, 5a or 45")
    return f"xml_usc{int(match['number']):02d}{match['appendix']}@{release}.zip"


def archive_url(release: str, title: str | None) -> str:
    """The address of the archive of `title`, or of every title when it is None."""
    congress, law = release_parts(release)
    return f"{BASE}/{congress}/{law}/{archive_file(release, title)}"


def digests_of(path: Path) -> Digests:
    sha256, digest, size = hashlib.sha256(), blake3.blake3(), 0
    with path.open("rb") as stream:
        while chunk := stream.read(CHUNK):
            sha256.update(chunk)
            digest.update(chunk)
            size += len(chunk)
    return Digests(size, sha256.hexdigest(), digest.hexdigest())


def require_fields(value: object, fields: Sequence[str], where: str) -> dict[str, object]:
    """`value` as an object of exactly `fields`; an unknown or a missing field is named."""
    if not isinstance(value, dict):
        raise FetchError(f"{where} is not an object")
    for name in value:
        if name not in fields:
            raise FetchError(f"{where}: unknown field `{name}`")
    for name in fields:
        if name not in value:
            raise FetchError(f"{where}: missing field `{name}`")
    return value


def archive_of(value: object, where: str) -> Archive:
    fields = require_fields(value, FIELDS, where)
    texts: dict[str, str] = {}
    for name in ("file", "url", "retrieved_at", "sha256", "blake3"):
        text = fields[name]
        if not isinstance(text, str):
            raise FetchError(f"{where}: `{name}` is not a string")
        texts[name] = text
    for name in ("sha256", "blake3"):
        if not DIGEST.fullmatch(texts[name]):
            raise FetchError(f"{where}: `{name}` is not 64 lower-case hexadecimal digits")
    size = fields["bytes"]
    if not isinstance(size, int) or isinstance(size, bool) or size < 0:
        raise FetchError(f"{where}: `bytes` is not a size")
    return Archive(
        texts["file"], texts["url"], texts["retrieved_at"], size, texts["sha256"], texts["blake3"]
    )


def read_record(path: Path, release: str) -> dict[str, Archive]:
    """The archives of the record at `path` by file name; none when there is no record."""
    if not path.exists():
        return {}
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise FetchError(f"{path}: {error}") from error
    document = require_fields(document, ("schema", "release", "archives"), str(path))
    if document["schema"] != SCHEMA:
        raise FetchError(f"{path}: `schema` is {document['schema']!r}, not {SCHEMA!r}")
    if document["release"] != release:
        raise RefusedError(
            f"{path} is the record of release {document['release']}, not of {release}"
        )
    if not isinstance(document["archives"], list):
        raise FetchError(f"{path}: `archives` is not a list")
    archives: dict[str, Archive] = {}
    for index, value in enumerate(document["archives"]):
        archive = archive_of(value, f"{path}: archives[{index}]")
        if archive.file in archives:
            raise FetchError(f"{path} records {archive.file} twice")
        archives[archive.file] = archive
    return archives


def record_text(release: str, archives: dict[str, Archive]) -> str:
    """The record's file: keys in the order of S11, archives sorted by file name."""
    document = {
        "schema": SCHEMA,
        "release": release,
        "archives": [
            archives[file].to_json() for file in sorted(archives, key=lambda f: f.encode())
        ],
    }
    return json.dumps(document, indent=2, ensure_ascii=False) + "\n"


def write_record(path: Path, release: str, archives: dict[str, Archive]) -> None:
    partial = path.with_name(f".{path.name}.part")
    try:
        partial.write_text(record_text(release, archives), encoding="utf-8", newline="\n")
        partial.replace(path)
    finally:
        partial.unlink(missing_ok=True)


def download(opener: Opener, url: str, target: Path) -> Digests:
    """Writes the body served at `url` to `target`, and returns its digests."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    sha256, digest, size = hashlib.sha256(), blake3.blake3(), 0
    try:
        response = opener(request)
        try:
            with target.open("wb") as stream:
                while chunk := response.read(CHUNK):
                    stream.write(chunk)
                    sha256.update(chunk)
                    digest.update(chunk)
                    size += len(chunk)
        finally:
            response.close()
    except (OSError, http.client.HTTPException) as error:
        raise FetchError(f"{url}: {error}") from error
    if not zipfile.is_zipfile(target):
        raise FetchError(f"{url} served {size} bytes that are not a zip archive")
    return Digests(size, sha256.hexdigest(), digest.hexdigest())


def fetch(
    release: str,
    titles: Sequence[str] | None,
    out: Path,
    opener: Opener = open_url,
    clock: Clock = utc_now,
) -> list[str]:
    """Fetches the archives of `titles` (of every title when None) of `release` into `out`.

    Returns one line for each archive: `fetched FILE` or `kept FILE`.
    """
    wanted = [None] if titles is None else list(titles)
    files: dict[str, str] = {}
    for title in wanted:
        files.setdefault(archive_file(release, title), archive_url(release, title))
    record = out / RECORD
    archives = read_record(record, release)
    try:
        out.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        raise FetchError(f"{out}: {error}") from error
    lines = []
    for file, url in files.items():
        target = out / file
        recorded = archives.get(file)
        if target.exists():
            if recorded is None:
                raise RefusedError(f"{file} is present with no record in {record}; it is kept")
            differences = digests_of(target).differences(recorded)
            if differences:
                raise RefusedError(
                    f"{file} differs from its record ({'; '.join(differences)}); it is kept"
                )
            lines.append(f"kept {file}")
            continue
        partial = out / f".{file}.part"
        try:
            served = download(opener, url, partial)
            retrieved_at = clock().astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
            if recorded is not None and (differences := served.differences(recorded)):
                raise RefusedError(
                    f"{file}: {url} now serves bytes that differ from the record "
                    f"({'; '.join(differences)}); nothing is written"
                )
            partial.replace(target)
        except OSError as error:
            raise FetchError(f"{target}: {error}") from error
        finally:
            partial.unlink(missing_ok=True)
        if recorded is None:
            archives[file] = Archive(
                file, url, retrieved_at, served.bytes, served.sha256, served.blake3
            )
            try:
                write_record(record, release, archives)
            except OSError as error:
                raise FetchError(f"{record}: {error}") from error
        lines.append(f"fetched {file}")
    return lines


def main(
    argv: Sequence[str] | None = None, opener: Opener = open_url, clock: Clock = utc_now
) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n", 1)[0])
    parser.add_argument("release", help="the release point, such as 119-142 or 119-73not60")
    parser.add_argument(
        "--out", type=Path, required=True, help="the directory of the archives and their record"
    )
    which = parser.add_mutually_exclusive_group(required=True)
    which.add_argument("--all", action="store_true", help="the one archive of every title")
    which.add_argument(
        "--title", nargs="+", action="extend", metavar="T", help="titles, such as 1, 5a or 45"
    )
    args = parser.parse_args(argv)
    titles: list[str] | None = None if args.all else args.title
    try:
        lines = fetch(args.release, titles, args.out, opener, clock)
    except RefusedError as error:
        print(f"refused: {error}", file=sys.stderr)
        return 1
    except (FetchError, OSError) as error:
        print(f"cannot fetch: {error}", file=sys.stderr)
        return 2
    for line in lines:
        print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
