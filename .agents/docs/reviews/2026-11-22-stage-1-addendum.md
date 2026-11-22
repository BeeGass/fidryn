# Stage 1 acceptance: addendum of 2026-11-22

The record of 2026-11-18,
[`2026-11-18-stage-1-acceptance.md`](2026-11-18-stage-1-acceptance.md), was
written before anything of Stage 1 had run on GitHub and before the samples
of definitions had been read. It left two of the design's thirteen items
open and listed the limits it knew. This addendum says what happened to each
since. It changes nothing in that record, which stays as it was written.

Everything below is true at `96372c6`, the commit of the branch
`followup/stage1` that this addendum was written on, unless a line names
another commit. The commit that adds this file changes no code.

## What changed, in one table

| In the record | Now |
| --- | --- |
| Item 13, on GitHub: nothing of the branch had run there | Met. Every required check passed on the pull request of the stage, on its first run |
| Item 7: the samples were drawn and not read | Still open. A machine has read both samples; no person has |
| Known limits, 3: the two readings of the publisher's XML read nine forms differently, and the replay did not compare them | The nine forms have one rule each in both readings, and the replay compares the readings of every release before it commits it. Three further forms were found and are stated as limits |
| Known limits, 2: the share of definitions the rules miss was not measured | Measured by the machine's reading: about 5,830 of the 5,940 near misses of release 119-142 are definitions the rules do not extract |
| Known limits, 5: a module that begins with a comment of certain lengths is refused; a cache directory of the user inside a workspace is used | Both corrected |
| No second implementation of the definition rules was in the repository | `conformance/usc_definitions.py` is, and the check of the sample holds the importer to it. It found two differences, in forms no release holds, and both are corrected |

## Item 13: the integration test, on GitHub

> 13. The integration test of section 4.3 runs. While the law repository
>     is local, it runs on this machine against the full workspace and in
>     CI against the in-repository sample. The cross-repository CI job is
>     written and switched on when the law repository is published.

The record gave the half of this item that runs on GitHub the verdict
"open", and said what closes it: the branch is pushed, and the jobs pass,
the lint of `law-integration.yml` among them.

The stage's branch went to GitHub as pull request 19,
"Import the United States Code as checked law modules (0.3.0)", against
`dev`, with the head `852584b4aa932857799802efa74502b321277142`. It was the
first run of the stage's code on macOS and on Windows. Every check that the
rules of `dev` require passed on the first run:

