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
- What was run again for this record is cheap and shows the state today: the
  release build, `fidryn check` of the law repository's workspaces, `git log`
  of the law repository, the script of item 13, the gate, the release check
  and the license check. These runs were made on 2026-11-17 between 23:06 and
  23:22, Eastern time, on the same workstation, through its scheduler.

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
| 13 | the integration test of section 4.3 | met with a written difference |

## Where things are

**This repository.** The record is added by the commit whose parent is
`71377f79e1b89afce3b954e5fa3b3cdc5fba297d` on the branch `lane/docs`. The
gate ran on the tree of that commit without this file; no step of the gate
reads `.agents/`.

**The binary.** It was built again from
`98d5765a9391a69da231d0a20f68b19aed662094`, on a clean tree:

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-model lease run --cpus 16 --mem 25G --time 1h -- /usr/bin/time -v cargo build --release -p fidryn-cli --locked --offline
```

- Exit 0, 26.15 s wall, 1,046,764 KiB peak.
- 13,837,000 bytes; SHA-256
  `f2dd81dde0ade77b922bfb1a37155ef26f803377fc6d0548f4eb8bfe681a6ef3`, BLAKE3
  `1588aa901707193fda1858482da57f874c76152eff36cd5af10564f1c1412427`
  (`sha256sum`, and `blake3` of the locked Python environment).
- These are the digests of the binary of the run's record, built from
  `d0658b6`, the one that made the import, the checks and the replay
  (`sha256sum /cache/fidryn/task-21/fidryn` prints the same).
- The commits after `98d5765` change no file under `crates/` or `xtask/`, nor
  `Cargo.toml` or `Cargo.lock` (`git diff --stat 98d5765 HEAD -- crates xtask
  Cargo.toml Cargo.lock rust-toolchain.toml` prints nothing).
- It was copied to `/cache/fidryn/task-23b/fidryn`. "BINARY" below is that
  file in the commands of this record, and the binary of the run in the
  commands copied from the run's record: the same bytes. `BINARY --version`
  prints `fidryn 0.3.0`.

Each run of BINARY for this record ran inside a job of the scheduler, one job
at a time, under these limits:

```sh
lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY ...
```

One job held six runs one after the other, each under its own limits ("Known
limits", 1). The script of item 13 ran inside a job of the same size and gave
each run of the binary the limits it sets itself.

**The law repository** is `/work/projects/law-local/`, local and with no
remote (decision O-6). It was only read. Today:

- `main` is at `c9bd0e44c865aaee3b52e086ddde66ca8808fab2` and holds 29 commits
  from the root `b8ec1e448a9ccd6336caf4a0e7a60477676393c9`: the 27 releases,
  then `encodings/`, then `review/`.
- `first-import` is at `218b87fb28eccc751436e5eb96d79c1cc1689d2c`, the first
  import of 119-142.
- `git --no-optional-locks status --short` prints nothing.

Its git commands were run with `--no-optional-locks`, so that not even git's
index was written. A file `MARKER` was made just before the first run of
BINARY on the law repository, and after the last one

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
this repository, which git ignores. "SCRATCH" below is that directory. The
caches and large outputs are under `/cache/fidryn/task-23b/`.

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

**Today**, on `uscode/` of `main` at `c9bd0e4`, with the cache the script of
item 13 had filled:

```sh
cd /work/projects/law-local && lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v -o /cache/fidryn/task-23b/check-uscode.time timeout 1800 prlimit --as=17179869184 --cpu=3600 -- BINARY check /work/projects/law-local/uscode --jobs 16 --cache-dir /cache/fidryn/task-23b/cache --report /cache/fidryn/task-23b/report-uscode.json --unresolved /cache/fidryn/task-23b/unresolved-uscode.jsonl
```

```text
modules: 70794; from cache: 70794; errors: 0; warnings: 0
ok
```

- Exit 0, 1.66 s wall, 831,296 KiB peak; nothing on stderr.
- `diff /cache/fidryn/task-21/report-cold.json /cache/fidryn/task-23b/report-uscode.json`
  prints only `"from_cache": 0` against `"from_cache": 70794`.
- `cmp /cache/fidryn/task-21/unresolved-cold.jsonl /cache/fidryn/task-23b/unresolved-uscode.jsonl`
  exits 0.

The workspace in the law repository today is the one the import wrote, and it
checks as it did then.

**Verdict: met.**

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
  target below `/us/usc` that names nothing the workspace holds. The run's record lists one example
  of it for each layer, and one of each other outcome but `exact` and
  `outside`, for each layer; `outside` is given by the body of law it names.
- `malformed` never occurs, so it has no example.
- Today's unresolved list is the same bytes as that of the import (item 1).

**Verdict: met.**

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

**Verdict: met.** The two readings read two forms of the XML differently; no
release of 2026 holds either form ("Known limits", 3).

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
  confirmed independently. Which list of a union an address is in, and the
  pairing of a move, are held by the tests of `fidryn-driver` only ("Known
  limits", 1).
- F10 counts 9 units with a change in operative text and 14 with only an
  editorial one; the change set gives 10 and 13. The tenth is 26 U.S.C. 45Y,
  where a link was taken off words that stayed, which S15 counts as a change
  of the provision's own words; the independent comparison agrees with the
  change set (the run's record, "The count of each list").

**Verdict: met.**

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
--hashes` printed for it in 119-138.

