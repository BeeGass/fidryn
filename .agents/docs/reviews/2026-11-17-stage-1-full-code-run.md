# The first full import of the Code: release 119-142

This is the record of the first import of the whole United States Code into a
workspace of law modules: release point 119-142 of the official XML release
point published by the Office of the Law Revision Counsel, one archive of 58
files, imported into a workspace of 70,794 modules, checked, measured,
compared with the independent reading of `conformance/`, and committed as the
first commit of a local law repository. It was run on 2026-11-17 (Eastern
time), on one workstation, through the scheduler of that machine.

Every number below was printed by a command whose line is given. Times and
peaks of memory come from `/usr/bin/time -v`: "wall" is its elapsed time and
"peak" its maximum resident set size, which is that of the largest single
process of the command. The record holds no text of the Code beyond addresses
and the short examples asked for.

**Result.** Every step succeeded, and nothing in the full release showed a
defect of the repository, so this repository changes only by this record.

| What was asked | What the run gave |
| --- | --- |
| The import | 61,171 units and 9,623 containers, 6.0 s, 2.2 GB peak |
| A second import | byte for byte the same as the first, by `diff -r` |
| Item 1: the cold check | exit 0, no error, no warning; 3.3 s, 0.85 GB peak (bound: 10 minutes, 16 GB) |
| The warm check | every module from the cache; 1.7 s (bound: 30 s) |
| Item 2: the counts | the `baseline` profile reproduces F1 exactly; the ledger equals the profile's `import` on all 14 keys they share; each difference from F1 is explained below |
| Item 3: the references | the check report's 1,236,877 references by layer and outcome equal the profile's and the ledger's |
| Item 4: the text | 1,015,386 lines compared, 0 differ; `--verify` prints `ok` |
| Definitions | 34,329 in the workspace and in the independent list, equal in every field |
| The law repository | one signed commit, `/us/usc: release 119-142` |

## Limits and the build

Every run of the built binary was one job of its own, one at a time, with these
limits: 16 threads and 16 GB from the scheduler, half an hour of wall time,
16 GiB of address space and an hour of processor time.

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ...
```

Python over the full release ran as
`lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I ...`,
from the root of the repository.

The binary is the release build of commit
`d0658b6bce0b04489a3c706a587b44abbc92716d`, on a clean tree:

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-check lease run --cpus 16 --mem 25G --time 1h -- cargo build --release -p fidryn-cli --locked --offline
```

- Rust 1.98.1; the build took 26 s.
- The binary is 13,837,000 bytes.
  - BLAKE3 `1588aa901707193fda1858482da57f874c76152eff36cd5af10564f1c1412427`
  - SHA-256 `f2dd81dde0ade77b922bfb1a37155ef26f803377fc6d0548f4eb8bfe681a6ef3`
- It was copied, unchanged, to `/cache/fidryn/task-21/fidryn`, so that no later
  build could replace it. "BINARY" below is that file.

The version is the same in all three places:

- `fidryn --version` prints `fidryn 0.3.0`;
- `toolchain` of the manifest the import wrote is `"0.3.0"`;
- `generator` of its lock is `{"name": "fidryn-ingest-uscode", "version": "0.3.0"}`.

No check warned `W760`.

Where things are:

- the archive: `/library/corpora/uscode/119-142/`;
- the law repository: `/work/projects/law-local/`;
- the cache of the checks: `/cache/fidryn/task-21-cache/`;
- every large output: `/cache/fidryn/task-21/`, among them the second import,
  the two text streams, the check reports, the unresolved list and the
  profile.

## The archive

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I tools/fetch_uscode.py 119-142 --out /library/corpora/uscode/119-142 --all
```

- It printed `kept xml_uscAll@119-142.zip` and exited 0, both times it was run.
  The second run, timed, took 0.10 s wall with a peak of 25 MB.
- It opened no connection: an archive that matches its record is kept.
- The record `retrieval.json` (schema `fidryn.retrieval/v0.1`) says the
  archive was retrieved at 2026-11-17T22:30:11Z:
  - 108,706,565 bytes;
  - SHA-256 `a216a8b0fc4ebc8500ae3c7dc626d9731804c00def2ba2e6e7900a0e34e856f0`;
  - BLAKE3 `4ee218a6b6447fd7b271961d3bf2915f75b3197eb729a4fb3df233c620c1a749`.

```sh
cmp /library/corpora/uscode/119-142/xml_uscAll@119-142.zip /library/corpora/uscode/119-142.measured/xml_uscAll@119-142.zip
```

`cmp` printed nothing and exited 0, and `sha256sum` gives both files the digest
above. The archive fetched again is the same bytes as the archive measured on
2026-11-16 for F1 to F15.

## The import

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ingest uscode /library/corpora/uscode/119-142 --out /work/projects/law-local/uscode --jobs 16
```

