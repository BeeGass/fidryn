# Stage 1 acceptance: what the thirteen items show

This record says, for each of the thirteen acceptance items of Stage 1, the
command that decides it, the output that decides it, and a verdict: "met",
"met with a written difference" (the difference and its reason are given), or
"open" (what closes it is given). It is written for the owner, who decides from
it whether the stage is merged.

- The items are those of section 9.6 of the design,
  `.agents/docs/specs/2026-11-13-law-monorepo-design.md`, quoted in its words
  and in its order. "Measured on release point 119-142 unless stated."
- The short names F1 to F16, S1 to S16 and O-1 to O-7, and "Task N", are those
  of the plan of the stage,
  `.agents/docs/plans/2026-11-16-stage-1-the-code-as-a-checked-program.md`:
  its "Measured facts", the sections of its "Specification", its "Decisions
  that are the owner's" and its tasks.
- Items 1 to 9 were decided by the first full import of the Code and by the
  replay of the 27 releases. Their record is
  `.agents/docs/reviews/2026-11-17-stage-1-full-code-run.md`, called "the
  run's record" below. Its sections are cited by their headings, and the lines
  that decide are copied from it. The import was not made again.
- The record was first committed in `e6fef77`, after cheap runs that showed
  the state of that day: the release build, `fidryn check` of the law
  repository's workspaces, `git log` of the law repository, the script of
  item 13, the gate, the release check and the license check, made on
  2026-11-17 between 23:06 and 23:22, Eastern time, on the same workstation,
  through its scheduler.
- A review of the whole branch then found eight faults, in cases no release
  of the Code holds, and they were corrected. The section "What a review of
  the whole branch found, and what was corrected", after the thirteen items,
  says what was found, what was done and what was checked again. This record
  was brought up to the corrected branch on 2026-11-18: the release build,
  the check of the law repository, the script of item 13, the gate, the
  release check, the license check and the package build were run again on
  it. Each item says whether its verdict stands after the corrections, and
  why. The last commit of the stage, which holds this version of the
  record, adds two checks the controller of the run made again with the
  corrected scripts (the profile of item 2 and the independent comparison of
  item 5), a count of the tables of contents that stand inside another, and
  the list of what the two readings differ on, in the reference as here; its
  gate is the one item 10 cites.

The record holds no text of the Code beyond addresses.

## The verdicts

| Item | In short | Verdict |
| --- | --- | --- |
| 1 | `fidryn check` accepts the workspace in one invocation | met |
| 2 | the counts against the design's profile | met with a written difference |
| 3 | every unresolved reference has a recorded reason | met |
| 4 | the rendered text is the official text; every module verifies | met |
| 5 | the change set on a real pair, confirmed independently | met |
| 6 | staleness follows section 9.4 | met |
| 7 | the two definition samples are drawn, read and recorded | **open** |
| 8 | the 27 releases replayed as commits | met with a written difference |
| 9 | ten minutes cold, thirty seconds unchanged, 16 GB | met |
| 10 | the existing tests and the Stage 0 checks pass | met with a written difference |
| 11 | the reference documents the new declarations; the site shows the ledger | met with a written difference |
| 12 | a second workspace at a pinned release | met |
| 13 | the integration test of section 4.3, on this machine | met |
| 13 | the integration test of section 4.3, on GitHub | **open** |

## Where things are

**This repository.** The record, as it stands now, is committed on the
branch `fix/records`, in a commit whose parent is
`47c9205a721739ecbe93add79aff079f7a0960e3`, which amends the plan on top of
`b6d8b49a84729e2c83e0985d76f79654ab45ac07`, the last commit that changes
code. Besides the records, that commit changes the section "Limits" of
`docs/law-modules.md` and the site generated from it, and no code. The gate
of item 10 ran on the tree of that commit before the record was given the
gate's numbers; no step of the gate reads `.agents/`.

**Two binaries.**

The binary of the run made the import, the checks and the replay that items 1
to 9 rest on. It was built from `d0658b6`, and built again from
`98d5765a9391a69da231d0a20f68b19aed662094` on a clean tree on 2026-11-17:

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-model lease run --cpus 16 --mem 25G --time 1h -- /usr/bin/time -v cargo build --release -p fidryn-cli --locked --offline
```

- Exit 0, 26.15 s wall, 1,046,764 KiB peak.
- 13,837,000 bytes; SHA-256
  `f2dd81dde0ade77b922bfb1a37155ef26f803377fc6d0548f4eb8bfe681a6ef3`, BLAKE3
  `1588aa901707193fda1858482da57f874c76152eff36cd5af10564f1c1412427`
  (`sha256sum`, and `blake3` of the locked Python environment), the digests
  `sha256sum /cache/fidryn/task-21/fidryn` gives of the run's own copy.
- It was copied to `/cache/fidryn/task-23b/fidryn`. "BINARY" below is that
  file in the commands of this record, and the binary of the run in the
  commands copied from the run's record: the same bytes.

The corrected binary was built from `47c9205` on a clean tree on 2026-11-18,
by the same command:

- Exit 0, 23.26 s wall, 1,047,700 KiB peak.
- 13,921,584 bytes; SHA-256
  `ec39b6b99f44d200b003aaeb7f5d1180a8c340c22d9838be34a499331b4cb0fa`, BLAKE3
  `56f9dc6a1f25cbebfd01a087ec8cb96fd7c9d9790448cf5f337ccc87899bf0c2`: the
  same bytes as `/cache/fidryn/final/fidryn`, the build of `b6d8b49` with which
  the controller of the run made the checks of the section on the corrections.
- It was copied to `/cache/fidryn/task-23b/fidryn-final`, "FINAL" below.
  `FINAL --version` prints `fidryn 0.3.0`. The commit that adds this version
  of the record changes no file the binary is built from.

Each run of BINARY or FINAL for this record ran inside a job of the
scheduler, one job at a time, under these limits:

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ...
```

