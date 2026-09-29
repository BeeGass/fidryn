# Conformance

JSON Schema checks for case records and source manifests under `examples/`
and `prelude/`. This is a document-shape check. It does not authenticate
artifacts, execute Rust, or review legal content.

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

The probe checks `schemas/outcome-v0.1.json` against three malformed
`fidryn.outcome/v0.1` documents (determinate without value / non-hex
trace, ignored open issues without a certificate, tagged bool with a
non-boolean). After the schema strengthening, all three should report
`accepted_by_current_schema: false`.

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
