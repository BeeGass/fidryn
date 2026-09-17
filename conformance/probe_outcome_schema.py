#!/usr/bin/env python3
"""Demonstrate underconstrained outcome schemas. No Rust execution involved.

Usage: python3 conformance/probe_outcome_schema.py [/path/to/fidryn]
Requires jsonschema.

Cargo tests do not invoke this script; Python is optional for schema probes.
Once schemas/outcome-v0.1.json is strengthened, all three
accepted_by_current_schema entries should be false.
"""

from __future__ import annotations
import argparse
import copy
import json
from pathlib import Path
from jsonschema import Draft202012Validator


def repo_root_from_script() -> Path:
    return Path(__file__).resolve().parent.parent


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "root",
        type=Path,
        nargs="?",
        default=None,
        help="Fidryn repository root (default: parent of this script's directory)",
    )
    args = parser.parse_args()
    root = (args.root or repo_root_from_script()).resolve()
    schema = json.loads((root / "schemas/outcome-v0.1.json").read_text())
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    base = {
        "schema": "fidryn.outcome/v0.1",
        "module": "Probe@0.1.0",
        "sourceSnapshot": "fixture",
        "query": "q",
        "asOf": {
            "validTime": "2033-01-01T00:00:00Z",
            "recordTime": "2033-01-01T00:00:00Z",
        },
        "modelBoundary": {"outsideScope": [], "admissibleCompletions": {}},
        "outcome": {"kind": "determinate", "trace": "not-a-hex-id"},
    }
    cases = {"determinate_without_value_and_invalid_trace": base}
    unresolved = copy.deepcopy(base)
    unresolved["outcome"].update(
        value={"kind": "bool", "data": True}, ignoredOpenIssues=["unresolved"]
    )
    cases["ignored_issue_without_certificate"] = unresolved
    malformed = copy.deepcopy(base)
    malformed["outcome"]["value"] = {"kind": "bool", "data": {"not": "a boolean"}}
    cases["ill_typed_tagged_value"] = malformed
    results = []
    for name, doc in cases.items():
        errors = [e.message for e in validator.iter_errors(doc)]
        results.append(
            {
                "name": name,
                "accepted_by_current_schema": not errors,
                "errors": errors,
            }
        )
    print(
        json.dumps(
            {
                "mode": "executed JSON Schema checks; no Rust execution",
                "probes": results,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
