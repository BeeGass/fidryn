"""Hold the second implementation of the definition rules to the cases of the importer's tests.

The documents are the invented fixtures `fixtures/uslm/definitions-*.xml`, which the
tests of `fidryn-ingest-uscode` read, and small invented documents of the title 99
written here. The definitions each one gives are those that the importer's tests
expect of the same document, written out by hand. The workspaces that `--against`
compares with are written here too, their `definition` blocks in the canonical form.
"""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import usc_definitions
from test_uslm_text import FIXTURES, HERE, MEMBER, fixture_input, run_script

SCRIPT = HERE / "usc_definitions.py"

S1 = "/us/usc/t99/s1"
S2 = "/us/usc/t99/s2"
S4 = "/us/usc/t99/s4"
S6 = "/us/usc/t99/s6"
CHAPTER = "/us/usc/t99/ch1"
NOTE_OF_S1 = "/us/usc/t99/s1/note/editorialNotes/nolaw/references-in-text"
NOTE_OF_S4 = "/us/usc/t99/s4/note/editorialNotes/nolaw/references-in-text"
FLAG_NO = '<property role="is-positive-law">no</property>'
FLAG_YES = '<property role="is-positive-law">yes</property>'
EN_DASH = chr(0x2013)
PUBLIC_LAW = f"Public Law 99{EN_DASH}1"

Record = dict[str, object]


def definition(
    at: str,
    terms: tuple[str, ...] = ("widget",),
    kind: str = "means",
    words: str | None = None,
    region: str | None = None,
    basis: str = "unresolved",
    borrowed_from: str | None = None,
    see_note: str | None = None,
) -> Record:
    """A definition as a line of `--records` holds it."""
    return {
        "at": at,
        "terms": list(terms),
        "kind": kind,
        "words": words,
        "region": region,
        "basis": basis,
        "borrowed_from": borrowed_from,
        "see_note": see_note,
    }


# The rows of the importer's table of cases, as `definitions-sentence.xml` holds them, in
# the order of the records. Section 3 holds the paragraphs that define nothing: one
# that borrows with no link, one whose text before `the words` is no reach, one that
# opens with `In` and a condition, two that define a second term, one with the sentence
# form in its second sentence, two whose aside speaks of a reach, one with a reach after
# its verb, one with no reach that speaks of one, and quoted content and a note that
# hold the sentence form.
SENTENCE = [
    definition(f"{S1}/a", words="As used in this section", region=S1, basis="enacted"),
    definition(f"{S1}/b", ("widget", "gadget")),
    definition(
        f"{S1}/c",
        kind="includes",
        words="For purposes of this chapter",
        region=CHAPTER,
        basis="enacted",
    ),
    definition(f"{S1}/d", kind="does_not_include"),
    definition(f"{S1}/e", kind="borrowed", borrowed_from=S2),
    definition(f"{S1}/f", words="For purposes of this Act"),
    definition(f"{S1}/g", words="As used in section 2 of this title", region=S2, basis="enacted"),
    definition(f"{S1}/h", words=f"As used in {PUBLIC_LAW}"),
    definition(f"{S1}/i", words="For the purpose of this section", region=S1, basis="enacted"),
    definition(f"{S1}/j"),
]

