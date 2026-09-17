# Getting started with Fidryn

Clone the repository, install Rust 1.98, then check and run a tiny
`.fr` program. This is a first hour, not a complete encoding of any
jurisdiction.

Fidryn (FID-rin) is a programming language for legal instruments.
Source files use the `.fr` extension. A computation may return one
determinate result only when that result is invariant across every
still-admissible resolution of the unresolved issues, or when a
competent authority has already made a determination that is operative
in the relevant context. The interpreter never invents a completion.

This repository is the v0.1 research interpreter. The modules under
`examples/` and the programs under `tests/` are fixtures. They are not
legal advice, not operative instruments, and not a complete statement
of any jurisdiction's law.

From the repository root, every command below uses
`cargo run -p fidryn-cli --` so it works before you install a `fidryn`
binary. After you build, the same arguments follow `fidryn`.

## Prerequisites

Install Rust 1.98. The workspace `rust-version` is 1.98.
[`rust-toolchain.toml`](../rust-toolchain.toml) pins the patch
(`1.98.1`) and the `rustfmt` / `clippy` components. `rustup` will pick
that toolchain up when you enter the repository.

You need a working C linker. On macOS, Command Line Tools clang is
enough.

## Build

From the repository root, compile the workspace and run the tests.
The first pass takes a bit: it builds every crate, then runs the
suite.

```
cargo test --workspace --offline
```

Confirm the CLI:

```
cargo run -p fidryn-cli -- --help
```

You should see the `fidryn` subcommands, including `check` and `run`.
`run` evaluates a query against a case record and never chooses a
completion. See the [CLI](cli.md) guide for the rest of the surface.

## Check a tiny program

[`tests/programs/require-gate.fr`](../tests/programs/require-gate.fr)
is a complete module with two queries. `q` requires `true` and returns
`7`. `r` requires `false` and does not reach `return 7`.

```
module Programs.RequireGate version "0.1.0" {
    jurisdiction Test
    source_snapshot "2026-09-17-require-gate"
    effective_at 2026-09-17
    recorded_at 2026-09-17T12:00:00Z

    outside_scope { complete_instruments }

    query q() -> Int { require true; return 7 }
    query r() -> Int { require false; return 7 }
}
```

Check it:

```
cargo run -p fidryn-cli -- check tests/programs/require-gate.fr
```

Once `fidryn` is on your PATH, that is
`fidryn check tests/programs/require-gate.fr`. Success prints `ok`
and exits 0. Diagnostics go to stderr and the process exits 1.
`check` parses, elaborates, and type-checks. It does not evaluate a
query and does not need a case file.

## Run a query

`run` requires a case record plus both clocks. `--valid-at` is the
instant on the legal timeline you are asking about. `--known-at` is
the instant of the file you are willing to treat as known. Both are
ISO 8601 / RFC 3339 (`Z` or a numeric offset). Use
`2026-09-17T12:00:00Z` for this hour. Details of the case JSON and
the two clocks are in [Cases and time](cases-and-time.md).

`require-gate` does not need facts or an interpretation. Write a
minimal empty case that satisfies the interchange schema
(`schema` and `admissibleCompletions` are the required properties):

```
cat > /tmp/fidryn-empty-case.json <<'EOF'
{"schema":"fidryn.case-record/v0.1","admissibleCompletions":{}}
EOF
```

Run `q`. It should be determinate `7`:

```
cargo run -p fidryn-cli -- run tests/programs/require-gate.fr \
    --query q \
    --case /tmp/fidryn-empty-case.json \
    --valid-at 2026-09-17T12:00:00Z \
    --known-at 2026-09-17T12:00:00Z
```

The CLI prints one line of canonical JSON
(`fidryn.evaluation-report/v0.1`). Pretty-printed, the important fields
look like this:

```json
{
  "schema": "fidryn.evaluation-report/v0.1",
  "executionMode": "operative",
  "sourceTrust": "unauthenticated",
  "verificationMethod": "none",
  "assumptions": [],
  "coverage": null,
  "outcomeDocument": {
    "schema": "fidryn.outcome/v0.1",
    "module": "Programs.RequireGate@0.1.0",
    "query": "q",
    "asOf": {
      "validTime": "2026-09-17T12:00:00Z",
      "recordTime": "2026-09-17T12:00:00Z"
    },
    "modelBoundary": {
      "outsideScope": ["complete_instruments"],
      "admissibleCompletions": {
        "interpretations": {},
        "evidence": {},
        "choices": {}
      }
    },
    "outcome": {
      "kind": "determinate",
      "value": { "kind": "int", "data": 7 }
    }
  }
}
```

The six-kind legal result is nested under `outcomeDocument`.
`modelBoundary` is part of that answer. The module declared
`complete_instruments` as outside scope, so the envelope repeats that
exclusion. An omitted interpretation cannot silently shrink the model.
Pass `--scenario` only when you intend `case.assumptions` as an overlay;
operative `run` (the default) does not apply them.

Now run `r`:

```
cargo run -p fidryn-cli -- run tests/programs/require-gate.fr \
    --query r \
    --case /tmp/fidryn-empty-case.json \
    --valid-at 2026-09-17T12:00:00Z \
    --known-at 2026-09-17T12:00:00Z
```

`require false` does not skip ahead to `return 7`. Nested
`outcomeDocument.outcome` is `suspended`, with a request that the
requirement failed:

```json
{
  "outcomeDocument": {
    "outcome": {
      "kind": "suspended",
      "requests": [
        {
          "kind": "needCustom",
          "effect": "require",
          "payload": "requirement failed"
        }
      ]
    }
  }
}
```

That is the governing rule in miniature: the interpreter will not
pretend the later `7` is the answer. How those kinds compose is in
[Outcomes](outcomes.md).

`--query` is the query name as written in the module (`q`, `r`).
`--arg KEY=VALUE` writes `case.facts["KEY"]`. You do not need it here.

## Check a larger fixture

The Massachusetts trust fixture is a bounded slice of a private
instrument, not Massachusetts trust law as a whole. Check it the same
way:

```
cargo run -p fidryn-cli -- check examples/trust/bryan-revocable-trust.fr
```

Success is again `ok`. The module lists `tax`, `creditor_priority`,
`real_property_recording`, and `complete_Massachusetts_trust_law` as
`outside_scope`. Those names will appear on `modelBoundary` if you
later `run` a query against a case under
[`examples/trust/cases/`](../examples/trust/cases/).

The [examples](examples.md) catalog has more fixtures. They encode
bounded high-impact slices. They do not contain the entire United
States Code.

## Next reading

1. [Language](language.md) — module headers, queries, rules, duties,
   and what Fidryn will not do.
2. [CLI](cli.md) — `check`, `run`, `explore`, and the other
   subcommands, with the flags the binary actually accepts.
3. [Outcomes](outcomes.md) — determinate, suspended, contingent, and
   the rest of the envelope.
4. [Cases and time](cases-and-time.md) — case JSON, admissible
   completions, `--valid-at` / `--known-at`.
5. [Examples](examples.md) — the fixture corpus under `examples/`.
6. [Mill](mill.md) — localhost `fidryn ui` on `127.0.0.1` (default
   port 8751). The mill checks a module in the browser. It does not
   live-file.
