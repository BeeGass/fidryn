"""Hold the second implementation of the definition rules to the cases of the importer's tests.

The documents are the invented fixtures `fixtures/uslm/definitions-*.xml`, which the
tests of `fidryn-ingest-uscode` read, and small invented documents of the title 99
written here. The definitions each one gives are those that the importer's tests
expect of the same document, written out by hand. The workspaces that `--against`
compares with are written here too, their `definition` blocks in the canonical form.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

import usc_definitions
from test_uslm_text import FIXTURES, HERE, MEMBER, fixture_input, run_script
from uslm_common import Container, Note, Para, Part, Provision, Table, Text, Unit

SCRIPT = HERE / "usc_definitions.py"
VECTORS = HERE / "fixtures" / "definitions" / "vectors.jsonl"
VECTORS_SHA256 = "ffc209c55aa669d3144d7ace126333e4a00d54f998a70246d4e75e74db66ab6c"
VECTOR_COUNT = 238
# The fields of a record, in their order.
RECORD_FIELDS = (
    "at",
    "terms",
    "kind",
    "words",
    "condition",
    "region",
    "basis",
    "borrowed_from",
    "borrowed_words",
    "see_note",
)

S1 = "/us/usc/t99/s1"
S2 = "/us/usc/t99/s2"
S3 = "/us/usc/t99/s3"
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
    *,
    condition: str | None = None,
    borrowed_words: str | None = None,
) -> Record:
    """A definition as a line of `--records` holds it, with the fields of D15.1 in
    their order."""
    return {
        "at": at,
        "terms": list(terms),
        "kind": kind,
        "words": words,
        "condition": condition,
        "region": region,
        "basis": basis,
        "borrowed_from": borrowed_from,
        "borrowed_words": borrowed_words,
        "see_note": see_note,
    }


# The definitions of `definitions-sentence.xml`, in the order of the records. Section 1
# holds the sentence form with a reach of its own or none; its subsection (a) has an
# aside that is a condition (D6.5, D9.6). Section 3 holds paragraphs that the first
# version of the rules refused. The second version reads six of them: one that borrows
# with no link (D12.3, D12.5), one that opens with a condition (D7.2.3), one whose two
# sentences each define a term (D3.3, D10, D11.1), one with the sentence form in its
# second sentence (D3.3, D10.2), one with a reach between commas after its term (D7.8.2)
# and one whose aside is a condition (D6.5, D9.6). It still refuses one whose
# text before `the words` is no reach (D7.2), one that defines a second term in the same
# sentence (D11.1), one with a reach after its verb (D11.4), one with no reach that speaks
# of one (D11.6), and quoted content and a note that hold the sentence form (D3.1).
SENTENCE = [
    definition(
        f"{S1}/a",
        words="As used in this section",
        region=S1,
        basis="enacted",
        condition="with respect to a gadget",
    ),
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
    definition(f"{S3}/a", kind="borrowed", borrowed_words="the Widget Act"),
    definition(f"{S3}/c", condition="In the case of a gadget"),
    definition(f"{S3}/e", ("gadget",)),
    definition(f"{S3}/e"),
    definition(f"{S3}/f"),
    definition(f"{S3}/g", words="as used in this section", region=S3, basis="enacted"),
    definition(f"{S3}/h", condition="in the case of a gadget"),
]

# The definitions of `definitions-chapeau.xml`, in the order of the records: reach
# chapeaus with and without a lead-in, a chapeau above a provision with a heading, two
# chapeaus that state no reach the rules read (subsections 1(b) and 5(c)), two that the
# first version did not read and the second does (an exception and a lowercase reach in
# subsection 5(a), D8.1.2.2; a later lead-in in subsection 5(b), D8.1.2), and two
# definitions in the parts of one provision.
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
    definition("/us/usc/t99/s5/a/1", words="in this chapter", region=CHAPTER, basis="enacted"),
    definition("/us/usc/t99/s5/b/1", words="In this chapter", region=CHAPTER, basis="enacted"),
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
    # An aside that begins `In the case of `, with a capital letter, speaks of no reach
    # (D11.5 compares in any case) and is no condition (D6.5 compares as written).
    (content("The term “widget”, In the case of a gadget, means a small device."), widget()),
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
    # After In, the Object is `this` and a space, a single link, or begins with a kind
    # word and a space (D4.3); a link that is not the whole Object gives no region.
    (
        content(f'In <ref href="{S2}">section 2</ref>, the term “widget” means a device.'),
        widget(words="In section 2", region=S2, basis="enacted"),
    ),
    (
        content(
            f'In <ref href="{S2}">section 2</ref> and section 3, the term “widget” means a device.'
        ),
        widget(words="In section 2 and section 3"),
    ),
    (content("In the Act, the term “widget” means a device."), []),
    (content("In such section, the term “widget” means a device."), []),
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
    # A borrowed definition takes the first link that begins after its verb (D12.1,
    # D12.2); when that link has no target, its source words (D12.3, D12.5).
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
        [definition(S1, kind="borrowed", borrowed_words="section 2 and section 2")],
    ),
    # A superscript of digits only is no text, and one space goes with it (D1.1.3); a
    # footnote marker gives nothing (D1.1.2).
    (
        content(
            "The term “widget” has the meaning given such term in section 5(a) <sup>1</sup> "
            "of this title."
        ),
        [definition(S1, kind="borrowed", borrowed_words="section 5(a) of this title")],
    ),
    (
        content(
            "The term “widget” has the meaning given such term in section 5"
            '<xhtml:sup xmlns:xhtml="http://www.w3.org/1999/xhtml">2</xhtml:sup> of this title.'
        ),
        [definition(S1, kind="borrowed", borrowed_words="section 5 of this title")],
    ),
    (
        content("The term “widget” has the meaning given such term in section 5<sup>a</sup>."),
        [definition(S1, kind="borrowed", borrowed_words="section 5a")],
    ),
    (
        '<content>In this section,<ref class="footnoteRef" idref="fn1">1</ref> the term '
        "“widget” means a device.</content>"
        '<note type="footnote" id="fn1"><num>1</num><p>An invented footnote.</p></note>',
        widget(words="In this section", region=S1, basis="enacted"),
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
        # Every line has the ten fields, in their order, `null` for an absent one.
        for record in records:
            self.assertEqual(tuple(record), RECORD_FIELDS)
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


def unstated(
    terms: tuple[str, ...] = ("x",),
    kind: str = "means",
    condition: str | None = None,
    borrowed_words: str | None = None,
) -> list[Record]:
    """The one record of a paragraph of section 1 with no reach."""
    return [definition(S1, terms, kind, condition=condition, borrowed_words=borrowed_words)]


def references_in_text(paragraph: str) -> str:
    """The editorial notes of section 1: one "References in Text" note whose paragraph
    is `paragraph`."""
    return (
        '<notes type="uscNote"><note role="crossHeading" topic="editorialNotes">'
        "<heading>Editorial Notes</heading></note>"
        '<note topic="referencesInText"><heading>References in Text</heading>'
        f"<p>{paragraph}</p></note></notes>"
    )


class Clarifications(unittest.TestCase):
    """The clarifications of the specification (`clarifications.md`, 2026-11-22), each on
    the text it gives, read by the script in this process."""

    def test_1_the_space_beside_superscript_numbers_is_judged_in_the_order_of_the_runs(
        self,
    ) -> None:
        # The runs `a `, superscript `1`, superscript `2`, ` b`; and superscript `1`,
        # superscript `2`, ` b`.
        self.assertEqual(
            usc_definitions.read_text(Text("a 12 b", (), ((2, 3), (3, 4)))).plain, "a b"
        )
        self.assertEqual(usc_definitions.read_text(Text("12 b", (), ((0, 1), (1, 2)))).plain, "b")
        self.assertEqual(
            defined(
                with_section(
                    content(
                        "The term “x” has the meaning given such term in section 5 "
                        "<sup>1</sup><sup>2</sup> of this title."
                    )
                )
            ),
            unstated(kind="borrowed", borrowed_words="section 5 of this title"),
        )
        self.assertEqual(
            defined(
                with_section(under("In this section:", "<sup>1</sup><sup>2</sup> " + SMALL_DEVICE))
            ),
            widget(f"{S1}/1", "In this section", S1, "enacted"),
        )

    def test_2_the_paragraphs_of_a_note_are_read_through_the_text_of_d1_1(self) -> None:
        runs = Text("This section1, referred to in subsec. (a), is …", (), ((12, 13),))
        self.assertEqual(
            usc_definitions.read_text(runs).plain, "This section, referred to in subsec. (a), is …"
        )
        inner = "<content>In this section, the term “x” means y.</content>" + references_in_text(
            "This section<sup>1</sup>, referred to in subsec. (a), is invented."
        )
        self.assertEqual(
            defined(with_section(inner).replace(FLAG_YES, FLAG_NO)),
            [
                definition(
                    S1,
                    ("x",),
                    words="In this section",
                    region=S1,
                    basis="restated",
                    see_note=f"{S1}/note/editorialNotes/nolaw/references-in-text",
                )
            ],
        )

    def test_3_shall_is_looked_for_as_a_standing_word(self) -> None:
        self.assertEqual(
            defined(with_section(content("In the case of Marshall County, the term “x” means y."))),
            unstated(condition="In the case of Marshall County"),
        )

    def test_4_a_link_counts_only_at_the_place_of_the_object(self) -> None:
        self.assertEqual(
            defined(
                with_section(
                    content(
                        "For purposes of this Act, the term “x” means a y of "
                        '<ref href="/us/usc/t99/s9">this Act</ref>.'
                    )
                )
            ),
            [definition(S1, ("x",), words="For purposes of this Act")],
        )

    def test_5_a_provision_with_two_chapeau_parts_reads_nothing_below_it(self) -> None:
        self.assertEqual(
            defined(
                with_section(
                    "<chapeau>In this section:</chapeau><chapeau>Also:</chapeau>"
                    f'<paragraph identifier="{S1}/1"><num value="1">(1)</num>'
                    "<content>“x” means y.</content></paragraph>"
                )
            ),
            [],
        )

    def test_6_the_second_term_is_compared_as_it_is_recorded(self) -> None:
        self.assertEqual(
            defined(with_section(content("The term “x” means y, and “x,” means z."))), unstated()
        )

    def test_7_the_second_term_and_its_verb_form_lie_within_the_sentence(self) -> None:
        self.assertEqual(
            defined(with_section(content("The term “x” means y or “z”, as defined. A, means w."))),
            unstated(),
        )

    def test_8_an_aside_may_be_empty(self) -> None:
        self.assertEqual(defined(with_section(content("The term “x”, , means y."))), unstated())

    def test_9_a_lowercase_letter_is_a_to_z(self) -> None:
        self.assertEqual(
            defined(
                with_section(
                    content("In the case of a gadget, élite or common, the term “x” means y.")
                )
            ),
            unstated(condition="In the case of a gadget, élite or common"),
        )

    def test_10_the_comma_that_ends_a_term_is_taken_off_once_and_not_from_a_comma(
        self,
    ) -> None:
        self.assertEqual(
            defined(with_section(content("The term “,” “x” means y."))), unstated((",", "x"))
        )
        self.assertEqual(
            defined(with_section(content("The term “a,,” “b” means c."))), unstated(("a,", "b"))
        )

    def test_11_head_words_stand_with_no_letter_just_before_them(self) -> None:
        self.assertEqual(
            defined(
                with_section(
                    content("Except for bathe terms elsewhere, in this section, “x” means y.")
                )
            ),
            [definition(S1, ("x",), words="in this section", region=S1, basis="enacted")],
        )

    def test_12_an_end_of_a_sentence_is_judged_on_the_objects_own_characters(self) -> None:
        self.assertEqual(
            defined(with_section(content("In this section. The term “x” means y."))),
            [definition(S1, ("x",), words="In this section.")],
        )


def string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def block(record: Record) -> list[str]:
    """The lines of the `definition` block of `record`, in its canonical form (D15.2):
    an optional field is written only when present."""
    terms = record["terms"]
    assert isinstance(terms, list)
    lines = [f"  definition {string(str(record['at']))} {{"]
    lines += [f"    term {string(term)}" for term in terms]
    lines.append(f"    kind {record['kind']}")
    words = record["words"]
    lines.append("    words unstated" if words is None else f"    words {string(str(words))}")
    for name in ("condition", "region"):
        if record[name] is not None:
            lines.append(f"    {name} {string(str(record[name]))}")
    lines.append(f"    basis {record['basis']}")
    for name in ("borrowed_from", "borrowed_words", "see_note"):
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
    """`--against DIR` on `definitions-restated.xml`, or on `definitions-sentence.xml`
    for the fields `condition` and `borrowed_words`, and a workspace written here."""

    def against(
        self, directory: Path, workspace: Path, fixture: str = "definitions-restated"
    ) -> subprocess.CompletedProcess[str]:
        return run_script(
            SCRIPT,
            fixture_input(fixture, directory),
            "--records",
            str(directory / "definitions.jsonl"),
            "--against",
            str(workspace),
        )

    def compare(
        self, records: list[Record], fixture: str = "definitions-restated"
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-definitions-") as directory:
            workspace = Path(directory) / "workspace"
            write_workspace(workspace, records)
            # An entry whose name begins with a dot is no part of the workspace.
            (workspace / ".fidryn-cache").mkdir()
            (workspace / ".fidryn-cache" / "x.fr").write_text(
                "\n".join(block(definition("/us/usc/t99/s9"))), encoding="utf-8"
            )
            return self.against(Path(directory), workspace, fixture)

    def refused_block(self, before: str, after: str) -> subprocess.CompletedProcess[str]:
        """`--against` on `definitions-sentence.xml` and a workspace whose module of
        section 1 has `before` replaced with `after`."""
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-definitions-") as directory:
            workspace = Path(directory) / "workspace"
            write_workspace(workspace, SENTENCE)
            module = workspace / "us" / "usc" / "t99" / "s1.fr"
            text = module.read_text(encoding="utf-8")
            self.assertIn(before, text)
            module.write_text(text.replace(before, after, 1), encoding="utf-8")
            return self.against(Path(directory), workspace, "definitions-sentence")

    def test_a_condition_and_borrowed_words_are_read_and_compared(self) -> None:
        result = self.compare(SENTENCE, "definitions-sentence")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertRegex(
            result.stdout,
            r"^17 definitions in the release 999-1, the same as \S+workspace holds\n$",
        )
        cases = [
            (0, "condition", "with respect to a lever", f"{S1}/a", "with respect to a gadget"),
            (10, "borrowed_words", "the Gadget Act", f"{S3}/a", "the Widget Act"),
            (11, "condition", None, f"{S3}/c", "In the case of a gadget"),
        ]
        for index, name, value, at, given in cases:
            with self.subTest(name=name, at=at):
                records = [dict(record) for record in SENTENCE]
                records[index][name] = value
                result = self.compare(records, "definitions-sentence")
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn(
                    f'the definition of `{at}` with the terms ["widget"] has the {name} '
                    f"{json.dumps(given)} in the release and {json.dumps(value)} in ",
                    result.stderr,
                )

    def test_a_block_with_both_borrowed_fields_is_refused(self) -> None:
        # D15.2: one slot takes `borrowed_from` or `borrowed_words`, not both.
        result = self.refused_block(
            '    borrowed_from "/us/usc/t99/s2"\n',
            '    borrowed_from "/us/usc/t99/s2"\n    borrowed_words "section 2"\n',
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertTrue(result.stderr.startswith("refused: "), result.stderr)
        self.assertIn(
            '`    borrowed_words "section 2"` stands where the definition block of line ',
            result.stderr,
        )
        self.assertIn(" ends in its canonical form", result.stderr)

    def test_a_condition_out_of_its_order_is_refused(self) -> None:
        # D15.2: `condition` stands after `words` and before `region`.
        result = self.refused_block(
            '    condition "with respect to a gadget"\n    region "/us/usc/t99/s1"\n',
            '    region "/us/usc/t99/s1"\n    condition "with respect to a gadget"\n',
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(
            '`    condition "with respect to a gadget"` stands where the definition block of '
            "line 3 has its `basis` in its canonical form",
            result.stderr,
        )

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


def vector_text(runs: list[dict[str, str]]) -> Text:
    """The text that the runs of a vector make, in the model of the independent reading:
    the words of each run but a footnote marker, concatenated, with the place of each
    link, superscript and marker."""
    plain = ""
    refs: list[tuple[int, int, str]] = []
    sup: list[tuple[int, int]] = []
    marks: list[tuple[int, str, int]] = []
    for run in runs:
        if "marker" in run:
            marks.append((len(plain), run["marker"], 0))
            continue
        start = len(plain)
        if "words" in run:
            plain += run["words"]
        elif "link" in run:
            plain += run["link"]
            refs.append((start, len(plain), run["to"]))
        elif "sup" in run:
            plain += run["sup"]
            sup.append((start, len(plain)))
        else:
            raise AssertionError(run)
    return Text(plain, tuple(refs), tuple(sup), (), tuple(marks))


def vector_provision(value: dict[str, Any]) -> Provision:
    items: list[Part | Provision] = []
    for item in value["items"]:
        if "child" in item:
            items.append(vector_provision(item["child"]))
            continue
        blocks: list[Para | Table] = []
        for block in item["blocks"]:
            if "p" in block:
                blocks.append(Para(vector_text(block["p"])))
            elif "table" in block:
                blocks.append(Table([]))
            else:
                raise AssertionError(block)
        items.append(Part(item["part"], list(blocks)))
    return Provision(value["address"], value["kind"], None, None, None, items)


def vector_records(vector: dict[str, Any]) -> list[Record]:
    """The records that the rules give for the input of `vector`, as the reading of the
    publisher's XML would hand it to them: a unit, and its containers."""
    unit = vector["unit"]
    containers = unit["containers"]
    notes = [
        Note(
            note["address"],
            note["layer"],
            "ungrouped",
            [],
            Text(note["heading"]) if note["heading"] is not None else None,
            [Para(Text(paragraph)) for paragraph in note["paragraphs"]],
        )
        for note in unit["notes"]
    ]
    sink = usc_definitions.DefinitionSink()
    sink.unit(
        Unit(
            unit["address"],
            [],
            containers[0]["address"] if containers else None,
            "",
            unit["positive_law"],
            vector_provision(unit["root"]),
            None,
            notes,
            [],
        )
    )
    for index, container in enumerate(containers):
        outer = containers[index + 1]["address"] if index + 1 < len(containers) else None
        sink.container(
            Container(
                container["address"],
                [],
                outer,
                "",
                unit["positive_law"],
                container["kind"],
                None,
                None,
                None,
                [],
                None,
                [],
                [],
            )
        )
    return [definition.record() for definition in sink.definitions()]


