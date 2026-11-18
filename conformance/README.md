# Conformance

JSON Schema checks for case records and source manifests under `examples/`
and `prelude/`, for the four files of a workspace of law modules
under `fixtures/workspace/` and under `samples/uscode/`, for the report of
the check of a workspace under `fixtures/check-report/`, for the change set
between two workspaces under `fixtures/change-set/`, and for every text
record of `fixtures/`. This is a document-shape check. It does not
authenticate artifacts, execute Rust, or review legal content. A second
implementation of the hashes of law modules is described at the end.

```sh
uv run python conformance/check_json_contracts.py
uv run python conformance/check_json_contracts.py /path/to/fidryn --output validation.json
```

The checker looks for `"schema": "fidryn.case-record/v0.1"`,
`"fidryn.source-manifest/v0.1"`, `"fidryn.workspace/v0.1"`,
`"fidryn.law-lock/v0.1"`, `"fidryn.retrieval/v0.1"`,
`"fidryn.check-report/v0.1"`, `"fidryn.coverage/v0.1"` and
`"fidryn.change-set/v0.1"`, and holds every line of every
`*.records.jsonl` under `fixtures/` to `schemas/text-record-v0.1.json`,
the schema of `fidryn.text-record/v0.1`, whose records hold no `schema`
key. Exit `0` if every checked document is valid, `1` on validation
failures, `2` on setup or read errors.

`fixtures/text/demo.records.jsonl` holds what `fidryn text --json` prints
of the demo workspace. The Rust test
`text_prints_one_record_for_each_provision_and_note_in_address_order` of
`fidryn-cli` holds it, byte for byte, to what the command prints.
`test_text_record_schema.py` checks that it and every record the fixtures
of the publisher's XML expect match the schema, and that the schema
refuses what is no text record.

`fixtures/workspace/` holds a generated workspace, `demo/`, in the
invented corpus `/zz/demo`, and in `archives/` the retrieval record of its
archive. Every name, text, address, digest of an archive and count in them
is invented. The Rust test
`the_committed_demo_workspace_is_what_the_writer_writes` of `fidryn-driver`
holds every file there, byte for byte, to what the writer of a generated
workspace writes, its coverage ledger `demo/coverage.json` among them.
`test_workspace_schemas.py` checks that the manifest, the lock and the
retrieval record match their schemas, and that the schemas refuse what a
workspace check refuses. `test_coverage_schema.py` checks that the ledger
of the demo workspace and that of the sample of the Code match
`schemas/coverage-v0.1.json`, in the layout the writer writes, and that the
schema refuses what is no ledger.

`fixtures/check-report/` holds `report.json`, the check report of an
invented workspace in the corpus `/zz/demo` that has a reference of every
outcome, and `unresolved.jsonl`, its list of the references whose outcome
is not `exact`. Every name and address in them is invented. The Rust test
`the_committed_check_report_is_what_the_check_writes` of `fidryn-driver`
holds both files, byte for byte, to what the check of that workspace
writes. `test_check_report_schema.py` checks that the report matches
`schemas/check-report-v0.1.json` and that the schema refuses what the
check never writes, and that the list has one object to a line with its
keys in order, sorted, one line for each reference the report counts
with an outcome other than `exact`.

`fixtures/change-set/change-set.json` is the change set between two
invented generated workspaces in the corpus `/zz/demo`, with the stale
bindings of two invented curated workspaces, in which every list holds
more than one entry. The Rust test
`the_committed_change_set_is_what_the_comparison_writes` of `fidryn-driver`
holds it, byte for byte, to what the comparison writes.
`test_change_set_schema.py` checks that it matches
`schemas/change-set-v0.1.json`, with every key in its order and every list
sorted, and that the schema refuses what the comparison never writes.

`uv run` provides Python and `jsonschema` from the repository's
`pyproject.toml` and `uv.lock`. Without `jsonschema` the script falls
back to a built-in Draft 2020-12 subset checker of the same files.

Outcome envelope probes (not invoked from cargo tests; `cargo xtask ci`
runs them):

```sh
uv run python conformance/probe_outcome_schema.py
```

The probe checks `schemas/outcome-v0.1.json` against four malformed
`fidryn.outcome/v0.1` documents: `determinate_without_value`,
`invalid_trace`, `ignored_issue_without_certificate` and
`ill_typed_tagged_value`. After the schema strengthening, all four should
report `accepted_by_current_schema: false`.