- Output: `ingested: 61171 units, 9623 containers from 1 archives (release 119-142)`.
- Exit 0, 6.04 s wall, 26.2 s user, 2,166,784 KiB peak.
- The workspace has 70,797 files in 1,680 directories: 70,794 modules,
  `workspace.json`, `law.lock.json` and `coverage.json`.

The same import was made a second time, into `/cache/fidryn/task-21/uscode-2`:

- It printed the same line and exited 0, in 6.14 s wall with a peak of
  2,200,012 KiB.
- `diff -r /work/projects/law-local/uscode /cache/fidryn/task-21/uscode-2`
  printed nothing and exited 0 (0.42 s wall).
- Both trees have 70,797 files and 624,035,591 bytes.

The import writes the same bytes each time it runs.

## The cold and the warm check (item 1)

The cache directory `/cache/fidryn/task-21-cache` was made empty just before:
`find` listed only the directory itself.

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY check /work/projects/law-local/uscode --jobs 16 --cache-dir /cache/fidryn/task-21-cache --report /cache/fidryn/task-21/report-cold.json --unresolved /cache/fidryn/task-21/unresolved-cold.jsonl
```

- Output: `modules: 70794; from cache: 0; errors: 0; warnings: 0`, then `ok`.
- Exit 0, 3.29 s wall, 18.6 s user, 831,792 KiB peak.
- Nothing was printed on stderr but the timer's lines.
- The bound of design 9.6, item 9 is 10 minutes and 16 GB. The check took
  0.55 percent of that time and 5 percent of that memory.

The same command again, with nothing changed, writing `report-warm.json` and
`unresolved-warm.jsonl`:

- Output: `modules: 70794; from cache: 70794; errors: 0; warnings: 0`, then `ok`.
- Exit 0, 1.65 s wall, 831,536 KiB peak. The bound is 30 seconds.
- The two reports differ in one line, `"from_cache": 0` and `"from_cache": 70794`.
  The two unresolved lists are the same bytes (`cmp`).

A check reads every module from the cache when nothing changed, and its
output is that of a check with an empty cache.

## The counts (item 2)

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I conformance/usc_profile.py /library/corpora/uscode/119-142 --out /cache/fidryn/task-21/profile.json
```

- Exit 0, 17.33 s wall, 686,108 KiB peak, four workers.
- What it printed is, value for value, the profile recorded in
  `conformance/usc-counting-rules.md`.

The workspace's ledger `coverage.json` (10,818 bytes) was compared with it key
for key, and the check report with both, by `compare_counts.py`, a script kept
outside the repository that reads the three JSON files:

- **`baseline` equals F1** in every number: provisions, sections, sections by
  status, and the four counts of references.
- **The ledger's `totals` and the profile's `import`** share 14 keys, and each
  is equal: `addresses`, `containers`, `containers_by_kind`, `footnotes`,
  `not_imported`, `notes`, `notes_by_group`, `outside_by_body`, `provisions`,
  `references`, `units`, `units_by_kind`, `units_by_status` and
  `with_images`. `prefix` and `release` are also equal: `/us/usc` and
  `119-142`.
  - Only the ledger has `definitions` and `definitions_by_kind`.
- **The check report's** `references` and `outside_by_body` equal both. Its
  1,236,877 references, with the 64,154 links in tables of contents that are
  not imported, make the 1,301,031 links with a target of F12.
- **`BINARY coverage /work/projects/law-local/uscode`**, under the limits
  above, took 1.47 s with a peak of 564,184 KiB, and printed `coverage.json`
  byte for byte (`cmp`).