One job held six runs one after the other, each under its own limits ("Known
limits", 1). The script of item 13 ran inside a job of the same size and gave
each run of the binary the limits it sets itself. Every check named its cache
with `--cache-dir` under `/cache/fidryn/task-23b/`; `~/.cache/fidryn`, the
default cache of a check since the corrections, did not exist before these
runs and does not exist after them.

**The law repository** is `/work/projects/law-local/`, local and with no
remote (decision O-6). It was only read. Today:

- `main` is at `c9bd0e44c865aaee3b52e086ddde66ca8808fab2` and holds 29 commits
  from the root `b8ec1e448a9ccd6336caf4a0e7a60477676393c9`: the 27 releases,
  then `encodings/`, then `review/`.
- `first-import` is at `218b87fb28eccc751436e5eb96d79c1cc1689d2c`, the first
  import of 119-142.
- `git --no-optional-locks status --short` prints nothing.

Its git commands were run with `--no-optional-locks`, so that not even git's
index was written. On each of the two days a file `MARKER` was made just
before the first run of a binary on the law repository, and after the last
one

```sh
find /work/projects/law-local -newer MARKER -not -path '/work/projects/law-local/.git/*' -not -path '/work/projects/law-local/.git'
```

found nothing.

**The archives** are `/library/corpora/uscode/<release>/xml_uscAll@<release>.zip`,
each with its retrieval record. That of 119-142 is 108,706,565 bytes, SHA-256
`a216a8b0fc4ebc8500ae3c7dc626d9731804c00def2ba2e6e7900a0e34e856f0`, the same
bytes as the archive the plan's facts were measured on (the run's record, "The
archive").

**The runs of the run's record**, which items 1 to 9 cite:

- The import, its checks, its counts and its text (items 1 to 4 and 9) ran
  from a checkout of this repository at `d0658b6`, with the binary built from
  it, on the archive of 119-142. The workspace was then committed as the
  first commit of the law repository, `218b87fb2`, now on `first-import`.
- The replay, the real pair and the samples (items 5 to 8) ran with the same
  binary and the scripts of `6e0f059` (here `98d5765`; the run's record says
  which guard each tool gained after its run), on the 27 archives. They made
  the commits of `main` from `b8ec1e448` to `37192eb42`, then `7493597`
  (`encodings/`) and `c9bd0e4` (`review/`).

**Scratch.** The scripts and logs of this record are in
`.agents/scratchpad/2026-11-17-stage1/work/task-23b/` of the main checkout of
this repository, which git ignores; those of 2026-11-18 begin with `fix1-`.
"SCRATCH" below is that directory. The caches and large outputs are under
`/cache/fidryn/task-23b/`.

## Item 1: the workspace is accepted in one invocation

> 1. The ingester produces a workspace that `fidryn check` accepts in one
>    invocation.

**At the import** (the run's record, "The cold and the warm check (item 1)"),
with the binary built from `d0658b6` and an empty cache:

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY check /work/projects/law-local/uscode --jobs 16 --cache-dir /cache/fidryn/task-21-cache --report /cache/fidryn/task-21/report-cold.json --unresolved /cache/fidryn/task-21/unresolved-cold.jsonl
```

> - Output: `modules: 70794; from cache: 0; errors: 0; warnings: 0`, then `ok`.
> - Exit 0, 3.29 s wall, 18.6 s user, 831,792 KiB peak.

**Today**, on `uscode/` of `main` at `c9bd0e4`, with FINAL and the cache the
script of item 13 had filled:

```sh
cd /work/projects/law-local && lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v -o /cache/fidryn/task-23b/fix1-check-uscode.time timeout 1800 prlimit --as=17179869184 --cpu=3600 -- FINAL check /work/projects/law-local/uscode --jobs 16 --cache-dir /cache/fidryn/task-23b/cache-final --report /cache/fidryn/task-23b/fix1-report-uscode.json --unresolved /cache/fidryn/task-23b/fix1-unresolved-uscode.jsonl
```

```text
modules: 70794; from cache: 70794; errors: 0; warnings: 0
ok
```

- Exit 0, 1.89 s wall, 831,244 KiB peak; nothing on stderr.
- `diff /cache/fidryn/task-21/report-cold.json /cache/fidryn/task-23b/fix1-report-uscode.json`
  prints only `"from_cache": 0` against `"from_cache": 70794`.
- `cmp /cache/fidryn/task-21/unresolved-cold.jsonl /cache/fidryn/task-23b/fix1-unresolved-uscode.jsonl`
  exits 0.
- The same check with BINARY, on 2026-11-17, gave the same output, the same
  report but for `from_cache`, and the same unresolved list
  (`/cache/fidryn/task-23b/report-uscode.json`, `unresolved-uscode.jsonl`).

The workspace in the law repository today is the one the import wrote, and
the corrected build checks it as the build of the run did.

**Verdict: met.** It stands after the corrections: the corrected build checks
the workspace the same way, and writes the same workspace for every release
(the section on the corrections).

## Item 2: the counts

> 2. Counts for the Code's own provisions match the independent profile in
>    section 3.1: 60,490 sections with the same status breakdown; 772,863
>    addressable provisions; 90,719 references to the Code in operative
>    text, of which 88,935 resolve exactly, 820 reach the section only and
>    964 reach no section. Any difference is explained in writing. Totals
>    for the note layers are reported beside them.

**Commands** (the run's record, "The counts (item 2)"), on the archive of
119-142:

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I conformance/usc_profile.py /library/corpora/uscode/119-142 --out /cache/fidryn/task-21/profile.json
```

The profile prints two sets of counts, by the two sets of rules of
`conformance/usc-counting-rules.md`: `baseline`, the counting of the design's
profile, and `import`, what the reading imports. The ledger of the workspace,
`coverage.json`, equals `import` on all 14 keys they share, and the check
report's references equal both.

**What decides** (the run's record, "The three sets side by side"):

| Count | F1 (the design's numbers) | `baseline` | Import |
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

> - **The notes:** 87,335 enacted and 83,116 editorial, 170,451 in all.

- `baseline` gives every number of the item exactly.
- The import's own counts differ. Each difference is explained in the run's
  record, "Where each difference comes from", element by element, with
  examples:
  - units +681 = 544 court rules + 107 reorganization plans + 11 generic
    levels that are units + 20 sections without an identifier − 1 section that
    stands inside another;
  - provisions +1,155 = the top provisions of those 682 units + 126 provisions
    of kind `level` in court rules + 347 in sections;
  - references +334 resolve exactly, +61 reach the section only and −395 reach
    no section, by three rules: targets of kind `level` (7), `ambiguous` (4),
    and sections whose identifier lists several sections or names a range (323
    and 72).
- The rule for each difference is now written in
  `conformance/usc-counting-rules.md`, "Where the two differ". The run's record
  found that paragraph incomplete. It named no rule for the 395 references,
  and its words "and their provisions" overstated what `baseline` leaves out
  of court rules. It was completed in `1133f24`, `Name every rule by which
  the two profiles of the Code differ`, with no number changed.

**Verdict: met with a written difference.** The design's numbers are
reproduced exactly by its own counting (`baseline`). The workspace's counts
differ from them, by the rules above, for the reasons the run's record gives;
the item asks for that explanation and it is written.

It stands after the corrections, on both sides of the comparison:

- The ledger is part of the workspace, and the corrected build writes the
  same workspace for 119-142, so its counts are the same bytes.
- The profile was counted again with the corrected scripts. The controller of
  the run ran, inside a `lease` job, from the worktree at `b6d8b49`,

  ```sh
  uv run --locked python -I conformance/usc_profile.py /library/corpora/uscode/119-142 --out /cache/fidryn/controller-compare-final/profile-119-142.json
  ```

  It exited 0, and the file is byte for byte the profile the full run kept,
  `/cache/fidryn/task-21/profile.json`: both have the SHA-256
  `0ae9dc9d8ec74c99aa44432c8cc2a6a37c6b545c89f90deb606462df16fc8259`
  (`sha256sum`, and `cmp` exits 0). So the corrected reading counts what the
  reading of the full run counted, `baseline` still reproduces F1, and
  `import` still equals the ledger key for key.

## Item 3: every unresolved reference has a recorded reason

> 3. Every unresolved reference in every layer has a recorded reason. None
>    is unknown without a listed example.

**Command:** the cold check of item 1, which writes the report and the
unresolved list.

**What decides** (the run's record, "The references (item 3)"):

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

> - **Outcome `outside`, by body:** `/us/pl` 599,160, `/us/stat` 321,830 and
>   `/us/act` 53,758. That makes 974,748.

> - **The unresolved list.** It has 1,001,197 lines, 136,460,771 bytes, one for
>   each reference whose outcome is not `exact`, sorted by the address it stands
>   in. `grep -c` of each outcome in it gives the totals of the table.

- Every reference of every layer has one of the eight outcomes of S9, a
  closed list; none is without one. Each that is not `exact` has its line in
  the unresolved list, with its outcome as the reason.
- Of these outcomes, the nearest to the design's "unknown" is `no-unit`: a
  target below `/us/usc` that names nothing the workspace holds. The run's
  record lists one example of it for each layer, and one of each other
  outcome but `exact` and `outside`, for each layer; `outside` is given by
  the body of law it names.
- `malformed` never occurs, so it has no example.
- The unresolved list that FINAL writes today is the same bytes as that of
  the import (item 1).

**Verdict: met.** It stands after the corrections, by the last point.

## Item 4: the text and the pinned artifact

> 4. For every provision and every note, the text rendered from the
>    generated module equals the normalized text taken from the official
>    XML, and every module verifies against the pinned artifact.

**Commands** (the run's record, "The text (item 4)"):

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY text /work/projects/law-local/uscode --json > /cache/fidryn/task-21/text-fidryn.jsonl
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I conformance/uslm_text.py /library/corpora/uscode/119-142 > /cache/fidryn/task-21/text-python.jsonl
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ingest uscode /library/corpora/uscode/119-142 --out /work/projects/law-local/uscode --jobs 16 --verify
```

The two outputs were compared line by line by position with a scratch
script, and with `cmp`.

**What decides:**

> - Each output has 1,015,386 lines.
> - 1,015,386 lines were compared, and **0 differ**. `cmp` of the two files also
>   exits 0.
> - The lines are 774,018 provisions, 9,623 containers, 170,451 notes, 51,567
>   source credits and 9,727 footnotes.

> - It printed `ok` and exited 0, in 5.94 s wall with a peak of 2,214,692 KiB.
> - The workspace holds every file the import writes, with the same bytes, and
>   nothing else.

- `conformance/uslm_text.py` is an independent reading of the publisher's
  XML: it shares no code with the importer.
- `--verify` reads the archive again, held to its retrieval record, writes
  nothing, and compares every file an import would write with the file in the
  workspace. The check of the workspace also holds every module file to the
  digest its lock records (`E730`).
- In CI the same comparison runs on the sample: the gate of item 10 printed
  "the text of its 7609 records is what conformance/uslm_text.py prints".
- **On every release, after the corrections.** The controller of the run
  compared the output of `fidryn text uscode --json` with that of
  `conformance/uslm_text.py` for each of the 27 releases of 2026, both with
  the code of `e6fef77` and the binary of the run, and with the corrected
  code of `b6d8b49` and its build. Both runs found the two outputs the same
  bytes for 27 of 27 releases, 27,376,226 lines in all, from 1,006,854 for
  119-73not60 to 1,015,386 for 119-142
  (`/cache/fidryn/controller-text27/result.tsv` and
  `/cache/fidryn/controller-text27-final/result.tsv`, one line for each
  release; the script is `work/controller/text27.py` with the scratch files
  of the run). The corrections changed both readings, so both runs count.

**Verdict: met.** It stands after the corrections, and its evidence is now
wider than the item asks: the two readings agree on every release of the
year, not only on 119-142. They read nine forms of the XML differently that
no release holds, and on four of them only such a comparison shows the
difference ("Known limits", 3).

## Item 5: the change set of a real pair, confirmed independently

> 5. The change-set command, run on two real consecutive releases, reports
>    exactly the provisions and notes whose normalized text or structure
>    differs, separately for operative text, enacted notes and editorial
>    matter, confirmed by an independent comparison of the XML.

**Commands** (the run's record, "Item 5: the change set and the independent
comparison"), with the binary of `d0658b6`, on the pair 119-138 and 119-142
(F10), 119-138 imported into `/work/projects/law-local-pair/119-138`:

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- sh -c 'BINARY changes /work/projects/law-local-pair/119-138 /work/projects/law-local/uscode --json --jobs 16 --cache-dir /cache/fidryn/task-22-cache > /cache/fidryn/task-22/pair-changes.json'
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I conformance/usc_compare.py /library/corpora/uscode/119-138 /library/corpora/uscode/119-142 --against /cache/fidryn/task-22/pair-changes.json --jobs 8
```

**What decides:**

> - Exit 0, 2.87 s wall, 1,081,856 KiB peak; 5,907 bytes.

> - `cmp` with `changes/119-142.json` of the law repository exits 0: the import
>   of 119-138 outside the repository is the one the replay committed, and the
>   change set is the one the 27th commit holds.

> It printed nothing and exited 0, in 42.25 s wall with a peak of 3,290,924 KiB.
> The independent reading of the two archives finds, in each group, the
> addresses the change set lists.

The change set lists, for operative text, 4 added, 0 removed, 11 amended in
text, 3 amended in structure and 0 moved; for enacted notes 9 added, 5
removed, 10 amended and 0 moved; for editorial matter 13 added, 9 removed, 3
amended, 0 moved, 4 credits and 18 footnotes changed; and no container.

- The same comparison was made for each of the 26 pairs of the replay, and
  each exited 0 (the run's record, "Every pair of the replay, compared
  independently").
- `usc_compare.py` reads both archives with the independent reading of item 4.
  For each layer it finds the addresses only in the older release, only in the
  newer, and on both sides with a different record. It holds those sets to the
  unions of the change set's lists that stand for them: added with the targets
  of moves, removed with their origins, and the two kinds of amended
  together. So the set of what changed in each of the three layers is
  confirmed independently.
- "Exactly" holds within what a change set lists ("Known limits", 1). The
  credit and the footnotes of a unit on one side only or moved, the place of
  a footnote marker inside its provision, and a change of a unit's aliases,
  of its container or of a file's positive-law flag are in no list. A
  superscript that is not marked as a footnote marker is text, so a provision
  that gained only one is listed as amended. Which list of a union an address
  is in, and the pairing of a move, are held by the tests of `fidryn-driver`
  only, not by the independent comparison.
- F10 counts 9 units with a change in operative text and 14 with only an
  editorial one; the change set gives 10 and 13. The tenth is 26 U.S.C. 45Y,
  where a link was taken off words that stayed, which S15 counts as a change
  of the provision's own words; the independent comparison agrees with the
  change set (the run's record, "The count of each list").

**Verdict: met.** It stands after the corrections. One of them changed how
like-named notes are listed, in `fidryn changes` and in `usc_compare.py`
alike.

- The 26 change sets the corrected build writes from the archives are the
  same bytes as those the replay committed (the section on the corrections).
- The corrected independent comparison agrees with every one of them. For
  each of the 26 release commits after the first on `main` of the law
  repository, the controller of the run gave the file
  `changes/<release>.json` of that commit to
  `conformance/usc_compare.py OLD NEW --against FILE` on the archives of its
  two releases, with the script of `b6d8b49`: 26 of 26 exited 0
  (`/cache/fidryn/controller-compare-final/result.tsv`, one line for each
  pair; the driver is `work/controller/compare26.py` with the scratch files
  of the run). The real pair of this item is the last of them.

## Item 6: staleness

> 6. Staleness follows section 9.4. A curated module bound to a provision
>    that changes between those two releases is reported stale. One bound
>    within a section whose enacted notes change is reported stale. One
>    bound within a section where nothing or only editorial matter changes
>    is not.

**The bindings** (the run's record, "The three bindings"): the curated
workspace `encodings/` of the law repository, pinned to 119-142, binds
`/us/usc/t11/s109/e` (its text changed), `/us/usc/t11/s101/51D` (its unit
changed only in its enacted notes) and `/us/usc/t26/s457/e/14` (its unit
changed only in editorial matter, a footnote marker added in this very
paragraph), each with the release 119-138 and the hashes `fidryn text
--hashes` printed for it in 119-138. The names of the bindings are
identifiers of the law repository's module, chosen when it was written, and
are given as the tool printed them.

**At the replay** (the run's record, "Item 6: what the check and the change
set say"): `fidryn check encodings` warned `W750` for the first two and not
the third, and `fidryn changes /work/projects/law-local-pair/119-138 uscode
--bindings encodings` listed the same two in `stale_bindings`.

**Today**, the script of item 13 checked `encodings/` of `main` at `c9bd0e4`
with FINAL (its whole output is under item 13). On stderr:

```text
/work/projects/law-local/encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `chapter_13_debt_limits` of `/us/usc/t11/s109/e` is stale (provision text changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
/work/projects/law-local/encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `small_business_debtor` of `/us/usc/t11/s101/51D` is stale (enacted notes of the unit changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision and the enacted notes of its unit again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
```

and on stdout `modules: 1; from cache: 0; errors: 0; warnings: 2`, then `ok`.
BINARY printed the same on 2026-11-17. That day the same check run alone, by
the command of item 1 with `encodings` in place of `uscode` (1.32 s,
577,396 KiB), printed the same two lines and
`modules: 1; from cache: 1; errors: 0; warnings: 2`, then `ok`, and its report
differed from that of the replay's check
(`/cache/fidryn/task-22/encodings-report.json`) only in `from_cache`.

The three cases are also held on invented text by the tests
`workspace::tests::staleness::a_binding_to_a_changed_provision_is_stale`,
`a_binding_in_a_unit_whose_enacted_notes_changed_is_stale` and
`editorial_changes_make_nothing_stale` of `crates/fidryn-driver`, which passed
in the gate of item 10. `cargo test -p fidryn-driver --locked --offline --
staleness::` runs them with the other tests of staleness, through `lease` on
this machine.

**Verdict: met.** It stands after the corrections: FINAL reports the same.

## Item 7: the definition samples

> 7. The definition samples of section 9.5 are drawn, read and recorded,
>    and the first has no errors.

**Drawn** (the run's record, "The definition samples (item 7)"):

```sh
lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I tools/draw_definition_samples.py --workspace /work/projects/law-local/uscode --records /cache/fidryn/task-22/text-119-142.jsonl --seed 20261117 --out /work/projects/law-local/review/definitions/2026-11-17-sample-1
```

> - It printed
>   `drew 500 of 34329 definitions and 500 of 5940 near misses of the release 119-142 with the seed 20261117`
>   and exited 0, in 6.94 s wall with a peak of 231,908 KiB.

The corrections made the drawing tool hold the definitions it reads to the
workspace's ledger, and the records to the workspace's counts. Run again on
the same inputs, the corrected tool drew the same two files byte for byte
(`fix-wave-Z-report.md` with the scratch files of the run, "The drawing of
definition samples").

**Where they are, today**, in the law repository, committed in `c9bd0e4`
(`review: two samples of definitions drawn from 119-142`):

- `review/definitions/2026-11-17-sample-1/extracted.jsonl`: 500 lines
  (`wc -l`), each one definition drawn from the 34,329 of the workspace;
- `review/definitions/2026-11-17-sample-1/near-misses.jsonl`: 500 lines,
  each one paragraph drawn from the 5,940 paragraphs of operative text that
  hold a term in curly quotation marks within 60 characters before `mean`,
  `means`, `include`, `includes`, `shall mean` or `has the meaning`, and from
  which nothing was extracted: the population the plan's Task 22 names, as
  the run's record says;
- in each file, `grep -c '"verdict":""'` prints 500: no line has a verdict;
- `review/definitions/estimate.json` says `"status": "drawn, not read"`.

**Not read.** Decision O-3 names nobody to read them.

**What closes it.** A person the owner names:

1. reads each of the 500 lines of `extracted.jsonl` against the official text,
   and writes into its `verdict` whether the terms, the kind and the reach
   words are right, and if not which is wrong;
2. reads each of the 500 lines of `near-misses.jsonl`, and writes whether the
   paragraph defines a term the closed patterns missed;
3. writes into `estimate.json` the errors found in the first sample and the
   share of the 5,940 that the definitions found among the near misses imply.

**What a line holds, and when it is not enough.** A line holds the
paragraph, and the nearest chapeau above it with its address. They are
enough to judge the terms, the kind, and reach words taken from them; the
bar is textual and needs no attorney. They are not always enough to judge
the reach. 6 of the 500 lines of the first sample have the words `unstated`
and no chapeau. For two of them, `/us/usc/t15/s1602/o` and
`/us/usc/t15/s1602/dd/7`, the reach is stated in the sibling subsection
`/us/usc/t15/s1602/a`, which no line holds ("Known limits", 2). For a line
whose words are `unstated`, and wherever the reach may be stated elsewhere in
the unit, the reader opens the unit: `fidryn text DIR UNIT`, with the
workspace as `DIR` and the address of the unit, or the official text.

The item is met when the first sample has no error. If it has one, the
patterns are corrected (with a test for the case), the sample and its
verdicts are kept, and a new sample is drawn with a new seed.

**What is not claimed.** The importer's 34,329 definitions equal, field for
field, those of a second implementation of the same rules (S10), written
apart from the repository and kept with the scratch files of the run (the
run's record, "Definitions"). The importer and that implementation agree on
every definition. That does not show that the rules find every definition,
or that each definition they find is right. No claim about the correctness
of the definitions is made until the samples are read.

**Verdict: open.**

## Item 8: the 27 releases as commits

> 8. All 27 release points the publisher offers are replayed as commits in
>    the local law repository, each with its change set as the commit
>    message.

"All 27" are the release points the publisher lists for 2026, in its order
(F9).

**Command** (the run's record, "The replay"), stopped once on purpose in the
middle of an import and then run again:

```sh
lease run --cpus 16 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I tools/replay_uscode.py --archives /library/corpora/uscode --repository /work/projects/law-local --fidryn /cache/fidryn/task-22/fidryn --cache-dir /cache/fidryn/task-22-cache --jobs 16
```

> It ended with `the replay is complete: 27 releases are committed` and exit 0,
> in 4 min 53.46 s wall, 1,176.77 s user, with a peak of 2,325,624 KiB. No
> release was committed twice, and none was skipped.

**Today:**

```sh
git -C /work/projects/law-local --no-optional-locks log --format='%h %G? %s' main
```

```text
c9bd0e44c G review: two samples of definitions drawn from 119-142
749359773 G encodings: three bindings written against 119-138
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

- The 27 commits of the releases are the last 27 lines, oldest last, each
  signed (`G`). `git rev-list --count main` prints 29, with the two after
  them; `git rev-list --max-parents=0 main` prints the one root,
  `b8ec1e448a9ccd6336caf4a0e7a60477676393c9`.
- The message of each of the 26 later commits is the text form of its change
  set, byte for byte. This was held by the tool as it committed, and again
  afterward by reading each message from its commit and each change set from
  the commit's own `changes/<release>.json` (the run's record, "The 27
  commits").
- The tree of the 27th commit holds the same `uscode/` as the first import
  (`git diff --quiet first-import main -- uscode .gitattributes .gitignore`
  exited 0 when the replay ended).

**Verdict: met with a written difference.** The first commit,
`/us/usc: release 119-73not60`, has no change set as its message. There is no
release before it among the 27 to compare it with, so its message names the
release, as the plan's Task 22 prescribes.

It stands after the corrections. The corrected replay tool and build,
replaying the 27 archives into a new repository, made 27 signed commits
whose trees are those of the 27 commits above, hash for hash (the section on
the corrections). So the law repository was not made again, and its commits
stand.

## Item 9: time and memory

> 9. Checking the full workspace, notes included, takes at most 10 minutes
>    from cold on 16 threads and at most 30 seconds when nothing changed,
>    within 16 GB of memory. These are initial targets to confirm with a
>    first spike.

**At the import** (the run's record, "The cold and the warm check (item 1)"),
with `--jobs 16` inside a job of 16 processors and 16 GB:

| Check | Wall | Peak | Bound |
| --- | --- | --- | --- |
| cold, the cache empty | 3.29 s | 831,792 KiB | 10 minutes, 16 GB |
| warm, nothing changed | 1.65 s | 831,536 KiB | 30 seconds, 16 GB |

**Later runs:**

- with BINARY, on 2026-11-17: the warm check of item 1, 1.66 s and
  831,296 KiB; the first run of the script of item 13, with an empty cache,
  4.78 s for the whole script and 830,144 KiB. Checking `encodings/` checked
  its dependency `uscode/` from cold, then `uscode/` itself from the cache.
- with FINAL, on 2026-11-18: the script of item 13 with an empty cache,
  4.63 s and 831,984 KiB; the warm check of item 1, 1.89 s and 831,244 KiB.

Each run of the binary in the script had at most 8 GiB of address space, the
script's own limit, and none reached it.

The cold check took 0.55 percent of the time the design allows, and the warm
check 5.5 percent of its 30 seconds (6.3 percent today). Each used about five
percent of the 16 GB.

**Verdict: met.** It stands after the corrections: FINAL takes the same time
and memory.

## Item 10: the existing tests and the Stage 0 checks

> 10. The existing 1,188 tests and the Stage 0 checks still pass.

**The gate**, on the tree of this commit, the last of the stage, before the
record was given its numbers, on 2026-11-18:

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-model lease run --cpus 16 --mem 25G --time 2h -- /usr/bin/time -v cargo xtask ci --fmt
```

It ended with exit 0, in 1 min 3.81 s wall, with a peak of 1,030,952 KiB.
Its log is `SCRATCH/closing-gate.log`. `git diff` of everything outside
`.agents/` had the same SHA-256 before and after it. In the order of its
steps:

- `cargo fmt --all -- --check` passed;
- the Rust tests: 72 lines `test result:`, together **1,831 passed, 0
  failed, 0 ignored**; no line of the log begins with `warning`;
- `cargo clippy ... -- -D warnings` and `cargo doc` passed;
- the schema probes: `"checked": 411`, `"failed": 0`;
- `All checks passed!`, `30 files already formatted`, `Success: no issues
  found in 30 source files`;
- the conformance tests: `Ran 231 tests`, `OK`;
- the sample step: "samples/uscode/archives matches retrieval.json;
  samples/uscode/workspace is what the ingester writes from it, on one worker
  and on eight, checks with no error, and holds the ledger `fidryn coverage`
  prints; samples/uscode/profile.json is current and counts the references
  the check counts and what the ledger counts; the text of its 7609 records
  is what conformance/uslm_text.py prints; samples/uscode/curated checks
  against it with no error and no warning, each of its 8 citations `exact`;
  and the table of docs/coverage.md is what the ledger gives";
- `site/ is up to date (30 generated files)`;
- the example status check and the documentation check, which print nothing
  when they pass; a failure of either ends the gate;
- the JavaScript tests: `tests 52`, `pass 52`, `fail 0`;
- `cargo bench --no-run` finished.

`~/.cache/fidryn` did not exist before the gate and did not exist after it.

The gate of `47c9205` before the last commit gave the same counts
(`SCRATCH/fix1-gate.log`, 1 min 51.44 s).

Then:

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-model lease run --cpus 4 --mem 8G --time 30m -- cargo run -p xtask --offline -- release-check
lease run --cpus 4 --mem 8G --time 30m -- cargo deny --offline check licenses bans sources
env CARGO_TARGET_DIR=/cache/cargo/target-lane-model CARGO_NET_OFFLINE=true lease run --cpus 16 --mem 25G --time 1h -- /usr/bin/time -v cargo xtask package --output-dir /cache/fidryn/task-23b/package-final
```

- The release check printed `release 0.3.0 (2026-11-18): workspace versions
  and changelog verified`, exit 0, on the tree of this commit and on
  `47c9205`.
- The license check, on `47c9205`, printed `bans ok, licenses ok, sources
  ok`, exit 0, and warned twice `license-not-encountered`, for two licenses
  `deny.toml` allows that no dependency has. `Cargo.lock`, `Cargo.toml` and
  `deny.toml` are the same at this commit.
- The package build refuses a tree with changes not committed, so it ran on
  `47c9205` itself, which differs from this commit only in the section
  "Limits" of the reference, the site and the records: exit 0,
  "Verified native package: .../fidryn-0.3.0-x86_64-unknown-linux-gnu.tar.gz",
  `Source: 47c9205a721739ecbe93add79aff079f7a0960e3`, SHA256
  `0948010775c28fcaf2e072bc2ec4dcf07e10359960768497b7c5639d0438d2b8`, BLAKE3
  `dacbc0da8d6ed2ccabcf618b48a6001f27f0e4e8116ed8535bcc7adb970e3be7`; the
  packaged binary checked `examples/package-smoke.fr` with `ok`. It had also
  passed on `e6fef77`, the commit that first added this record
  (`SCRATCH/package-2.log`).

On `e6fef77` the gate had passed with 1,800 Rust tests and 199 conformance
tests (`SCRATCH/gate-1.log`); the corrections added the others.

**The 1,188.** The design counted them at `1a70382` with
`cargo test --workspace --offline`. Which tests then stand, unchanged, at
`47c9205` was found by reading the test functions of the commits from git:

```sh
uv run --locked python -I SCRATCH/test_names.py /work/projects/fidryn-lanes/model 1a70382 47c9205
uv run --locked python -I SCRATCH/test_names.py /work/projects/fidryn-lanes/model fd88364 47c9205
uv run --locked python -I SCRATCH/test_names.py /work/projects/fidryn-lanes/model e6fef77 47c9205
```

The script lists every `fn` that follows a `#[test]` or `#[tokio::test]`
attribute, by its file and name, and compares the lines of each function on
both sides. `fd88364` is the commit the stage began from.

- `1a70382` has 1,186 test functions. With the two doc tests of
  `crates/fidryn-core/src/outcome.rs`, they are the design's 1,188.
  `47c9205` has 1,830, one of which, a form of
  `the_cache_directory_is_chosen_in_the_stated_order` for Windows, is not
  compiled here; the other 1,829 and the same two doc tests are the 1,831 the
  gate ran.
- Of the 1,467 test functions of `fd88364`, every one but one stands at
  `47c9205` in its file under its name. The one is
  `numbered_guides_run_one_to_eight_in_order` of `xtask/src/site/guides.rs`,
  which is now `numbered_guides_run_one_to_ten_in_order`, since the site has
  two guides more.
- Nine of the others have lines this stage changed, all in `xtask/src/site/`:
  `by_file_finds_published_guides_only`,
  `doc_page_has_kicker_title_links_and_body`,
  `guides_nav_lists_the_groups_in_order_then_implementers`,
  `landing_page_has_hero_specimen_and_contents`,
  `pager_links_the_neighbors_in_reading_order`,
  `page_then_sections_in_order_with_numbers`,
  `real_guides_index_every_section_with_short_snippets`,
  `sitemap_is_a_complete_urlset_without_dates` and
  `sitemap_lists_the_landing_page_then_every_guide_in_order`. Read in the
  diff, each change is to the numbers, the lists or the count of the site's
  guides, for the two new guides Law modules and Coverage, or adds an
  assertion about one of them; none removes an assertion about anything
  else. The corrections changed none of these ten.
- Between `1a70382` and `fd88364`, Stage 0 changed four more:
  `keywords_are_the_grammars_lowercase_quoted_terminals` and
  `sniff_recognizes_unlabeled_blocks` of `highlight.rs`,
  `commands_are_the_equivalent_cli_lines` and
  `frames_show_the_numbered_source_and_the_evaluated_case` of `specimen.rs`.
- Every other test that existed when the stage began is unchanged, and all of
  them pass.
- Every test function of `e6fef77` stands at `47c9205`; the corrections
  changed eleven tests the stage had written, to hold what they corrected
  (`SCRATCH/fix1-test-names-e6fef77.txt`).

**Verdict: met with a written difference.** Every test passes and the Stage
0 checks pass. Ten tests that existed when the stage began were changed by
it, all tests of the site's list of guides, because the stage added two
guides; one of them was renamed for that reason.

## Item 11: the reference and the ledger on the site

> 11. The language reference documents workspaces, provisions, citations,
>     definitions and bindings, and the site shows the coverage ledger.

**The reference** is `docs/law-modules.md`, to which `docs/language.md`
points ("Editions"). Its sections, at `47c9205`:

| Subject | Section of `docs/law-modules.md` |
| --- | --- |
| provisions | "Provisions, notes and layers", "Syntax", "Hashes" |
| workspaces | "Workspaces", "How a workspace is checked", "The cache", "Depending on another workspace" |
| citations | "References", "Curated modules: imports, citations and names" |
| definitions | "Definitions" |
| bindings | "Bindings", "Change sets" |
| the ledger | "The coverage ledger" |
| what it does not do | "Limits" |

It is on the site as the guide "Law modules", number 4 in the group "Write",
`site/docs/law-modules.html`. The guide "Coverage", `docs/coverage.md`, is
`site/docs/coverage.html`.

**What holds them** (all in the gate of item 10):

- the documentation check (`cargo xtask docs`) reads every block of the
  documents that begins `edition 2` as a whole module and checks it, and runs
  each `fidryn` command of their blocks tagged `sh`;
- the site check (`cargo xtask site --check`) holds `site/` to what the
  guides give: "site/ is up to date (30 generated files)";
- `site::guides::tests::by_file_finds_published_guides_only` holds that the
  reference is published as guide 4 of "Write";
- step 8 of the sample check holds the table of `docs/coverage.md` to the
  ledger of the sample, and
  `uscode_sample::tests::the_table_of_the_page_is_what_the_ledger_gives`
  holds that step. `cargo test -p xtask --bin xtask --locked --offline --
  site::guides uscode_sample` runs them with the other tests of those two
  modules, through `lease` on this machine.

**Verdict: met with a written difference.** The site shows the ledger of the
sample of the Code committed in this repository (three titles), not that of
the whole Code. That is the default of decision O-2, whether the public site
shows figures of the whole Code while the law repository is local and
unnamed; the page says in its first sentence that its figures are the
sample's. The ledger of the whole Code is `uscode/coverage.json` of the law
repository, and its counts are those of item 2. It stands after the
corrections, which changed sections of the reference and regenerated the
site; the site check and the documentation check passed on the result.

## Item 12: a second workspace at a pinned release

> 12. A second workspace that declares a dependency on the generated one at
>     a pinned release can cite its provisions and checks cleanly. Pointing
>     it at a different release produces a diagnostic, not a silent pass.

**The tests**, by name, all of which passed in the gate of item 10:

- `workspace::tests::a_curated_workspace_cites_its_dependency_and_checks_clean`
  (`crates/fidryn-driver/src/workspace/tests.rs`): a curated workspace pinned
  to the release and the lock digest of an invented generated one cites a
  provision of it; the check has no diagnostic, counts one citation `exact`,
  and prints `modules: 1; from cache: 0; errors: 0; warnings: 0`;
- `workspace::tests::a_different_release_is_E740_and_names_both`: pinned to
  another release, to another lock digest, or to both, the check is `E740`,
  naming the pin and what the dependency's lock holds, with one error, and
  the citation into the dependency is not counted;
- on the sample of the Code (`xtask/src/uscode_sample.rs`):
  `uscode_sample::tests::the_curated_sample_checks_against_the_generated_one`
  holds that the curated sample checks against the generated one, and
  `uscode_sample::tests::a_pin_that_is_not_the_lock_fails_the_step_and_names_the_curated_manifest`
  that a pin to another lock digest fails. That test changes the lock digest
  only, so a pin to a different release is shown on invented workspaces
  only, by the second test above.

These commands run them alone, through `lease` on this machine:

```sh
cargo test -p fidryn-driver --locked --offline -- a_curated_workspace_cites_its_dependency_and_checks_clean a_different_release_is_E740_and_names_both
cargo test -p xtask --bin xtask --locked --offline -- the_curated_sample_checks_against_the_generated_one a_pin_that_is_not_the_lock_fails_the_step
```

**On the Code:**

- the sample step of the gate checks `samples/uscode/curated`, pinned to the
  sample's workspace: "checks against it with no error and no warning, each
  of its 8 citations `exact`";
- in the law repository, `encodings/workspace.json` declares the dependency
  `uscode` at `../uscode`, pinned to the release `119-142` and the lock
  digest `83918ea6e9e0a63195f096fdb05b60dbd2574f43e4467d8ad2143ae1424389dd`,
  and checks with no error (item 6; its two warnings are the stale bindings
  item 6 asks for).

**Verdict: met.** It stands after the corrections: the tests pass in the gate
of the corrected branch, and FINAL checks `encodings/` as BINARY did.

## Item 13: the integration test

> 13. The integration test of section 4.3 runs. While the law repository
>     is local, it runs on this machine against the full workspace and in
>     CI against the in-repository sample. The cross-repository CI job is
>     written and switched on when the law repository is published.

Section 4.3: "The law repository records the version of the toolchain it is
checked with."

The item has two halves: what runs on this machine, and what runs on GitHub.
Each has a verdict of its own.

### On this machine

**The script on the law repository.** `tools/check-law-repository.sh`, as
`docs/development.md` ("Checking a law repository") describes it, with its
default limits, the binary named by `FIDRYN_BINARY`, and a cache under
`/cache/fidryn/`. The binary is FINAL, built through the scheduler as above,
since a build by the script through `lease` from inside a job would queue a
job from within one. Its run of 2026-11-18, with no cache directory yet:

```sh
cd /work/projects/fidryn-lanes/model && lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v -o /cache/fidryn/task-23b/fix1-script.time env FIDRYN_BINARY=/cache/fidryn/task-23b/fidryn-final tools/check-law-repository.sh --cache-dir /cache/fidryn/task-23b/cache-final /work/projects/law-local
```

Its whole output, stdout:

```text
limits of each run of the binary: 600 s of time, 900 s of processor time, 8589934592 bytes of address space
binary: /cache/fidryn/task-23b/fidryn-final, fidryn 0.3.0 (given by FIDRYN_BINARY)
cache: /cache/fidryn/task-23b/cache-final (from --cache-dir)

encodings: the manifest records the toolchain 0.3.0, which is the version of the binary
encodings: fidryn check /work/projects/law-local/encodings --cache-dir /cache/fidryn/task-23b/cache-final
modules: 1; from cache: 0; errors: 0; warnings: 2
ok
encodings: the check passed with warnings, which do not fail it

uscode: the manifest records the toolchain 0.3.0, which is the version of the binary
uscode: fidryn check /work/projects/law-local/uscode --cache-dir /cache/fidryn/task-23b/cache-final
modules: 70794; from cache: 70794; errors: 0; warnings: 0
ok
uscode: the check passed

checked 2 workspaces: every version is the binary's, and every check passed
```

and stderr:

```text
/work/projects/law-local/encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `chapter_13_debt_limits` of `/us/usc/t11/s109/e` is stale (provision text changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
/work/projects/law-local/encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `small_business_debtor` of `/us/usc/t11/s101/51D` is stale (enacted notes of the unit changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision and the enacted notes of its unit again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
```

- Exit 0, 4.63 s wall, 25.87 s user, 831,984 KiB peak.
- The two workspaces at the top of the law repository are `encodings/` and
  `uscode/`; `changes/` and `review/` hold no manifest. Each manifest records
  the toolchain `0.3.0`, the version of the binary.
- No limit was reached. The two warnings are the stale bindings of item 6,
  which do not fail a check.
- The first run of 2026-11-17 with BINARY, with an empty cache
  (`SCRATCH/script-1.out`, `.err`), printed the same, but for the binary's
  path and the cache's. The second, warm one (`SCRATCH/script-2.out`) printed
  the same as the first, but for `modules: 1; from cache: 1; errors: 0;
  warnings: 2` for `encodings`. The controller's run with the same build as
  FINAL (`/cache/fidryn/final/check-law.out`, `.err`) ended the same way.
- The script wrote nothing into the law repository (above).

**The gate with the sample step.** The job "cargo xtask ci" of
`.github/workflows/ci.yml` runs `cargo xtask ci --fmt`, whose sample step
checks the committed sample of the Code, its curated workspace and its text.
That command passed on this tree, on this machine (item 10).

**The tests of the script and of the workflow.**
`conformance/test_check_law_repository.py` holds the script, among others by
`test_a_manifest_of_another_version_fails_and_names_both_versions`,
`test_a_stale_binding_warns_and_does_not_fail`, and, with stand-ins for
`lease` and `cargo`, the two tests of the build, which the runs above did not
take. `conformance/test_law_integration_workflow.py` reads the workflow as
YAML and holds its trigger, its jobs and what each does, among others by
`test_its_only_trigger_is_a_manual_run`,
`test_the_two_lines_its_comment_names_switch_on_the_other_triggers` and
`test_the_last_step_of_each_job_fails_when_the_check_fails`; the scripts of
its steps were checked with `shellcheck`. Both files ran in the gate
(`Ran 231 tests`, `OK`), and

```sh
env FIDRYN_BINARY=/cache/fidryn/task-23b/fidryn-final uv run --locked python -m unittest discover -s conformance -p 'test_*law*.py'
```

runs them alone, through `lease` on this machine.

**Verdict: met.**

### On GitHub

What would run there:

- the job "cargo xtask ci", the gate with the sample step, on Ubuntu;
- the Rust tests on macOS and Windows, among them the code that finds the
  user's cache directory on Windows, which has never been compiled;
- the job "Workflow lint", which runs `actionlint` on the workflows,
  `law-integration.yml` among them. `actionlint` is not installed on this
  machine, so no lint has read that workflow;
- the workflow `Law integration` (`.github/workflows/law-integration.yml`),
  with the two jobs of section 4.3: "Law repository at its pin" checks the
  law repository at the commit `law-repository.pin.json` names, with the
  toolchain of the commit under test; "Drift of the newest law repository"
  checks the newest of each. Both run the script. Its only trigger is a
  manual run, and the comment at its top names the two lines that switch on
  the others. `law-repository.pin.json` is
  `{"repository": "", "commit": ""}`: while it names no repository, each job
  says so and ends.

None of it has run: this branch has not been pushed, and pushing is the
owner's step. The cross-repository job is written and is not switched on, as
the item says it should be while the law repository is not published
(decision O-6).

**Verdict: open.** What closes it: the owner pushes the branch, and the jobs
above pass, the lint of `law-integration.yml` among them. The workflow
`Law integration` itself waits for the law repository to be published and
named in `law-repository.pin.json`.

## What a review of the whole branch found, and what was corrected

This record was first written before the branch was reviewed as a whole. It
is set down here as it happened.

### What was reviewed, and how

After every task was done, five readers read the whole branch at `98d5765`,
side by side, each with one question:

- whether a word of the law can be lost, altered or invented between the
  publisher's XML and what the tools print;
- whether everything that reads a workspace agrees on what a workspace is;
- whether every sentence a reader would act on is true of the code;
- whether the first edition of the language is untouched and the branch can
  be released;
- whether a file the tools read can make them write or remove outside the
  directory they were given, run without bound, or accept what they should
  refuse.

The review of the task that made the replay came back at the same time.
The readers' reports are kept word for word with the scratch files of the
run (`.agents/scratchpad/2026-11-17-stage1/final/reports/A.md` to `E.md`),
and are not part of the repository.

### What they found

No fault that changes a result on any release of the Code or on the
committed sample. Eight faults in cases that no release holds. The first
seven were each shown on the built binary or on an invented release; the
eighth was found by reading the fixture.

1. **The cache.** A check trusted a cache that by default lay inside the
   directory under check, `.fidryn-cache/`. A workspace received from someone
   else, such as a clone with a committed `.fidryn-cache/`, could carry
   entries that made `fidryn check` pass a module whose text no longer
   matched its hashes, drop a module with no diagnostic, or hide a stale
   binding. Two readers found it independently, each by altering an entry:
   a check of a module whose words were changed then printed `ok` and exit
   0, and `fidryn changes` from the unchanged workspace to it printed a
   change set with every count 0. The module files of a dependency went
   through the same cache, so a curated workspace from someone else could
   make its bindings to the Code look fresh, whatever its pin said. A
   symbolic link named `.fidryn-cache` made the check write its entries
   outside the workspace.
2. **The import over a directory.** `fidryn ingest uscode --out DIR` removed
   `.fr` files it had not written, where the documents said such a directory
   is refused.
3. **The manifest and the lock** were read through symbolic links, named
   pipes and devices, with no bound on their size. With `workspace.json` a
   named pipe, the check waited until something was written into the pipe.
   With `workspace.json` a link to `/dev/zero`, the check took 4,198,936 KiB
   of memory before it reported that it could not read the file; without a
   limit of address space it reads until the machine runs out of memory. The
   same held for a dependency's manifest and lock, for the manifest an import
   examines, and for the retrieval record of an archive.
4. **A piped input.** The check for the edition marker read the start of an
   input before its command did, which emptied an input given through a
   pipe, `/dev/stdin` or a process substitution: a module given that way to
   `fmt`, `check` or `inputs`, and a trace or a packet given to `explain` or
   `file`, were read as empty. It was a regression against 0.2.0 for anyone
   who pipes a module, a trace or a packet into `fidryn`, as an editor
   integration does.
5. **Topics and origins.** A `topic` attribute that was empty, that held two
   spaces or that held a no-break space, and an empty `origin` of quoted
   content, made the importer write a module its own check refused
   (`E710`), and the two readings read them differently.
6. **A reference that is both a link and a footnote marker** was read as a
   link by the importer, which put the marker's digit into the operative
   text, and as a marker by the independent reading.
7. **Like-named notes.** In a change set, a note inserted before others of
   the same group, law and heading made the unchanged ones show as amended,
   against a sentence of the reference.
8. **A fixture.** A fixture that said it held the text of no law held an
   abridged sentence of 1 U.S.C. 1; two readers found it. It was a breach of
   the rule that text of the Code enters the repository only under
   `samples/uscode/`.

The review of the replay's task found four places where the run's record
quoted the Code; they were reworded in `71377f7`, and the readers' smaller
points were triaged into corrections and recorded limits.

### What was done

Three implementers corrected them on three branches, each correction was
reviewed again, and the three were joined on the stage branch: nineteen
commits, `e6fef77..b6d8b49`, every one signed, then `47c9205`, which amends
the plan to what changed. The reports of the implementers and of the
reviews of their work are with the scratch files of the run
(`fix-wave-X-report.md`, `fix-wave-Y-report.md`, `fix-wave-Z-report.md`).

By what they correct
(`git log --reverse --format='%h %s' e6fef77..b6d8b49`):

| Fault | Commits | The rule that now holds | Where the reference says it |
| --- | --- | --- | --- |
| 1 | `2997df9` Keep the cache of a check outside the workspace it checks | The default cache is `fidryn` in the user's cache directory, outside every workspace: `XDG_CACHE_HOME`, else `.cache` in `HOME`, on Windows `LOCALAPPDATA`, each only as an absolute path; when none names one, the check keeps no cache and says so once. Unless the cache is named inside the workspace, a check reads no entry of it whose name begins with a dot and follows no symbolic link in it; a cache named inside it is used like any other. | `docs/law-modules.md`, "The cache"; `docs/cli.md`, `check` |
| 2 | `fa7a0f3` Refuse to import over a directory that holds a file the importer did not write | `ingest uscode --out DIR` refuses a directory that holds a `.fr` file no earlier run could have written (no module of edition 2, of another prefix, or at another path, judged by its head), names every such file and changes nothing; it removes only what it takes for modules of an earlier run. It judges a file by its head alone, so a module that a person changed below its head counts as an earlier output, which a run writes again or removes ("Known limits", 5). | "Writing a generated workspace" (`create`, `finish`); `docs/cli.md`, `ingest` |
| 3 | `ffe326e` Read the manifest and the lock of a workspace only as regular files of bounded size; `9d580da` Read the retrieval record only as a regular file of bounded size | A manifest, a lock and an entry of the cache are read only from a regular file of at most 64 MiB, the retrieval record from one of at most 1 MiB; anything else in their place is refused before it is opened (an entry of the cache is a miss). | "What the checker verifies in a workspace", "What it reads", "Limits" |
| 4 | `dcf83fe` Leave an input that is no regular file for its command to read | The command line looks for the edition marker in a regular file only; any other input, such as a pipe, is read by its command as a file of the first edition, so a module of edition 2 is read from a regular file. | "Editions"; `docs/cli.md`, `check` |
| 5 | `6c2bf09` Read an empty topic as none and an empty origin as none, and refuse a topic with other white space | A `topic` attribute is split at U+0020 and empty members are dropped; a member that holds another white-space character is refused, naming the note; an empty `origin` is none. Both readings follow the rule. | "Notes and their layers", "What it reads", "The classes of elements" |
| 6 | `32df3db` Refuse a reference that is both a link and a footnote marker | A `ref` with both an `href` and the class `footnoteRef` is refused, by both readings. | "What it reads" |
| 7 | `48bef5b` Pair like-named notes by their text before comparing them by address; `b6d8b49` Say what a change set lists for a like-named note that is amended and moved | Within one module, one layer and one family of like-named notes, notes of equal hash are paired first, in the order of their addresses, a pair at two addresses being `moved`; the rest are compared by address. `usc_compare.py` pairs the same way. | "Moved provisions and notes", "Limits" |
| 8 | `e8660af` Replace a sentence of the Code in a fixture with invented words | The fixture holds invented words of the same shape; the plan's table was amended in `47c9205`. | (a fixture of `conformance/fixtures/uslm/`) |
| readers' smaller points | `21e1daf` Refuse a document nested or grown beyond what the importer reads; `aed288a` Correct small points of the workspace check and its help; `5e46e51` Correct small points of the importer's documents and the independent reading; `84dedeb` Correct sentences of the changelog, the README and the reference | The importer refuses an element more than 64 deep in its module, a container more than 64 deep in its file, and a document of more than 8,388,608 nodes, and the independent reading refuses the same nesting. `fidryn changes` holds each module file it reads again to the BLAKE3 its check accepted, and ends with `E730` naming a file that changed after the check read it. `conformance/uslm_text.py` and `usc_profile.py` refuse an archive member whose header declares fewer bytes than its compressed stream holds. The other points are of wording and of help. | "What it reads", "Change sets", "Limits"; `conformance/README.md` |
| the tools | `6b5910d` Close the edges of the replay tool and test it against a repository; `9bf8fe6` Keep the replay's test out of the caller's git configuration and remove a leftover partial file; `6796ee3` Hold a sample of definitions to the ledger and the records it is drawn from; `8c98aba` Follow a redirect only to the same host and bound the size of a fetched archive; `1dda1ea` Keep no token in the law repository's checkout and say why the drift job failed | The replay tool holds its repository, `uscode.next/` and its temporary files to stated conditions and is tested against a real repository; the drawing tool holds the definitions to the ledger; the fetching tool follows a redirect only to the same host over `https` and takes at most 512 MiB in one response; the workflow keeps no token in its checkouts. | `conformance/README.md`; "Limits", "Fetching"; `docs/development.md` |

What a user of the tools sees change:

- the default place of the cache, which is a directory of the user and no
  longer `.fidryn-cache/` in the workspace. This changes a default that the
  plan's specification had set (S13); the plan was amended in `47c9205`;
- what an import refuses: a directory with a `.fr` file it did not write,
  and the new refusals of the importer (a topic with other white space, a
  `ref` that is both a link and a marker, a document nested or grown beyond
  the bounds, a retrieval record that is no regular file or is larger than
  1 MiB);
- that a module of edition 2 is read from a regular file only;
- what a change set lists for like-named notes;
- that `fidryn changes` ends with `E730` when a module file changed after
  its check read it;
- that a check refuses a manifest or a lock that is no regular file or is
  larger than 64 MiB;
- that `conformance/uslm_text.py` and `usc_profile.py` refuse an archive
  member whose header understates its size;
- that the fetching tool refuses a redirect to another host and a response
  larger than its bound.

### What was checked again after the corrections

By the controller of the run, with the release build of `b6d8b49`,
`/cache/fidryn/final/fidryn` (SHA-256
`ec39b6b99f44d200b003aaeb7f5d1180a8c340c22d9838be34a499331b4cb0fa`,
`fidryn 0.3.0`, the bytes of FINAL):

- **The gate** on `b6d8b49`, in a worktree of its own: exit 0; 1,831 Rust
  tests passed, none failed; `Ran 231 tests`, `OK`; 52 JavaScript tests
  passed; no line beginning with `warning`
  (`work/gates/gate-stage-b6d8b49.log` with the scratch files). The gate of
  item 10 is the one run on the commit of this record.
- **Every release replayed again.** `tools/replay_uscode.py` of `b6d8b49`,
  with that binary, from the 27 archives under `/library/corpora/uscode`,
  into a new empty repository, `/cache/fidryn/controller-replay-final/law`:
  exit 0, "the replay is complete: 27 releases are committed", 7 min
  39.74 s wall, 2,297,852 KiB peak, 27 commits, each signed (`G`).
  The tree of each of its 27 commits is the tree of the same release's
  commit on `main` of `/work/projects/law-local`: the two lists of 27 tree
  hashes, `/cache/fidryn/controller-replay-final/trees-mine.txt` and
  `trees-law.txt`, are the same bytes (`cmp` exits 0). For this record the
  second list was made again from the law repository,
  `git -C /work/projects/law-local --no-optional-locks log --format=%T main | sed -n '3,29p'`,
  and `cmp` finds it the same bytes too
  (`SCRATCH/fix1-trees-law.txt`). A tree holds the workspace of its release
  and every change set up to it. So the corrected importer writes the same
  modules, lock and ledger for every release of 2026, and the corrected
  comparison the same 26 change sets, byte for byte, as the build that made
  the law repository. The law repository was therefore not made again, and
  its commits stand.
- **The text of the two readings on every release**, with the corrected
  code: 27 of 27 equal, as item 4 says.
- **The profile of 119-142, counted again** with `conformance/usc_profile.py`
  of `b6d8b49`: byte for byte the profile of the full run, as item 2 says.
- **Every change set of the law repository, held to the corrected
  independent comparison**, `conformance/usc_compare.py --against` of
  `b6d8b49`: 26 of 26 exited 0, as item 5 says.
- **Tables of contents inside another**, the one form on which the two
  readings differ only in a count and no text (see "Known limits", 3): for
  this record, a scratch script counted, in every member of each of the 27
  archives, the tables of contents of the publisher's namespace that stand
  inside another,

  ```sh
  lease run --cpus 8 --mem 16G --time 2h -- /usr/bin/time -v uv run --locked python -I SCRATCH/nested_toc.py /library/corpora/uscode --jobs 8
  ```

  It exited 0 in 50.97 s and found none in any release: 58 members each,
  from 3,266 tables of contents in 119-73not60 to 3,303 in 119-142, 0 inside
  another (`SCRATCH/closing-nested-toc.tsv`). For 119-142, 3,303 is the
  count of the ledger and of the profile.
- **The script on the law repository**, with the same build, inside one
  `lease` job, `--cache-dir /cache/fidryn/final/check-cache`: exit 0, two
  workspaces, each recording the toolchain 0.3.0; `uscode` with no error and
  no warning, `encodings` with the two stale bindings (`W750`)
  (`/cache/fidryn/final/check-law.out` and `.err`). Item 13 gives the run
  made for this record.
- **No cache in the user's directory.** `~/.cache/fidryn` did not exist
  before the controller's gate and did not exist after it or after any of
  these runs: no test and no documented command writes there. The same held
  for every run of this record.

### What it means for the verdicts

Each item says so under its verdict, in short:

- Items 1, 3, 6 and 9 rest on checks that FINAL made again on the law
  repository today, with the same outputs.
- Items 2, 5 and 8 rest on the workspaces, the ledgers and the change sets
  the build of the run wrote. The corrected build writes the same bytes for
  every release, so they stand. The other side of items 2 and 5 was made
  again with the corrected scripts: the profile of 119-142 is the same bytes,
  and the independent comparison agrees with all 26 change sets.
- Item 4 stands with wider evidence: the two readings were compared on every
  release of the year, before and after the corrections.
- Item 7 is open, as it was. The corrected drawing tool draws the same
  samples.
- Item 10 is decided by the gate on the tree of this commit.
- Items 11 and 12 stand on the gate of the corrected branch.
- Item 13 is split; the half on this machine was run again with FINAL.

## Known limits

Each of these is true at `47c9205`, and so at the commit of this record,
which changes no code. Most are also stated, without counts of the whole
Code, in "Limits" of `docs/law-modules.md`. The notices that
`fidryn changes` loses are stated under `changes` in `docs/cli.md`. Stated
here only are: a module that begins with a comment of certain lengths; a
`HOME` or `XDG_CACHE_HOME` inside a workspace; and the body of a redirect.

1. **Change sets.**
   - The credit and the footnotes of a unit that is on one side only, or that
     moved, are in no list.
   - Where a footnote marker stands inside one provision is in no hash, and
     so in no list.
   - A superscript that the source does not mark as a footnote marker is
     text. A provision that gained only the superscript number of a footnote
     is listed as amended in its text, and a binding to it goes stale. Three
     provisions of the pair 119-138 to 119-142 are listed for that reason
     alone: `/us/usc/t26/s527/j/1`, `/us/usc/t26/s6049/b/3` and
     `/us/usc/t26/s6049/d/8/A`. For each, `BINARY text DIR ADDRESS --json` on
     both releases (six runs in one job, each under the limits above, 23.10 s,
     1,414,920 KiB), and a comparison of the two records by the scratch
     scripts `record_diff.py` and `superscript_only.py`, show the same thing.
     The words of one paragraph of its own record differ, and they are the
     same once the superscript digits the newer release added are taken out
     and a run of spaces is read as one (a space came with two of the four
     digits). Nothing else of the record differs, but in
     `/us/usc/t26/s6049/d/8/A`: there 119-138 had two footnote markers, and
     119-142 has none and the two digits in their places. The cost is a false
     alarm, never a missed change.
   - A change of a unit's aliases, of the container it stands in, or of a
     file's positive-law flag is in no list: a title enacted as positive law
     shows nothing from the flag alone.
   - `usc_compare.py --against` checks the unions of the lists. It cannot see
     a provision in the wrong one of two lists, or a wrong pairing of a move;
     the Rust tests hold those list by list.
   - Like-named notes of equal text are paired in the order of the bytes of
     their addresses, so `…/10` comes before `…/2`: in a family of ten or
     more, a note may be listed as moved where pairing by the order of the
     ordinals would leave it in place.
   - A like-named note that is amended while another is inserted before it is
     listed as amended at its old address and added at its new one, or as
     removed there and added; a change set cannot tell that the note at the
     new address is the old one amended. Units and containers that `~2` and
     `~3` mark are compared by address, not paired this way.
   - When the comparison fails, `fidryn changes` prints only the notices it
     knew before reading the workspaces; what the checks it ran found of
     their cache is lost.
2. **Definitions.**
   - The rules do not find every definition; the share they miss is not
     measured until the samples of item 7 are read.
   - `unstated` is given when neither the paragraph nor a chapeau above it
     states a reach; a reach stated in a sibling provision, such as a
     subsection (a) that states the reach of the definitions of a subsection
     (b), is not seen. Two lines of the first sample are such cases (item 7).
   - An exception written inside a definition's own text does not narrow its
     region. About 25 of the 34,329 definitions of release 119-142 hold the
     words of such an exception in their first 300 characters, by the count
     of the review of Task 15; this record did not count them again.
3. **The two readings of the publisher's XML** read nine forms differently.
   The reference now lists them too ("Limits", "The ingester and the
   independent reading"). Each was taken from the code of both readings at
   `47c9205`. They fall in two groups, and only the first is loud.
   - **One of the two refuses the document**, so a release that holds the
     form is refused by that reading:
     - a `continuation` or a `proviso` inside a `content` that stands
       directly in a note: read by the importer, refused by the independent
       reading;
     - a note inside a paragraph or a text part of quoted content: refused by
       the importer, read by the independent reading;
     - white space inside a `br`, an `img` or a column: refused by the
       importer as content, taken for none by the independent reading;
     - an identifier of a unit or a container whose list has an empty
       member, as two spaces or a space at either end make, and an
       identifier of a provision that begins with a space: refused by the
       importer as no address, read by the independent reading without the
       empty member. (Of a provision's identifier the importer reads the
       first member only, so an empty member after it is no difference.)
     - a `class` attribute of a `ref` whose words are separated by
       white space that is not ASCII: the importer splits it at ASCII white
       space only and the independent reading at any, so one finds the class
       `footnoteRef` where the other does not, and one of them refuses the
       `ref`, as having neither an `href` nor that class or as having both.
   - **Both read the document, and read it differently.** Neither refuses
     it, and nothing warns:
     - a `num` or a `heading` among the words of a level of quoted content:
       the independent reading ends the open paragraph there, the importer
       does not;
     - a footnote inside a footnote: the importer counts it as a footnote of
       its own, the independent reading leaves it out;
     - a table of contents inside another: the importer counts it twice
       among the tables of contents of the lock, the independent reading
       once; no text differs;
     - a footnote inside the release note of a title or an appendix: the
       importer keeps it as a footnote of the container, the independent
       reading leaves it out with the note.

   No release of 2026 holds any of the nine. The importer imported all 27
   releases with no refusal, the independent reading read all 27 with none,
   and the text of the two was the same on all 27 (item 4); and the scratch
   count of the section on the corrections found no table of contents inside
   another in any of them.

   What follows for whoever keeps the law repository: a new release that
   holds a form of the second group is imported, checked and replayed
   without a word, and the two readings differ on it. The import and the
   check of a new release do not show such a difference. A comparison of the
   two readings on that release does: `conformance/uslm_text.py` on its
   archives against `fidryn text DIR --json` of its workspace, and for a
   table of contents the counts of `conformance/usc_profile.py` against the
   ledger. This run made the comparison of the text for all 27 releases;
   the replay tool does not make it.

   The independent reading refuses the nesting the importer refuses, but has
   no bound on the number of nodes of a document, so it can run out of
   memory on a document the importer refuses for its size (about 3 GB for a
   member of 64 MiB of empty elements).
4. **Diagnostics.** `fidryn check` prints no line or column for a diagnostic
   of a module of edition 2. `E740` carries the title `PinMismatch` also when
   a dependency fails for another reason than its pin.
5. **Files and the cache.**
   - A check trusts its cache: an entry written by hand under the key of a
     module's bytes is taken as that module's result. The default cache is a
     directory of the user, outside every workspace, unless the user's own
     `HOME` or `XDG_CACHE_HOME` points into a workspace; then it lies inside
     that one.
   - An import judges an existing module file by its head alone. A module
     that a person changed below its head is therefore an earlier output,
     which a run writes again or removes. A module that begins with a
     comment of certain lengths is refused as no module of edition 2, and so
     is one whose head lies past its first 64 KiB; there it errs toward
     refusing: nothing is removed, and no writer writes such a file.
   - A module file name such as `s1932~2.fr` has the shape of a Windows 8.3
     short name, and on a volume that gives files short names it could be
     the short name of another file. A check that is killed while it writes
     leaves a temporary file in the cache, which nothing reads and nothing
     removes.
6. **Fetching.** `tools/fetch_uscode.py` follows a redirect only to the host
   it asked, over `https`, and records the address it asked for, not the one
   a redirect led to. A redirect that it follows has its own body read whole
   before the bound on a fetched archive, 512 MiB, applies to the final
   response.

The run's record, "What is open after the second part", adds the population
of near misses, which is not that of F14. Its other point, a stop of the
replay between the replacement of `uscode/` and its commit, is now held by
the tests of the replay tool against a real repository
(`conformance/test_replay_repository.py`).

## Differences from the design's numbers

Every "written difference" above, and the open half of item 13, in one table.

| Item | The design | What was found | Why |
| --- | --- | --- | --- |
| 2 | 60,490 sections | 61,171 units, of which 60,509 are sections | the import counts court rules (544), reorganization plans (107), generic levels that are units (11) and sections without an identifier (20) as units, and the section inside 25 U.S.C. 5329 as a provision of it (−1) |
| 2 | the status breakdown (in force, repealed, omitted, transferred, renumbered, vacant, reserved, unknown) 51,009 / 5,131 / 2,037 / 1,911 / 368 / 17 / 16 / 1 | 51,664 / 5,143 / 2,043 / 1,914 / 369 / 17 / 18 / 3 | the same units, by status (the run's record, "Where each difference comes from") |
| 2 | 772,863 provisions | 774,018 | +1,155: the top provisions of those 682 units, and 473 provisions of kind `level` |
| 2 | 88,935 references resolve exactly | 89,269 | +334: 7 references to a target of kind `level`, 4 `ambiguous`, and 323 to a section whose identifier lists several sections or names a range |
| 2 | 820 reach the section only | 881 | +61 = 72 below such sections, less those 7 and 4 |
| 2 | 964 reach no section | 569 | −395 = −323 − 72 |
| 8 | each commit with its change set as its message | 26 of the 27; the first is `/us/usc: release 119-73not60` | no release before it among the 27 |
| 10 | the existing 1,188 tests pass | all pass; of those that existed when the stage began, 10 were changed, one of them also renamed | the stage added two guides to the site, and these tests count and list its guides |
| 11 | the site shows the coverage ledger | the site shows the ledger of the sample | the default of decision O-2 |
| 13 | it runs in CI against the sample | run here with the command CI runs; nothing has run on GitHub, and `actionlint` has not read the new workflow (open) | the branch is not pushed, which is the owner's step; `actionlint` is not installed here |

For item 2, the design's own numbers are reproduced exactly by `baseline`;
the rule for each difference is in `conformance/usc-counting-rules.md`, "Where
the two differ".

## What only the owner can do

1. **Read the two samples of definitions, or name someone who will**
   (decision O-3). The reading decides item 7: it is met only if the first
   sample has no error, and otherwise the patterns are corrected and a new
   sample is drawn. Until then it is open, and no claim about the
   correctness of the definitions is made beyond their agreement with a
   second implementation of the same rules.
2. **Push the branch.** Nothing of it has run on GitHub. The first push runs
   the gate on Ubuntu, the Rust tests on macOS and Windows (the code that
   finds the user's cache directory on Windows has never been compiled), and
   the lint of the new workflow, which could not be run here. If those jobs
   pass, the half of item 13 on GitHub is closed.
3. **Decide whether the stage is merged into `dev`.**
4. **Decide the name, the visibility and the license of the law repository**
   (decision O-6). Until then it stays local at `/work/projects/law-local`,
   the workflow `Law integration` does nothing, and a law repository that is
   not public will need a token of its own in that workflow.
5. **Decide whether the second implementation of the definition rules**,
   with which the importer's 34,329 definitions were found equal, becomes
   part of the repository as a third independent check. It was written to
   test the rules and is kept with the scratch files of the run. While it is
   outside, that comparison cannot be made again from the repository alone.
6. **Weigh a hazard older than this stage**, found while it was corrected.
   The Rust tests of `xtask` in `release.rs` and `package.rs` start git with
   the environment they inherit. If the gate were ever run from inside a git
   hook, where `GIT_DIR` is set, the release fixtures would commit and tag in
   the repository the gate was started from, and could delete a tag of it.
   Nothing triggers it today. The correction is to take git's variables of
   place and configuration out of the environment of those commands, as the
   test of the replay tool now does.
7. **Decide about the plan's header.** The plan of this stage, like the plan
   of the stage before it, names in its header the tooling it was written
   for. It is no reader document; whether it should say so in a public
   repository is his to decide.
8. **Decide how the branch's history enters the public repository.** A
   sentence taken from the Code is gone from every file of the branch, but it
   is still in the branch's history: the abridged sentence of 1 U.S.C. 1 in
   the fixture before `e8660af` and in the plan before `47c9205`, and the
   four quotations in the record of the run before `71377f7`. If the branch
   is pushed with its history, those words stand in the history of the
   public repository outside `samples/uscode/`. They are text of a work of
   the United States Government, and no license is at stake; the rule they
   break is the repository's own, that the text of the Code has one place.
   Whether the branch is merged with its history or as one commit is his to
   decide, and this is one thing that depends on it.
