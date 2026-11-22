"""Hold the independent profile of the Code to the hand-written profiles of the fixtures.

`<name>.profile.json` beside each fixture is the profile of what the reading rules
import (`import`), written by hand. The baseline profile (`baseline`), by the rules
of the first profile of the Code (2026-11-13), of a few fixtures is written out
below, where it shows how the two sets of counting rules differ.
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from test_uslm_text import CASES, FIXTURES, HERE, TWO_FILES, UNNAMED, fixture_input, run_script

PROFILE_SCRIPT = HERE / "usc_profile.py"


def baseline(
    provisions: int,
    sections_by_status: dict[str, int],
    to_the_code: int = 0,
    exact: int = 0,
    section_only: int = 0,
    no_section: int = 0,
) -> dict[str, object]:
    return {
        "provisions": provisions,
        "references": {
            "exact": exact,
            "no_section": no_section,
            "section_only": section_only,
            "to_the_code": to_the_code,
        },
        "sections": sum(sections_by_status.values()),
        "sections_by_status": sections_by_status,
    }


# The baseline counts the nine tags with an identifier, outside notes, quoted content,
# tables of contents and source credits, and resolves only against those identifiers.
BASELINE = {
    "section": baseline(6, {"operative": 1}),
    # The link whose words are white space is counted; it reaches no section.
    "links": baseline(2, {"operative": 2}, to_the_code=2, exact=1, no_section=1),
    # Sections without an identifier are not counted; the paragraph that has one is.
    "no-identifier": baseline(1, {}),
    # References to an alias and to a member of a range reach no section.
    "identifier-lists": baseline(
        5,
        {"repealed": 2, "omitted": 1, "transferred": 1, "operative": 1},
        to_the_code=2,
        no_section=2,
    ),
    # Court rules, reorganization plans and generic levels are not among the nine tags.
    "unit-kinds": baseline(0, {}),
    # Elements are counted, not distinct identifiers.
    "repeated-identifiers": baseline(6, {"operative": 3}, to_the_code=1, exact=1),
    # The section inside a section is a section of its own.
    "section-in-section": baseline(3, {"operative": 3}),
    # The quoted section is inside a note.
    "notes-content": baseline(1, {"operative": 1}),
}


class ProfileFixtures(unittest.TestCase):
    def output_of(self, name: str, *args: str) -> str:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-profile-") as directory:
            result = run_script(PROFILE_SCRIPT, fixture_input(name, Path(directory)), *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        return result.stdout

    def profile_of(self, name: str, *args: str) -> dict[str, object]:
        profile = json.loads(self.output_of(name, *args))
        if not isinstance(profile, dict):
            raise AssertionError(profile)
        return profile

    def check(self, name: str) -> None:
        expected = json.loads((FIXTURES / f"{name}.profile.json").read_text(encoding="utf-8"))
        profile = self.profile_of(name)
        self.assertEqual(profile["import"], expected)
        if name in BASELINE:
            self.assertEqual(profile["baseline"], BASELINE[name])

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

    def test_marker_labels(self) -> None:
        self.check("marker-labels")

    def test_subheadings(self) -> None:
        self.check("subheadings")

    def test_quoted_operative(self) -> None:
        self.check("quoted-operative")

    def test_meta_dash_name(self) -> None:
        # The release is the one the name gives, its en dash read as a hyphen.
        self.check("meta-dash-name")

    def test_meta_no_flag(self) -> None:
        self.check("meta-no-flag")

    def test_definitions_sentence(self) -> None:
        self.check("definitions-sentence")

    def test_definitions_chapeau(self) -> None:
        self.check("definitions-chapeau")

    def test_definitions_restated(self) -> None:
        self.check("definitions-restated")

    def test_notes_empty_topic(self) -> None:
        self.check("notes-empty-topic")

    def test_notes_spaced_topics(self) -> None:
        self.check("notes-spaced-topics")

    def test_quoted_empty_origin(self) -> None:
        self.check("quoted-empty-origin")

    def test_notes_split_content(self) -> None:
        self.check("notes-split-content")

    def test_quoted_level_labels(self) -> None:
        self.check("quoted-level-labels")

    def test_notes_layer_quoted(self) -> None:
        self.check("notes-layer-quoted")

    def test_notes_layer_groups(self) -> None:
        self.check("notes-layer-groups")

    def test_notes_layer_topics(self) -> None:
        self.check("notes-layer-topics")

    def test_notes_layer_headings(self) -> None:
        self.check("notes-layer-headings")

    def test_toc_nested(self) -> None:
        # Each table of contents counts once, the one inside the other included.
        self.check("toc-nested")

    def test_release_note_footnote(self) -> None:
        # The footnote of the release note is left out with it.
        self.check("release-note-footnote")

    def test_ref_classes(self) -> None:
        self.check("ref-classes")

    def test_top_elements_without_an_identifier_are_counted_over_the_release(self) -> None:
        expected = json.loads((FIXTURES / f"{UNNAMED}.profile.json").read_text(encoding="utf-8"))
        for jobs in ("1", "2"):
            with self.subTest(jobs=jobs):
                self.assertEqual(self.profile_of(UNNAMED, "--jobs", jobs)["import"], expected)

    def test_two_files_are_counted_the_same_for_any_number_of_workers(self) -> None:
        expected = json.loads((FIXTURES / f"{TWO_FILES}.profile.json").read_text(encoding="utf-8"))
        for jobs in ("1", "2"):
            with self.subTest(jobs=jobs):
                self.assertEqual(self.profile_of(TWO_FILES, "--jobs", jobs)["import"], expected)

    def test_every_fixture_has_a_profile_test(self) -> None:
        tested = {name.removeprefix("test_").replace("_", "-") for name in dir(self)}
        self.assertLessEqual(set(CASES), tested)
        self.assertLessEqual(set(BASELINE), set(CASES))

    def test_the_profile_is_printed_with_sorted_keys_under_two_members(self) -> None:
        text = self.output_of("section")
        profile = json.loads(text)
        self.assertEqual(list(profile), ["baseline", "import"])
        self.assertEqual(text, json.dumps(profile, indent=2, sort_keys=True) + "\n")

    def test_out_writes_what_is_printed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="fidryn-usc-profile-out-") as directory:
            out = Path(directory) / "out" / "profile.json"
            out.parent.mkdir()
            source = fixture_input("links", Path(directory))
            result = run_script(PROFILE_SCRIPT, source, "--out", str(out))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(out.read_text(encoding="utf-8"), result.stdout)
            self.assertEqual(
                sorted(p.name for p in Path(directory).iterdir()), ["out", "usc99.xml"]
            )


if __name__ == "__main__":
    unittest.main()
