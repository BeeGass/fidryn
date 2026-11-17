"""Hold the tool that fetches a release to its addresses and its record, with no network.

Every archive here is invented: the fake opener serves small archives of one member,
whose first line says that it is invented, and never opens a connection.
"""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
import sys
import tempfile
import unittest
import urllib.error
import urllib.request
import zipfile
from collections.abc import Callable
from datetime import UTC, datetime
from email.message import Message
from pathlib import Path

import blake3

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import fetch_uscode

BASE = "https://uscode.house.gov/download/releasepoints/us/pl"
FIRST = datetime(2026, 11, 16, 22, 14, 3, tzinfo=UTC)
LATER = datetime(2026, 11, 17, 9, 30, 0, tzinfo=UTC)


def invented_archive(words: str) -> bytes:
    """A zip archive of one member whose first line says that it is invented."""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("usc99.xml", f"<!-- Invented for the tests. -->\n<p>{words}</p>\n")
    return buffer.getvalue()


TITLE_1 = invented_archive("The first invented archive.")
TITLE_11A = invented_archive("The second invented archive.")
TITLE_45 = invented_archive("The third invented archive.")
URL_1 = f"{BASE}/119/142/xml_usc01@119-142.zip"
URL_11A = f"{BASE}/119/142/xml_usc11a@119-142.zip"
URL_45 = f"{BASE}/119/142/xml_usc45@119-142.zip"


class Response(io.BytesIO):
    """A served body that tells its opener when it is closed."""

    def __init__(self, body: bytes, on_close: Callable[[], None]) -> None:
        super().__init__(body)
        self.on_close = on_close

    def close(self) -> None:
        if not self.closed:
            self.on_close()
        super().close()


class FakeOpener:
    """Serves the bodies it is given by address, and records every request."""

    def __init__(self, served: dict[str, bytes]) -> None:
        self.served = served
        self.requests: list[urllib.request.Request] = []
        self.open = 0

    def __call__(self, request: urllib.request.Request) -> Response:
        if self.open:
            raise AssertionError("a request was made while another response was open")
        self.requests.append(request)
        url = request.full_url
        if url not in self.served:
            raise urllib.error.HTTPError(url, 404, "Not Found", Message(), None)
        self.open += 1
        return Response(self.served[url], self.closed)

    def closed(self) -> None:
        self.open -= 1

    def urls(self) -> list[str]:
        return [request.full_url for request in self.requests]


def clock_at(moment: datetime) -> Callable[[], datetime]:
    return lambda: moment


def entry(file: str, url: str, retrieved_at: str, body: bytes) -> str:
    return (
        "    {\n"
        f'      "file": "{file}",\n'
        f'      "url": "{url}",\n'
        f'      "retrieved_at": "{retrieved_at}",\n'
        f'      "bytes": {len(body)},\n'
        f'      "sha256": "{hashlib.sha256(body).hexdigest()}",\n'
        f'      "blake3": "{blake3.blake3(body).hexdigest()}"\n'
        "    }"
    )


def record(release: str, *entries: str) -> str:
    return (
        "{\n"
        '  "schema": "fidryn.retrieval/v0.1",\n'
        f'  "release": "{release}",\n'
        '  "archives": [\n' + ",\n".join(entries) + "\n  ]\n}\n"
    )


class Addresses(unittest.TestCase):
    def test_the_address_of_title_1_of_119_142(self) -> None:
        self.assertEqual(fetch_uscode.archive_url("119-142", "1"), URL_1)

    def test_the_address_of_title_5a(self) -> None:
        self.assertEqual(
            fetch_uscode.archive_url("119-142", "5a"), f"{BASE}/119/142/xml_usc05a@119-142.zip"
        )

    def test_the_address_of_title_11a_of_119_73not60(self) -> None:
        self.assertEqual(
            fetch_uscode.archive_url("119-73not60", "11a"),
            f"{BASE}/119/73not60/xml_usc11a@119-73not60.zip",
        )

    def test_the_address_of_all_titles(self) -> None:
        self.assertEqual(
            fetch_uscode.archive_url("119-142", None), f"{BASE}/119/142/xml_uscAll@119-142.zip"
        )

    def test_a_malformed_release_or_title_is_refused(self) -> None:
        for release in ("119", "119-", "-142", "x-142", "119-142/..", "119-142not"):
            with self.subTest(release=release), self.assertRaises(fetch_uscode.RefusedError):
                fetch_uscode.archive_url(release, "1")
        for title in ("", "0", "00", "123", "5b", "a", "1/../2"):
            with self.subTest(title=title), self.assertRaises(fetch_uscode.RefusedError):
                fetch_uscode.archive_url("119-142", title)