Evaluation-report and mill-transport probes (not invoked from cargo
tests; `cargo xtask ci` runs them):

```sh
uv run python conformance/schema_boundary_probes.py
```

The probe checks `schemas/evaluation-report-v0.1.json` and
`schemas/mill-evaluation-response-v0.1.json`. `uv run` provides
`jsonschema` and `referencing`; without them the script uses a local
Draft 2020-12 subset and does not fetch `$schema` URLs. A baseline
operative report and `{ "ok": true, "report": ... }` should be accepted.
Flattened mill `{ ..., "ok": true }` on the report object, scenario
without `assumptions`, operative with a nonempty `assumptions` array,
and `finiteReplay` without a coverage object should be rejected.

Known answers of the hashes of law modules (`cargo xtask ci` runs them
through `test_hash_vectors.py`):

```sh
uv run --locked python conformance/hash_vectors.py
uv run --locked python conformance/hash_vectors.py --write
```

`hash_vectors.py` computes the hashes of the "Hashes" section of
`docs/law-modules.md` from the rules alone, and shares no code with the
crate `fidryn-provision`. Without `--write` it verifies the digests recorded
in `fixtures/hash/vectors.json` and exits `1` when one differs, `2` when
the file cannot be read; `--write` fills them in. The Rust test
`known_answers_match_the_independent_implementation` holds the crate to the
same digests.

`vectors.json` is one object. Its first key, `invented`, says on the first
line that every model in the file is invented. Then four lists, each entry
with a `name`, a model in the JSON form of the crate's model types, and its
digests:

| List | Model | Digests |
| --- | --- | --- |
| `provisions` | `provision` | `hash`: its tree hash |
| `notes` | `note` | `hash` |
| `units` | `unit` | `layers`: `operative`, `enacted`, `editorial` |
| `containers` | `container` | `hash`, and `layers` |

## The official XML of the Code

`uslm_text.py` reads the official XML release point published by the
Office of the Law Revision Counsel and prints one text record to a line
for every provision, container, note, source credit and footnote, sorted
by the bytes of their addresses. It is a second reading of the
publisher's XML, written apart from the Rust importer and sharing no code
with it, so that the two can be compared line for line. `uslm_common.py`
holds what it reads: archives and members, the checks on each document,
the classes of elements, and the units, containers and notes they give.

```sh
uv run --locked python -I conformance/uslm_text.py /path/to/release
uv run --locked python -I conformance/uslm_text.py usc01.xml --jobs 1
```

The input is a directory of archives (every `.zip` in it is read, and
nothing else), one archive, or one XML file named as an archive member
is (`usc`, two digits, an optional `a` or `A`, `.xml`). Members are read
in the order of their names, at most `--jobs` at a time (default 4), and
the output is the same for any number. Downloaded archives are untrusted
data: run the script with `python -I`. It parses with `defusedxml` and
refuses a document type declaration and an entity declaration. Exit
`0` when the records are printed, `1` when the input is refused by a
reading rule (the message names the archive, the member, the element and
the nearest identifier), `2` when it cannot be read at all.

