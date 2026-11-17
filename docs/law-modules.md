# Law modules

This reference gives the rules by which a module of law, and each provision
and note in it, is named, where the file of a module stands in its
workspace, and how the text of a module is normalized. The crate
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
