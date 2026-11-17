# Counting rules of the Code's official XML

`usc_profile.py` counts the official XML release point published by the
Office of the Law Revision Counsel by two sets of rules and prints both as one
JSON object with sorted keys: `baseline` and `import`. `baseline` repeats the
counting of the first profile of the Code (2026-11-13), so that its numbers can
be reproduced and compared on their own. `import` counts what the reading rules
of `uslm_common.py` import, the same reading that `uslm_text.py` prints.

```sh
uv run --locked python -I conformance/usc_profile.py ARCHIVE_DIR_OR_XML [--out FILE] [--jobs N]
```

## The profile `baseline`

- Excluded: every `note`, `notes`, `quotedContent`, `toc` and `sourceCredit`
  element and everything inside one.
- A provision is a counted element: not excluded, with one of the nine tags
  `section`, `subsection`, `paragraph`, `subparagraph`, `clause`, `subclause`,
  `item`, `subitem`, `subsubitem`, and with a non-empty identifier. Elements are
  counted, not distinct identifiers.
- A section is a provision with the tag `section`; its status is the `status`
  attribute, or `operative` when it has none.
- A reference to the Code in operative text is a `ref` that is not excluded and
  whose `href` begins `/us/usc/`. It resolves exactly when its `href` is the
  identifier of a provision. Its section is the prefix matched by
  `^/us/usc/t[0-9A-Za-z]+/s[^/]+`; it reaches the section only when it does not
  resolve exactly and that prefix is the section prefix of some provision;
  otherwise it reaches no section.

| Key | Count |
| --- | --- |
| `provisions` | provisions |
| `sections` | sections |
| `sections_by_status` | sections by status word, `operative` for none |
| `references.to_the_code` | references to the Code in operative text |
| `references.exact` | those that resolve exactly |
| `references.section_only` | those that reach the section only |
| `references.no_section` | those that reach no section |

## The profile `import`

What is counted is what the reading imports, element by element:

- **Units** are `section`, `courtRule`, `reorganizationPlan`, and a generic
  `level` that has a text part (`chapeau`, `content`, `continuation`,
  `proviso`) or a `p` as a child and no unit above it. The outermost unit holds
  everything inside it: a section that stands inside a section is a provision of
  the outer unit.
- **Containers** are `title`, `appendix`, `subtitle`, `chapter`, `subchapter`,
  `part`, `subpart`, `division`, `subdivision`, `article`, `courtRules`,
  `compiledAct`, `reorganizationPlans`, and a generic `level` with no unit above
  it and neither a text part nor a `p` as a child. The top container of a file is
  the `title` inside `main`, or the `appendix` that stands directly under
  `uscDoc` (an appendix file has no `main`).
- **Provisions** are every level inside a unit, the unit's own top level
  included.
- **Addresses.** A unit or container has its identifier as its address; the
  first member of an identifier that lists several, separated by U+0020 and no
  other character, with the others as aliases; an identifier whose last segment
  holds `...` keeps the range as its address and has its two ends as aliases, and
  every whole number between ends that differ only in a whole number (at most 999
  apart). A later holder of an identifier that a unit or container before it in
  the release already has gets `~2`, `~3` and so on, counting over the files in
  the order of their names. A provision's address is its parent's followed by the
  last segment of its identifier, and `~2`, `~3` for a later sibling with the same
  address. An element without an identifier gets the segment `~<tag>.<n>` below
  its parent; the top element of a file gets it below `/us/usc`, counted over the
  release in the order of member names. An alias is dropped when it is the address
  of a unit, a container or a provision, or one the element already has.
- **Notes** are the `note` children of a `notes` element and the notes that
  stand directly in a level, footnotes and release notes left out. A note's group
  is the topic of the last group heading (`note role="crossHeading"`) before it
  in its `notes` element, and `ungrouped` before any. The topic of a group
  heading is one word of ASCII letters and digits that begins with a letter, and
  a word these rules do not know is a group like any other. A group heading is no
  note: its label, its first heading, is not imported and is counted; what it holds
  after that label is a note of the group it opens, with no heading and no
  topics. Topics are the `topic` attribute split at U+0020. A note is editorial
  when its group is `editorialNotes`, or when it is ungrouped and one of its
  topics is `removalDescription` or `historicalAndRevision`; every other note is
  enacted. A note's text may hold a text part that stands directly in it, read as
  the blocks of its own flow, and a `signature`, also inside quoted content. Any
  `subheading` in a note and any `heading` after the note's first is a subheading
  of its text, and so is a `heading` or `subheading` that stands directly in quoted
  content.