# The rows of the table as `definitions-chapeau.xml` holds them: reach chapeaus with and
# without a lead-in, a chapeau above a provision with a heading, chapeaus that state no
# reach the rules read (subsections 1(b) and 5(a) to 5(c)), and two definitions in the
# parts of one provision.
CHAPEAU = [
    definition(f"{S1}/a/1", words="In this chapter", region=CHAPTER, basis="enacted"),
    definition(f"{S1}/a/2", ("gadget",), "includes", "In this chapter", CHAPTER, "enacted"),
    definition(f"{S1}/c/1", words="As used in this chapter", region=CHAPTER, basis="enacted"),
    definition(f"{S2}/1", words="In this section", region=S2, basis="enacted"),
    definition(
        "/us/usc/t99/s3/a/1/A",
        words="For the purpose of this chapter",
        region=CHAPTER,
        basis="enacted",
    ),
    definition(f"{S4}/a/1", words="In this section", region=S4, basis="enacted"),
    definition(f"{S4}/b/1", words="In this section", region=S4, basis="enacted"),
    definition(
        f"{S4}/c/1",
        words="In this chapter, unless the context otherwise requires",
        region=CHAPTER,
        basis="enacted",
    ),
    definition(f"{S4}/d/1", words="In this chapter and chapter 5"),
    definition(f"{S4}/e/1", words="In this subsection", region=f"{S4}/e", basis="enacted"),
    definition(
        f"{S6}/a", ("apple",), words="For purposes of this section", region=S6, basis="enacted"
    ),
    definition(
        f"{S6}/a", ("zebra",), words="For purposes of this section", region=S6, basis="enacted"
    ),
    definition(f"{S6}/b", kind="includes"),
    definition(f"{S6}/b"),
]


def restated(bases: tuple[str, str, str, str, str], notes: tuple[str | None, ...]) -> list[Record]:
    """The definitions of `definitions-restated.xml`, with each basis and note given."""
    found = [
        definition(S1, words="In this chapter", region=CHAPTER, basis=bases[0]),
        definition(S2, ("gadget",), words="In this section", region=S2, basis=bases[1]),
        definition("/us/usc/t99/s3", ("sprocket",), basis=bases[2]),
        definition(
            S4, ("cog",), words="As used in section 2 of this title", region=S2, basis=bases[3]
        ),
        definition(
            "/us/usc/t99/s5", ("gear",), words="In this chapter", region=CHAPTER, basis=bases[4]
        ),
    ]
    for record, note in zip(found, notes, strict=True):
        record["see_note"] = note
    return found


RESTATED_BASES = ("restated", "restated", "unresolved", "restated", "restated")
RESTATED_NOTES = (NOTE_OF_S1, None, None, NOTE_OF_S4, None)


def fixture_text(name: str) -> str:
    return (FIXTURES / f"{name}.xml").read_text(encoding="utf-8")


def with_section(inner: str) -> str:
    """An invented document of the title 99 whose positive-law section 1 holds `inner`."""
    return (
        "<!-- Invented for tests: this document is not the text of any law. -->\n"
        '<uscDoc xmlns="http://xml.house.gov/schemas/uslm/1.0" '
        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
        'xsi:schemaLocation="http://xml.house.gov/schemas/uslm/1.0 USLM-1.0.15.xsd">\n'
        f"<meta><docPublicationName>Online@999-1</docPublicationName>{FLAG_YES}</meta>\n"
        '<main><title identifier="/us/usc/t99"><num value="99">Title 99</num>'
        "<heading>Invented</heading>\n"
        f'<section identifier="{S1}"><num value="1">§ 1.</num><heading>Invented</heading>'
        f"{inner}</section>\n</title></main>\n</uscDoc>\n"
    )


def defined(text: str) -> list[Record]:
    """The definitions that the rules find in the document `text`, read in this process."""
    with tempfile.TemporaryDirectory(prefix="fidryn-usc-definitions-") as directory:
        path = Path(directory) / MEMBER
        path.write_text(text, encoding="utf-8")
        _, found = usc_definitions.find_definitions(path, 1)
    return [one.record() for one in found]


def content(words: str) -> str:
    return f"<content>{words}</content>"


def under(chapeau: str, child: str) -> str:
    """A chapeau of section 1, and below it paragraph (1) whose content is `child`."""
    return (
        f'<chapeau>{chapeau}</chapeau><paragraph identifier="{S1}/1">'
        f'<num value="1">(1)</num><content>{child}</content></paragraph>'
    )


SMALL_DEVICE = "The term “widget” means a small device."


