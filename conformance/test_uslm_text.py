"""Hold the independent text of the Code's official XML to its hand-written fixtures.

Each fixture in `fixtures/uslm/` is one invented document, read as the member
`usc99.xml` (the fixture `two-files` is two members packed into one archive). Its
expected records, `<name>.records.jsonl`, are written by hand from the reading
rules, not by running the script.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

import blake3

HERE = Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures" / "uslm"
TEXT_SCRIPT = HERE / "uslm_text.py"
MEMBER = "usc99.xml"

# One fixture for each case of the reading rules, and what it holds.
CASES = {
    "section": "a section with a heading, a chapeau, three subsections and paragraphs below one",
    "continuation": "continuation after the last child, between two children, inside a content",
    "proviso": "a proviso as a child of a level and inside a content",
    "content": "a content of text only, of several p, of text then a p, of a p holding a table",
    "links": "a link to the Code, a link to a public law, a link whose words are white space",
    "footnotes": "a marker and its footnote, a second marker, one between two spaces, "
    "one at the start of a heading, one in a number",
    "inline": "a superscript, a subscript, a bare superscript beside a footnote of its number, "
    "the numerator of a fraction, dropped typography",
    "table": "a header row, a footer row, column and row spans, an empty cell, one and two p",
    "images": "an image in a unit and one in a note",
    "statuses": "each status of a section, a status on a paragraph and a chapter, none",
    "unit-kinds": "a court rule, a reorganization plan, a generic level that is a unit",
    "containers": "the container kinds of a title nested as the Code nests them, a level",
    "appendix-containers": "an appendix, a compiled act with a title, court rules, an article",
    "repeated-identifiers": "two sections with one identifier, two paragraphs with one",
    "identifier-lists": "a list of two, a range of whole numbers, a range whose ends carry "
    "letters, an alias that is another element's address",
    "no-identifier": "a container, a section and a small level without an identifier",
    "section-in-section": "a section inside a section",
    "no-title-segment": "identifiers without a title segment",
    "no-units": "a file with no units, whose appendix has no identifier",
    "notes-groups": "notes in each of the four groups and before any group",
    "notes-anchors": "a note directly under a section, under a paragraph, under a chapter",
    "not-imported": "the release note under a title, a table of contents with a footnote and a link",
    "notes-content": "a quoted section with a bracketed note, a table, a signature, a second "
    "heading, a proviso directly in a note",
    "notes-addresses": "the law and heading that name a note, ordinals, a note named by its hash",
    "notes-group-headings": "group headings that hold text: one with a link to a law, one with "
    "neither a law nor a heading",
    "source-credit": "a source credit with links and a date, and a container's credit",
    "characters": "U+00A0 in a number, U+202F, U+2000, a composing sequence, U+200B, [sic]",
    "empty-labels": "a quoted section with an empty number, a number with a value and no text, "
    "an empty heading, an empty source credit",
    "notes-unknown-group": "a group heading with a group word the rules do not know",
}
TWO_FILES = "two-files"


def run_script(script: Path, path: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(script), str(path), *args],
        capture_output=True,
        check=False,
        encoding="utf-8",
    )


def fixture_input(name: str, directory: Path) -> Path:
    """The fixture as the scripts read it: a member file, or for two files an archive."""
    if name == TWO_FILES:
        release = directory / "release"
        release.mkdir()
        with zipfile.ZipFile(release / "two-files.zip", "w", zipfile.ZIP_DEFLATED) as archive:
            for member in ("usc99.xml", "usc99a.xml"):
                archive.write(FIXTURES / TWO_FILES / member, member)
        return release
    target = directory / MEMBER
    shutil.copyfile(FIXTURES / f"{name}.xml", target)
    return target


def expected_lines(name: str) -> list[str]:
    return (FIXTURES / f"{name}.records.jsonl").read_text(encoding="utf-8").splitlines()


class TextFixtures(unittest.TestCase):
    def text_of(self, name: str, *args: str) -> list[str]:
        with tempfile.TemporaryDirectory(prefix="fidryn-uslm-text-") as directory:
            result = run_script(TEXT_SCRIPT, fixture_input(name, Path(directory)), *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        return result.stdout.split("\n")[:-1] if result.stdout else []

    def check(self, name: str) -> None:
        self.assertEqual(self.text_of(name), expected_lines(name))

    def test_section(self) -> None:
        self.check("section")

    def test_continuation(self) -> None:
        self.check("continuation")

    def test_proviso(self) -> None:
        self.check("proviso")

    def test_content(self) -> None:
        self.check("content")

    def test_links(self) -> None:
        self.check("links")

    def test_footnotes(self) -> None:
        self.check("footnotes")

    def test_inline(self) -> None:
        self.check("inline")

    def test_table(self) -> None:
        self.check("table")

    def test_images(self) -> None:
        self.check("images")

    def test_statuses(self) -> None:
        self.check("statuses")

    def test_unit_kinds(self) -> None:
        self.check("unit-kinds")

    def test_containers(self) -> None:
        self.check("containers")

    def test_appendix_containers(self) -> None:
        self.check("appendix-containers")

    def test_repeated_identifiers(self) -> None:
        self.check("repeated-identifiers")

    def test_identifier_lists(self) -> None:
        self.check("identifier-lists")

    def test_no_identifier(self) -> None:
        self.check("no-identifier")

    def test_section_in_section(self) -> None:
        self.check("section-in-section")

    def test_no_title_segment(self) -> None:
        self.check("no-title-segment")

    def test_no_units(self) -> None:
        self.check("no-units")

    def test_notes_groups(self) -> None:
        self.check("notes-groups")

    def test_notes_anchors(self) -> None:
        self.check("notes-anchors")

    def test_not_imported(self) -> None:
        self.check("not-imported")

    def test_notes_content(self) -> None:
        self.check("notes-content")

    def test_notes_addresses(self) -> None:
        self.check("notes-addresses")

    def test_notes_group_headings(self) -> None:
        self.check("notes-group-headings")

    def test_source_credit(self) -> None:
        self.check("source-credit")

    def test_characters(self) -> None:
        self.check("characters")

    def test_empty_labels(self) -> None:
        self.check("empty-labels")

    def test_notes_unknown_group(self) -> None:
        self.check("notes-unknown-group")

    def test_two_files_are_numbered_over_the_release_for_any_number_of_workers(self) -> None:
        expected = expected_lines(TWO_FILES)
        self.assertIn('"address":"/us/usc/t99/s1~2"', expected[2])
        for jobs in ("1", "2"):
            with self.subTest(jobs=jobs):
                self.assertEqual(self.text_of(TWO_FILES, "--jobs", jobs), expected)

    def test_every_fixture_is_tested(self) -> None:
        names = {path.stem for path in FIXTURES.glob("*.xml")}
        self.assertEqual(names, set(CASES))
        for name in [*CASES, TWO_FILES]:
            self.assertTrue((FIXTURES / f"{name}.records.jsonl").is_file(), name)
            self.assertTrue((FIXTURES / f"{name}.profile.json").is_file(), name)
        for path in [*FIXTURES.glob("*.xml"), *(FIXTURES / TWO_FILES).glob("*.xml")]:
            first = path.read_text(encoding="utf-8").split("\n", 1)[0]
            self.assertIn("Invented for tests", first, path.name)

    def test_a_note_with_neither_law_nor_heading_is_named_by_its_hash(self) -> None:
        # The note hash, written out frame by frame: one tag byte, the length of the
        # value as eight bytes little-endian, the value.
        #   00 | 0e 00 00 00 00 00 00 00 | "fidryn.note/v1"        domain
        #   70 | 0e 00 00 00 00 00 00 00 | "statutoryNotes"        group
        #   71 | 0d 00 00 00 00 00 00 00 | "miscellaneous"         topics
        #   1f | 00 00 00 00 00 00 00 00 |                         no heading
        #   20 | 25 00 00 00 00 00 00 00 | the text below          a paragraph
        #        10 | 1c 00 00 00 00 00 00 00 | "Neither a law nor a heading."
        frames = (
            bytes([0x00, 0x0E, 0, 0, 0, 0, 0, 0, 0])
            + b"fidryn.note/v1"
            + bytes([0x70, 0x0E, 0, 0, 0, 0, 0, 0, 0])
            + b"statutoryNotes"
            + bytes([0x71, 0x0D, 0, 0, 0, 0, 0, 0, 0])
            + b"miscellaneous"
            + bytes([0x1F, 0, 0, 0, 0, 0, 0, 0, 0])
            + bytes([0x20, 0x25, 0, 0, 0, 0, 0, 0, 0])
            + bytes([0x10, 0x1C, 0, 0, 0, 0, 0, 0, 0])
            + b"Neither a law nor a heading."
        )
        address = (
            "/us/usc/t99/s1/note/statutoryNotes/nolaw/h." + blake3.blake3(frames).hexdigest()[:16]
        )
        records = [json.loads(line) for line in expected_lines("notes-addresses")]
        named = [r for r in records if r["address"] == address]
        self.assertEqual(len(named), 1, address)
        self.assertIsNone(named[0]["heading"])
        self.assertEqual(named[0]["blocks"][0]["p"], "Neither a law nor a heading.")

    def test_the_text_a_group_heading_holds_is_a_note_named_by_its_hash(self) -> None:
        # The statutory group heading of the fixture holds one paragraph and gives a
        # note with no heading and no topics:
        #   00 | 0e 00 00 00 00 00 00 00 | "fidryn.note/v1"        domain
        #   70 | 0e 00 00 00 00 00 00 00 | "statutoryNotes"        group
        #   71 | 00 00 00 00 00 00 00 00 |                         no topics
        #   1f | 00 00 00 00 00 00 00 00 |                         no heading
        #   20 | 22 00 00 00 00 00 00 00 | the text below          a paragraph
        #        10 | 19 00 00 00 00 00 00 00 | "These notes are invented."
        frames = (
            bytes([0x00, 0x0E, 0, 0, 0, 0, 0, 0, 0])
            + b"fidryn.note/v1"
            + bytes([0x70, 0x0E, 0, 0, 0, 0, 0, 0, 0])
            + b"statutoryNotes"
            + bytes([0x71, 0, 0, 0, 0, 0, 0, 0, 0])
            + bytes([0x1F, 0, 0, 0, 0, 0, 0, 0, 0])
            + bytes([0x20, 0x22, 0, 0, 0, 0, 0, 0, 0])
            + bytes([0x10, 0x19, 0, 0, 0, 0, 0, 0, 0])
            + b"These notes are invented."
        )
        address = (
            "/us/usc/t99/s1/note/statutoryNotes/nolaw/h." + blake3.blake3(frames).hexdigest()[:16]
        )
        records = [json.loads(line) for line in expected_lines("notes-group-headings")]
        named = [r for r in records if r["address"] == address]
        self.assertEqual(len(named), 1, address)
        self.assertEqual(named[0]["topics"], [])
        self.assertIsNone(named[0]["heading"])

    def test_text_records_are_sorted_and_match_the_schema_shape(self) -> None:
        for name in [*CASES, TWO_FILES]:
            with self.subTest(fixture=name):
                records = [json.loads(line) for line in self.text_of(name)]
                keys = [record_key(r) for r in records]
                self.assertEqual(keys, sorted(keys))
                for record in records:
                    check_record(self, record)


# The scripts whose reading the refusal test holds to the rules.
SCRIPTS = [TEXT_SCRIPT, HERE / "usc_profile.py"]

DOCUMENT = """<!-- Invented for tests: this document is not the text of any law. -->
{doctype}<{root} xmlns="{namespace}" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" \
xsi:schemaLocation="http://xml.house.gov/schemas/uslm/1.0 {schema}">
<meta><docPublicationName>{release}</docPublicationName>\
<property role="is-positive-law">{flag}</property></meta>
<main><title identifier="/us/usc/t99"><num value="99">Title 99</num><heading>Invented</heading>
{body}
</title></main>
</{root}>
"""
SECTION = (
    '<section identifier="/us/usc/t99/s1"><num value="1">§ 1.</num><heading>Refused</heading>'
    "{inner}</section>"
)


def document(**changes: str) -> str:
    fields = {
        "doctype": "",
        "root": "uscDoc",
        "namespace": "http://xml.house.gov/schemas/uslm/1.0",
        "schema": "USLM-1.0.15.xsd",
        "release": "Online@999-1",
        "flag": "yes",
        "body": SECTION.format(inner=changes.pop("inner", "<content>Text.</content>")),
    }
    fields.update(changes)
    return DOCUMENT.format(**fields)


def archive(directory: Path, name: str, members: dict[str, str]) -> None:
    with zipfile.ZipFile(directory / name, "w", zipfile.ZIP_DEFLATED) as opened:
        for member, text in members.items():
            opened.writestr(member, text)


def oversized_archive(directory: Path) -> None:
    """An archive whose one member is 513 MiB of zeros, which deflate to little."""
    chunk = bytes(1024 * 1024)
    path = directory / "large.zip"
    with (
        zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED, compresslevel=1) as opened,
        opened.open(MEMBER, "w", force_zip64=True) as stream,
    ):
        for _ in range(513):
            stream.write(chunk)


DOCUMENT_REFUSALS: dict[str, tuple[str, str]] = {
    "document type declaration": (document(doctype="<!DOCTYPE uscDoc>\n"), "document type"),
    "entity declaration": (
        document(doctype='<!DOCTYPE uscDoc [<!ENTITY w "widget">]>\n'),
        "declaration",
    ),
    "root that is not uscDoc": (document(root="uscDocument"), "not uscDoc"),
    "root in another namespace": (document(namespace="http://example.invalid/other"), "not uscDoc"),
    "schema location naming another file": (document(schema="USLM-1.0.14.xsd"), "schema"),
    "publication name not beginning Online@": (document(release="Print@999-1"), "Online@"),
    "positive-law flag that is neither yes nor no": (document(flag="maybe"), "is-positive-law"),
    "element in no class": (document(inner="<widget>Text.</widget>"), "does not stand"),
    "element where its class does not stand": (
        document(
            body='<subsection identifier="/us/usc/t99/s1/a"><content>Text.</content></subsection>'
        ),
        "does not stand in a container",
    ),
    "top element that is not a title": (
        document(body="")
        .replace("<main><title", "<main><chapter")
        .replace("</title></main>", "</chapter></main>"),
        "not a title",
    ),
    "ref with neither an href nor the class footnoteRef": (
        document(inner="<content>See <ref>this</ref>.</content>"),
        "neither an href nor the class footnoteRef",
    ),
    "status that is not one of the schema's": (
        document(body=SECTION.format(inner="").replace("<section ", '<section status="gone" ')),
        "status",
    ),
    "img with another attribute": (
        document(inner='<content><img src="file:///a.eps" alt="a"/></content>'),
        "attribute other than src",
    ),
    "img with text": (
        document(inner='<content><img src="file:///a.eps">A picture</img></content>'),
        "holds text",
    ),
    "marker whose idref names no footnote": (
        document(inner='<content>Text<ref class="footnoteRef" idref="none">1</ref>.</content>'),
        "names no footnote",
    ),
    "note citing a law in fewer than four segments": (
        document(
            inner='<notes type="uscNote"><note topic="miscellaneous"><heading>Odd</heading>'
            '<p><ref href="/us/pl/999">Pub. L. 999</ref></p></note></notes>'
        ),
        "four segments",
    ),
    "cross heading with no topic": (
        document(
            inner='<notes type="uscNote"><note role="crossHeading"><heading>Notes</heading>'
            "</note></notes>"
        ),
        "not one word of ASCII letters",
    ),
    "cross heading whose topic is not one word": (
        document(
            inner='<notes type="uscNote"><note role="crossHeading" topic="editorial-notes">'
            "<heading>Notes</heading></note></notes>"
        ),
        "not one word of ASCII letters",
    ),
    "second source credit": (
        document(inner="<sourceCredit>(One.)</sourceCredit><sourceCredit>(Two.)</sourceCredit>"),
        "second source credit",
    ),
    "source credit that is not the direct child of its unit": (
        document(
            inner='<subsection identifier="/us/usc/t99/s1/a"><content>Text.</content>'
            "<sourceCredit>(One.)</sourceCredit></subsection>"
        ),
        "not the direct child",
    ),
}


class Refusals(unittest.TestCase):
    def refused(self, path: Path, phrase: str) -> None:
        for script in SCRIPTS:
            with self.subTest(script=script.name):
                result = run_script(script, path)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertTrue(result.stderr.startswith("refused: "), result.stderr)
                self.assertIn(phrase, result.stderr)
                self.assertEqual(result.stdout, "")

    def test_the_scripts_refuse_what_the_rules_refuse(self) -> None:
        with tempfile.TemporaryDirectory(prefix="fidryn-uslm-refused-") as name:
            directory = Path(name)
            for label, (text, phrase) in DOCUMENT_REFUSALS.items():
                with self.subTest(refusal=label):
                    path = directory / MEMBER
                    path.write_text(text, encoding="utf-8")
                    self.refused(path, phrase)
            for member in ("x.txt", "dir/usc99.xml", "../usc99.xml", "usc999.xml"):
                with self.subTest(refusal="member name", member=member):
                    case = directory / member.replace("/", "_").replace(".", "_")
                    case.mkdir()
                    archive(case, "a.zip", {member: document()})
                    self.refused(case, "is not named usc")
            with self.subTest(refusal="a member two archives hold"):
                case = directory / "twice"
                case.mkdir()
                archive(case, "a.zip", {MEMBER: document()})
                archive(case, "b.zip", {MEMBER: document()})
                self.refused(case, "held by two archives")
            with self.subTest(refusal="a member whose release differs from the first's"):
                case = directory / "releases"
                case.mkdir()
                other = document(release="Online@999-2").replace("t99", "t99a")
                archive(case, "a.zip", {MEMBER: document(), "usc99a.xml": other})
                self.refused(case, "the first member says 999-1")
            with self.subTest(refusal="a member larger than 512 MiB"):
                case = directory / "large"
                case.mkdir()
                oversized_archive(case)
                self.refused(case, "larger than 512 MiB")


def record_key(record: dict[str, object]) -> tuple[bytes, int, int]:
    address = record["address"]
    ordinal = record.get("ordinal", 0)
    if not isinstance(address, str) or not isinstance(ordinal, int):
        raise AssertionError(record)
    rank = {"credit": 1, "footnote": 2}.get(str(record.get("record")), 0)
    return address.encode("utf-8"), rank, ordinal


PROVISION_KEYS = ["address", "layer", "kind", "status", "num", "heading", "items"]
UNIT_KEYS = [*PROVISION_KEYS, "aliases", "within", "source", "positive_law"]
CONTAINER_KEYS = [
    "address", "layer", "kind", "status", "num", "heading", "aliases", "within", "members",
]  # fmt: skip
NOTE_KEYS = ["address", "layer", "group", "topics", "heading", "blocks"]
FOOTNOTE_KEYS = ["address", "record", "ordinal", "shown", "blocks"]


def check_record(case: unittest.TestCase, record: dict[str, object]) -> None:
    keys = list(record)
    if record.get("record") == "credit":
        case.assertEqual(keys[:2], ["address", "record"])
        check_text(case, {k: record[k] for k in keys[2:]}, "p")
    elif record.get("record") == "footnote":
        case.assertEqual(keys, FOOTNOTE_KEYS)
        case.assertIsInstance(record["shown"], str)
        check_blocks(case, record["blocks"])
    elif "group" in record:
        case.assertEqual(keys, NOTE_KEYS)
        case.assertIn(record["layer"], ("enacted", "editorial"))
        case.assertIsInstance(record["topics"], list)
        check_optional_text(case, record["heading"])
        check_blocks(case, record["blocks"])
    else:
        case.assertIn(keys, (PROVISION_KEYS, UNIT_KEYS, CONTAINER_KEYS))
        case.assertEqual(record["layer"], "operative")
        case.assertIsInstance(record["kind"], str)
        case.assertTrue(record["status"] is None or isinstance(record["status"], str))
        check_num(case, record["num"])
        check_optional_text(case, record["heading"])
        if keys == CONTAINER_KEYS:
            case.assertIsInstance(record["members"], list)
        else:
            items = record["items"]
            if not isinstance(items, list):
                raise AssertionError(items)
            for item in items:
                if "child" in item:
                    case.assertEqual(list(item), ["child"])
                else:
                    check_part(case, item)
        if keys == UNIT_KEYS:
            case.assertIsInstance(record["source"], str)
            case.assertIsInstance(record["positive_law"], bool)


def check_text(case: unittest.TestCase, value: object, key: str = "text") -> None:
    if not isinstance(value, dict):
        raise AssertionError(value)
    keys = list(value)
    case.assertIn(keys, ([key, "refs", "sup", "sub"], [key, "refs", "sup", "sub", "marks"]))
    plain = value[key]
    if not isinstance(plain, str):
        raise AssertionError(plain)
    case.assertEqual(plain, " ".join(plain.split(" ")).strip(" "))
    for start, end, target in value["refs"]:
        case.assertTrue(0 <= start < end <= len(plain) and isinstance(target, str))
    for start, end in [*value["sup"], *value["sub"]]:
        case.assertTrue(0 <= start < end <= len(plain))
    for offset, shown, footnote in value.get("marks", []):
        case.assertTrue(0 <= offset <= len(plain) and isinstance(shown, str) and footnote >= 1)


def check_optional_text(case: unittest.TestCase, value: object) -> None:
    if value is not None:
        check_text(case, value)


def check_num(case: unittest.TestCase, value: object) -> None:
    if value is None:
        return
    if not isinstance(value, list) or len(value) != 2 or not isinstance(value[0], str):
        raise AssertionError(value)
    if not isinstance(value[1], str):
        check_text(case, value[1])


def check_part(case: unittest.TestCase, value: object) -> None:
    if not isinstance(value, dict):
        raise AssertionError(value)
    case.assertEqual(list(value), ["part", "blocks"])
    case.assertIn(value["part"], ("chapeau", "content", "continuation", "proviso"))
    case.assertTrue(value["blocks"])
    check_blocks(case, value["blocks"])


def check_blocks(case: unittest.TestCase, blocks: object) -> None:
    if not isinstance(blocks, list):
        raise AssertionError(blocks)
    for block in blocks:
        check_block(case, block)


def check_block(case: unittest.TestCase, block: object) -> None:
    if not isinstance(block, dict):
        raise AssertionError(block)
    first = next(iter(block))
    if first in ("p", "subheading"):
        check_text(case, block, first)
    elif first == "image":
        case.assertEqual(list(block), ["image"])
    elif first == "table":
        case.assertEqual(list(block), ["table"])
        for group, cells in block["table"]:
            case.assertIn(group, ("head", "body", "foot"))
            for header, colspan, rowspan, inner in cells:
                case.assertIsInstance(header, bool)
                case.assertTrue(colspan >= 1 and rowspan >= 1)
                check_blocks(case, inner)
    elif first == "quoted":
        case.assertEqual(list(block), ["quoted"])
        case.assertEqual(list(block["quoted"]), ["origin", "items"])
        check_quoted_items(case, block["quoted"]["items"])
    else:
        raise AssertionError(block)


def check_quoted_items(case: unittest.TestCase, items: object) -> None:
    if not isinstance(items, list):
        raise AssertionError(items)
    for item in items:
        if "part" in item:
            check_part(case, item)
        elif "level" in item:
            case.assertEqual(list(item), ["level", "num", "heading", "items"])
            check_num(case, item["num"])
            check_optional_text(case, item["heading"])
            check_quoted_items(case, item["items"])
        else:
            check_block(case, item)


if __name__ == "__main__":
    unittest.main()