- **Source credits.** A unit or a container has the credit that is its direct
  child; its links are references of the editorial layer.
- **Footnotes** are the `note type="footnote"` elements of a unit or a
  container, wherever they stand in it, outside tables of contents.
- **References** are the links (`ref` with an `href`) that the reading keeps,
  each in a layer: operative in a provision and in the number or heading of a
  container; the note's own layer in a note, quoted content included; editorial
  in a source credit and in a footnote. Each gets the first outcome of this list
  that applies, resolved against the addresses of every unit, container,
  provision and note of the release and the aliases of every unit and container:
  - `malformed`: the target is not an address;
  - `ambiguous`: a beginning of the target, or the target itself, is an address
    and the same address with `~2` exists;
  - `exact`: the target is an address or an alias, and neither it nor the unit it
    lies in has a status;
  - `target-status`: as `exact`, but the target or its unit has a status;
  - `unit-only`: the longest beginning of the target that is an address or alias
    is a unit or lies in one;
  - `range-form`: that longest beginning, or `/us/usc` when there is none, is
    followed in the target by a segment with a dash (`–` or `-`), and that
    segment cut at its first dash names an address or alias;
  - `no-unit`: the target lies below `/us/usc`;
  - `outside`: anything else, recorded by its first two segments.
- **Not imported, and counted:** tables of contents (`toc`) with the footnotes
  and links inside them, release notes (a note directly under a `title` or an
  `appendix` whose text begins `Current through `), links whose words are only
  white space, and the labels of group headings.

These cases of the release are rules of the reading: a container's own
`sourceCredit` is its credit; a text part that stands directly in a note is read
as the blocks of its own flow; an element whose own segment is `~<tag>.<n>`
counts as assigned, whether it is a unit, a container or a provision; the
`appendix` of an appendix file stands directly under `uscDoc`, and a
`signature` is a block in quoted content inside a note as in the note itself;
the text a group heading holds after its label is a note of its group. A number
with neither a value nor text is absent, as it is in 201 sections quoted in notes
of release 119-142; a number with a value and no text keeps the value with an empty
shown form; a heading and a source credit with no text are absent. A number, a
heading or a credit that holds a footnote marker keeps it, with an empty text when
it has no other.

| Key | Count |
| --- | --- |
| `prefix`, `release` | the corpus prefix, and the release that every document names in `docPublicationName` after `Online@` |
| `units` | units |
| `units_by_kind` | units by tag |
| `units_by_status` | units by status word, `in-force` for none |
| `containers` | containers |
| `containers_by_kind` | containers by tag |
| `provisions` | provisions |
| `addresses.aliases` | aliases kept |
| `addresses.disambiguated` | elements whose address got `~2`, `~3` and so on (what stands inside such an element is not counted again) |
| `addresses.assigned` | elements whose own segment is `~<tag>.<n>` (what stands inside is not counted again) |
| `notes.enacted`, `notes.editorial` | notes by layer |
| `notes_by_group` | notes by group word, `ungrouped` included |
| `footnotes` | footnotes |
| `with_images` | provisions and notes that hold an `img` |
| `references.<layer>.<outcome>` | references by layer (`operative`, `enacted`, `editorial`) and outcome, all eight outcomes for each layer |
| `outside_by_body` | references of outcome `outside`, all layers, by their first two segments |
| `not_imported.tables_of_contents` | `toc` elements |
| `not_imported.footnotes_in_tables_of_contents` | footnotes inside them |
| `not_imported.links_in_tables_of_contents` | links inside them |
| `not_imported.release_notes` | release notes |
| `not_imported.links_without_words` | links whose words are only white space |
| `not_imported.note_group_headings` | group headings, each of whose labels is not imported |