- **The notes:** 87,335 enacted and 83,116 editorial, 170,451 in all.

### The three sets side by side

"Import" is the ledger, which equals the profile's `import`. Where F1 counts
sections, the import counts units: a unit of kind `section` is a section.

| Count | F1 | `baseline` | Import |
| --- | --- | --- | --- |
| sections | 60,490 | 60,490 | 61,171 units, of which 60,509 are sections |
| in force (`none` in F1) | 51,009 | 51,009 | 51,664 |
| repealed | 5,131 | 5,131 | 5,143 |
| omitted | 2,037 | 2,037 | 2,043 |
| transferred | 1,911 | 1,911 | 1,914 |
| renumbered | 368 | 368 | 369 |
| vacant | 17 | 17 | 17 |
| reserved | 16 | 16 | 18 |
| unknown | 1 | 1 | 3 |
| provisions | 772,863 | 772,863 | 774,018 |
| operative references to the Code | 90,719 | 90,719 | 90,719 |
| resolve exactly | 88,935 | 88,935 | 89,269: `exact` 88,299, `target-status` 900 and `ambiguous` 70 |
| reach the section only | 820 | 820 | 881: `unit-only` |
| reach no section | 964 | 964 | 569: `range-form` 291 and `no-unit` 278 |

The import is grouped against F1 as S9 does it:

- `exact`, `target-status` and `ambiguous` together are "resolve exactly";
- `unit-only` is "reach the section only";
- `range-form` with `no-unit` is "reach no section".

### Where each difference comes from

The differences were found element by element by a script kept outside the
repository, `analyze_counts.py`, run like the profile (17.54 s, 734,812 KiB).
On one parse of each file, it reads with the reading of
`conformance/uslm_common.py`, the reading the `import` profile uses. It also
counts with the rules of `baseline`, keeping for each counted element the kind
of the outermost unit it stands in, and for each link the address it stands
in. Its result agrees with both profiles in every total:

- 772,863 and 774,018 provisions;
- the same multiset of 90,719 operative targets on both sides.

**Sections and units, +681.** 61,171 = 60,490 + 544 + 107 + 11 + 20 − 1.

| Difference | Count | Rule | Examples |
| --- | --- | --- | --- |
| court rules | +544: 538 in force, 2 reserved, 2 unknown, 1 renumbered, 1 transferred | S16, "court rules, reorganization plans and the other units that are not sections" | `/us/usc/t28a/courtRules/Civil/rule1`, `/us/usc/t18a/courtRules/Crim/rule1`, `/us/usc/ptI/rule1002` |
| reorganization plans | +107, all in force | the same | `/us/usc/t5a/reorgPlan/1939/I`, `/us/usc/t5a/reorgPlan/1939/II`, `/us/usc/t5a/reorgPlan/1940/III` |
| generic levels that are units | +11, all in force; 2 of them without an identifier | the same | `/us/usc/t28a/courtRules/App/1A`, `/us/usc/t28a/courtRules/App/1B`, `/us/usc/t11a/~courtRules.1/~level.2` |
| sections without an identifier | +20: 12 repealed, 6 omitted, 2 transferred | S16, "the 20 sections without an identifier" | `/us/usc/t5a/~compiledAct.1/~section.1`, `/us/usc/t5a/~compiledAct.2/~section.3`, `/us/usc/t18a/~level.1/~section.1` |
| the section inside a section | −1, in force | S7, "the unit of a text is the outermost unit that holds it" | `/us/usc/t25/s5329/s1` |

By status, these add up to the differences in the table:

| Status | Difference | Made of |
| --- | --- | --- |
| in force | +655 | 538 court rules, 107 plans and 11 levels, less the section inside a section |
| repealed | +12 | 12 sections without an identifier |
| omitted | +6 | 6 sections without an identifier |
| transferred | +3 | 2 sections without an identifier and the court rule `/us/usc/t18a/courtRules/Crim/rule22` |
| renumbered | +1 | the court rule `/us/usc/t28a/courtRules/Civil/rule71A` |
| reserved | +2 | the court rules `/us/usc/t18a/courtRules/Crim/rule19` and `rule39` |
| unknown | +2 | the court rules `/us/usc/t28a/courtRules/App/rule3.1` and `rule5.1` |
| vacant | 0 | |