def canonical(record: Record) -> str:
    return json.dumps(record, ensure_ascii=False, sort_keys=True)


def vector_difference(vector: dict[str, Any], found: list[Record]) -> str | None:
    """`None` when the rules give exactly the records the vector expects, as a multiset;
    otherwise a sentence that names the vector and the first record that differs, and,
    when the other side has a record of the same provision and terms, the first field
    that differs."""
    # A key that an expected record leaves out reads as `null`; the script's own records
    # have every key, in order.
    expected: list[Record] = [
        {name: record.get(name) for name in RECORD_FIELDS} for record in vector["expect"]
    ]
    for record in found:
        if tuple(record) != RECORD_FIELDS:
            return f"vector {vector['id']}: a record has the fields {list(record)}"
    keyed_found = Counter(canonical(record) for record in found)
    keyed_expected = Counter(canonical(record) for record in expected)
    only_found = sorted((keyed_found - keyed_expected).elements())
    only_expected = sorted((keyed_expected - keyed_found).elements())
    if not only_found and not only_expected:
        return None
    prefix = f"vector {vector['id']} ({vector['rule']}, {vector['holds']})"
    if only_expected:
        missing = json.loads(only_expected[0])
        for other in only_found:
            given = json.loads(other)
            if (given["at"], given["terms"]) == (missing["at"], missing["terms"]):
                field = next(name for name in RECORD_FIELDS if given[name] != missing[name])
                return (
                    f"{prefix}: the record of `{missing['at']}` with the terms "
                    f"{missing['terms']} has the {field} {given[field]!r}, and the vector "
                    f"expects {missing[field]!r}"
                )
        return f"{prefix}: the rules give no record {only_expected[0]}"
    because = vector.get("none_because")
    return f"{prefix}: the rules give {only_found[0]}, which the vector does not expect" + (
        f" ({because})" if because else ""
    )