def widget(
    at: str = S1, words: str | None = None, region: str | None = None, basis: str = "unresolved"
) -> list[Record]:
    return [definition(at, words=words, region=region, basis=basis)]


# Paragraphs of section 1 of an invented document, and what the rules find in each: the
# cases of the importer's test of the words of the rules, and further cases of the same
# rules.
WORDS_OF_THE_RULES: list[tuple[str, list[Record]]] = [
    # A verb form stands before the end or a character that is not a letter, and a term
    # has at least one character.
    (content("The term “widget” meanest a small device."), []),
    (content("The term “” means a small device."), []),
    # Where the rules say "in any case", in any case, and words are whole words.
    (content("The term “widget” means a device AS USED IN commerce."), []),
    (content("The term “widget”, In the case of a gadget, means a small device."), []),
    (content("The term “widget” means, For Purposes Of this section, a device."), []),
    (content("For purposes of the Terms below, the term “widget” means a device."), []),
    (
        content("For purposes of this termination, the term “widget” means a device."),
        widget(words="For purposes of this termination"),
    ),
    (content("The term “widget”, of gas used in commerce, means a device."), widget()),
    (content("The term “widget” means a device of gas used in commerce."), widget()),
    (content("The term “widget” means a device as used insofar as it is small."), widget()),
    # Elsewhere, exactly.
    (content("for purposes of this section, the term “widget” means a device."), []),
    (content("The Term “widget” means a small device."), []),
    (content("The term “widget” Means a small device."), []),
    # After In, the object is `this` and a space, or a single link.
    (
        content(f'In <ref href="{S2}">section 2</ref>, the term “widget” means a device.'),
        widget(words="In section 2", region=S2, basis="enacted"),
    ),
    (
        content(
            f'In <ref href="{S2}">section 2</ref> and section 3, the term “widget” means a device.'
        ),
        [],
    ),
    (
        content(
            f'As used in <ref href="{S2}">section 2</ref> and section 3, the term “widget” '
            "means a device."
        ),
        widget(words="As used in section 2 and section 3"),
    ),
    (
        content(f'<ref href="/us/pl/99/1">{PUBLIC_LAW}</ref>, the term “widget” means a device.'),
        [],
    ),
    (
        content(
            f'In <ref href="/us/pl/99/1">{PUBLIC_LAW}</ref>, the term “widget” means a device.'
        ),
        widget(words=f"In {PUBLIC_LAW}"),
    ),
    # A link to the prefix of the corpus names no region below it.
    (
        content('As used in <ref href="/us/usc">the Code</ref>, the term “widget” means a device.'),
        widget(words="As used in the Code"),
    ),
    # An object that begins with `this` and is not `this` and a kind of region is read
    # as any other: a single link below the prefix names its target; and `this` and a
    # kind names the nearest of that kind, whatever its link names.
    (
        content(f'As used in <ref href="{S2}">this Act</ref>, the term “widget” means a device.'),
        widget(words="As used in this Act", region=S2, basis="enacted"),
    ),
    (
        content(
            f'As used in <ref href="{S2}">this section</ref>, the term “widget” means a device.'
        ),
        widget(words="As used in this section", region=S1, basis="enacted"),
    ),
    # The word of a region is followed by nothing more, or by a comma, a space and
    # `unless the context`.
    (
        content(
            "In this section, unless the context requires otherwise, the term “widget” means "
            "a device."
        ),
        widget(
            words="In this section, unless the context requires otherwise",
            region=S1,
            basis="enacted",
        ),
    ),
    (
        content(
            "For purposes of this section, except as provided, the term “widget” means a device."
        ),
        widget(words="For purposes of this section, except as provided"),
    ),
    # A close is a dash, a hyphen or a colon, with at most one space before it; an en
    # dash is none.
    (under("In this section :", SMALL_DEVICE), widget(f"{S1}/1", "In this section", S1, "enacted")),
    (under("In this section-", SMALL_DEVICE), widget(f"{S1}/1", "In this section", S1, "enacted")),
    (under(f"In this section{EN_DASH}", SMALL_DEVICE), []),
    (
        under("In this section, the terms—", "“widget” means a small device."),
        widget(f"{S1}/1", "In this section", S1, "enacted"),
    ),
    # A chapeau of two parts is no reach chapeau.
    (
        "<chapeau>In this section:</chapeau><chapeau>Also:</chapeau>"
        f'<paragraph identifier="{S1}/1"><num value="1">(1)</num>'
        f"<content>{SMALL_DEVICE}</content></paragraph>",
        [],
    ),
    # A paragraph of the list form under no chapeau is not extracted.
    (content("“widget” means a small device."), []),
    # A borrowed definition takes the first link that begins after its verb, and is not
    # extracted when that link has no target.
    (
        content(
            f'As used in <ref href="{S2}">section 2</ref>, the term “widget” has the meaning '
            f'given such term in <ref href="/us/usc/t99/s3">section 3</ref>.'
        ),
        [
            definition(
                S1,
                kind="borrowed",
                words="As used in section 2",
                region=S2,
                basis="enacted",
                borrowed_from="/us/usc/t99/s3",
            )
        ],
    ),
    (
        content(
            'The term “widget” has the meaning given such term in <ref href="">section 2'
            f'</ref> and <ref href="{S2}">section 2</ref>.'
        ),
        [],
    ),
]