class Fetching(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.out = self.directory / "archives"
        self.opener = FakeOpener({URL_1: TITLE_1, URL_11A: TITLE_11A, URL_45: TITLE_45})

    def fetch(self, *titles: str, moment: datetime = FIRST) -> list[str]:
        return fetch_uscode.fetch("119-142", list(titles), self.out, self.opener, clock_at(moment))

    def record_text(self) -> str:
        return (self.out / "retrieval.json").read_text(encoding="utf-8")

    def names(self) -> list[str]:
        return sorted(child.name for child in self.out.iterdir())

    def test_the_record_that_is_written(self) -> None:
        self.fetch("11a", "1")
        # One request to an archive at a time, in the order given, naming only the tool.
        self.assertEqual(self.opener.urls(), [URL_11A, URL_1])
        self.assertEqual(self.opener.open, 0)
        for request in self.opener.requests:
            self.assertEqual(request.header_items(), [("User-agent", fetch_uscode.USER_AGENT)])
        self.assertEqual(fetch_uscode.USER_AGENT, f"fidryn-fetch/{fetch_uscode.VERSION}")
        # The archives exactly as served, and the record sorted by file name.
        self.assertEqual(
            self.names(), ["retrieval.json", "xml_usc01@119-142.zip", "xml_usc11a@119-142.zip"]
        )
        self.assertEqual((self.out / "xml_usc01@119-142.zip").read_bytes(), TITLE_1)
        self.assertEqual((self.out / "xml_usc11a@119-142.zip").read_bytes(), TITLE_11A)
        self.assertEqual(
            self.record_text(),
            record(
                "119-142",
                entry("xml_usc01@119-142.zip", URL_1, "2026-11-16T22:14:03Z", TITLE_1),
                entry("xml_usc11a@119-142.zip", URL_11A, "2026-11-16T22:14:03Z", TITLE_11A),
            ),
        )

    def test_all_titles_are_one_archive(self) -> None:
        url = f"{BASE}/119/142/xml_uscAll@119-142.zip"
        self.opener.served[url] = TITLE_45
        fetch_uscode.fetch("119-142", None, self.out, self.opener, clock_at(FIRST))
        self.assertEqual(self.opener.urls(), [url])
        self.assertEqual(
            self.record_text(),
            record(
                "119-142", entry("xml_uscAll@119-142.zip", url, "2026-11-16T22:14:03Z", TITLE_45)
            ),
        )

    def test_a_second_run_changes_nothing(self) -> None:
        self.fetch("1", "11a")
        before = {name: (self.out / name).read_bytes() for name in self.names()}
        times = {name: (self.out / name).stat().st_mtime_ns for name in self.names()}
        lines = self.fetch("1", "11a", moment=LATER)
        self.assertEqual(lines, ["kept xml_usc01@119-142.zip", "kept xml_usc11a@119-142.zip"])
        self.assertEqual(len(self.opener.requests), 2, "the second run opened a connection")
        self.assertEqual({name: (self.out / name).read_bytes() for name in self.names()}, before)
        self.assertEqual(
            {name: (self.out / name).stat().st_mtime_ns for name in self.names()}, times
        )

    def test_a_later_run_extends_the_record_and_keeps_each_time(self) -> None:
        self.fetch("1", "45")
        lines = self.fetch("11a", "1", moment=LATER)
        self.assertEqual(lines, ["fetched xml_usc11a@119-142.zip", "kept xml_usc01@119-142.zip"])
        self.assertEqual(self.opener.urls(), [URL_1, URL_45, URL_11A])
        self.assertEqual(
            self.record_text(),
            record(
                "119-142",
                entry("xml_usc01@119-142.zip", URL_1, "2026-11-16T22:14:03Z", TITLE_1),
                entry("xml_usc11a@119-142.zip", URL_11A, "2026-11-17T09:30:00Z", TITLE_11A),
                entry("xml_usc45@119-142.zip", URL_45, "2026-11-16T22:14:03Z", TITLE_45),
            ),
        )

    def test_an_archive_whose_bytes_changed_is_refused(self) -> None:
        self.fetch("1")
        recorded = self.record_text()
        changed = TITLE_1[:-1] + bytes([TITLE_1[-1] ^ 1])
        (self.out / "xml_usc01@119-142.zip").write_bytes(changed)
        with self.assertRaises(fetch_uscode.RefusedError) as caught:
            self.fetch("1", moment=LATER)
        self.assertIn("xml_usc01@119-142.zip", str(caught.exception))
        self.assertIn("sha256", str(caught.exception))
        self.assertEqual((self.out / "xml_usc01@119-142.zip").read_bytes(), changed)
        self.assertEqual(self.record_text(), recorded)
        self.assertEqual(len(self.opener.requests), 1)

    def test_an_archive_the_server_changed_is_refused(self) -> None:
        self.fetch("1")
        recorded = self.record_text()
        (self.out / "xml_usc01@119-142.zip").unlink()
        self.opener.served[URL_1] = TITLE_45
        with self.assertRaises(fetch_uscode.RefusedError) as caught:
            self.fetch("1", moment=LATER)
        self.assertIn("xml_usc01@119-142.zip", str(caught.exception))
        self.assertEqual(self.names(), ["retrieval.json"])
        self.assertEqual(self.record_text(), recorded)

    def test_a_missing_archive_equal_to_its_record_is_restored_with_its_time(self) -> None:
        self.fetch("1")
        recorded = self.record_text()
        (self.out / "xml_usc01@119-142.zip").unlink()
        self.assertEqual(self.fetch("1", moment=LATER), ["fetched xml_usc01@119-142.zip"])
        self.assertEqual((self.out / "xml_usc01@119-142.zip").read_bytes(), TITLE_1)
        self.assertEqual(self.record_text(), recorded)

    def test_an_archive_without_a_record_is_refused(self) -> None:
        self.out.mkdir()
        (self.out / "xml_usc01@119-142.zip").write_bytes(TITLE_45)
        with self.assertRaises(fetch_uscode.RefusedError) as caught:
            self.fetch("1")
        self.assertIn("xml_usc01@119-142.zip", str(caught.exception))
        self.assertEqual((self.out / "xml_usc01@119-142.zip").read_bytes(), TITLE_45)
        self.assertEqual(self.opener.requests, [])
        self.assertEqual(self.names(), ["xml_usc01@119-142.zip"])

    def test_the_record_of_another_release_is_refused(self) -> None:
        self.fetch("1")
        with self.assertRaises(fetch_uscode.RefusedError) as caught:
            fetch_uscode.fetch("119-73not60", ["1"], self.out, self.opener, clock_at(LATER))
        self.assertIn("119-142", str(caught.exception))
        self.assertEqual(len(self.opener.requests), 1)

    def test_a_record_with_an_unknown_field_cannot_be_read(self) -> None:
        self.fetch("1")
        document = json.loads(self.record_text())
        document["archives"][0]["note"] = "an unknown field"
        (self.out / "retrieval.json").write_text(json.dumps(document), encoding="utf-8")
        with self.assertRaises(fetch_uscode.FetchError) as caught:
            self.fetch("1")
        self.assertIn("note", str(caught.exception))

    def test_a_body_that_is_not_an_archive_is_not_kept(self) -> None:
        self.opener.served[URL_1] = b"<html><body>Invented page.</body></html>\n"
        with self.assertRaises(fetch_uscode.FetchError) as caught:
            self.fetch("1")
        self.assertIn(URL_1, str(caught.exception))
        self.assertEqual(self.names(), [])

    def test_a_refused_request_keeps_what_came_before(self) -> None:
        del self.opener.served[URL_45]
        with self.assertRaises(fetch_uscode.FetchError) as caught:
            self.fetch("1", "45")
        self.assertIn(URL_45, str(caught.exception))
        self.assertEqual(self.names(), ["retrieval.json", "xml_usc01@119-142.zip"])
        self.assertEqual(
            self.record_text(),
            record(
                "119-142", entry("xml_usc01@119-142.zip", URL_1, "2026-11-16T22:14:03Z", TITLE_1)
            ),
        )


class CommandLine(unittest.TestCase):
    def run_main(self, opener: FakeOpener, *argv: str) -> tuple[int, str, str]:
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = fetch_uscode.main(list(argv), opener, clock_at(FIRST))
        return status, stdout.getvalue(), stderr.getvalue()

    def test_the_exit_status_says_what_happened(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out = str(Path(directory) / "archives")
            opener = FakeOpener({URL_1: TITLE_1})
            status, stdout, stderr = self.run_main(opener, "119-142", "--out", out, "--title", "1")
            self.assertEqual((status, stdout, stderr), (0, "fetched xml_usc01@119-142.zip\n", ""))
            (Path(out) / "xml_usc01@119-142.zip").write_bytes(TITLE_45)
            status, stdout, stderr = self.run_main(opener, "119-142", "--out", out, "--title", "1")
            self.assertEqual((status, stdout), (1, ""))
            self.assertTrue(stderr.startswith("refused: xml_usc01@119-142.zip"), stderr)
            status, stdout, stderr = self.run_main(opener, "119-142", "--out", out, "--title", "45")
            self.assertEqual((status, stdout), (2, ""))
            self.assertTrue(stderr.startswith(f"cannot fetch: {URL_45}"), stderr)


if __name__ == "__main__":
    unittest.main()
