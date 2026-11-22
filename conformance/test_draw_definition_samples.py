"""Hold the drawing of the definition samples to its generator, its populations and its lines.

Every workspace and every text record here is invented, in the corpus `/zz/demo`, and
written by the tests into a scratch directory: the module files hold the declarations
the tool reads, the `definition` blocks in their canonical form, and the records the
provisions as `fidryn text --json` prints them.
"""

from __future__ import annotations

import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import draw_definition_samples as draw

T1 = "/zz/demo/t1"


def text(words: str) -> dict[str, Any]:
    return {"p": words, "refs": [], "sup": [], "sub": []}


def provision(
    address: str, kind: str, items: list[dict[str, Any]], unit: bool = False
) -> dict[str, Any]:
    record: dict[str, Any] = {
        "address": address,
        "layer": "operative",
        "kind": kind,
        "status": None,
        "num": None,
        "heading": None,
        "items": items,
    }
    if unit:
        record.update(aliases=[], within=T1, source="demo01.xml", positive_law=True)
    return record


def part(role: str, *paragraphs: str) -> dict[str, Any]:
    return {"part": role, "blocks": [text(words) for words in paragraphs]}


def definition(
    address: str,
    term: str,
    words: str,
    region: str | None,
    *,
    kind: str = "means",
    condition: str | None = None,
    borrowed_words: str | None = None,
) -> str:
    lines = [
        f'  definition "{address}" {{',
        f'    term "{term}"',
        f"    kind {kind}",
        f'    words "{words}"',
    ]
    if condition is not None:
        lines.append(f'    condition "{condition}"')
    if region is not None:
        lines.append(f'    region "{region}"')
    lines.append("    basis enacted")
    if borrowed_words is not None:
        lines.append(f'    borrowed_words "{borrowed_words}"')
    lines.append("  }")
    return "\n".join(lines) + "\n"


def module(address: str, definitions: str) -> str:
    return f'edition 2\nmodule "{address}" {{\n{definitions}}}\n'


# Section 1: a chapeau that states a reach, and twelve paragraphs that define a term
# under it. Section 2: a paragraph that defines two terms, and is not extracted.
# Section 3: a subsection that states its own reach in two sentences, each of which
# defines a term, and a reach in its proviso part that defines a third; and a
# continuation after the subsection that defines a term and is not read. Section 4: a
# table whose cell defines a term.
TERMS = [f"term{number:02d}" for number in range(1, 13)]
S1_CHILDREN = [
    provision(f"{T1}/s1/{number}", "paragraph", [part("content", f"“{term}” means thing {term}.")])
    for number, term in enumerate(TERMS, start=1)
]
S2 = "The term “gadget” means an invented device, and “gizmo” includes a gadget."
S3A = (
    "In this subsection, the term “sprocket” means an invented gear. For purposes of this "
    "subsection, the term “spoke” has the meaning given that term in the Invented Act."
)
S3A_PROVISO = (
    "For purposes of this subsection, the term “gear”, with respect to a wheel, includes a cog."
)
S3_CONTINUATION = "The term “cog” includes a sprocket."
S3_DEFINITIONS = (
    definition(
        f"{T1}/s3/a",
        "gear",
        "For purposes of this subsection",
        f"{T1}/s3/a",
        kind="includes",
        condition="with respect to a wheel",
    )
    + definition(
        f"{T1}/s3/a",
        "spoke",
        "For purposes of this subsection",
        f"{T1}/s3/a",
        kind="borrowed",
        borrowed_words="the Invented Act",
    )
    + definition(f"{T1}/s3/a", "sprocket", "In this subsection", f"{T1}/s3/a")
)
S4_CELL = "“bolt” means an invented fastener."
TABLE = {
    "table": [
        ["head", [[True, 1, 1, [text("Word")]]]],
        ["body", [[False, 1, 1, [text(S4_CELL)]]]],
    ]
}
RECORDS: list[dict[str, Any]] = [
    {
        "address": T1,
        "layer": "operative",
        "kind": "title",
        "status": None,
        "num": None,
        "heading": None,
        "aliases": [],
        "within": None,
        "members": [f"{T1}/s1", f"{T1}/s2", f"{T1}/s3", f"{T1}/s4"],
    },
    provision(
        f"{T1}/s1",
        "section",
        [part("chapeau", "In this section:")] + [{"child": c["address"]} for c in S1_CHILDREN],
        unit=True,
    ),
    *S1_CHILDREN,
    {"address": f"{T1}/s1", "record": "credit", **text("(Invented, and “never” means here.)")},
    provision(f"{T1}/s2", "section", [part("content", S2)], unit=True),
    {
        "address": f"{T1}/s2/note/demoNotes/nolaw/amendment",
        "layer": "editorial",
        "group": "demoNotes",
        "topics": [],
        "heading": None,
        "blocks": [text("In the invented note, “widget” means a note.")],
    },
    provision(
        f"{T1}/s3",
        "section",
        [{"child": f"{T1}/s3/a"}, part("continuation", S3_CONTINUATION)],
        unit=True,
    ),
    provision(f"{T1}/s3/a", "subsection", [part("content", S3A), part("proviso", S3A_PROVISO)]),
    provision(
        f"{T1}/s4", "section", [{"part": "content", "blocks": [text("A table."), TABLE]}], unit=True
    ),
]
RECORDS.sort(key=lambda record: str(record["address"]).encode())


