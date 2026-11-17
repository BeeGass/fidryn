# Sample of the United States Code

This directory holds three whole titles of the official XML release point
`119-142` published by the Office of the Law Revision Counsel of the
United States House of Representatives, the counts of their text, and the
workspace of law modules imported from them. Together the three titles
hold a table in operative text, continuation text, court rules, notes with
quoted provisions and a reference that does not resolve. Title 1 is
positive law; Title 11 Appendix and Title 45 are not.

- `archives/` holds the publisher's archives, exactly as served, and
  `retrieval.json`, the record of each archive: its address, the time of
  its download in UTC, its size, its SHA-256 and its BLAKE3. The directory
  holds nothing else, and nothing is run from it.
- `profile.json` is what `conformance/usc_profile.py` prints for the
  archives: the counts of the two sets of counting rules that
  [usc-counting-rules.md](../../conformance/usc-counting-rules.md) states.
- `workspace/` is the generated workspace that `fidryn ingest uscode`
  writes from the archives: its manifest `workspace.json`, its lock
  `law.lock.json`, and one module of edition 2 for each of the 563 units
  and 72 containers of the three titles, 637 files in all. It is generated,
  never written by hand.

| Title | Archive | Bytes | Retrieved (UTC) |
| --- | --- | --- | --- |
| 1 | `https://uscode.house.gov/download/releasepoints/us/pl/119/142/xml_usc01@119-142.zip` | 42,241 | 2026-11-17T18:44:23Z |
| 11 Appendix | `https://uscode.house.gov/download/releasepoints/us/pl/119/142/xml_usc11a@119-142.zip` | 375,834 | 2026-11-17T18:44:24Z |
| 45 | `https://uscode.house.gov/download/releasepoints/us/pl/119/142/xml_usc45@119-142.zip` | 524,586 | 2026-11-17T18:44:24Z |

The archives were fetched, and their record written, with:

```sh
uv run --locked python -I tools/fetch_uscode.py 119-142 --out samples/uscode/archives --title 1 11a 45
```

Run again while every archive matches its record, the command opens no
connection and changes nothing. It refuses to replace an archive whose
bytes differ from its record.

The profile is regenerated with the command below; `--out` writes it to
`profile.json`, and the same text is printed.

```sh
uv run --locked python -I conformance/usc_profile.py samples/uscode/archives --out samples/uscode/profile.json
```

The workspace is regenerated, from the repository root, with the command
below, which writes it with
`fidryn ingest uscode samples/uscode/archives --out samples/uscode/workspace`
and then runs every check of the sample on it:

```sh
cargo xtask uscode-sample --update
```

Whether the committed workspace is what the ingester writes, that it
checks, and the text of a section and of the notes below it:

```sh
fidryn ingest uscode samples/uscode/archives --out samples/uscode/workspace --verify
fidryn check samples/uscode/workspace --no-cache
fidryn text samples/uscode/workspace /us/usc/t1/s1
```

`cargo xtask uscode-sample`, which `cargo xtask ci` runs, checks six
things, in this order, and fails with a message that names the first
difference:

1. it recomputes the size and both digests of every archive and compares
   them with the record, and fails naming the archive on any difference,
   on an archive without a record and on a record without its archive;
2. it imports the archives into a temporary directory and compares every
   directory and file there, byte for byte, with `workspace/`, in both
   directions, and fails naming the first path that differs;
3. it imports them again on one worker, and fails when that gives other
   bytes than the import on eight;
4. it checks `workspace/` with `fidryn check --no-cache`, and fails on an
   error;
5. it fails when `profile.json` is not, byte for byte, what the script
   prints for the archives, and when its counts of references by layer and
   outcome are not those of the check report;
6. it fails when what `conformance/uslm_text.py` prints for the archives,
   an independent reading of the publisher's XML, is not, line for line,
   what `fidryn text samples/uscode/workspace --json` prints, and names the
   first line that differs.

The step writes nothing in the repository, a cache included; with
`--update` it writes `workspace/` alone.
[Law modules](../../docs/law-modules.md#the-sample) describes it.

The text of the archives, and of the workspace generated from them, is a
work of the United States Government; the MIT License does not apply to
it. See [LICENSING.md](../../LICENSING.md#united-states-code-sample).
