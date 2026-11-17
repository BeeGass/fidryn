# Law modules

This reference gives the rules by which a module of law, and each provision
and note in it, is named, where the file of a module stands in its
workspace, how the text of a module is normalized, what a module holds, and
how its provisions, notes and layers are hashed. The crate
`fidryn-provision` implements them. In the examples, `/zz/demo` is an
invented corpus.

## Addresses

An address names a module, a provision or a note. It is `/` followed by one
or more segments separated by `/`:

```text
Address = "/" Segment { "/" Segment }
Segment = one or more of: ASCII letters, ASCII digits, "-", ".", "~" and the en dash U+2013
```

- A segment is never empty, never `.` and never `..`.
- Two addresses are equal when their texts are equal byte for byte:
  `/us/usc/t1/s8/a` and `/us/usc/t1/s8/A` are different addresses.
  Addresses are ordered by the bytes of their text.
- The full stop and the en dash occur in the identifiers of the United
  States Code: `s1395i–3`, `rule71.1`, `s44...46`. No identifier of the Code
  holds `~`. An ingester writes `~`, and only an ingester, to mark what it
  added to an identifier, as in `s1932~2`.
- The first segment names the authority that issues the names below it, the
  second the corpus. Nothing interprets either.
- The parent of an address is the address without its last segment. An
  address of one segment has no parent.
- An address is within another when it equals the other or lies below it at
  a segment boundary: `/us/usc/t1/s8/a` is within `/us/usc/t1/s8`, and
  `/us/usc/t1/s80` is not.

In JSON an address is a string that holds its text. A string that is no
address is refused when it is read.

`Address::parse` refuses a text that is no address and names the fault, with
the segment it lies in, counted from 0:

| Text | Refused because |
| --- | --- |
| the empty text | it is empty |
| `us/usc/t1` | it does not begin with `/` |
| `/us//t1` | segment 1 is empty |
| `/us/usc/` | segment 2 is empty |
| `/us/./t1`, `/us/../t1` | segment 1 is `.` or `..` |
| `/us/usc/t1 s8`, `/us/usc/t1_s8`, `/us/usc/t1%20s8` | segment 2 holds a space, `_` or `%` |

## File paths

A module with address `A` is the file `layout::module_path(A)` under its
workspace's root:

1. Each segment of the address maps to one path segment, character by
   character:

   | Character of the address | In the path |
   | --- | --- |
   | a lower-case ASCII letter, a digit or `~` | kept |
   | an upper-case ASCII letter | `_` followed by its lower-case form |
   | the en dash U+2013 | `-` |
   | the ASCII hyphen | `%2D` |
   | a full stop | kept, except as the first or the last character of a segment and before the letters `fr` that end a segment, where it is `%2E` |

2. If the mapped segment, or its part before its first full stop, compared
   without regard to case, is one of `con`, `prn`, `aux`, `nul`, `com1` to
   `com9` or `lpt1` to `lpt9`, its first character is written as `%`
   followed by the two upper-case hexadecimal digits of its code.
3. The path is the mapped segments joined by `/`, followed by `.fr`. The
   separator is `/` on every host.

| Address | File path |
| --- | --- |
| `/us/usc/t29/s2611` | `us/usc/t29/s2611.fr` |
| `/us/usc/t42/s1395i–3` | `us/usc/t42/s1395i-3.fr` |
| `/us/usc/t2/s44...46` | `us/usc/t2/s44...46.fr` |
| `/us/usc/t5/ptIII/sptE` | `us/usc/t5/pt_i_i_i/spt_e.fr` |
| `/us/usc/t28a/courtRules/Civil/rule6` | `us/usc/t28a/court_rules/_civil/rule6.fr` |
| `/us/usc/t28/s1932~2` | `us/usc/t28/s1932~2.fr` |
| `/zz/demo/a-b` | `zz/demo/a%2Db.fr` |
| `/zz/demo/a.` | `zz/demo/a%2E.fr` |
| `/zz/demo/con` | `zz/demo/%63on.fr` |
| `/zz/demo/con.1` | `zz/demo/%63on.1.fr` |
| `/zz/demo/lpt1` | `zz/demo/%6Cpt1.fr` |
| `/zz/demo/.a` | `zz/demo/%2Ea.fr` |
| `/zz/demo/a.fr/b` | `zz/demo/a%2Efr/b.fr` |
| `/zz/demo/a..fr` | `zz/demo/a.%2Efr.fr` |

