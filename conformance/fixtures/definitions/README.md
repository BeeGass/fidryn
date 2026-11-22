# Fixtures of the definition rules

Invented for tests: every file in this directory holds invented text of
the invented title 99 (`/us/usc/t99/...`), not the text of any law. The
files are JSON lines, which cannot carry a first line that says so; this
README says it for them.

Both implementations of the rules of
[Definitions](../../../docs/law-modules.md#definitions) are held to the
same two files: the importer (`crates/fidryn-ingest-uscode`) and the
independent script (`conformance/usc_definitions.py`).

## `vectors.jsonl`

The test vectors of the rules: 238 lines, one JSON object to a line, each
an invented unit with the records the rules give it. The file is, byte for
byte, the file of vectors that was written with the rules, and it may not
be altered: its SHA-256 is
`ffc209c55aa669d3144d7ace126333e4a00d54f998a70246d4e75e74db66ab6c`.

A vector has an `id`, the `rule` it holds by its number in the reference
(such as `D7.3.4`), what it `holds` in words, the `unit` it is read from,
the records it `expect`s, and, when it expects none, why
(`none_because`). The unit has an `address`, `positive_law`, its
`containers` from the nearest outward (each an `address` and a `kind`), its
`notes` (each an `address`, a `layer`, a `heading` and the text of each of
its own `paragraphs`) and its `root` provision. A provision has an
`address`, a `kind` and `items` in order, each a part
(`{"part": role, "blocks": [...]}`) or a child (`{"child": provision}`). A
block is a paragraph (`{"p": [runs]}`) or a block that is not one
(`{"table": []}`). A run is words (`{"words": text}`), a link
(`{"link": text, "to": target}`), a superscript (`{"sup": text}`) or a
footnote marker (`{"marker": shown}`). A record has all ten fields of a
definition, `at`, `terms`, `kind`, `words`, `condition`, `region`,
`basis`, `borrowed_from`, `borrowed_words` and `see_note`, in that order,
`null` for an absent one; `words` is `null` for `unstated`. The records of
a vector are compared as a multiset: their order does not matter.

The importer's test `every_vector_gives_the_records_it_expects`
(`crates/fidryn-ingest-uscode/src/definitions/tests.rs`) checks the
SHA-256, builds each unit in the model, and names the vector and the first
field that differs when the records do not match. The tests of the script,
in `conformance/test_usc_definitions.py`, run every vector too.

## `second-expected.jsonl`

The records of the invented fixture
[`../uslm/definitions-second.xml`](../uslm/definitions-second.xml), read
as the member `usc99.xml`: one JSON object to a line, with the ten fields
of a record in their order and `null` for an absent one, sorted by the
address of the provision and then by the terms. The fixture holds one
paragraph at least for each form of a condition (before the terms, at the
end of the reach's Object, in a chapeau, in the aside and after the verb
form), for source words, for a meaning made by regulations under a linked
provision, for a later sentence, a later block of a part and a
continuation, for nested quotation, for a footnote number marked only as a
superscript, and for a digit written as words. The records were written
from the rules, one by one.

The importer's test `the_second_fixture_gives_the_records_written_for_it`
(`crates/fidryn-ingest-uscode/src/definitions/tests.rs`) holds the
importer to this file line for line, and the tests of the script hold the
script to it.