| Check | Result |
| --- | --- |
| `cargo xtask ci` | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723171/job/137110971776) |
| Release metadata | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723171/job/137110971841) |
| Rust tests (ubuntu-24.04) | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723171/job/137110971821) |
| Rust tests (macos-15) | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723171/job/137110971954) |
| Rust tests (windows-2025) | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723171/job/137110971919) |
| Security | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723171/job/137110971598) |
| Workflow lint | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723171/job/137110971823) |
| CodeQL (rust) | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723190/job/137110971576) |
| CodeQL (python) | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723190/job/137110971699) |
| CodeQL (javascript-typescript) | [passed](https://github.com/BeeGass/fidryn/actions/runs/44990723190/job/137110971734) |
| CodeQL | [passed](https://github.com/BeeGass/fidryn/runs/137113521838) |

`cargo xtask ci` runs the check of the pinned sample
(`cargo xtask uscode-sample`), so the sample was checked there. The job
`Publish release` was skipped, as it is on every branch but `main`. The pull
request was merged into `dev` on 2026-11-22 as
`1b6f78e0c5c5f0eb589dddd41221598b031edc12`.

The workflow `Law integration` still does nothing, because no law repository
is named in `law-repository.pin.json`: the law repository is local, and its
name, its visibility and its license are the owner's to decide. That half of
the item is as the record left it.

**Verdict: met for what can run on GitHub today.**

## Item 7: the samples of definitions

> 7. The definition samples of section 9.5 are drawn, read and recorded,
>    and the first has no errors.

No person has read them. On 2026-11-22 a machine read every line of both
samples against the text of the modules of release 119-142. Its reading is
in the law repository, apart from the samples and labeled as a machine's, at
`review/definitions/2026-11-17-sample-1/machine-reading-2026-11-22/` (commit
`b46905c5e`); the `verdict` field of each sample line is still empty and
`review/definitions/estimate.json` still says `drawn, not read`.

| Sample | Lines | What the machine found |
| --- | --- | --- |
| Extracted | 500 | terms right in 500; kind right in 500; reach right in 496, and in 4 stated in another provision that the rules do not read |
| Near misses | 500 | 491 define their term, 7 do not, 2 are unclear; none should have been extracted by the rules as they are written |

What follows from it, with the weight of a machine's reading of one sample:

- What the rules extract is right in its terms and its kind. The four
  reaches that are stated elsewhere are the case the record's limit 2
  describes (the record knew two of them). Whether they count against
  "the first has no errors" is for the person's reading to say.
- The rules miss many definitions. 491 of 500 near misses, applied to the
  5,940 near misses of the release, is about 5,830 definitions that are in
  the Code and in no module's definitions, beside the 34,329 that are.
- A proposal for a second version of the rules was measured against the
  whole Code on the same day and is to be built after this release. It
  changes nothing in this one.

**Verdict: open, as the record left it.** The reading the item asks for is a
person's.

## What was corrected after the record

Nineteen commits on `followup/stage1`, each reviewed, on top of the stage's
head `852584b`:

| Commits | What |
| --- | --- |
| `f584536`, `d6938e6`, `ce48425` | The tests of `xtask` start git without the caller's git environment. A generated module that begins with a comment of any length is recognized. No cache is kept when the user's cache directory lies inside the workspace |
| `d8cc960` | A second implementation of the definition rules, `conformance/usc_definitions.py`, written from the rules and not from the importer; the check of the sample compares the two |
| `48c11ad`, `0eb2086`, `a8667eb` | The two differences it found, corrected: the importer gives a reach written as one link below the prefix the link's target as its region, as the rules say; the independent reader lists a unit's notes in document order. The check of the sample names the module when the two implementations differ |
| `a9a9597` | The importer and the independent reading follow one rule for each of the nine forms of the record's limit 3 |
| `37c0115`, `25191c2`, `48ae03c` | The replay compares the two readings of each release before it commits it; it ends every process of a run at its time limit; a release kept without the comparison is compared before it is committed |
| `ce7a906`, `d44df28` | A roadmap page |
| `5eda0b3`, `9b90c9d`, `2485147`, `c9d68df`, `0c0c9c1`, `12e47d9` | Dependencies brought up to date. `ureq` stays at version 2, because version 3 changes how the first edition's filing client classifies failures; the reason stands in `.github/dependabot.yml` |

## The 27 releases, replayed with the corrected tools

On 2026-11-22 all 27 release points of 2026 were replayed again, with
`tools/replay_uscode.py` of `96372c6` and the release binary built from
that commit (`fidryn 0.3.0`, SHA-256
`b8e21246dd21990e024233eee3a0fa6ee8d2c620e3ecb8c63d8b5299a1030b8a`), from
the archives under `/library/corpora/uscode`, into a new and empty
repository:

```sh
env TMPDIR=/cache/fidryn/controller-replay-f-tmp lease run --cpus 16 --mem 40G --time 2h -- uv run --locked python -I tools/replay_uscode.py --archives /library/corpora/uscode --repository /work/projects/law-replay-f --fidryn /cache/cargo/target-verify/release/fidryn --cache-dir /cache/fidryn/controller-replay-f --jobs 8
```

> the replay is complete: 27 releases are committed

It exited 0.

- For each of the 27 releases the tool compared the two readings before the
  commit, and they agree in all 27. The last line of that kind:
  `119-142: the two readings agree: 1015386 records, 3303 tables of contents`.
  This is the first time the replay itself made the comparison; the record
  made it once, by hand, for all 27.
- Each of the 27 commits is signed.
- The tree of each commit, and its message, are those of the same release's
  commit on `main` of the law repository (`/work/projects/law-local`): 27 of
  27 trees equal, 27 of 27 messages equal.

So the corrections of this branch change no module of any release of 2026.
The correction of the importer's region rule could have: no release holds
the form it concerns. The law repository stays as it is, with one commit
added since the record, the machine's reading of the samples (`b46905c5e`).

## Limits, as they stand now

The record's limits 1, 4 and 6 are unchanged, and so is the rest of 2 and 5.
Gone:

- limit 3, as far as the nine forms and the missing comparison go;
- of limit 5, the module that begins with a comment and the cache inside a
  workspace;
- of limit 2, "the share they miss is not measured".

New, each stated in "Limits" of `docs/law-modules.md` or in
`conformance/README.md`:

- Three more forms that the two readings treat differently: a comment or a
  processing instruction inside a `br` or a column; a schema location that is
  split and paired in an unusual way; a note with the role `crossHeading`
  directly under a title whose text begins "Current through". No release of
  2026 holds one. Since the replay now compares the readings, a release that
  held one would stop the replay before its commit.
- The memory limit of the replay tool bounds each process of a script's
  run, not the run: a script with `--jobs N` may use up to N + 2 times it.
- A signal other than an interrupt (a hangup, a termination) ends the replay
  tool without ending the run it started.

## Validation at `96372c6`

- The gate, `cargo xtask ci --fmt`, exits 0: 1,852 Rust tests, 264
  conformance tests and 52 JavaScript tests, none failing; the check of the
  sample passes with its nine steps.
- `cargo audit --deny warnings` and `cargo deny check` exit 0 against the
  advisory database of 2026-11-21.
- `tools/check-law-repository.sh /work/projects/law-local`, with the binary
  above, exits 0: `checked 2 workspaces: every version is the binary's, and
  every check passed`. The import of the Code: 70,794 modules, no error and
  no warning. The curated workspace: no error and two warnings. Two of its
  three bindings, written against release 119-138, are stale against
  119-142: the text of 11 U.S.C. 109(e) changed, and so did the enacted
  notes of 11 U.S.C. 101. That is what those bindings are there to show.

## What only the owner can do

As in the record. In particular:

- name a person to read the two samples of definitions (item 7);
- decide the law repository's name, visibility and license, without which
  `Law integration` cannot run and nothing of the law repository is pushed.