No path segment begins with a full stop, and none ends in a full stop or in
`.fr`, so the file of one module never has the name of another module's
directory: `/zz/demo/a.fr/b` is `zz/demo/a%2Efr/b.fr`, beside the file
`zz/demo/a.fr` of the address `/zz/demo/a`.

The mapping is one to one, so two addresses never share a path, also on a
file system that ignores case: `/zz/demo/ch6A` is `zz/demo/ch6_a.fr` and
`/zz/demo/ch6a` is `zz/demo/ch6a.fr`.

`layout::address_of_path` reads a path back as its address. It refuses
every path that the three rules do not write, such as `zz/demo/CH1.fr`,
`zz/demo/a_.fr`, `zz/demo/a%2d.fr`, `zz/demo/a%5F.fr`, `zz/demo/con.fr`,
`zz/demo/.a.fr`, `zz/demo/a.fr/b.fr`, `zz/demo/a.txt` and `zz\demo\a.fr`.
The path `zz/demo/a-b.fr` is the address `/zz/demo/a–b`, with an en dash.

## Normalization

The text of a law module is normalized. `normalize::normalize` normalizes a
text in four steps:

1. Put the text into Unicode Normalization Form C.
2. Replace each of the 25 characters with the Unicode property `White_Space`
   with U+0020: U+0009 to U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000 to
   U+200A, U+2028, U+2029, U+202F, U+205F and U+3000.
3. Replace each run of U+0020 with one.
4. Remove U+0020 at the start and at the end.

Nothing else changes: no case folding, and no change to quotation marks or
dashes. U+200B (zero width space) and U+FEFF are not white space and are
kept. `normalize::is_normalized` tells whether a text is normalized, that
is, whether normalizing it leaves it unchanged.

| Text | Normalized |
| --- | --- |
| two spaces, `a`, two spaces, `b`, one space | `a b` |
| `a`, U+00A0, `b` | `a b` |
| `§`, U+202F, `1` | `§ 1` |
| `Current`, U+2000, `through` | `Current through` |
| `a`, tab, `b`, line feed, `c`, carriage return, line feed, `d` | `a b c d` |
| `e`, U+0301 | `é` (U+00E9) |
| `1`, U+00A0, `000` | `1 000` |
| U+3000, `a`, U+1680 | `a` |
| `a`, U+200B, `b` | unchanged |
| U+FEFF, `a` | unchanged |
| `[sic]` | unchanged |
| `“x” — y` | unchanged |

The unit of normalization is the block: a paragraph, a heading, a number or
a paragraph of a table cell. The text of a block is the words of its runs,
concatenated, and that text is normalized: a run may begin or end with a
space, a block may not.

## Provisions, notes and layers

A module of law has one of three shapes:

- A **unit** holds one tree of provisions, whose top provision has the
  module's address. It also records the other addresses the source gives
  it (`aliases`), the container it stands in (`within`), the member of the
  source archive it was read from (`source`) and whether the source marks it
  as positive law (`positive_law`, which may be unknown), and it holds a
  source credit, notes, footnotes and definitions.
- A **container**, such as a chapter, has a kind, a number, a heading and a
  status of its own and lists its members: the containers and units directly
  below it, in document order. It holds a source credit, notes and
  footnotes, and records `aliases`, `within`, `source` and `positive_law` as
  a unit does.
- A **curated** module is written by hand. It imports modules, names
  addresses (citations) and binds names to provisions (bindings). A binding
  records the release it was made against and two hashes, one of the
  provision and one of the notes.

The crate holds the model in its module `model`.

### Provisions

A provision has an address, a kind, a number, a heading and a status, each
of the last three optional, and its items: text parts and child provisions,
in document order. The address of a child lies below the address of its
parent.