class Vectors(unittest.TestCase):
    """The test vectors of the rules, on invented text: each vector's input, read as the
    reading of the publisher's XML hands a unit to the rules, gives the records it expects."""

    def vectors(self) -> list[dict[str, Any]]:
        lines = VECTORS.read_text(encoding="utf-8").splitlines()
        return [json.loads(line) for line in lines]

    def test_the_vectors_are_the_ones_of_the_rules(self) -> None:
        self.assertEqual(hashlib.sha256(VECTORS.read_bytes()).hexdigest(), VECTORS_SHA256)
        vectors = self.vectors()
        self.assertEqual(len(vectors), VECTOR_COUNT)
        self.assertEqual(len({vector["id"] for vector in vectors}), VECTOR_COUNT)

    def test_a_missing_key_is_read_as_null(self) -> None:
        # A record that leaves out the keys of absent fields, as the third reading of the
        # rules writes them, is the record with those fields `null`.
        vector = self.vectors()[0]
        found = vector_records(vector)
        self.assertEqual(found[0]["condition"], None)
        shortened = {
            key: value
            for key, value in vector["expect"][0].items()
            if key not in ("condition", "borrowed_words")
        }
        self.assertIsNone(vector_difference({**vector, "expect": [shortened]}, found))
        # The script's own records keep every key.
        self.assertIsNotNone(vector_difference({**vector, "expect": vector["expect"]}, [shortened]))

    def test_every_vector_gives_the_records_it_expects(self) -> None:
        failures = []
        for vector in self.vectors():
            difference = vector_difference(vector, vector_records(vector))
            if difference is not None:
                failures.append(difference)
        self.assertEqual(failures, [], f"{len(failures)} of {VECTOR_COUNT} vectors differ")


if __name__ == "__main__":
    unittest.main()