**At the replay** (the run's record, "Item 6: what the check and the change
set say"): `fidryn check encodings` warned `W750` for the first two and not
the third, and `fidryn changes /work/projects/law-local-pair/119-138 uscode
--bindings encodings` listed the same two in `stale_bindings`.

**Today**, the script of item 13 checked `encodings/` of `main` at `c9bd0e4`
(its whole output is under item 13). On stderr:

```text
/work/projects/law-local/encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `chapter_13_debt_limits` of `/us/usc/t11/s109/e` is stale (provision text changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
/work/projects/law-local/encodings/local/encodings/bindings.fr: W750 StaleBinding: the binding `small_business_debtor` of `/us/usc/t11/s101/51D` is stale (enacted notes of the unit changed) in the release `119-142` of the dependency `uscode`, and was written against the release `119-138`; read the provision and the enacted notes of its unit again, and write into the binding the hashes that `fidryn text` prints for it with `--hashes`
```

and on stdout `modules: 1; from cache: 0; errors: 0; warnings: 2`, then `ok`.
The same check run alone, by the command of item 1 with `encodings` in place
of `uscode` (1.32 s, 577,396 KiB), printed the same two lines and
`modules: 1; from cache: 1; errors: 0; warnings: 2`, then `ok`. Its
report differs from that of the replay's check
(`/cache/fidryn/task-22/encodings-report.json`) only in `from_cache`.

The three cases are also held on invented text by the tests
`workspace::tests::staleness::a_binding_to_a_changed_provision_is_stale`,
`a_binding_in_a_unit_whose_enacted_notes_changed_is_stale` and
`editorial_changes_make_nothing_stale` of `crates/fidryn-driver`, which passed
in the gate of item 10. `cargo test -p fidryn-driver --locked --offline --
staleness::` runs them with the other tests of staleness, through `lease` on
this machine.

**Verdict: met.**

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

**Where they are, today**, in the law repository, committed in `c9bd0e4`
(`review: two samples of definitions drawn from 119-142`):

- `review/definitions/2026-11-17-sample-1/extracted.jsonl`: 500 lines
  (`wc -l`), each one definition drawn from the 34,329 of the workspace;
- `review/definitions/2026-11-17-sample-1/near-misses.jsonl`: 500 lines,
  each one paragraph drawn from the 5,940 that look definitional under the
  looser pattern of design 9.5 and from which nothing was extracted;
- in each file, `grep -c '"verdict":""'` prints 500: no line has a verdict;
- `review/definitions/estimate.json` says `"status": "drawn, not read"`.

**Not read.** Decision O-3 names nobody to read them.

**What closes it.** A person the owner names:

1. reads each of the 500 lines of `extracted.jsonl` against the official text,
   and writes into its `verdict` whether the terms, the kind and the reach
   words are right, and if not which is wrong. Each line holds the paragraph,
   and the nearest chapeau above it, so the Code need not be opened; the bar
   is textual and needs no attorney;
2. reads each of the 500 lines of `near-misses.jsonl`, and writes whether the
   paragraph defines a term the closed patterns missed;
3. writes into `estimate.json` the errors found in the first sample and the
   share of the 5,940 that the definitions found among the near misses imply.

The item is met when the first sample has no error. If it has one, the
patterns are corrected (with a test for the case), the sample and its
verdicts are kept, and a new sample is drawn with a new seed.

**What is not claimed.** The importer's 34,329 definitions equal, field for
field, those of a second implementation of the same rules (S10), written
apart from the repository (the run's record, "Definitions"). That shows the
importer applies its rules as written. It does not show that the rules find
every definition, or that each definition they find is right. No claim about
the correctness of the definitions is made until the samples are read.

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

**Today:**

- the warm check of item 1: 1.66 s, 831,296 KiB;
- the first run of the script of item 13, with an empty cache: checking
  `encodings/` checked its dependency `uscode/` from cold, then `uscode/`
  itself from the cache. The whole script took 4.78 s, with a peak of
  830,144 KiB, under the script's own limit of 8 GiB of address space for
  each run of the binary.

The checks take less than one percent of the time and about five percent of
the memory the design allows.

**Verdict: met.**

## Item 10: the existing tests and the Stage 0 checks

> 10. The existing 1,188 tests and the Stage 0 checks still pass.

**The gate**, on the tree of this commit without this file (`HEAD`
`71377f7` and the date of `CHANGELOG.md`):

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-model lease run --cpus 16 --mem 25G --time 2h -- /usr/bin/time -v cargo xtask ci --fmt
```

It ended with exit 0, in 1 min 38.03 s wall, with a peak of 1,295,524 KiB.
Its log is `SCRATCH/gate-1.log`. In the order of its steps:

- `cargo fmt --all -- --check` passed;
- the Rust tests: 71 lines `test result:`, together **1,800 passed, 0
  failed, 0 ignored**; no line of the log begins with `warning`;
- `cargo clippy ... -- -D warnings` and `cargo doc` passed;
- the schema probes: `"checked": 399`, `"failed": 0`;
- `All checks passed!`, `29 files already formatted`, `Success: no issues
  found in 29 source files`;
- the conformance tests: `Ran 199 tests`, `OK`;
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

Then:

```sh
env CARGO_TARGET_DIR=/cache/cargo/target-lane-model lease run --cpus 4 --mem 8G --time 30m -- cargo run -p xtask --offline -- release-check
lease run --cpus 4 --mem 8G --time 30m -- cargo deny --offline check licenses bans sources
```

printed `release 0.3.0 (2026-11-18): workspace versions and changelog
verified` and `bans ok, licenses ok, sources ok`, each with exit 0; the
license check also warned twice `license-not-encountered`, for two licenses
`deny.toml` allows that no dependency has. The package build refuses a tree
with changes not committed, so it is not part of this record.

**The 1,188.** The design counted them at `1a70382` with
`cargo test --workspace --offline`. Which tests then stand, unchanged, at this
commit was found by reading the test functions of both commits from git:

```sh
uv run --locked python -I SCRATCH/test_names.py /work/projects/fidryn-lanes/model 1a70382 HEAD
uv run --locked python -I SCRATCH/test_names.py /work/projects/fidryn-lanes/model fd88364 HEAD
```

The script lists every `fn` that follows a `#[test]` or `#[tokio::test]`
attribute, by its file and name, and compares the lines of each function on
both sides. `fd88364` is the commit the stage began from.

- `1a70382` has 1,186 test functions. With the two doc tests of
  `crates/fidryn-core/src/outcome.rs`, they are the design's 1,188. This
  commit has 1,798, and with the same two doc tests they are the 1,800 the
  gate ran.
- Of the 1,467 test functions of `fd88364`, every one but one stands at this
  commit in its file under its name. The one is
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
  else.
- Between `1a70382` and `fd88364`, Stage 0 changed four more:
  `keywords_are_the_grammars_lowercase_quoted_terminals` and
  `sniff_recognizes_unlabeled_blocks` of `highlight.rs`,
  `commands_are_the_equivalent_cli_lines` and
  `frames_show_the_numbered_source_and_the_evaluated_case` of `specimen.rs`.
- Every other test that existed when the stage began is unchanged, and all of
  them pass.

**Verdict: met with a written difference.** Every test passes and the Stage
0 checks pass. Ten tests that existed when the stage began were changed by
it, all tests of the site's list of guides, because the stage added two
guides; one of them was renamed for that reason.

## Item 11: the reference and the ledger on the site

> 11. The language reference documents workspaces, provisions, citations,
>     definitions and bindings, and the site shows the coverage ledger.

**The reference** is `docs/law-modules.md`, to which `docs/language.md`
points ("Editions"). Its sections, at this commit:

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
repository, and its counts are those of item 2.

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
- `uscode_sample::tests::the_curated_sample_checks_against_the_generated_one`
  and
  `uscode_sample::tests::a_pin_that_is_not_the_lock_fails_the_step_and_names_the_curated_manifest`
  (`xtask/src/uscode_sample.rs`) hold the same for the sample of the Code.

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

**Verdict: met.**

## Item 13: the integration test

> 13. The integration test of section 4.3 runs. While the law repository
>     is local, it runs on this machine against the full workspace and in
>     CI against the in-repository sample. The cross-repository CI job is
>     written and switched on when the law repository is published.

Section 4.3: "The law repository records the version of the toolchain it is
checked with."

**On this machine**, the script `tools/check-law-repository.sh`, as
`docs/development.md` ("Checking a law repository") describes it, with its
default limits, the binary named by `FIDRYN_BINARY`, and a cache under
`/cache/fidryn/`. The binary is BINARY, built through the scheduler as above,
since a build by the script through `lease` from inside a job would queue a
job from within one. Its first run, with no cache directory yet:

```sh
cd /work/projects/fidryn-lanes/model && lease run --cpus 16 --mem 16G --time 1h -- /usr/bin/time -v -o /cache/fidryn/task-23b/script-1.time env FIDRYN_BINARY=/cache/fidryn/task-23b/fidryn tools/check-law-repository.sh --cache-dir /cache/fidryn/task-23b/cache /work/projects/law-local
```

Its whole output, stdout:

```text
limits of each run of the binary: 600 s of time, 900 s of processor time, 8589934592 bytes of address space
binary: /cache/fidryn/task-23b/fidryn, fidryn 0.3.0 (given by FIDRYN_BINARY)
cache: /cache/fidryn/task-23b/cache (from --cache-dir)

encodings: the manifest records the toolchain 0.3.0, which is the version of the binary
encodings: fidryn check /work/projects/law-local/encodings --cache-dir /cache/fidryn/task-23b/cache
modules: 1; from cache: 0; errors: 0; warnings: 2
ok
encodings: the check passed with warnings, which do not fail it

uscode: the manifest records the toolchain 0.3.0, which is the version of the binary
uscode: fidryn check /work/projects/law-local/uscode --cache-dir /cache/fidryn/task-23b/cache
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

- Exit 0, 4.78 s wall, 25.05 s user, 830,144 KiB peak.
- The two workspaces at the top of the law repository are `encodings/` and
  `uscode/`; `changes/` and `review/` hold no manifest. Each manifest records
  the toolchain `0.3.0`, the version of the binary.
- No limit was reached. The two warnings are the stale bindings of item 6,
  which do not fail a check.
- The same command again, its time written to `script-2.time`, printed the
  same, but for one line,
  `modules: 1; from cache: 1; errors: 0; warnings: 2`, and the same stderr;
  exit 0, 2.96 s wall.
- The script wrote nothing into the law repository (above), and the cache
  holds 70,795 files, 283,255,566 bytes (`find -type f | wc -l`, `du -sb`).
- The script itself is held by `conformance/test_check_law_repository.py`
  (11 tests), among them `test_a_manifest_of_another_version_fails_and_names_both_versions`,
  `test_a_stale_binding_warns_and_does_not_fail`, and, with stand-ins for
  `lease` and `cargo`, the two tests of the build, which this run did not
  take.

**In CI, against the sample.** The job "cargo xtask ci" of
`.github/workflows/ci.yml` runs `cargo xtask ci --fmt`, whose sample step
checks the committed sample of the Code, its curated workspace and its text
(item 10). That command passed on this tree, on this machine.

**The cross-repository job.** `.github/workflows/law-integration.yml` has the
two jobs of section 4.3: "Law repository at its pin" checks the law
repository at the commit `law-repository.pin.json` names, with the toolchain
of the commit under test; "Drift of the newest law repository" checks the
newest of each. Both run the script. Its only trigger is a manual run, and
the comment at its top names the two lines that switch on the others.
`law-repository.pin.json` is `{"repository": "", "commit": ""}`: while it
names no repository, each job says so and ends.
`conformance/test_law_integration_workflow.py` (9 tests) holds all this,
among them `test_its_only_trigger_is_a_manual_run`,
`test_the_two_lines_its_comment_names_switch_on_the_other_triggers` and
`test_the_last_step_of_each_job_fails_when_the_check_fails`.

Both test files ran in the gate (`Ran 199 tests`, `OK`):

```sh
env FIDRYN_BINARY=/cache/fidryn/task-23b/fidryn uv run --locked python -m unittest discover -s conformance -p 'test_*law*.py'
```

runs them alone, through `lease` on this machine.

**Verdict: met with a written difference.**

- What CI runs was run here with the same command, not on GitHub: this
  branch has not been pushed, since pushing is the owner's step. The first
  push shows it.
- `actionlint`, which the job "Workflow lint" of CI runs, is not installed on
  this machine, so no lint has read `law-integration.yml` yet; the test file
  above reads it as YAML, and its scripts were checked with `shellcheck`.
- The cross-repository job is written and is not switched on, as the item
  says it should be while the law repository is not published (decision
  O-6).

## Known limits

Each of these is true at this commit. Each is also stated, without counts of
the whole Code, in "Limits" of `docs/law-modules.md`.

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
     scripts `record_diff.py` and `superscript_only.py`, show the same thing
     for each: nothing of its own record differs but the words of one
     paragraph, and they are the same once the superscript digits the newer
     release added are taken out and a run of spaces is read as one (a space
     came with two of the four digits). In `/us/usc/t26/s6049/d/8/A` the two
     digits stand where 119-138 had two footnote markers. The cost is a false
     alarm, never a missed change.
   - A change of a unit's aliases, of the container it stands in, or of a
     file's positive-law flag is in no list: a title enacted as positive law
     shows nothing from the flag alone.
   - `usc_compare.py --against` checks the unions of the lists. It cannot see
     a provision in the wrong one of two lists, or a wrong pairing of a move;
     the Rust tests hold those list by list.
2. **Definitions.**
   - The rules do not find every definition; the share they miss is not
     measured until the samples of item 7 are read.
   - `unstated` is given when neither the paragraph nor a chapeau above it
     states a reach; a reach stated in a sibling provision, such as a
     subsection (a) that states the reach of the definitions of a subsection
     (b), is not seen.
   - An exception written inside a definition's own text does not narrow its
     region. About 25 of the 34,329 definitions of release 119-142 hold the
     words of such an exception in their first 300 characters, by the count
     of the review of Task 15; this record did not count them again.
3. **The two readings of the publisher's XML.** Two forms are read
   differently by the importer and by the independent reading: a
   `continuation` or a `proviso` inside a `content` that stands directly in a
   note, and a note deeper inside quoted content. Each side fails loudly when
   one appears: a refusal, or a comparison of the two that fails, never
   altered text. No release of 2026 holds either: the importer imported all
   27 with no refusal, and the independent reading read all 27 in the
   comparisons of the 26 pairs with none. The text of the two readings was
   compared record for record on 119-142 (item 4) and, in every gate, on the
   sample; on the other 26 releases it was not.
4. **Diagnostics.** `fidryn check` prints no line or column for a diagnostic
   of a module of edition 2. `E740` carries the title `PinMismatch` also when
   a dependency fails for another reason than its pin.
5. **Files and the cache.** A module file name such as `s1932~2.fr` has the
   shape of a Windows 8.3 short name, and on a volume that gives files short
   names it could be the short name of another file. A check that is killed
   while it writes leaves a temporary file in the cache, which nothing reads
   and nothing removes.
6. **Fetching.** `tools/fetch_uscode.py` follows a redirect without checking
   its host, and records the address it asked for.

The run's record, "What is open after the second part", adds two points about
the replay and the draw: a stop of the replay between the replacement of
`uscode/` and its commit never occurred, and the population of near misses is
not that of F14.

## Differences from the design's numbers

Every "written difference" above, in one table.

| Item | The design | What was found | Why |
| --- | --- | --- | --- |
| 2 | 60,490 sections | 61,171 units, of which 60,509 are sections | the import counts court rules (544), reorganization plans (107), generic levels that are units (11) and sections without an identifier (20) as units, and the section inside 25 U.S.C. 5329 as a provision of it (−1) |
| 2 | the status breakdown (in force, repealed, omitted, transferred, renumbered, vacant, reserved, unknown) 51,009 / 5,131 / 2,037 / 1,911 / 368 / 17 / 16 / 1 | 51,664 / 5,143 / 2,043 / 1,914 / 369 / 17 / 18 / 3 | the same units, by status (the run's record, "Where each difference comes from") |
| 2 | 772,863 provisions | 774,018 | +1,155: the top provisions of those 682 units, and 473 provisions of kind `level` |
| 2 | 88,935 references resolve exactly | 89,269 | +334: 7 references to a target of kind `level`, 4 `ambiguous`, and 323 to a section whose identifier lists several sections or names a range |
| 2 | 820 reach the section only | 881 | +61 = 72 below such sections, less those 7 and 4 |
| 2 | 964 reach no section | 569 | −395 = −323 − 72 |
| 8 | each commit with its change set as its message | 26 of the 27; the first is `/us/usc: release 119-73not60` | no release before it among the 27 |
| 10 | the existing 1,188 tests pass | 1,800 pass; of those that existed when the stage began, 10 were changed, one of them also renamed | the stage added two guides to the site, and these tests count and list its guides |
| 11 | the site shows the coverage ledger | the site shows the ledger of the sample | the default of decision O-2 |
| 13 | it runs in CI against the sample | run here with the command CI runs; not yet on GitHub; `actionlint` not run on the new workflow | the branch is not pushed, which is the owner's step; `actionlint` is not installed here |

For item 2, the design's own numbers are reproduced exactly by `baseline`;
the rule for each difference is in `conformance/usc-counting-rules.md`, "Where
the two differ".

## What only the owner can do

(to be completed)
