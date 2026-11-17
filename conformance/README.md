# Conformance

JSON Schema checks for case records and source manifests under `examples/`
and `prelude/`. This is a document-shape check. It does not authenticate
artifacts, execute Rust, or review legal content. A second implementation
of the hashes of law modules is described at the end.

```sh
uv run python conformance/check_json_contracts.py
uv run python conformance/check_json_contracts.py /path/to/fidryn --output validation.json
```

The checker looks for `"schema": "fidryn.case-record/v0.1"` and
`"fidryn.source-manifest/v0.1"`. Exit `0` if every checked document is
valid, `1` on validation failures, `2` on setup or read errors.

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
