# The first full import of the Code: release 119-142

This is the record of the first import of the whole United States Code into a
workspace of law modules: release point 119-142 of the official XML release
point published by the Office of the Law Revision Counsel, one archive of 58
files, imported into a workspace of 70,794 modules, checked, measured,
compared with the independent reading of `conformance/`, and committed as the
first commit of a local law repository. It was run on 2026-11-17 (Eastern
time), on one workstation, through the scheduler of that machine.

Every number below was printed by a command. The line of each command is
given, or the command is named where its line is plain from the text (`cmp`,
`sha256sum`, `grep -c`), with three exceptions: the sizes of files and
directories (in "The import" and in "Sizes"), whose commands this record does
not give; the run of `compare_counts.py`, which was not timed and whose log
holds its output but not its line; and `git init`, which was not timed. Times and peaks of memory come
from `/usr/bin/time -v`: "wall" is its elapsed time and "peak" its maximum
resident set size, which is that of the largest single process of the command.
The record holds no text of the Code beyond addresses and the short examples
asked for.

The short names this record uses are those of the plan of the stage,
`.agents/docs/plans/2026-11-16-stage-1-the-code-as-a-checked-program.md`: F1
to F16 and F7a are its "Measured facts", S1 to S16 the sections of its
"Specification", and O-1 to O-7 its "Decisions that are the owner's". "Design
9.5" and "design 9.6, item N" are sections of the design,
`.agents/docs/specs/2026-11-13-law-monorepo-design.md`, and the items named in
the tables below ("Item 1" and so on) are the acceptance items of its section
9.6. "Task N" is a task of the plan.

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

Every run of the built binary on the full release was one job of its own, one
at a time, with these limits: 16 threads and 16 GB from the scheduler, half an
hour of wall time, 16 GiB of address space and an hour of processor time.

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
  profile;
- the scratch scripts of this part and the logs of every command:
  `.agents/scratchpad/2026-11-17-stage1/work/task-21/` in the main checkout of
  this repository on the workstation, which git ignores, so that they are in
  no commit. "SCRATCH" below is that directory.

## The archive

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I tools/fetch_uscode.py 119-142 --out /library/corpora/uscode/119-142 --all
```

- It printed `kept xml_uscAll@119-142.zip` and exited 0, both times it was run.
  The second run, timed, took 0.10 s wall with a peak of 25,868 KiB.
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
  printed nothing and exited 0 (0.42 s wall, 15,584 KiB peak).
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
for key, and the check report with both, by `SCRATCH/compare_counts.py`, which
takes the three JSON files as its arguments, the ledger, the profile and the
check report, in that order. Its run was not timed:

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

The differences were found element by element by the scratch script
`analyze_counts.py`, run like the profile. The timed command, whose first
argument is the root of the checkout whose `conformance/` it reads, was

```sh
uv run --locked python -I SCRATCH/analyze_counts.py /work/projects/fidryn-lanes/check /library/corpora/uscode/119-142 --jobs 4
```

It took 17.54 s, with a peak of 734,812 KiB. On one parse of each file, it
reads with the reading of
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

The time is twice that of the rehearsal (a run of the same commands made on
the same workstation before this one, with an earlier build, whose notes are
kept with the scratch files and not in the repository), because the output
went through the scheduler's stream to the file. Run once more with the binary
writing the file itself inside the job, through
`sh -c "BINARY text ... --json > FILE"` and the
same limits, it took 5.47 s wall with a peak of 2,426,052 KiB, and gave the
same bytes (`cmp`).

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I conformance/uslm_text.py /library/corpora/uscode/119-142 > /cache/fidryn/task-21/text-python.jsonl
```

- Exit 0, 23.74 s wall, 1,583,884 KiB peak.
- It printed 652,907,379 bytes.

The two outputs were compared line by line, by position: the first line of one
with the first line of the other, and so on, each pair as bytes, one line of
each held at a time. Nothing was sorted, and no line was paired by its
address, so the two outputs had to hold the same records in the same order.
The scratch script `stream_text.py` did it, run like the profile; the timed
command was

```sh
uv run --locked python -I SCRATCH/stream_text.py /cache/fidryn/task-21/text-fidryn.jsonl /cache/fidryn/task-21/text-python.jsonl
```

It took 4.36 s, with a peak of 48,996 KiB.

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
by the scratch script `compare_definitions.py`. The timed command was

```sh
uv run --locked python -I SCRATCH/compare_definitions.py /work/projects/law-local/uscode SCRATCH/../controller/definitions-119-142.jsonl
```

It took 1.87 s, with a peak of 129,988 KiB. The definitions were compared with
the independent list of S10, `definitions-119-142.jsonl`, paired by address
and first term, and compared on all eight fields: `at`, `terms`, `kind`,
`words`, `region`, `basis`, `borrowed_from` and `see_note`.

The list was written by a second implementation of the rules of S10, which is
no part of this repository and shares no code with the importer. The
implementation, `definition_rules.py`, and the list are kept with the scratch
files of the run, in `.agents/scratchpad/2026-11-17-stage1/work/controller/`
of the same checkout. So this comparison cannot be made again from the
repository alone.

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