`fixtures/uslm/` holds invented documents in the publisher's XML, one
for each case of the reading rules, in the invented title 99 and the
invented release `999-1`. Each `<name>.xml` is read as the member
`usc99.xml`; `two-files/` holds the members `usc99.xml` and `usc99a.xml`,
which the test packs into one archive. Beside each, `<name>.records.jsonl`
holds the expected records, written by hand from the reading rules.
`test_uslm_text.py` compares the script's output with them and checks
that the script refuses what the rules refuse. `reissue/` holds two
documents, `old.xml` of the release `999-1` and `new.xml` of the release
`999-2`, which differ in every element id, in the publication name, in the
creation time of the metadata and in the release note, and in nothing
else; the test checks that the script prints the same records for both.
The documents `definitions-*.xml` hold the cases of the rules of
[Definitions](../docs/law-modules.md#definitions). No text record shows a
definition, so their records and profiles are those of any other document;
the Rust tests of `fidryn-ingest-uscode` hold the definitions the importer
extracts from them.

`usc_compare.py` reads two releases, each an input as `uslm_text.py` reads
it, into their text records, and from the records alone prints which
addresses differ: the provisions, the notes of each layer and the
containers only in the old, only in the new and on both sides with another
record, footnote markers left out; and the units and containers whose
credit, or whose footnotes or footnote markers, differ. With
`--against CHANGES.json`, a change set that `fidryn changes --json` printed
for the same releases, it exits `1` and names the first address that
differs unless each of those sets is the union of the lists of the change
set that it stands for, such as `removed` with the `from` of `moved`
([Change sets](../docs/law-modules.md#change-sets)); which of the lists an
address is in, and how moved entries are paired, it does not see. The
other exit codes are those of `uslm_text.py`.

```sh
uv run --locked python -I conformance/usc_compare.py OLD_RELEASE NEW_RELEASE
uv run --locked python -I conformance/usc_compare.py OLD_RELEASE NEW_RELEASE --against changes.json
```

`fixtures/uslm/pairs/` holds pairs of invented documents of the title 99,
`<name>/old.xml` of the release `999-1` and `<name>/new.xml` of `999-2`,
one pair for each case: a footnote marker added, a source credit changed,
the heading of a chapter changed, a word amended, a subsection added, a
section renumbered with its note, a section removed, and notes amended,
removed and added. `test_usc_compare.py` holds the script to the sets
written by hand for each pair, and fails for a pair it does not name. Its
test of agreement imports both documents of each pair with the built
`fidryn`, which `FIDRYN_BINARY` names, runs `fidryn changes --json`, and
checks the change set against the script with `--against`; it fails,
and does not skip, when `FIDRYN_BINARY` is not set. `cargo xtask ci` sets
it.

`usc_profile.py` reads the same inputs and prints the counts of two sets of
counting rules as one JSON object with sorted keys: `baseline`, the counting
of the first profile of the Code (2026-11-13), and `import`, the counts of
what the reading imports. [usc-counting-rules.md](usc-counting-rules.md) states both sets of
rules and every key, and gives the profile of release 119-142 with the
command that measured it.

```sh
uv run --locked python -I conformance/usc_profile.py /path/to/release --out profile.json
```

`--out FILE` also writes the profile to that file; nothing is written beside
the input. The exit codes are those of `uslm_text.py`. Each fixture has its
expected `import` profile beside it, `<name>.profile.json`, written by hand;
`test_usc_profile.py` compares the script's output with them.

## Fetching a release

`tools/fetch_uscode.py` downloads archives of a release point from the
publisher, one request at a time, and writes or extends the retrieval
record `retrieval.json` beside them. It is the only program in the
repository that opens a connection; the sample it fetched is described in
[samples/uscode/README.md](../samples/uscode/README.md).

```sh
uv run --locked python -I tools/fetch_uscode.py 119-142 --out DIR --title 1 11a 45
uv run --locked python -I tools/fetch_uscode.py 119-142 --out DIR --all
```

An archive that is present and equal to its record is kept, and is not
fetched again. An archive and the record are written through a new file
beside each, and an entry of that name there already, a link among them,
is not written through. A redirect is followed only to the host the tool
asked, over `https`, and a response of more than `--max-bytes` bytes
(512 MiB unless it says otherwise) is refused while it is read. Exit `0`
when every archive is fetched or kept, `1` when the tool refuses (an
archive whose bytes differ from its record, an archive without a record,
a record of another release, a redirect to another host or scheme, a
response larger than the bound), `2` when an archive cannot be fetched or
the directory cannot be read or written, such as when the name of a file
it writes through is taken. `test_fetch_uscode.py` serves invented
archives through a fake opener and opens no connection. It holds the tool
to its addresses (titles `1` and `5a` of `119-142`, title `11a` of
`119-73not60`, and `--all`), to the record it writes, to a second run that
changes nothing, to the refusal of an archive whose bytes changed, of a
temporary file that is there already and of a body larger than the bound,
which it reads no further than one byte past it. Its tests of redirects
run the tool's own opener, in which a handler of the test serves invented
responses before any handler that would connect, and the opening of a
socket fails the test.

## Checking a law repository

`test_check_law_repository.py` runs `tools/check-law-repository.sh` on
copies of `fixtures/workspace/demo` in temporary directories, with the
built `fidryn` that `FIDRYN_BINARY` names, and fails, and does not skip,
when it is not set; one test runs the script on a search path that holds
neither `timeout` nor `prlimit`, and its two tests of the build put
stand-ins for `cargo` and `lease` on the search path and build nothing.
They need `sh`, and are skipped, saying so, where there is none.
`test_law_integration_workflow.py` reads
`.github/workflows/law-integration.yml` as YAML, with a small reader of
its own, and holds the workflow and `law-repository.pin.json` to what
[Checking a law repository](../docs/development.md#checking-a-law-repository)
says.

## Replaying releases

`tools/replay_uscode.py` makes each of the 27 release points the publisher
lists for 2026, in the publisher's order, one commit of a repository of
the imported law. For each release it fetches the archive of every title
with `fetch_uscode.py` when it is absent; imports it with
`fidryn ingest uscode` into `uscode.next/` and checks it, which must find
no error and no warning; for every release after the first writes the
change set of `fidryn changes uscode uscode.next` as JSON to
`changes/<release>.json`; replaces `uscode/` with `uscode.next/`; and
commits, with the text form of the change set as the message given to git
as it is (`/us/usc: release 119-73not60` for the first), then reads the
message back from the commit and stops unless it is the text form byte
for byte.

```sh
uv run --locked python -I tools/replay_uscode.py --archives DIR --repository DIR --fidryn PATH --cache-dir DIR
```

The commits of the branch that is checked out, from its root, say where
the replay stands, so a replay that is stopped goes on from the release it
was in, and never commits a release twice or skips one. Before it asks
git anything or writes anything, it holds the repository to be one of its
own: `.git` is a directory in it, and git takes it to be the top of that
repository, not a directory inside another one. It imports only into a
`uscode.next` that is absent or a directory. It removes only `uscode/`,
and only when the repository holds `.git`, `uscode` is a directory and no
link with the manifest of a generated workspace of `/us/usc`, or an empty
directory that a replay stopped between the removal of the manifest and
of the directory left, when the run found the text form of the release
kept, and `uscode.next/` holds the complete import of the next release. It
writes each file through a new file beside it, and an entry of that name
there already, a link among them, stops it and is left alone. Anything
else stops it with what it found. The cache may not be the repository nor
lie inside it, with links resolved on both sides. Each run of `fidryn`
has a limit of its own on its address space and its time
(`--memory-limit`, `--time-limit`). Exit `0` when every release is
committed, `1` when the replay stops at what it found, `2` when a command
or a file cannot be run, read or written. `test_replay_order.py` holds
the list to the publisher's, the reading of a history to the commits it
names, and the replacement to its conditions, with no network and no
binary. `test_replay_repository.py` replays three invented releases into
a repository made with `git init` in a temporary directory, whose own
configuration needs no signing key, with a stand-in for `fidryn`: a
complete replay; replays stopped by a failure, of the stand-in, of a hook
or of one call of the tool, before the replacement, before the commit,
between the removal of the manifest and of the directory, and after the
commit, and started again; a workspace changed after its commit; a path
staged beyond what a commit holds; and the refusals of a `uscode.next`
that is a link or a file, of a directory that is no repository of its
own, of a cache inside the repository through a link, and of a temporary
file that is there already. It needs `git` and `sh`, and is skipped where
either is missing.

## Drawing definition samples

`tools/draw_definition_samples.py` draws the two samples of 500 that a
person reads to judge the definitions of a generated workspace:
`extracted.jsonl`, from every definition its module files hold, and
`near-misses.jsonl`, from the paragraphs of operative text that hold a
term in curly quotation marks within 60 characters before `mean`,
`means`, `include`, `includes`, `shall mean` or `has the meaning`, and
from which no definition was extracted. It reads the `definition` blocks
in their canonical form, stops at a block in another form, and holds the
number it read, by basis, to the workspace's coverage ledger
`coverage.json`. It reads the text records that `fidryn text DIR --json`
printed for the workspace, and holds them to it: they count the units,
containers, provisions, notes and footnotes the ledger counts, and every
definition's provision has a record. It starts no program. Each line
holds the seed, the release, the size of the population, the place in the draw,
what a reader needs to judge it without opening the Code (the paragraph's
text and the nearest chapeau above it among them), and an empty
`verdict`. The draw is a Fisher-Yates shuffle driven by SplitMix64, written
out in the tool, so that one seed draws the same lines with any version of
the language; a sample once drawn is not written over.

```sh
uv run --locked python -I tools/draw_definition_samples.py --workspace DIR --records FILE --seed N --out DIR
```

Exit `0` when both samples are drawn, `1` when an input is not in the form
the tool reads or a sample is there already, `2` when an input cannot be
read or the output written. `test_draw_definition_samples.py` holds the
generator to the known answers of SplitMix64, the draws of the two samples
drawn from the release 119-142 to theirs, and the draw, the two
populations, the lines and the refusals of a block in another form, of a
ledger that counts other definitions and of records of other provisions to
an invented workspace.