- The kind and the status are identifiers, such as `section` and
  `repealed`. The model gives them no meaning and knows no list of them. A
  provision without a status is in force.
- A number has a value, its machine form, and the text it is shown as,
  which may hold a footnote marker: the value `8` shown as `§ 8.`.
- A text part has a role and at least one block:

| Role | The part holds |
| --- | --- |
| `chapeau` | the words that lead into the children |
| `content` | the text of the provision |
| `continuation` | words that follow children |
| `proviso` | a proviso |

### Texts and blocks

A text is a list of runs:

| Run | Holds |
| --- | --- |
| words | plain words |
| link | the linked words, and the target as the source writes it |
| superscript | superscript words |
| subscript | subscript words |
| footnote marker | the marker as shown, and the number of the footnote it points at |

The plain text of a text is the words of its runs, footnote markers left
out, concatenated; it is the text that is normalized. `Text::from_runs`
merges adjacent runs of words into one and drops every run whose words are
empty. A footnote marker has no words of its own and is kept.

A block is a paragraph, a subheading, a table, quoted content or an image:

- A table is a list of rows. A row belongs to the head, the body or the foot
  of its table and holds cells. A cell is a header cell or not, spans one or
  more columns and one or more rows, and holds blocks. An empty cell holds
  none.
- Quoted content has the origin the source names for it, if any, and items:
  blocks, text parts and levels. A level, such as a quoted section, has a
  kind, an optional number, an optional heading and items of its own, and
  no address.
- An image names the source of its picture. The picture is not part of the
  model.

### Notes, footnotes and definitions

A note has an address, a layer, the group word of the source (`ungrouped`
when there is none), its topics, an optional heading and blocks. The layer
of a note is enacted or editorial. A note belongs to the unit or the
container it is anchored in.

A footnote belongs to its module. The footnotes of a module are numbered
from 1 in document order; each has the marker it shows and its blocks. A
footnote marker names its footnote by that number.

A definition records the provision that holds it (`at`), the terms it
defines, its kind (`means`, `includes`, `does_not_include` or `borrowed`),
the words that state its reach or none when the text states none, the
region the reach covers, its basis (`enacted`, `restated` or `unresolved`),
what a borrowed definition takes its meaning from (`borrowed_from`), and a
note to read with it (`see_note`). Definitions are in no hash.

### Layers

The text of a module falls into three layers:

| Layer | Holds |
| --- | --- |
| operative | the provisions of a unit; the kind, number, heading, status and members of a container |
| enacted | the enacted notes |
| editorial | the source credit, the editorial notes, the footnotes and the footnote markers |

### In JSON

In JSON the model has the form that `serde` derives: a struct is an object
of its fields, an absent value is `null`, a variant without fields is its
name in snake case (`content`, `does_not_include`), and any other variant
is an object whose one key is its name in snake case. A text is
`{"runs": [...]}`, and a paragraph that holds a link is:

```json
{"para": {"runs": [{"words": "As used in "}, {"ref": {"words": "section 2", "target": "/zz/demo/t1/s2"}}]}}
```

## Hashes

A hash is 32 bytes of BLAKE3, written as 64 lower-case hexadecimal digits.
`Hash32` reads only that form: a text of another length, or one with an
upper-case or any other character, is refused and the fault is named.

Every hash is BLAKE3 over a sequence of frames. A frame is one tag byte,
the length of its value as eight bytes little-endian, and the value. The
first frame of every hash has the tag `0x00` and holds a domain string.
Below, `‖` is concatenation and `F(t, v)` the frame with tag `t` and
value `v`:

| What | Frames |
| --- | --- |
| a text `T` | `F(0x10, UTF-8 of the plain text of T)`, then for each run that is not words, in order: a link `F(0x11, start ‖ end ‖ UTF-8 of its target)`; a superscript `F(0x12, start ‖ end)`; a subscript `F(0x13, start ‖ end)`; a footnote marker nothing. `start` and `end` are byte offsets into the plain text, eight bytes little-endian each, `end` exclusive. |
| an absent text | `F(0x1F, empty)` |
| a block | a paragraph `F(0x20, text)`; a subheading `F(0x21, text)`; a table `F(0x22, table)`; quoted content `F(0x23, quoted)`; an image `F(0x24, UTF-8 of its source)` |
| a table | for each row `F(0x30 + group, cells)`, with the group 0 for the head, 1 for the body and 2 for the foot; a cell is `F(0x34, colspan ‖ rowspan ‖ blocks)`, or `F(0x35, ...)` for a header cell, each span four bytes little-endian |
| quoted content | `F(0x40, UTF-8 of the origin, or empty)`, then each item: a block as above, a part as below, a level `F(0x41, F(0x01, kind) ‖ number ‖ heading ‖ items)` |
| a number | `F(0x02, UTF-8 of the value) ‖ F(0x03, the shown text as a text)`, or `F(0x04, empty)` when there is none |
| a text part | `F(0x50 + role, blocks)`, with the role 0 for `chapeau`, 1 `content`, 2 `continuation` and 3 `proviso` |
| a provision (its tree hash) | BLAKE3 of `F(0x00, "fidryn.provision/v1") ‖ F(0x01, kind) ‖ number ‖ heading ‖ F(0x05, status or empty) ‖ items`, where a text part is as above and a child is `F(0x60, its tree hash)` |
| a note | BLAKE3 of `F(0x00, "fidryn.note/v1") ‖ F(0x70, group) ‖ F(0x71, topics joined by one space) ‖ heading ‖ blocks` |
| a container | BLAKE3 of `F(0x00, "fidryn.container/v1") ‖ F(0x01, kind) ‖ number ‖ heading ‖ F(0x05, status or empty)`, then `F(0x61, address)` for each member in order |
| the enacted layer | BLAKE3 of `F(0x00, "fidryn.layer.enacted/v1")`, then for each enacted note in the byte order of its address `F(0x72, address) ‖ F(0x73, note hash)` |
| the editorial layer | BLAKE3 of `F(0x00, "fidryn.layer.editorial/v1") ‖ the credit as a text, or an absent text`, then the editorial notes as in the enacted layer, then for each footnote in ordinal order `F(0x74, UTF-8 of its marker) ‖ blocks`, then for each footnote marker of the module in document order `F(0x75, UTF-8 of the address it is recorded with) ‖ F(0x76, UTF-8 of the marker as shown) ‖ F(0x77, its footnote number as four bytes little-endian)` |
| the operative layer | of a unit, the tree hash of its top provision; of a container, its container hash |

The address of a provision is not in its tree hash, and the address and
the layer of a note are not in its hash. Definitions are in no hash. A
container's credit is in its editorial layer and not in its container hash.

The document order of the footnote markers of a unit is: the tree of
provisions depth first, taking for each provision its number, its heading
and then its items in order; then the credit; then the notes in the byte
order of their addresses, as the layers take them, the heading of each
before its blocks; then each footnote in ordinal order. For a container it
is its number, its heading, its credit, its notes in the same order, and
its footnotes. A marker in a provision is recorded with that
provision's address, and a marker in a note, its quoted content included,
with the note's address. A marker in a credit, in a footnote, or in a
container's own number or heading is recorded with the module's address.

What follows from these rules:

- A change to any word, number, heading, status, link target or link
  extent, superscript, subscript, table cell or span of a provision changes
  its tree hash and the tree hash of every provision above it, and of no
  other provision.
- A footnote, a footnote marker, and the provision a marker stands in
  change the editorial layer and nothing else. Where in its provision a
  marker stands is in no hash.
- The notes and their footnote markers are taken in the byte order of the
  notes' addresses, so the order in which a module lists its notes changes
  no layer hash. Moving a note to a new address changes the hash of its
  layer and not the note's own hash.

`hash::hash_provision`, `hash::hash_note`, `hash::hash_container`,
`hash::layer_hashes_of_unit` and `hash::layer_hashes_of_container` compute
them. A second implementation in Python, `conformance/hash_vectors.py`,
computes the same hashes of the invented models in
`conformance/fixtures/hash/vectors.json`, and the tests of both hold their
values to the digests recorded there.