class FixtureRows(unittest.TestCase):
    """The script, run as its command line runs it, on the three fixtures."""

    def run_on(self, name: str) -> tuple[subprocess.CompletedProcess[str], str]:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-definitions-") as directory:
            records = Path(directory) / "definitions.jsonl"
            result = run_script(
                SCRIPT, fixture_input(name, Path(directory)), "--records", str(records)
            )
            written = records.read_text(encoding="utf-8") if records.exists() else ""
        return result, written

    def records_of(self, name: str) -> list[Record]:
        result, written = self.run_on(name)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        records = [json.loads(line) for line in written.splitlines()]
        self.assertEqual(result.stdout, f"{len(records)} definitions in the release 999-1\n")
        return records

    def test_the_sentence_form_has_a_reach_of_its_own_or_none(self) -> None:
        self.assertEqual(self.records_of("definitions-sentence"), SENTENCE)

    def test_a_chapeau_that_states_a_reach_gives_it_to_the_paragraphs_below(self) -> None:
        self.assertEqual(self.records_of("definitions-chapeau"), CHAPEAU)

    def test_a_restated_definition_points_at_its_note(self) -> None:
        self.assertEqual(
            self.records_of("definitions-restated"), restated(RESTATED_BASES, RESTATED_NOTES)
        )

    def test_the_records_are_compact_json_sorted_by_address_and_line(self) -> None:
        result, written = self.run_on("definitions-sentence")
        self.assertEqual(result.returncode, 0, result.stderr)
        expected = "".join(
            json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n"
            for record in SENTENCE
        )
        self.assertEqual(written, expected)
        # A character outside ASCII is written as it is.
        self.assertIn(f'"As used in {PUBLIC_LAW}"', written)
        lines = written.splitlines()
        self.assertEqual(
            lines,
            sorted(lines, key=lambda line: (json.loads(line)["at"].encode(), line.encode())),
        )