The fixtures in `fixtures/uslm/` each have their `import` profile in
`<name>.profile.json`, written by hand.

## Where the two differ

`baseline` leaves out what the reading imports: sections without an identifier;
court rules, reorganization plans and the other units that are not sections, and
their provisions; provisions of kinds other than the nine; and references whose
target is a container or a unit that is not a section, which `baseline` counts as
reaching no section. `import` also tells references to a unit or provision with a
status (`target-status`) and to a repeated identifier (`ambiguous`) from the
others, counts the nested section of a unit as one of its provisions rather than
as a unit, and gives every reference of every layer an outcome.

## Release 119-142

Measured on the archive `xml_uscAll@119-142.zip` (108,706,565 bytes, 58 members),
with four workers, on a machine where work runs through the scheduler:

```sh
lease run --cpus 8 --mem 16G --time 2h -- uv run --locked python -I conformance/usc_profile.py /library/corpora/uscode/119-142
```

The run read 58 members, 61,171 units, 9,623 containers and 774,018 provisions
in 16.5 seconds of wall-clock time. Its largest process peaked at 685,756 KiB of
resident memory, and the whole run at 1,238,089,728 bytes, page cache included.
It printed:

```json
{
  "baseline": {
    "provisions": 772863,
    "references": {
      "exact": 88935,
      "no_section": 964,
      "section_only": 820,
      "to_the_code": 90719
    },
    "sections": 60490,
    "sections_by_status": {
      "omitted": 2037,
      "operative": 51009,
      "renumbered": 368,
      "repealed": 5131,
      "reserved": 16,
      "transferred": 1911,
      "unknown": 1,
      "vacant": 17
    }
  },
  "import": {
    "addresses": {
      "aliases": 10530,
      "assigned": 172,
      "disambiguated": 269
    },
    "containers": 9623,
    "containers_by_kind": {
      "appendix": 5,
      "article": 11,
      "chapter": 2936,
      "compiledAct": 5,
      "courtRules": 5,
      "division": 41,
      "level": 110,
      "part": 1892,
      "reorganizationPlans": 1,
      "subchapter": 3881,
      "subdivision": 9,
      "subpart": 544,
      "subtitle": 98,
      "title": 85
    },
    "footnotes": 9727,
    "not_imported": {
      "footnotes_in_tables_of_contents": 328,
      "links_in_tables_of_contents": 64154,
      "links_without_words": 0,
      "note_group_headings": 73310,
      "release_notes": 58,
      "tables_of_contents": 3303
    },
    "notes": {
      "editorial": 83116,
      "enacted": 87335
    },
    "notes_by_group": {
      "courtRules": 20,
      "editorialNotes": 60853,
      "execDoc": 6419,
      "statutoryNotes": 76635,
      "ungrouped": 26524
    },
    "outside_by_body": {
      "/us/act": 53758,
      "/us/pl": 599160,
      "/us/stat": 321830
    },
    "prefix": "/us/usc",
    "provisions": 774018,
    "references": {
      "editorial": {
        "ambiguous": 70,
        "exact": 52559,
        "malformed": 0,
        "no-unit": 6736,
        "outside": 728990,
        "range-form": 291,
        "target-status": 8248,
        "unit-only": 2344
      },
      "enacted": {
        "ambiguous": 50,
        "exact": 94822,
        "malformed": 0,
        "no-unit": 1259,
        "outside": 229559,
        "range-form": 253,
        "target-status": 3598,
        "unit-only": 1180
      },
      "operative": {
        "ambiguous": 70,
        "exact": 88299,
        "malformed": 0,
        "no-unit": 278,
        "outside": 16199,
        "range-form": 291,
        "target-status": 900,
        "unit-only": 881
      }
    },
    "release": "119-142",
    "units": 61171,
    "units_by_kind": {
      "courtRule": 544,
      "level": 11,
      "reorganizationPlan": 107,
      "section": 60509
    },
    "units_by_status": {
      "in-force": 51664,
      "omitted": 2043,
      "renumbered": 369,
      "repealed": 5143,
      "reserved": 18,
      "transferred": 1914,
      "unknown": 3,
      "vacant": 17
    },
    "with_images": 17
  }
}
```