def manifest(kind: str = "generated") -> str:
    return json.dumps(
        {
            "schema": "fidryn.workspace/v0.1",
            "name": "demo",
            "kind": kind,
            "prefix": "/zz/demo",
            "release": "42-7",
            "toolchain": "0.3.0",
            "dependencies": [],
        }
    )


def ledger(release: str = "42-7", **totals: object) -> str:
    """The parts of an invented coverage ledger that the tool reads: what the records
    above and the definitions of `Samples.write_workspace` count, unless `totals` says
    otherwise."""
    counted: dict[str, object] = {
        "units": 4,
        "containers": 1,
        "provisions": 17,
        "notes": {"enacted": 0, "editorial": 1},
        "footnotes": 0,
        "definitions": {"enacted": 15, "restated": 0, "unresolved": 0},
    }
    counted.update(totals)
    return json.dumps(
        {
            "schema": "fidryn.coverage/v0.1",
            "prefix": "/zz/demo",
            "release": release,
            "totals": counted,
        }
    )


class Generator(unittest.TestCase):
    def test_the_generator_gives_the_known_answers_of_splitmix64(self) -> None:
        zero = draw.SplitMix64(0)
        self.assertEqual(
            [zero.next() for _ in range(3)],
            [0xE220A8397B1DCDAF, 0x6E789E6AA1B965F4, 0x06C45D188009454F],
        )
        other = draw.SplitMix64(1234567)
        self.assertEqual(
            [other.next() for _ in range(5)],
            [
                6457827717110365317,
                3203168211198807973,
                9817491932198370423,
                4593380528125082431,
                16408922859458223821,
            ],
        )

    def test_a_number_below_a_bound_is_below_it(self) -> None:
        generator = draw.SplitMix64(42)
        for bound in (1, 2, 3, 7, 500, 34329):
            with self.subTest(bound=bound):
                for _ in range(50):
                    self.assertLess(generator.below(bound), bound)

    def test_the_draws_of_the_two_samples_drawn_give_their_known_answers(self) -> None:
        # The seed and the populations of the two samples drawn from the release 119-142:
        # the first five places and the last three of each draw of 500.
        definitions = draw.draw(34329, 500, 20261117)
        self.assertEqual(definitions[:5], [19521, 22410, 23053, 3582, 30023])
        self.assertEqual(definitions[-3:], [17500, 13555, 5951])
        near_misses = draw.draw(5940, 500, 20261117)
        self.assertEqual(near_misses[:5], [735, 853, 3403, 777, 2404])
        self.assertEqual(near_misses[-3:], [4072, 2617, 4662])

    def test_a_draw_is_without_replacement_and_given_by_its_seed(self) -> None:
        first = draw.draw(1000, 500, 20261117)
        self.assertEqual(first, draw.draw(1000, 500, 20261117))
        self.assertEqual(len(set(first)), 500)
        self.assertTrue(all(0 <= index < 1000 for index in first))
        self.assertNotEqual(first, draw.draw(1000, 500, 20261118))
        self.assertEqual(sorted(draw.draw(7, 500, 3)), list(range(7)))
        self.assertEqual(draw.draw(0, 500, 3), [])


class FirstTerms(unittest.TestCase):
    def test_each_sentence_offers_its_first_term_as_the_rules_record_it(self) -> None:
        cases = [
            (text("The term “a” means b. The term “c,” means d."), ["a", "c"]),
            (text("A rule. Another rule, with no term."), []),
            # The text the rules read: a superscript of digits only is no text, so the
            # period before it ends a sentence.
            (
                {
                    "p": "The term “a” means b.1 The term “c” means d.",
                    "refs": [],
                    "sup": [[21, 22]],
                },
                ["a", "c"],
            ),
            (text("The term “a” means b.1 The term “c” means d."), ["a"]),
        ]
        for block, expected in cases:
            with self.subTest(block["p"]):
                self.assertEqual(draw.first_terms(block), expected)


