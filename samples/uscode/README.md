# Sample of the United States Code

This directory holds three whole titles of the official XML release point
`119-142` published by the Office of the Law Revision Counsel of the
United States House of Representatives, and the counts of their text.
Together the three titles hold a table in operative text, continuation
text, court rules, notes with quoted provisions and a reference that does
not resolve. Title 1 is positive law; Title 11 Appendix and Title 45 are
not.

- `archives/` holds the publisher's archives, exactly as served, and
  `retrieval.json`, the record of each archive: its address, the time of
  its download in UTC, its size, its SHA-256 and its BLAKE3. The directory
  holds nothing else, and nothing is run from it.
- `profile.json` is what `conformance/usc_profile.py` prints for the
  archives: the counts of the two sets of counting rules that
  [usc-counting-rules.md](../../conformance/usc-counting-rules.md) states.

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

`cargo xtask uscode-sample` recomputes the size and both digests of every
archive and compares them with the record, fails naming the archive on
any difference, on an archive without a record and on a record without
its archive, and fails when `profile.json` is not, byte for byte, what the
script prints for the archives. `cargo xtask ci` runs it.

The text of the archives is a work of the United States Government; the
MIT License does not apply to it. See
[LICENSING.md](../../LICENSING.md#united-states-code-sample).