class Rules(unittest.TestCase):
    """The rules, read in this process, on documents that differ from a fixture in one place."""

    def test_the_words_of_the_rules_are_read_as_they_say(self) -> None:
        for inner, expected in WORDS_OF_THE_RULES:
            with self.subTest(inner=inner):
                self.assertEqual(defined(with_section(inner)), expected)

    def test_basis_follows_the_files_flag(self) -> None:
        text = fixture_text("definitions-restated")
        self.assertIn(FLAG_NO, text)
        expected = restated(RESTATED_BASES, RESTATED_NOTES)
        self.assertEqual(defined(text), expected)
        # A file without the flag declares no positive law either.
        self.assertEqual(defined(text.replace(FLAG_NO, "")), expected)
        # With the flag on, a definition with a region is enacted and points at no note.
        self.assertEqual(
            defined(text.replace(FLAG_NO, FLAG_YES)),
            restated(("enacted", "enacted", "unresolved", "enacted", "enacted"), (None,) * 5),
        )

    def test_the_references_in_text_note_is_linked(self) -> None:
        text = fixture_text("definitions-restated")

        def see_notes(text: str) -> list[object]:
            return [record["see_note"] for record in defined(text)]

        # Section 2's note names the chapter, not the section; section 5's words stand
        # in quoted content and in a note of the statutory group.
        self.assertEqual(see_notes(text), list(RESTATED_NOTES))
        first = "<p>This chapter, referred to in text, was"
        self.assertIn(first, text)
        # The words are compared in any case.
        upper = text.replace(first, "<p>THIS CHAPTER, REFERRED TO IN text, was")
        self.assertEqual(see_notes(upper)[0], NOTE_OF_S1)
        # The first note that holds them: here the second one.
        self.assertEqual(
            see_notes(text.replace(first, "<p>This chapter was"))[0], f"{NOTE_OF_S1}/2"
        )
        # The words are whole words.
        self.assertEqual(
            see_notes(text.replace(first, "<p>This chapter, referred to infra, was"))[0],
            f"{NOTE_OF_S1}/2",
        )
        # A note with another heading is not read.
        heading = "<heading>References in Text</heading><p>Section 2 of this title"
        self.assertIn(heading, text)
        changed = text.replace(heading, "<heading>Codification</heading><p>Section 2 of this title")
        self.assertIsNone(see_notes(changed)[3])

    def test_the_first_note_is_the_first_in_the_document(self) -> None:
        def notes(which: str) -> str:
            return (
                '<notes type="uscNote"><note role="crossHeading" topic="editorialNotes">'
                "<heading>Editorial Notes</heading></note>"
                '<note topic="referencesInText"><heading>References in Text</heading>'
                f"<p>This section, referred to in text, is the {which} note.</p></note></notes>"
            )

        # The section's note stands before subsection (a), which has a note of its own.
        inner = (
            "<chapeau>In this section, the term “gear” means a wheel with teeth.</chapeau>"
            f'{notes("first")}<subsection identifier="{S1}/a"><num value="a">(a)</num>'
            f"<content>Gears turn.</content>{notes('second')}</subsection>"
        )
        self.assertEqual(
            defined(with_section(inner).replace(FLAG_YES, FLAG_NO)),
            [
                definition(
                    S1,
                    ("gear",),
                    words="In this section",
                    region=S1,
                    basis="restated",
                    see_note=f"{S1}/note/editorialNotes/nolaw/references-in-text",
                )
            ],
        )


def string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def block(record: Record) -> list[str]:
    """The lines of the `definition` block of `record`, in its canonical form."""
    terms = record["terms"]
    assert isinstance(terms, list)
    lines = [f"  definition {string(str(record['at']))} {{"]
    lines += [f"    term {string(term)}" for term in terms]
    lines.append(f"    kind {record['kind']}")
    words = record["words"]
    lines.append("    words unstated" if words is None else f"    words {string(str(words))}")
    if record["region"] is not None:
        lines.append(f"    region {string(str(record['region']))}")
    lines.append(f"    basis {record['basis']}")
    for name in ("borrowed_from", "see_note"):
        if record[name] is not None:
            lines.append(f"    {name} {string(str(record[name]))}")
    lines.append("  }")
    return lines


def write_workspace(directory: Path, records: list[Record]) -> None:
    """A workspace whose module of each unit holds the definition blocks of `records`."""
    units: dict[str, list[str]] = {}
    for record in records:
        unit = "/".join(str(record["at"]).split("/")[:5])
        units.setdefault(unit, []).extend(block(record))
    for unit, lines in units.items():
        path = directory / f"{unit[1:]}.fr"
        path.parent.mkdir(parents=True, exist_ok=True)
        text = "\n".join(["edition 2", f"module {string(unit)} {{", *lines, "}", ""])
        path.write_text(text, encoding="utf-8")


