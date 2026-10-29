#!/usr/bin/env python3
"""Assert independent positive and negative controls for the outcome schema."""

from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


def probe_cases() -> list[tuple[str, dict[str, Any], bool]]:
    base: dict[str, Any] = {
        "schema": "fidryn.outcome/v0.1",
        "module": "Probe@0.1.0",
        "sourceSnapshot": "0" * 32,
        "query": "q",
        "asOf": {
            "validTime": "2033-01-01T00:00:00Z",
            "recordTime": "2033-01-01T00:00:00Z",
        },
        "modelBoundary": {"outsideScope": [], "admissibleCompletions": {}},
        "outcome": {
            "kind": "determinate",
            "trace": "1" * 32,
            "value": {"kind": "bool", "data": True},
            "ignoredOpenIssues": [],
        },
    }
    cases = [("valid_determinate", base, True)]
    missing = copy.deepcopy(base)
    del missing["outcome"]["value"]
    cases.append(("determinate_without_value", missing, False))
    trace = copy.deepcopy(base)
    trace["outcome"]["trace"] = "not-a-hex-id"
    cases.append(("invalid_trace", trace, False))
    unresolved = copy.deepcopy(base)
    unresolved["outcome"]["ignoredOpenIssues"] = [
        {"kind": "needCustom", "effect": "Review", "payload": "unresolved"}
    ]
    cases.append(("ignored_issue_without_certificate", unresolved, False))
    malformed = copy.deepcopy(base)
    malformed["outcome"]["value"] = {"kind": "bool", "data": {"not": "a boolean"}}
    cases.append(("ill_typed_tagged_value", malformed, False))
    return cases


def evaluate(schema: dict[str, Any]) -> tuple[list[dict[str, Any]], list[str]]:
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    results = []
    failures = []
    for name, document, expected in probe_cases():
        errors = [error.message for error in validator.iter_errors(document)]
        accepted = not errors
        results.append(
            {
                "name": name,
                "accepted_by_current_schema": accepted,
                "expected_acceptance": expected,
                "errors": errors,
            }
        )
        if accepted != expected:
            failures.append(name)
    return results, failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "root", type=Path, nargs="?", default=Path(__file__).resolve().parent.parent
    )
    args = parser.parse_args()
    schema = json.loads((args.root / "schemas/outcome-v0.1.json").read_text())
    results, failures = evaluate(schema)
    print(
        json.dumps(
            {"mode": "executed JSON Schema assertions; no Rust execution", "probes": results},
            indent=2,
        )
    )
    if failures:
        print("outcome schema expectation mismatch: " + ", ".join(failures), file=sys.stderr)
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