The sections of kind `section` are 60,509 = 60,490 + 20 − 1.

**The one unit of F4.** F4 counted 61,172 units, of which 60,510 were sections;
the import has 61,171 and 60,509. The one is the section that 25 U.S.C. 5329
holds:

- It is a model agreement marked up as a section, with the identifier
  `/us/usc/t25/s1` (F2), and it stands in no quoted content.
- `baseline` counts it as a section in force, as F1 does, and F4 counted it as
  a unit of its own.
- By S7 ("the unit of a text is the outermost unit that holds it: the section
  that stands inside 25 U.S.C. 5329 is a provision of kind `section` of that
  unit"), the import makes it the provision `/us/usc/t25/s5329/s1` of the unit
  `/us/usc/t25/s5329`.
- So the real 25 U.S.C. 1 keeps the address `/us/usc/t25/s1`, and no `~2` is
  needed for it.

It is the only section inside a section in the release.

**Provisions, +1,155.** Every provision of the nine kinds that has an
identifier is counted by both profiles, in every kind of unit. Kind by kind and
unit kind by unit kind, the two counts are equal: for example 60,490 sections,
133,693 subsections and 248,888 paragraphs in sections, and 1,500 subsections
in court rules. The import adds:

| What `baseline` does not count | Count | Rule | Examples |
| --- | --- | --- | --- |
| the top provision of a court rule | 544 | S16, the units that are not sections | `/us/usc/ptI/rule1002`, `/us/usc/ptI/rule1003`, `/us/usc/ptI/rule1004` |
| the top provision of a reorganization plan | 107 | the same | `/us/usc/t5a/reorgPlan/1939/I`, `/us/usc/t5a/reorgPlan/1939/II`, `/us/usc/t5a/reorgPlan/1940/III` |
| the top provision of a generic level that is a unit | 11 | the same | `/us/usc/t28a/courtRules/App/1A`, `/us/usc/t28a/courtRules/App/1B`, `/us/usc/t28a/courtRules/App/~level.2` |
| the top provision of a section without an identifier | 20 | S16, the 20 sections without an identifier | `/us/usc/t18a/~level.1/~section.1`, `/us/usc/t5a/~compiledAct.1/~section.1`, `/us/usc/t5a/~compiledAct.1/~section.2` |
| provisions of kind `level` in court rules | 126 | S16, "provisions of kinds other than the nine" | `/us/usc/t18a/courtRules/Crim/rule4/c/3/D/ii/a`, `.../ii/b`, `.../ii/c` |
| provisions of kind `level` in sections | 347, 2 of them without an identifier | the same | `/us/usc/t15/s1114/1/a`, `/us/usc/t15/s45/m/1/B/1`, `/us/usc/t42/s6914a/~level.1` |

544 + 107 + 11 + 20 + 126 + 347 = 1,155. S16 says `baseline` leaves out court
rules and plans "and their provisions". That holds only for their top
provisions and their `level` provisions. The 5,953 provisions of the nine kinds
inside court rules have identifiers, and `baseline` counts them.

**References, +334, +61 and −395.** For each operative reference to the Code,
its class in `baseline` and its outcome in the import:

| `baseline` | Import | Count | What the target is | Examples: target (stands in) |
| --- | --- | --- | --- | --- |
| exact | `exact` | 88,292 | | |
| exact | `target-status` | 577 | a section with a status | `/us/usc/t42/s1395ggg` (`/us/usc/t10/s1108/c/3`), `/us/usc/t50/s4601` (`/us/usc/t10/s130/a`), `/us/usc/t5/s5532` (`/us/usc/t10/s1466/d/1`) |
| exact | `ambiguous` | 66 | a beginning of the target is held twice | `/us/usc/t15/s1693b/a/2` (`/us/usc/t15/s1693o–1/a/6/B`), `/us/usc/t21/s812/c` (`/us/usc/t20/s1415/k/7/A`), `/us/usc/t10/s3744/a/16` (`/us/usc/t41/s1127/b`) |
| section only | `unit-only` | 809 | below a section | |
| section only | `exact` | 7 | a provision of kind `level` | `/us/usc/t15/s1114/1/a` (`/us/usc/t15/s1116/d/1/A`), `/us/usc/t18/s2511/2/i` (`/us/usc/t50/s1801/f/2`), `/us/usc/t8/s1101/a/15/P/i/a` (`/us/usc/t8/s1184/c/4/A`) |
| section only | `ambiguous` | 4 | below a provision whose address is held twice | `/us/usc/t42/s1395x/v/1/L/ix` (`/us/usc/t42/s1395fff/b/3/A/i/I`), `/us/usc/t42/s1395x/v/1/U` (`/us/usc/t42/s1395l/t/10`), `/us/usc/t42/s1395x/v/1/L/viii` (`/us/usc/t42/s1395r/g/1`) |
| no section | `target-status` | 278 | an alias of a section: a member of an identifier that lists several, or of a range | `/us/usc/t42/s662` (`/us/usc/t10/s1408/e/4/B`), `/us/usc/t47/s756` (`/us/usc/t10/s2281/c`), `/us/usc/t50/s2512` (`/us/usc/t10/s6103/c`) |
| no section | `target-status` | 45 | a section whose identifier lists several, by its first member | `/us/usc/t10/s4008` (`/us/usc/t10/s4841/d/1`), `/us/usc/t42/s1450` (`/us/usc/t12/s1715k/d/1/A`), `/us/usc/t16/s581` (`/us/usc/t16/s1647/a`) |
| no section | `unit-only` | 66 | below an alias of a section | `/us/usc/t50/s4605/j/1/A` (`/us/usc/t10/s361/a/1`), `/us/usc/t42/s6683/d` (`/us/usc/t10/s4801/7`), `/us/usc/t12/s1724/a` (`/us/usc/t12/s1470/b/1`) |
| no section | `unit-only` | 6 | below a section whose identifier lists several | `/us/usc/t15/s2012/b/2` (`/us/usc/t15/s2512/c/1`), `/us/usc/t19/s1901/b` (`/us/usc/t19/s1981/a/1`), `/us/usc/t22/s5113/b` (`/us/usc/t22/s2778/g/1/A/xi`) |
| no section | `range-form` | 291 | | |
| no section | `no-unit` | 278 | | |

What explains each move between groups:

- **"Resolve exactly", +334 = 7 + 4 + 323.**
  - The 7 are targets of kind `level`, which `baseline` does not count. S16:
    "provisions of kinds other than the nine".
  - The 4 are `ambiguous`. A beginning of the target is held by two elements,
    the second at `~2`, so the reference cannot say which one it means. S9 tells
    `ambiguous` before `unit-only`. S16's list does not name it; the paragraph
    "Where the two differ" of `conformance/usc-counting-rules.md` does: the
    import tells references "to a repeated identifier (`ambiguous`) from the
    others".
  - The 323 are targets the import knows by S7, Addresses, rule 1: identifiers
    that list several sections, and ranges. The first member of a list is the
    address and the others are aliases; the ends and the members of a range
    are aliases. **S16 names no rule for these.** `baseline` compares a target
    with the whole identifier attribute. The section prefix it takes from a
    list keeps the space (`/us/usc/t2/s3 ` from `/us/usc/t2/s3 /us/usc/t2/s4`),
    and the one it takes from a range is the whole range
    (`/us/usc/t2/s44...46`). So the target is neither exact nor of a known
    section. Every such section has a status (F3), so each of the 323 is
    `target-status`.
- **"Reach the section only", +61 = 72 − 7 − 4.** The 72 lie below an alias of
  a section, or below a section whose identifier lists several: the same rule
  of S7, which S16 does not name.
- **"Reach no section", −395 = −323 − 72.**
- **One rule of S16 has no case.** S16's last rule ("references whose target
  is a container or a unit that is not a section") has no case among the
  operative references of this release: no operative reference to the Code
  names a container, a court rule, a plan or a level.

## The references (item 3)

From `report-cold.json` (`fidryn.check-report/v0.1`), every reference of every
layer by its outcome:

| Outcome | Operative | Enacted | Editorial | All |
| --- | --- | --- | --- | --- |
| `exact` | 88,299 | 94,822 | 52,559 | 235,680 |
| `target-status` | 900 | 3,598 | 8,248 | 12,746 |
| `ambiguous` | 70 | 50 | 70 | 190 |
| `unit-only` | 881 | 1,180 | 2,344 | 4,405 |
| `range-form` | 291 | 253 | 291 | 835 |
| `no-unit` | 278 | 1,259 | 6,736 | 8,273 |
| `outside` | 16,199 | 229,559 | 728,990 | 974,748 |
| `malformed` | 0 | 0 | 0 | 0 |
| all | 106,918 | 330,721 | 799,238 | 1,236,877 |

- **Outcome `outside`, by body:** `/us/pl` 599,160, `/us/stat` 321,830 and
  `/us/act` 53,758. That makes 974,748.
- **Citations:** the workspace is generated, so it has none, and `citations` is
  zero in every outcome.
- **The unresolved list.** It has 1,001,197 lines, 136,460,771 bytes, one for
  each reference whose outcome is not `exact`, sorted by the address it stands
  in. `grep -c` of each outcome in it gives the totals of the table.

Three examples of each outcome other than `exact` and `outside`: the first line
of the list for each layer. There is no example of `malformed`, which never
occurs.

| Outcome | Layer | Stands in | Target |
| --- | --- | --- | --- |
| `ambiguous` | operative | `/us/usc/t15/s1693o–1/a/6/B` | `/us/usc/t15/s1693b/a/2` |
| `ambiguous` | enacted | `/us/usc/t10/s130g/note/statutoryNotes/pl.119.60/application-of-notifications` | `/us/usc/t10/s130g` |
| `ambiguous` | editorial | `/us/usc/t10/s394/note/editorialNotes/pl.115.232/amendments` | `/us/usc/t10/s130g` |
| `target-status` | operative | `/us/usc/t10/s1108/c/3` | `/us/usc/t42/s1395ggg` |
| `target-status` | enacted | `/us/usc/t1/s112a/note/statutoryNotes/pl.108.458/effective-date-of-2004-amendment` | `/us/usc/t22/s2374` |
| `target-status` | editorial | `/us/usc/t10/s10541/note/editorialNotes/pl.115.232/amendments` | `/us/usc/t10/s115b` |
| `unit-only` | operative | `/us/usc/t10/s101/a/11/A` | `/us/usc/t10/s125/d` |
| `unit-only` | enacted | `/us/usc/t10/s1074/note/statutoryNotes/pl.103.335/funding-of-fisher-houses-associated-with-army-medical-treatm` | `/us/usc/t10/s2221/c/1` |
| `unit-only` | editorial | `/us/usc/t10/s101/note/editorialNotes/pl.107.296/codification` | `/us/usc/t10/s101/9` |
| `range-form` | operative | `/us/usc/t10/s123/b` | `/us/usc/t50/s1621–162` |
| `range-form` | enacted | `/us/usc/t10/s2675/note/statutoryNotes/pl.96.418/effective-date-of-1980-amendment` | `/us/usc/t42/s1594h–3` |
| `range-form` | editorial | `/us/usc/t10/s7771/note/ungrouped/act.1948-04-21.ch224/noheading` | `/us/usc/t7/s436–438` |
| `no-unit` | operative | `/us/usc/t10/s1174a/b/2/B` | `/us/usc/t37/s476` |
| `no-unit` | enacted | `/us/usc/ptIX/rule9020/note/ungrouped/nolaw/notes-of-advisory-committee-on-rules-1983` | `/us/usc/t28/s1481` |
| `no-unit` | editorial | `/us/usc/t10/s10102/note/editorialNotes/pl.103.337/prior-provisions` | `/us/usc/t10/s262` |

What the examples show:

- `/us/usc/t10/s130g` is one of the seven identifiers that two sections hold
  (F2).
- `/us/usc/t50/s1621–162` reads as a range cut short by the end of its link,
  which F12 finds 111 times in operative text.
- `/us/usc/t42/s1594h–3` names `/us/usc/t42/s1594h` followed by a dash: a range
  or a dashed number that is not in the workspace, and the reference cannot say
  which (S9).

## The text (item 4)

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY text /work/projects/law-local/uscode --json > /cache/fidryn/task-21/text-fidryn.jsonl
```

- Exit 0, 11.46 s wall, 12.4 s user, 2,423,580 KiB peak.
- It printed 1,015,386 lines, 652,907,379 bytes.

The time is twice the rehearsal's because the output went through the
scheduler's stream to the file. Run once more with the binary writing the file
itself inside the job, through `sh -c "BINARY text ... --json > FILE"` and the
same limits, it took 5.47 s wall with a peak of 2,426,052 KiB, and gave the
same bytes (`cmp`).

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I conformance/uslm_text.py /library/corpora/uscode/119-142 > /cache/fidryn/task-21/text-python.jsonl
```

- Exit 0, 23.74 s wall, 1,583,884 KiB peak.
- It printed 652,907,379 bytes.

The two outputs were compared as two sorted streams, held one line at a time.
The script, `stream_text.py`, is kept outside the repository; it was run like
the profile and took 4.36 s with a peak of 48,996 KiB.

- Each output has 1,015,386 lines.
- 1,015,386 lines were compared, and **0 differ**. `cmp` of the two files also
  exits 0.
- The lines are 774,018 provisions, 9,623 containers, 170,451 notes, 51,567
  source credits and 9,727 footnotes.

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ingest uscode /library/corpora/uscode/119-142 --out /work/projects/law-local/uscode --jobs 16 --verify
```

- It printed `ok` and exited 0, in 5.94 s wall with a peak of 2,214,692 KiB.
- The workspace holds every file the import writes, with the same bytes, and
  nothing else.

The independent reading of the publisher's XML and the text the modules hold
are the same, record for record, over the whole Code.

## Definitions

The `definition` blocks of all 70,794 modules were read, one module at a time,
by `compare_definitions.py`, a script kept outside the repository (1.87 s,
129,988 KiB). They were compared with the independent list of S10 in
`definitions-119-142.jsonl`, paired by address and first term, and compared on
all eight fields: `at`, `terms`, `kind`, `words`, `region`, `basis`,
`borrowed_from` and `see_note`.

- The workspace has 34,329 definitions, and so does the list.
- Every (address, first term) names one definition, and the 34,329 keys are
  the same on both sides.
- **No pair differs in any field.** No definition is on one side only.
- By basis: 6,955 `enacted`, 23,986 `restated` and 3,388 `unresolved`, as F15
  says. By kind: 28,618 `means`, 2,394 `includes`, 674 `does_not_include` and
  2,643 `borrowed`. These are the ledger's counts.

There was no departure to decide on either side.

## Sizes

| Measure | Value |
| --- | --- |
| modules | 70,794: 61,171 units and 9,623 containers |
| workspace | 624,035,591 bytes in 70,797 files and 1,680 directories, 782,364,672 bytes on disk |
| modules alone | 617,169,405 bytes |
| `law.lock.json` | 6,855,192 bytes |
| `coverage.json` | 10,818 bytes |
| `workspace.json` | 176 bytes |
| cache after the checks | 70,794 entries, 283,253,854 bytes, 438,149,120 bytes on disk |
| the law repository's `.git` after its first commit | 231,601,114 bytes |

**The largest unit is 42 U.S.C. 1395ww.**

- Its module `us/usc/t42/s1395ww.fr` is 1,429,024 bytes.
- It has 1,600 provisions and 167 notes, as F6 says.
- Its text records are 1,557,118 bytes.
- Next, by the size of the module: 42 U.S.C. 1396a (1,276,459 bytes) and
  19 U.S.C. 3805 (1,217,238 bytes). These are the only three modules larger
  than 1 MiB.
- By provisions, 15 U.S.C. 636 comes second (1,224) and 42 U.S.C. 1395w–4
  third (1,222).

**The deepest nesting.**

- A provision stands at most 8 levels below its unit's top provision, as F6
  says, for example at `/us/usc/t30/s825/e/2/B/iii/II/bb/BB/aaa`. 11
  provisions are that deep.
- By depth, from the top provision (0) down: 61,171; 161,651; 238,764;
  187,387; 90,847; 28,929; 4,691; 567; 11.
- At most six containers stand above a unit, as F5 says. They do for 1,126
  units, for example `/us/usc/t10/s3001`.

These measures come from the stream pass over `text-fidryn.jsonl`. The depth
of a provision is the number of segments its address has below the address of
its unit: each provision's address is its parent's followed by one segment
(S7, Addresses, rule 3).

## The law repository

```sh
git init --initial-branch=main /work/projects/law-local
```

It printed `Initialized empty Git repository in /work/projects/law-local/.git/`.

At its top it holds two files:

- `.gitattributes`, the one line `* -text`, so that git never converts a byte
  of a file whose digest the lock records;
- `.gitignore`, the two lines `.fidryn-cache/` and `uscode.next/`.

The workspace is in `uscode/`, which is the directory the import wrote.

```sh
cd /work/projects/law-local && git add .gitattributes .gitignore uscode && git commit -q -m "/us/usc: release 119-142"
```

- `git add` took 6.40 s with a peak of 22,408 KiB, and `git commit` 1.09 s.
- The commit is `218b87fb28eccc751436e5eb96d79c1cc1689d2c` on `main`, with the
  tree `a1a6ae4f425dc966e09199a89eabfcd95c3b9fde`. It holds 70,799 files: the
  two above and the 70,797 of the workspace.
- `git log --format='%H %G? %s' main` prints
  `218b87fb28eccc751436e5eb96d79c1cc1689d2c G /us/usc: release 119-142`, and
  `git verify-commit HEAD` reports a good signature. The commit is signed by
  the configuration of git, with the key this repository's commits use
  (decision O-5); no signing option was given.
- `git status` is clean after it, so git changed no byte of the workspace.
- The repository has no remote (decision O-6). The check cache lies outside
  it, in `/cache/fidryn/task-21-cache`.

The replay of the 27 releases replaces it as the history of `main`.

`BINARY text /work/projects/law-local/uscode /us/usc/t45/s151 --hashes`, under
the same limits, took 3.38 s with a peak of 1,835,964 KiB. It printed the same
two hashes as the command run on the sample of the repository
(`provision_hash "180a7eb8..."`, `notes_hash "31a067f1..."`), so a binding to
45 U.S.C. 151 records the same hashes against either workspace.

## What was not done, or not met

- **Nothing in the repository was corrected.** The full release showed no
  defect: the ingester refused no file, and the two readings of the text
  agree. The definitions agree with the independent list, and every count
  agrees with the independent profile. The only commit is this record.
- **S16's account of the differences is not complete**, and the record says so
  rather than name a rule S16 does not have:
  - 395 operative references move between the groups of F1 by the rule of
    aliases (S7, Addresses, rule 1), which neither S16 nor the paragraph
    "Where the two differ" of `conformance/usc-counting-rules.md` names;
  - 4 move by `ambiguous`, which only that paragraph names;
  - the one unit of F4 is a provision by S7's rule of the outermost unit, which
    S16 does not name either;
  - S16's rule for references to containers and to units that are not
    sections has no case in this release;
  - its words "and their provisions" overstate what `baseline` leaves out of
    court rules: it counts their 5,953 provisions of the nine kinds.

  No number is wrong. The wording of `usc-counting-rules.md` is left as it is.
- **F2 says the 20 sections without an identifier stand inside the compiled
  acts of two appendices.** 19 of them stand in the compiled acts of Title 5
  Appendix. The twentieth, `/us/usc/t18a/~level.1/~section.1`, stands in a
  generic level of Title 18 Appendix that has no identifier.
- **The text comparison** was made on two sorted streams, which the dispatch
  allows, not a file of the release at a time. The two outputs are equal, so
  every file's records are.
- **The fetch** was timed only on its second run. The first, untimed, printed
  the same line.
- **The definitions are not yet read by a person** (design 9.5, the audit of
  two samples of 500, decision O-3). This run shows that the importer's
  definitions are those of an independent reading of S10. It does not show
  that S10 finds every definition, or that each one is right.
- **The analysis scripts** (`compare_counts.py`, `analyze_counts.py`,
  `stream_text.py`, `compare_definitions.py`) and the logs of every command
  are kept outside the repository, with the session's other scratch files, as
  are the large outputs under `/cache/fidryn/task-21/`. Each script is
  described above by what it reads and how it pairs what it compares, so that
  the comparison can be made again from the repository's own scripts and the
  archive.