class Against(unittest.TestCase):
    """`--against DIR` on `definitions-restated.xml` and a workspace written here."""

    def against(self, directory: Path, workspace: Path) -> subprocess.CompletedProcess[str]:
        return run_script(
            SCRIPT,
            fixture_input("definitions-restated", directory),
            "--records",
            str(directory / "definitions.jsonl"),
            "--against",
            str(workspace),
        )

    def compare(self, records: list[Record]) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-definitions-") as directory:
            workspace = Path(directory) / "workspace"
            write_workspace(workspace, records)
            # An entry whose name begins with a dot is no part of the workspace.
            (workspace / ".fidryn-cache").mkdir()
            (workspace / ".fidryn-cache" / "x.fr").write_text(
                "\n".join(block(definition("/us/usc/t99/s9"))), encoding="utf-8"
            )
            return self.against(Path(directory), workspace)

    def expected(self) -> list[Record]:
        return restated(RESTATED_BASES, RESTATED_NOTES)

    def test_equal_definitions_pass(self) -> None:
        result = self.compare(self.expected())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertRegex(
            result.stdout, r"^5 definitions in the release 999-1, the same as \S+workspace holds\n$"
        )

    def test_a_definition_the_workspace_lacks_is_named(self) -> None:
        records = self.expected()
        missing = records.pop(2)
        result = self.compare(records)
        self.assertEqual(result.returncode, 1, result.stderr)
        line = json.dumps(missing, ensure_ascii=False, separators=(",", ":"))
        self.assertIn("the release gives 5 definitions and ", result.stderr)
        self.assertIn(" holds 4; the first that differs: ", result.stderr)
        self.assertIn(f"the release gives {line}, and ", result.stderr)
        self.assertTrue(
            result.stderr.endswith("workspace holds no such definition\n"), result.stderr
        )

    def test_a_definition_the_release_lacks_is_named(self) -> None:
        extra = definition("/us/usc/t99/s3/a", ("cog",))
        result = self.compare([*self.expected(), extra])
        self.assertEqual(result.returncode, 1, result.stderr)
        line = json.dumps(extra, ensure_ascii=False, separators=(",", ":"))
        self.assertIn("the release gives 5 definitions and ", result.stderr)
        self.assertIn(" holds 6; the first that differs: ", result.stderr)
        self.assertIn(f"holds {line}, and the release gives no such definition\n", result.stderr)

    def test_a_field_that_differs_is_named_with_both_values(self) -> None:
        records = self.expected()
        records[3]["see_note"] = None
        result = self.compare(records)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(
            f'the first that differs: the definition of `{S4}` with the terms ["cog"] has the '
            f'see_note "{NOTE_OF_S4}" in the release and null in ',
            result.stderr,
        )
        records = self.expected()
        records[0]["kind"] = "includes"
        result = self.compare(records)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(
            f'the definition of `{S1}` with the terms ["widget"] has the kind "means" in the '
            'release and "includes" in ',
            result.stderr,
        )

    def test_a_definition_block_in_another_form_is_refused(self) -> None:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-definitions-") as directory:
            workspace = Path(directory) / "workspace"
            write_workspace(workspace, self.expected())
            module = workspace / "us" / "usc" / "t99" / "s3.fr"
            text = module.read_text(encoding="utf-8")
            module.write_text(text.replace("    kind means", "    kind  means"), encoding="utf-8")
            result = self.against(Path(directory), workspace)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertTrue(result.stderr.startswith("refused: "), result.stderr)
        self.assertIn(
            "us/usc/t99/s3.fr:5: `    kind  means` stands where the definition block of line 3 "
            "has its `kind`",
            result.stderr,
        )


if __name__ == "__main__":
    unittest.main()