class LooseMatch(unittest.TestCase):
    def test_a_term_before_a_verb_within_60_characters_is_a_near_miss(self) -> None:
        cases = {
            "“widget” means a device.": ("widget", "means"),
            "Each “widget” includes a part.": ("widget", "includes"),
            "“a” and “b” shall mean the two.": ("a", "shall mean"),
            "The word “widget”, for these invented purposes only, has the meaning given.": (
                "widget",
                "has the meaning",
            ),
            # 60 characters from the end of the term to the start of the verb.
            "“x” is long " + "y" * 50 + " include": ("x", "include"),
        }
        for words, expected in cases.items():
            with self.subTest(words):
                self.assertEqual(draw.loose_match(words), expected)

    def test_a_verb_too_far_or_part_of_a_word_is_not(self) -> None:
        for words in (
            "“x” is long " + "y" * 51 + " include",
            "“widget” meaning a device.",
            "“widget” remeans a device.",
            "“widget” Means a device.",
            '"widget" means a device.',
            "widget means a device.",
        ):
            with self.subTest(words):
                self.assertIsNone(draw.loose_match(words))


class Samples(unittest.TestCase):
    def setUp(self) -> None:
        self.scratch = tempfile.TemporaryDirectory()
        self.root = Path(self.scratch.name)
        self.workspace = self.root / "workspace"
        self.records = self.root / "records.jsonl"
        self.write_workspace()

    def tearDown(self) -> None:
        self.scratch.cleanup()

    def write_workspace(
        self,
        s3: str | None = None,
        coverage: str | None = None,
        records: list[dict[str, Any]] | None = None,
    ) -> None:
        (self.workspace / "zz" / "demo" / "t1").mkdir(parents=True, exist_ok=True)
        (self.workspace / ".fidryn-cache").mkdir(exist_ok=True)
        (self.workspace / "workspace.json").write_text(manifest(), encoding="utf-8")
        (self.workspace / "coverage.json").write_text(coverage or ledger(), encoding="utf-8")
        s1 = "".join(
            definition(f"{T1}/s1/{number}", term, "In this section", f"{T1}/s1")
            for number, term in enumerate(TERMS, start=1)
        )
        units = {
            "s1": s1,
            "s2": "",
            "s3": s3 if s3 is not None else S3_DEFINITIONS,
            "s4": "",
        }
        for name, definitions in units.items():
            (self.workspace / "zz" / "demo" / "t1" / f"{name}.fr").write_text(
                module(f"{T1}/{name}", definitions), encoding="utf-8"
            )
        (self.workspace / "zz" / "demo" / "t1.fr").write_text(module(T1, ""), encoding="utf-8")
        self.records.write_text(
            "".join(
                json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n"
                for record in (RECORDS if records is None else records)
            ),
            encoding="utf-8",
        )

    def drawn(self, seed: int, name: str, size: int = draw.SIZE) -> tuple[list[Any], list[Any]]:
        out = self.root / name
        draw.draw_samples(self.workspace, self.records, seed, out, size)
        return tuple(  # type: ignore[return-value]
            [json.loads(line) for line in (out / file).read_text(encoding="utf-8").splitlines()]
            for file in ("extracted.jsonl", "near-misses.jsonl")
        )

    def test_each_line_holds_what_a_reader_needs(self) -> None:
        extracted, near = self.drawn(20261117, "sample")
        self.assertEqual(len(extracted), 15)
        self.assertEqual(sorted(line["draw"] for line in extracted), list(range(1, 16)))
        by_term = {line["terms"][0]: line for line in extracted}
        self.assertEqual(
            by_term["term02"],
            {
                "seed": 20261117,
                "release": "42-7",
                "population": 15,
                "draw": by_term["term02"]["draw"],
                "address": f"{T1}/s1/2",
                "terms": ["term02"],
                "kind": "means",
                "words": "In this section",
                "condition": None,
                "region": f"{T1}/s1",
                "basis": "enacted",
                "borrowed_words": None,
                "text": "“term02” means thing term02.",
                "chapeau": {"address": f"{T1}/s1", "text": "In this section:"},
                "verdict": "",
            },
        )
        # The two sentences of the content of subsection 3(a) each give a definition,
        # and its proviso part a third.
        self.assertEqual(by_term["sprocket"]["text"], S3A)
        self.assertIsNone(by_term["sprocket"]["chapeau"])
        self.assertEqual(
            [by_term["spoke"][name] for name in ("kind", "text", "condition", "borrowed_words")],
            ["borrowed", S3A, None, "the Invented Act"],
        )
        self.assertEqual(
            [by_term["gear"][name] for name in ("kind", "text", "condition", "borrowed_words")],
            ["includes", S3A_PROVISO, "with respect to a wheel", None],
        )
        # The twelve paragraphs of section 1 and the two paragraphs of subsection 3(a)
        # hold a term before a verb, and were extracted, the proviso among them; the note
        # and the credit are not operative text.
        self.assertEqual(
            sorted((line["address"], line["part"], line["paragraph"]) for line in near),
            [
                (f"{T1}/s2", "content", 1),
                (f"{T1}/s3", "continuation", 1),
                (f"{T1}/s4", "content", 3),
            ],
        )
        self.assertEqual(sorted(line["draw"] for line in near), [1, 2, 3])
        by_address = {line["address"]: line for line in near}
        self.assertEqual(
            by_address[f"{T1}/s3"],
            {
                "seed": 20261117,
                "release": "42-7",
                "population": 3,
                "draw": by_address[f"{T1}/s3"]["draw"],
                "address": f"{T1}/s3",
                "part": "continuation",
                "paragraph": 1,
                "text": S3_CONTINUATION,
                "chapeau": None,
                "term": "cog",
                "verb": "includes",
                "verdict": "",
            },
        )
        self.assertEqual(by_address[f"{T1}/s2"]["term"], "gadget")
        self.assertEqual(by_address[f"{T1}/s4"]["text"], S4_CELL)

    def test_one_seed_draws_the_same_lines_and_another_seed_others(self) -> None:
        first = self.drawn(7, "first", 5)
        self.assertEqual(len(first[0]), 5)
        self.assertEqual(len({(line["address"], line["terms"][0]) for line in first[0]}), 5)
        for line in first[0]:
            self.assertEqual(line["population"], 15)
        self.assertEqual(first, self.drawn(7, "again", 5))
        self.assertEqual(
            (self.root / "first" / "extracted.jsonl").read_bytes(),
            (self.root / "again" / "extracted.jsonl").read_bytes(),
        )
        other = self.drawn(8, "other", 5)
        self.assertNotEqual(
            [line["address"] for line in first[0]], [line["address"] for line in other[0]]
        )

    def test_a_definition_block_out_of_its_form_stops_the_reading(self) -> None:
        self.write_workspace(
            '  definition "/zz/demo/t1/s3/a" {\n    kind means\n    term "sprocket"\n  }\n'
        )
        with self.assertRaises(draw.FormError) as caught:
            self.drawn(1, "refused")
        self.assertIn("zz/demo/t1/s3.fr:4", str(caught.exception))
        self.assertFalse((self.root / "refused").exists())
        # A block holds `borrowed_from` or `borrowed_words`, not both.
        both = definition(
            f"{T1}/s3/a",
            "spoke",
            "For purposes of this subsection",
            None,
            kind="borrowed",
            borrowed_words="the Invented Act",
        ).replace("    borrowed_words", '    borrowed_from "/zz/demo/t1/s9"\n    borrowed_words')
        self.write_workspace(both)
        with self.assertRaises(draw.FormError) as caught:
            self.drawn(1, "refused both")
        self.assertIn("zz/demo/t1/s3.fr:9: `    borrowed_words", str(caught.exception))
        self.assertIn("ends", str(caught.exception))
        self.assertFalse((self.root / "refused both").exists())

    def test_a_definition_block_in_another_form_stops_the_reading(self) -> None:
        canonical = definition(f"{T1}/s3/a", "sprocket", "In this subsection", f"{T1}/s3/a")
        lines = canonical.splitlines(keepends=True)
        forms = {
            "indented more": "".join(f"  {line}" for line in lines),
            "indented less": "".join(line[2:] for line in lines),
            "two spaces before its brace": canonical.replace('" {\n', '"  {\n', 1),
            "on one line": canonical.replace("\n    ", " ").replace("\n  }", " }"),
        }
        for name, block in forms.items():
            with self.subTest(name):
                self.write_workspace(block)
                with self.assertRaises(draw.FormError) as caught:
                    self.drawn(1, f"refused {name}")
                self.assertIn("zz/demo/t1/s3.fr:3", str(caught.exception))
                self.assertIn("is no header of a definition block", str(caught.exception))
                self.assertFalse((self.root / f"refused {name}").exists())

    def test_the_definitions_read_are_held_to_the_ledger_of_the_workspace(self) -> None:
        cases = {
            "one more": ledger(definitions={"enacted": 16, "restated": 0, "unresolved": 0}),
            "another basis": ledger(definitions={"enacted": 14, "restated": 1, "unresolved": 0}),
            "another release": ledger("42-6"),
        }
        for name, coverage in cases.items():
            with self.subTest(name):
                self.write_workspace(coverage=coverage)
                with self.assertRaises(draw.FormError) as caught:
                    self.drawn(1, f"refused {name}")
                self.assertIn(str(self.workspace / "coverage.json"), str(caught.exception))
                self.assertFalse((self.root / f"refused {name}").exists())
        self.write_workspace(coverage=ledger(definitions={"enacted": 14}))
        with self.assertRaises(draw.FormError) as caught:
            self.drawn(1, "refused by the count")
        self.assertIn(
            "the module files hold the definitions enacted: 15, restated: 0, unresolved: 0",
            str(caught.exception),
        )
        (self.workspace / "coverage.json").unlink()
        with self.assertRaises(FileNotFoundError):
            self.drawn(1, "refused without a ledger")

    def test_the_records_are_held_to_the_workspace(self) -> None:
        moved = json.loads(
            json.dumps(RECORDS, ensure_ascii=False).replace(f'"{T1}/s3/a"', f'"{T1}/s3/b"')
        )
        unit = provision(f"{T1}/s5", "section", [part("content", "An invented unit.")], unit=True)
        cases = {
            "no record of the provision of a definition": (
                moved,
                f"the records hold no provision at `{T1}/s3/a`",
            ),
            "one unit more": (
                [*RECORDS, unit],
                "the records count units: 5, containers: 1, provisions: 18, "
                "enacted notes: 0, editorial notes: 1, footnotes: 0",
            ),
            "one note fewer": (
                [record for record in RECORDS if "group" not in record],
                f"{self.workspace / 'coverage.json'} counts units: 4, containers: 1, "
                "provisions: 17, enacted notes: 0, editorial notes: 1, footnotes: 0",
            ),
        }
        for name, (records, words) in cases.items():
            with self.subTest(name):
                self.write_workspace(records=records)
                with self.assertRaises(draw.FormError) as caught:
                    self.drawn(1, f"refused {name}")
                self.assertIn(words, str(caught.exception))
                self.assertFalse((self.root / f"refused {name}").exists())

    def test_a_definition_whose_paragraph_the_records_do_not_hold_stops_the_drawing(self) -> None:
        cases = {
            # No paragraph of the provision holds the term.
            "spanner": S3_DEFINITIONS,
            # One sentence begins with the term, and the provision holds two definitions of it.
            "sprocket": S3_DEFINITIONS,
        }
        for term, others in cases.items():
            with self.subTest(term):
                self.write_workspace(
                    others + definition(f"{T1}/s3/a", term, "In this subsection", None),
                    coverage=ledger(
                        definitions={
                            "enacted": 13 + others.count("definition"),
                            "restated": 0,
                            "unresolved": 0,
                        }
                    ),
                )
                with self.assertRaises(draw.FormError) as caught:
                    self.drawn(1, f"refused {term}")
                self.assertIn(f"{T1}/s3/a", str(caught.exception))
                self.assertIn(f"whose first term is “{term}”", str(caught.exception))

    def test_a_sample_once_drawn_is_not_written_over(self) -> None:
        self.drawn(1, "kept")
        with self.assertRaises(draw.FormError) as caught:
            self.drawn(2, "kept")
        self.assertIn("extracted.jsonl", str(caught.exception))
        self.assertEqual(self.drawn(1, "copy"), self.drawn(1, "copy-2"))

    def test_the_command_writes_both_files_and_says_what_it_drew(self) -> None:
        def run(seed: str, out: Path) -> tuple[int, str, str]:
            stdout, stderr = io.StringIO(), io.StringIO()
            arguments = ["--workspace", str(self.workspace), "--records", str(self.records)]
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = draw.main([*arguments, "--seed", seed, "--out", str(out)])
            return status, stdout.getvalue(), stderr.getvalue()

        out = self.root / "command"
        self.assertEqual(
            run("11", out),
            (
                0,
                "drew 15 of 15 definitions and 3 of 3 near misses of the release 42-7 "
                "with the seed 11\n",
                "",
            ),
        )
        self.assertEqual(
            sorted(entry.name for entry in out.iterdir()), ["extracted.jsonl", "near-misses.jsonl"]
        )
        status, stdout, stderr = run("11", out)
        self.assertEqual((status, stdout), (1, ""))
        self.assertTrue(stderr.startswith("refused: "), stderr)
        with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
            run(str(1 << 64), self.root / "too-large")
        self.assertFalse((self.root / "too-large").exists())


if __name__ == "__main__":
    unittest.main()