These sizes were read from the files and directories; this record does not
give the commands that read them.

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

It printed `Initialized empty Git repository in /work/projects/law-local/.git/`,
and was not timed.

At its top it holds two files:

- `.gitattributes`, the one line `* -text`, so that git never converts a byte
  of a file whose digest the lock records;
- `.gitignore`, the two lines `.fidryn-cache/` and `uscode.next/`.

The workspace is in `uscode/`, which is the directory the import wrote.

```sh
cd /work/projects/law-local && git add .gitattributes .gitignore uscode && git commit -q -m "/us/usc: release 119-142"
```

- `git add` took 6.40 s with a peak of 22,408 KiB, and `git commit` 1.09 s
  with a peak of 21,644 KiB.
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
  - 395 operative references move between the groups of F1 by the rule of S7
    for identifiers that list several sections and for ranges (Addresses,
    rule 1): 344 because their target is an alias of a section or lies below
    one (278 and 66), and 51 because it is the first member of a list, which
    is the section's address and not an alias, or lies below one (45 and 6).
    Neither S16 nor, when the run was made, the paragraph "Where the two
    differ" of `conformance/usc-counting-rules.md` named that rule;
  - 4 move by `ambiguous`, which only that paragraph names;
  - the one unit of F4 is a provision by S7's rule of the outermost unit,
    which S16 does not name either; the paragraph does ("counts the nested
    section of a unit as one of its provisions rather than as a unit");
  - its words "and their provisions" overstate what `baseline` leaves out of
    court rules: it counts their 5,953 provisions of the nine kinds.

  No number is wrong. The run left the wording of `usc-counting-rules.md` as
  it was. Its paragraph "Where the two differ" was completed afterward, in the
  commit `Name every rule by which the two profiles of the Code differ`, so
  that it names a rule for each difference above.
- **One rule of S16 has no case.** Its rule for references to containers and
  to units that are not sections is no gap in its account, but no operative
  reference of this release falls under it.
- **F2 says the 20 sections without an identifier stand inside the compiled
  acts of two appendices.** 19 of them stand in the compiled acts of Title 5
  Appendix. The twentieth, `/us/usc/t18a/~level.1/~section.1`, stands in a
  generic level of Title 18 Appendix that has no identifier.
- **The text comparison** was made on the two whole outputs, line by line by
  position, which the instructions of the run allowed in place of the plan's
  "a file of the release at a time". The two outputs are equal, so every
  file's records are.
- **The fetch** was timed only on its second run. The first, untimed, printed
  the same line.
- **The definitions are not yet read by a person** (design 9.5, the audit of
  two samples of 500, decision O-3). This run shows that the importer's
  definitions are those of an independent reading of S10. It does not show
  that S10 finds every definition, or that each one is right.
- **The analysis scripts** (`compare_counts.py`, `analyze_counts.py`,
  `stream_text.py`, `compare_definitions.py`) and the logs of every command
  are in SCRATCH, which git ignores, and the large outputs under
  `/cache/fidryn/task-21/`; none of them is in a commit. Each script is
  described above by what it reads and how it pairs what it compares. The
  counts and the text can be compared again from the repository's own
  scripts, `conformance/usc_profile.py` and `conformance/uslm_text.py`, and
  the archive. The definitions cannot: the independent list, and the
  implementation that wrote it, are not in the repository.

# The second part: the releases replayed, the real pair, and the definition samples

The same evening, on the same workstation, the 27 release points the
publisher lists for 2026 were replayed as the history of the local law
repository, one commit for each, with the change set from the release
before it as the message. The pair 119-138 to 119-142 was then held to an
independent comparison of the publisher's XML and to three bindings, and the
two samples of definitions were drawn. Nothing of the full Code entered this
repository. The record keeps the form of the first part: each command, its
time and peak from `/usr/bin/time -v` where it was timed, what it printed,
and what it shows. The lines of three scratch scripts, `check_messages.py`,
`units_by_layer.py` and `log_table.py`, are not given; each is described by
what it reads and what it compares.

| What was asked | What the run gave |
| --- | --- |
| Item 8: the 27 releases as commits | 27 commits on `main` from a root of their own, each signed (`G`), each message the text form of its change set byte for byte |
| A replay stopped and started again | stopped on purpose in the middle of the import of 119-95; the second run finished that release and the 17 after it |
| Every import and every check | no error and no warning in any of the 27 |
| The last release of the replay | its `uscode/` is the tree of the first import, with no difference (`git diff`) |
| Item 5: the real pair, confirmed independently | `usc_compare.py --against` exits 0; so it does for each of the 26 pairs of the replay |
| Item 6: staleness on the real pair | the binding of a changed provision and the one in a unit whose enacted notes changed are `W750`; the one in a unit whose only change is editorial is not |
| Item 7: the samples | 500 of 34,329 definitions and 500 of 5,940 near misses, drawn with the seed 20261117; not read |

## The binary of the second part

The tools of this part are Python and read no Rust source, so the release
build was made once they were written, on the tree of commit
`be229891b0bfdfd6f371bd77704748bfa4fb94fe` with only the new Python files and
their documents beside it:

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-check lease run --cpus 16 --mem 25G --time 1h -- /usr/bin/time -v cargo build --release -p fidryn-cli --locked --offline
```

- It took 22.72 s, with a peak of 1,051,276 KiB.
- The binary is 13,837,000 bytes, BLAKE3
  `1588aa901707193fda1858482da57f874c76152eff36cd5af10564f1c1412427` and
  SHA-256 `f2dd81dde0ade77b922bfb1a37155ef26f803377fc6d0548f4eb8bfe681a6ef3`:
  the same bytes as the binary of the first part, built from `d0658b6`.
- It was copied to `/cache/fidryn/task-22/fidryn`; "BINARY" below is that
  file. `BINARY --version` prints `fidryn 0.3.0`. In each of the 27 commits,
  read with `git show`, `toolchain` of `uscode/workspace.json` is `"0.3.0"`
  and `generator` of `uscode/law.lock.json` is
  `{"name": "fidryn-ingest-uscode", "version": "0.3.0"}`, and no check warned
  `W760`.

Outside the replay, each run of BINARY had its own limits, those of the first
part, and the runs were made one at a time, inside jobs of the scheduler. A
job held one run, or several one after the other, each under its own limits:
six for the text of the three bindings' units, three for their hashes against
119-138, and five for the hashes against 119-142 and the two change sets with
`--bindings`, as each is said below. A job of one run was

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ...
```

Where things are:

- the archives: `/library/corpora/uscode/<release>/`;
- the law repository: `/work/projects/law-local/`;
- the older release of the pair: `/work/projects/law-local-pair/119-138/`;
- the cache of every check of this part: `/cache/fidryn/task-22-cache/`;
- every large output: `/cache/fidryn/task-22/`;
- the scratch scripts of this part and the logs of every command:
  `.agents/scratchpad/2026-11-17-stage1/work/task-22/` in the main checkout of
  this repository, which git ignores, so that they are in no commit.

## The archives

The 27 directories `/library/corpora/uscode/<release>/` each hold one archive
of every title, `xml_uscAll@<release>.zip`, and its retrieval record, written
by `tools/fetch_uscode.py` when they were fetched, one request at a time.

- By their records, the 27 archives are 2,930,114,329 bytes in all, as F9
  says: from 107,704,149 bytes (119-73not60) to 108,706,565 (119-142).
- The replay holds each archive to its record before it imports it. It
  printed `kept xml_uscAll@<release>.zip` for each of the 27 (for 119-95 once
  in each of its two runs), and opened no connection.
- `cmp /library/corpora/uscode/119-138/xml_uscAll@119-138.zip /library/corpora/uscode/119-138.measured/xml_uscAll@119-138.zip`
  printed nothing and exited 0. The older release of the pair is the archive
  measured on 2026-11-16 for F10, as 119-142 was for the first part.

## The law repository before the replay

Before anything else, the commit of the first part was kept on a branch of
its own:

```sh
cd /work/projects/law-local && git branch first-import 218b87fb28eccc751436e5eb96d79c1cc1689d2c
```

`first-import` holds `218b87fb28eccc751436e5eb96d79c1cc1689d2c`, with the
message `/us/usc: release 119-142`, signed (`G`). Then `main` was made unborn,
so that the replay builds it from a root of its own:

```sh
cd /work/projects/law-local && git checkout -q first-import && git update-ref -d refs/heads/main 218b87fb28eccc751436e5eb96d79c1cc1689d2c && git checkout -q --orphan main
```

`git update-ref -d` removes the name `main` only while it names that commit,
and the commit stays on `first-import`; nothing was deleted from the
repository, and `.git` was not touched otherwise. The repository has no
remote, and nothing was pushed (decision O-6).

A first job made one signed commit in a throwaway repository under
`/cache/fidryn/task-22/`, given its message on stdin with
`--cleanup=verbatim`. Git signed it inside the scheduler's job (`G`), and its
message, with blank lines, a trailing space and a line that begins with `#`,
came back from the commit object byte for byte. The repository was then
removed.

## The replay

```sh
lease run --cpus 16 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I tools/replay_uscode.py --archives /library/corpora/uscode --repository /work/projects/law-local --fidryn /cache/fidryn/task-22/fidryn --cache-dir /cache/fidryn/task-22-cache --jobs 16
```

For each release the tool fetched or kept the archive; imported it into
`uscode.next/` and checked it; after the first release, wrote
`changes/<release>.json` and kept the text form; replaced `uscode/`; and
committed, reading the message back from the commit object. Each run of
BINARY inside it had its own limits, 16 GiB of address space and 30 minutes.
Git's automatic housekeeping was off for these commits, and the repository
was packed once afterward (below). The tool as committed differs from the one
that ran by one guard, added after: the message of a run ended by a signal
that Python does not name gives the signal's number. No run was ended by a
signal.

**The stop.** The replay was stopped on purpose once, in the middle of an
import. A watcher waited until `uscode.next/workspace.json` named 119-95,
which the importer writes first, waited 1.5 seconds and canceled the job
(at 22:24:16, Eastern time). The first run, started at 22:21:50, had
committed 9 releases, 119-73not60 to 119-94; the ninth commit is of 22:24:12.
What it left:

- `main` had 9 commits, the last `7969c856010f81e3786e6c6376ecd85ffe416813`,
  `/us/usc: 119-93 to 119-94`;
- `uscode/` was 119-94 and `git status` was clean;
- `uscode.next/` held the manifest of 119-95 and 34,943 of its module files,
  and no lock and no ledger;
- no text form was kept in `.git/fidryn-replay.json`, since the stop came
  before the change set.

**The second run** was the same command. It printed
`9 of 27 releases are committed`, imported 119-95 again into the
`uscode.next/` it found, and committed 119-95 and the 17 releases after it.
It ended with `the replay is complete: 27 releases are committed` and exit 0,
in 4 min 53.46 s wall, 1,176.77 s user, with a peak of 2,325,624 KiB. No
release was committed twice, and none was skipped.

**Each release** took about 16 seconds:

| Step | Time |
| --- | --- |
| `fidryn ingest uscode` | 6.0 to 6.8 s |
| `fidryn check uscode.next` | 1.6 to 1.8 s, and 3.3 s for the first release, with an empty cache |
| `fidryn changes uscode uscode.next`, `--json` and the text form | 2.8 to 3.0 s each |
| `git add` and `git commit`, with the reading back | 2.0 to 2.7 s |

Every check printed `errors: 0; warnings: 0`; the tool stops on either. The
releases grow from 60,922 units and 9,578 containers (70,500 modules) in
119-73not60 to 61,171 and 9,623 (70,794) in 119-142, as F16 says. The release
119-83, whose `usc22.xml` writes its name with an en dash and has no flag,
imported and checked as the others did.

### The 27 commits

`git log --format='%h %G? %s' main`, printed when the replay ended, oldest
last:

```text
37192eb42 G /us/usc: 119-138 to 119-142
bc2638aaa G /us/usc: 119-136 to 119-138
959d79944 G /us/usc: 119-134 to 119-136
128627b3f G /us/usc: 119-124 to 119-134
a9435b02e G /us/usc: 119-121 to 119-124
67552ed10 G /us/usc: 119-120 to 119-121
eb7cd7c9d G /us/usc: 119-118 to 119-120
dd20a1053 G /us/usc: 119-117 to 119-118
1bf077e2e G /us/usc: 119-111 to 119-117
64d3102c6 G /us/usc: 119-110 to 119-111
bec34a573 G /us/usc: 119-108 to 119-110
b3876e041 G /us/usc: 119-103 to 119-108
362729acf G /us/usc: 119-102 to 119-103
24dbdb1e4 G /us/usc: 119-102not101 to 119-102
421e96072 G /us/usc: 119-100 to 119-102not101
1f2749caf G /us/usc: 119-99 to 119-100
d24938283 G /us/usc: 119-95 to 119-99
b8c94a801 G /us/usc: 119-94 to 119-95
7969c8560 G /us/usc: 119-93 to 119-94
ebb91d01f G /us/usc: 119-90 to 119-93
2574428cc G /us/usc: 119-88 to 119-90
5226a93bd G /us/usc: 119-87 to 119-88
fa0329a3c G /us/usc: 119-84 to 119-87
8704f9a21 G /us/usc: 119-83 to 119-84
ba82c0701 G /us/usc: 119-73 to 119-83
bb7b4ccc5 G /us/usc: 119-73not60 to 119-73
b8ec1e448 G /us/usc: release 119-73not60
```

All 27 lines have `G`, and `git rev-list --count main` printed 27. The root
is `b8ec1e448a9ccd6336caf4a0e7a60477676393c9`. The first commit holds
`.gitattributes`, `.gitignore` and `uscode/`; each later one also
`changes/<release>.json`, 26 in all. Design 9.6, item 8 is met.

The messages were checked twice:

- by the tool, as it went: each message read back from the commit object
  equals the text form `fidryn changes` printed, byte for byte;
- afterward, by the scratch script `check_messages.py` (0.09 s,
  27,884 KiB). For each commit it reads the message from the commit
  object, and the change set from `changes/<release>.json` of the commit's
  own tree. It writes the text form of that change set by the layout of
  `docs/law-modules.md` ("The two forms"), and compares bytes. All 27 are
  equal.

The counts of each change set, in the order of its lists: `operative`
added, removed, `amended_text`, `amended_structure`, moved; `enacted_notes`
added, removed, amended, moved; `editorial` added, removed, amended, moved,
`credits_changed`, `footnotes_changed`; `containers` added, removed,
changed.

| # | Commit | Release | Message, bytes | Operative | Enacted notes | Editorial | Containers |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `b8ec1e448` | 119-73not60 | 29 | | | | |
| 2 | `bb7b4ccc5` | 119-73 | 357,694 | 1235/751/1512/888/155 | 626/259/255/12 | 1168/744/128/18/564/130 | 36/0/99 |
| 3 | `ba82c0701` | 119-83 | 71,153 | 155/73/359/151/7 | 219/104/104/0 | 114/93/11/0/100/16 | 0/0/13 |
| 4 | `8704f9a21` | 119-84 | 5,980 | 0/7/3/1/0 | 12/23/26/0 | 1/6/0/0/3/0 | 0/0/5 |
| 5 | `fa0329a3c` | 119-87 | 3,973 | 0/0/0/1/0 | 13/6/24/0 | 0/0/0/0/0/0 | 0/0/0 |
| 6 | `5226a93bd` | 119-88 | 970 | 3/0/1/0/0 | 2/1/0/0 | 0/1/2/0/0/0 | 1/0/1 |
| 7 | `2574428cc` | 119-90 | 1,197 | 1/0/0/1/0 | 6/0/0/0 | 1/1/0/0/1/0 | 0/0/0 |
| 8 | `ebb91d01f` | 119-93 | 2,795 | 9/2/12/8/0 | 6/0/4/0 | 5/2/0/0/5/1 | 0/0/0 |
| 9 | `7969c8560` | 119-94 | 761 | 1/0/0/0/0 | 4/0/0/0 | 0/0/0/0/0/0 | 1/0/1 |
| 10 | `b8c94a801` | 119-95 | 1,412 | 0/0/2/0/0 | 2/1/4/0 | 2/2/0/0/2/0 | 0/0/0 |
| 11 | `d24938283` | 119-99 | 1,249 | 12/0/4/4/0 | 1/0/1/0 | 1/1/0/0/1/0 | 0/0/0 |
| 12 | `1f2749caf` | 119-100 | 526 | 0/0/0/0/0 | 1/0/1/0 | 0/0/0/0/0/0 | 0/0/0 |
| 13 | `421e96072` | 119-102not101 | 5,651 | 13/0/7/6/0 | 22/9/13/0 | 8/3/2/0/5/0 | 0/0/5 |
| 14 | `24dbdb1e4` | 119-102 | 28,608 | 127/50/141/98/7 | 55/8/12/0 | 79/65/10/0/68/17 | 4/0/7 |
| 15 | `362729acf` | 119-103 | 34,286 | 11/11/65/12/1 | 60/55/161/0 | 41/51/54/0/38/9 | 0/0/1 |
| 16 | `b3876e041` | 119-108 | 2,383 | 3/0/4/2/0 | 7/1/4/0 | 3/3/2/0/2/2 | 0/0/1 |
| 17 | `bec34a573` | 119-110 | 1,242 | 0/0/2/0/0 | 3/1/2/0 | 2/2/0/0/2/0 | 0/0/0 |
| 18 | `64d3102c6` | 119-111 | 2,605 | 19/0/3/2/0 | 6/1/2/0 | 12/1/0/0/2/0 | 1/0/1 |
| 19 | `1bf077e2e` | 119-117 | 1,429 | 6/0/0/0/0 | 5/2/1/0 | 1/0/1/0/0/0 | 1/0/2 |
| 20 | `dd20a1053` | 119-118 | 741 | 0/0/4/0/0 | 1/0/0/0 | 2/0/0/0/2/0 | 0/0/0 |
| 21 | `eb7cd7c9d` | 119-120 | 729 | 1/0/0/0/0 | 3/1/0/0 | 0/0/0/0/0/0 | 0/0/1 |
| 22 | `67552ed10` | 119-121 | 1,138 | 7/0/8/5/0 | 1/0/0/0 | 2/0/0/0/2/1 | 0/0/0 |
| 23 | `a9435b02e` | 119-124 | 1,393 | 3/0/2/2/0 | 5/1/0/0 | 1/1/1/0/2/0 | 0/0/0 |
| 24 | `128627b3f` | 119-134 | 3,636 | 5/2/7/1/0 | 13/3/4/0 | 5/2/5/0/3/7 | 1/0/5 |
| 25 | `959d79944` | 119-136 | 408 | 0/0/0/0/0 | 1/0/0/0 | 0/0/0/0/0/0 | 0/0/0 |
| 26 | `bc2638aaa` | 119-138 | 1,982 | 0/0/3/0/0 | 7/2/3/0 | 2/2/1/0/2/0 | 0/0/0 |
| 27 | `37192eb42` | 119-142 | 4,822 | 4/0/11/3/0 | 9/5/10/0 | 13/9/3/0/4/18 | 0/0/0 |

The largest change set is the first, 119-73not60 to 119-73: 1,235 provisions
added, 751 removed and 1,512 amended in text, with the largest message, as
the rehearsal of the replay found (a replay made before this one with an
earlier build, into a scratch directory; its notes are kept with the scratch
files and not in the repository). 119-100 and 119-136 changed nothing but
notes.

### The last release against the first import

```sh
cd /work/projects/law-local && git diff --quiet first-import main -- uscode .gitattributes .gitignore
```

It exited 0: the tree of the 27th commit holds the same `uscode/`,
`.gitattributes` and `.gitignore` as the commit of the first part, file for
file. The release 119-142 imported at the end of the replay is the import of
the first part, and so are its lock and its ledger.

### After the replay

- `git status` was clean, and `.git/fidryn-replay.json` was gone.
- `git count-objects -v` counted 6,866 loose objects and one pack, and `.git`
  held 336,204,602 bytes. The repository was packed once:
  `lease run --cpus 8 --mem 16G --time 1h -- /usr/bin/time -v git -C /work/projects/law-local gc --quiet`
  took 2.90 s with a peak of 539,308 KiB. Then `.git` held 209,572,665 bytes
  in one pack of 79,347 objects. `first-import` kept its commit, and
  `git fsck --connectivity-only` reported nothing.
- The working tree holds `uscode/`, 624,035,591 bytes as in the first part,
  and `changes/`, 26 files and 664,576 bytes; the largest is
  `changes/119-73.json`, 442,475 bytes.
- After the checks of the pair below, the cache held 77,033 entries,
  326,857,692 bytes: one for each module file of the 27 releases, a file of
  the same bytes in several releases counted once, and one for the curated
  module.

### Every pair of the replay, compared independently

Each of the 26 change sets the replay committed was held to the independent
comparison of its two archives, one pair after the other, by the scratch
script `compare_pairs.sh`. For each pair it runs

```sh
uv run --locked python -I conformance/usc_compare.py /library/corpora/uscode/OLDER /library/corpora/uscode/NEWER --against /work/projects/law-local/changes/NEWER.json --jobs 8
```

and the whole loop ran as one job,
`lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v sh compare_pairs.sh`.

- Each of the 26 printed nothing and exited 0, in 41.49 to 43.83 s, with
  peaks from 3,283,836 to 3,317,508 KiB.
- The job ended with exit 0, in 18 min 21.11 s wall.
- So the change set of every pair of neighboring releases, from 119-73not60
  to 119-73 to 119-138 to 119-142, lists in each group the addresses the
  independent reading of the publisher's XML finds, as the rehearsal found.

## The real pair, 119-138 to 119-142 (items 5 and 6)

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ingest uscode /library/corpora/uscode/119-138 --out /work/projects/law-local-pair/119-138 --jobs 16
```

It printed `ingested: 61171 units, 9623 containers from 1 archives (release 119-138)`
and exited 0, in 6.09 s wall with a peak of 2,226,048 KiB.

### Item 5: the change set and the independent comparison

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- sh -c 'BINARY changes /work/projects/law-local-pair/119-138 /work/projects/law-local/uscode --json --jobs 16 --cache-dir /cache/fidryn/task-22-cache > /cache/fidryn/task-22/pair-changes.json'
```

- Exit 0, 2.87 s wall, 1,081,856 KiB peak; 5,907 bytes.
- `old` is 119-138 with the lock digest
  `1e4924bf09eccba146ef5f9241caa33e27f7a1d6c2afbf5ed64ff5a6748048fc`, and
  `new` 119-142 with `83918ea6e9e0a63195f096fdb05b60dbd2574f43e4467d8ad2143ae1424389dd`,
  the BLAKE3 of each `law.lock.json`.
- `cmp` with `changes/119-142.json` of the law repository exits 0: the import
  of 119-138 outside the repository is the one the replay committed, and the
  change set is the one the 27th commit holds.

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I conformance/usc_compare.py /library/corpora/uscode/119-138 /library/corpora/uscode/119-142 --against /cache/fidryn/task-22/pair-changes.json --jobs 8
```

It printed nothing and exited 0, in 42.25 s wall with a peak of 3,290,924 KiB.
The independent reading of the two archives finds, in each group, the
addresses the change set lists. Design 9.6, item 5 is met.

The count of each list:

| Group | List | Count |
| --- | --- | --- |
| `operative` | `added` | 4 |
| | `removed` | 0 |
| | `amended_text` | 11 |
| | `amended_structure` | 3 |
| | `moved` | 0 |
| `enacted_notes` | `added` | 9 |
| | `removed` | 5 |
| | `amended` | 10 |
| | `moved` | 0 |
| `editorial` | `added` | 13 |
| | `removed` | 9 |
| | `amended` | 3 |
| | `moved` | 0 |
| | `credits_changed` | 4 |
| | `footnotes_changed` | 18 |
| `containers` | `added`, `removed`, `changed` | 0, 0, 0 |
| `stale_bindings` | (no `--bindings`) | 0 |

These are the counts Task 20 measured on the same pair. By unit, from the
lists, with the scratch script `units_by_layer.py`:

- **10 units have a change in operative text.** F10 counts 9, its four
  amendments proper (11 U.S.C. 109 and 1182, 26 U.S.C. 7803, 42 U.S.C. 1395m)
  among them. The tenth is 26 U.S.C. 45Y, where a link was taken off words of
  `/us/usc/t26/s45Y/g/11/B/i` and the words stayed. By S15 a link is part
  of a provision's own words, so 45Y/g/11/B/i is `amended_text`, as Task 20
  found. 26 U.S.C. 527 and 6049 are `amended_text` only because a bare
  superscript came with a new footnote marker, which F7a counts as text.
- **12 units changed only in enacted notes**, as F10 says, among them
  11 U.S.C. 101 and 42 U.S.C. 1305.
- **13 units changed only in editorial matter**, all in Title 26, among them
  26 U.S.C. 26 and 457. F10 counts 14. The fourteenth is 45Y, above.

The pair holds the three cases of design 9.6, item 6, so F10 is not
contradicted.

### The three bindings

Each was chosen from the change set, and its text read on both sides with
`BINARY text DIR UNIT --json`, six runs in one job (24.81 s, 1,417,448 KiB):

| Binding | Address | Why |
| --- | --- | --- |
| `chapter_13_debt_limits` | `/us/usc/t11/s109/e` | It is in `operative.amended_text`, and a law amended its words: the limits of debt for chapter 13. 11 U.S.C. 109 is one of the four amendments proper of F10, and the change is not a footnote's superscript. |
| `small_business_debtor` | `/us/usc/t11/s101/51D` | 11 U.S.C. 101 changed only in its enacted notes: a statutory note of Pub. L. 119–142 was added. Its paragraph (51D) is unchanged. |
| `governmental_excess_benefit_arrangements` | `/us/usc/t26/s457/e/14` | 26 U.S.C. 457 changed only in editorial matter: a footnote that points to an editorial note was added, with its marker in this paragraph, and its editorial note "References in Text" now stands under Pub. L. 107–16, with a second paragraph on the subsection (c) that (e)(14) refers to. The words of (e)(14) are unchanged, so the binding stands in the very provision whose footnote marker changed. |

The hashes were printed against the older release:

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v sh -c 'for address in /us/usc/t11/s109/e /us/usc/t11/s101/51D /us/usc/t26/s457/e/14; do timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY text /work/projects/law-local-pair/119-138 $address --hashes; done'
```

The three runs took 10.97 s with a peak of 1,835,724 KiB. The same command
against `/work/projects/law-local/uscode` printed the hashes of 119-142:

| Address | `provision_hash`, 119-138 | 119-142 | `notes_hash`, 119-138 | 119-142 |
| --- | --- | --- | --- | --- |
| `/us/usc/t11/s109/e` | `43d5ecd8...` | `3ea906e8...` | `79699aaf...` | `64958031...` |
| `/us/usc/t11/s101/51D` | `9896a7c7...` | the same | `c654ed4c...` | `43aa01de...` |
| `/us/usc/t26/s457/e/14` | `245780f3...` | the same | `7fdab647...` | the same |

The curated workspace is `encodings/` of the law repository:

- `encodings/workspace.json`: kind `curated`, name `encodings`, prefix
  `/local/encodings`, toolchain `0.3.0`, one dependency `uscode` at
  `../uscode`, pinned to the release `119-142` and the lock digest
  `83918ea6e9e0a63195f096fdb05b60dbd2574f43e4467d8ad2143ae1424389dd`, that of
  `uscode/law.lock.json` as the replay left it.
- `encodings/local/encodings/bindings.fr`: the module
  `/local/encodings/bindings`, with the three bindings, each `binds` its
  address written out, `release "119-138"` and the two hashes printed against
  119-138.

### Item 6: what the check and the change set say

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v sh -c 'cd /work/projects/law-local && timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY check encodings --jobs 16 --cache-dir /cache/fidryn/task-22-cache --report /cache/fidryn/task-22/encodings-report.json --unresolved /cache/fidryn/task-22/encodings-unresolved.jsonl'
```

Exit 0, 1.69 s wall, 574,596 KiB peak. It printed, on stderr:

```text
encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `chapter_13_debt_limits` of `/us/usc/t11/s109/e` is stale (provision text changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `small_business_debtor` of `/us/usc/t11/s101/51D` is stale (enacted notes of the unit changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision and the enacted notes of its unit again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
```

and on stdout `modules: 1; from cache: 0; errors: 0; warnings: 2`, then `ok`.
The binding in 26 U.S.C. 457 is not stale, though a footnote marker was
added to its own text.

```sh
cd /work/projects/law-local && timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY changes /work/projects/law-local-pair/119-138 uscode --bindings encodings --json --jobs 16 --cache-dir /cache/fidryn/task-22-cache > /cache/fidryn/task-22/pair-changes-bindings.json
```

This ran in one job, `lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v sh -c '...'`,
after the three runs of `--hashes` against 119-142 and before the same
command without `--json`, each under the limits (18.82 s for the five, with a
peak of 1,834,248 KiB). Its
`stale_bindings` lists the same two, and nothing else of it differs from the
change set without `--bindings`:

```text
stale_bindings:
/us/usc/t11/s109/e: chapter_13_debt_limits of /local/encodings/bindings in encodings (provision text changed)
/us/usc/t11/s101/51D: small_business_debtor of /local/encodings/bindings in encodings (enacted notes of the unit changed)
```

Design 9.6, item 6 is met on the real pair. The workspace was committed to
the law repository:

```sh
cd /work/projects/law-local && git add encodings && git commit -q -m "encodings: three bindings written against 119-138"
```

The commit is `7493597736a053dca527324a840ad92c995e71b7`, signed (`G`).

## The definition samples (item 7)

The text records of `uscode/` were written by the binary inside the job:

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- sh -c 'BINARY text /work/projects/law-local/uscode --json > /cache/fidryn/task-22/text-119-142.jsonl'
```

Exit 0, 6.11 s wall, 2,423,840 KiB peak; 1,015,386 lines and 652,907,379
bytes, the same bytes as the text of the first part (`cmp`).

The tool was tried first on the committed sample, with the records that
`fidryn text samples/uscode/workspace --json` prints: it drew all 144
definitions, each placed in its paragraph, and 70 near misses. Then:

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I tools/draw_definition_samples.py --workspace /work/projects/law-local/uscode --records /cache/fidryn/task-22/text-119-142.jsonl --seed 20261117 --out /work/projects/law-local/review/definitions/2026-11-17-sample-1
```

- It printed
  `drew 500 of 34329 definitions and 500 of 5940 near misses of the release 119-142 with the seed 20261117`
  and exited 0, in 6.94 s wall with a peak of 231,908 KiB.
- The seed is the date of the draw, written as a number before the draw was
  made.
- The same command with `--out /cache/fidryn/task-22/draw-again` wrote the
  same bytes (`cmp`, both files). So did the tool as it is committed, into
  `/cache/fidryn/task-22/draw-final` (5.03 s, 232,372 KiB), after a guard
  for a text part without blocks was added to it, a case the workspace does
  not hold.
- `extracted.jsonl` is 254,393 bytes and `near-misses.jsonl` 297,422.

**The populations.**

- The 34,329 definitions are every `definition` block of the workspace, the
  count of the ledger. Each was placed in the paragraph it was extracted from,
  and none was left without one.
- The 5,940 near misses are the paragraphs of operative text that hold a term
  in curly quotation marks within 60 characters before `mean`, `means`,
  `include`, `includes`, `shall mean` or `has the meaning`, and from which no
  definition was extracted. F14's 5,707 counts something else: text parts
  that match none of the patterns. The population here counts paragraphs, in
  every part and in tables, so it also holds paragraphs that match a pattern
  and that a rule of S10 leaves out, such as one under a chapeau that is not a
  reach chapeau.

**What was drawn.**

- *The first sample:* 500 definitions from 38 titles. By basis: 362
  `restated`, 100 `enacted`, 38 `unresolved`. By kind: 412 `means`, 42
  `borrowed`, 36 `includes`, 10 `does_not_include`. 412 have a chapeau above
  them, and 6 have the words `unstated`.
- *The second sample:* 500 paragraphs, 390 in a content, 76 in a chapeau and
  34 in a continuation; 447 are the first paragraph of their provision. By
  the verb that put them in: 277 `means`, 91 `has the meaning`, 59
  `includes`, 51 `include`, 15 `mean`, 7 `shall mean`.

Each line holds what a reader needs to judge it without opening the Code:

- For a definition: the address, the terms, the kind, the reach words, the
  region, the basis, the text of the paragraph, and, when a provision above it
  has a chapeau, the address and the text of the nearest one.
- For a near miss: the address, the part, the place of the paragraph, its
  text, the same chapeau, and the term and the verb that put it in.
- For both: the seed, the release, the size of the population, the place in
  the draw, and an empty `verdict`.

`review/definitions/estimate.json`:

```json
{
  "release": "119-142",
  "lock_digest": "83918ea6e9e0a63195f096fdb05b60dbd2574f43e4467d8ad2143ae1424389dd",
  "seed": 20261117,
  "sample": "2026-11-17-sample-1",
  "populations": {
    "extracted": 34329,
    "near_misses": 5940
  },
  "drawn": {
    "extracted": 500,
    "near_misses": 500
  },
  "status": "drawn, not read"
}
```

The samples and the estimate were committed to the law repository with the
message `review: two samples of definitions drawn from 119-142`, as
`c9bd0e44c865aaee3b52e086ddde66ca8808fab2`, signed (`G`). The history of
`main` is now the 27 commits of the releases from its root, then the commit
of `encodings/` and this one.

## What is open after the second part

- **Acceptance item 7 is open.** By decision O-3 nobody is named yet to read
  the samples. What closes it:
  - a person the owner names reads each of the 500 lines of
    `extracted.jsonl` against the official text and writes into `verdict`
    whether the terms, the kind and the reach words are right, and if not,
    which is wrong;
  - they read each of the 500 lines of `near-misses.jsonl` and write whether
    the paragraph defines a term that the closed patterns missed;
  - `estimate.json` is then written with the errors found in the first sample
    (the bar of design 9.5 is none), and with the definitions found among the
    near misses and the share of the 5,940 they imply.

  If the first sample has an error, the patterns are corrected (Task 15's
  tests gain the case), the sample and its verdicts are kept, and a new one is
  drawn with a new seed into `review/definitions/<date>-sample-2/`. The tool
  refuses to write over a sample that is there.
- **One way of stopping the replay was not tried on the real run.** The stop
  came in the middle of an import, the case the second run resumed. A stop
  between the replacement of `uscode/` and its commit, the case the kept text
  form in `.git/fidryn-replay.json` is for, did not occur. The tests of
  `conformance/test_replay_order.py` hold the replacement to its conditions
  and the reading of a history to the commits it names; they make no commit.
- **The near-miss population is not F14's**, for the reasons above. It is the
  population the plan's Task 22 names, counted from the text records.
- **The scripts** `units_by_layer.py`, `check_messages.py`, `log_table.py`
  and `compare_pairs.sh`, and the logs of every command, are in
  `.agents/scratchpad/2026-11-17-stage1/work/task-22/` of the main checkout,
  which git ignores, so that none of them is in a commit. The large outputs
  are under `/cache/fidryn/task-22/`.
