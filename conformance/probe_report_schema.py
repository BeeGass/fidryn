#!/usr/bin/env python3
"""Validate real run/scenario/explore reports and independent schema mutations."""

from __future__ import annotations

import argparse
import copy
import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource


def report_validator(root: Path) -> Draft202012Validator:
    registry: Registry[Any] = Registry()
    for path in (root / "schemas").glob("*.json"):
        schema = json.loads(path.read_text())
        resource = Resource.from_contents(schema)
        registry = registry.with_resource(path.as_uri(), resource)
        if "$id" in schema:
            registry = registry.with_resource(schema["$id"], resource)
    schema = json.loads((root / "schemas/evaluation-report-v0.2.json").read_text())
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, registry=registry, format_checker=FormatChecker())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "root", type=Path, nargs="?", default=Path(__file__).resolve().parent.parent
    )
    args = parser.parse_args()
    root = args.root.resolve()
    binary = os.environ.get("FIDRYN_BINARY")
    if not binary:
        parser.error("set FIDRYN_BINARY to the built interpreter being qualified")
    validator = report_validator(root)
    results: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="fidryn-report-probe-") as directory:
        fixture = Path(directory)
        module = fixture / "module.fr"
        case = fixture / "case.json"
        module.write_text(
            'module ReportProbe version "0.1.0" { query q(x: Int) -> Int { return x } }'
        )
        case.write_text(
            json.dumps(
                {
                    "schema": "fidryn.case-record/v0.1",
                    "admissibleCompletions": {},
                    "facts": {"x": 1},
                    "assumptions": [{"id": "H", "payload": {"x": 2}}],
                }
            )
        )
        for operation, flags in [("run", []), ("run", ["--scenario"]), ("explore", [])]:
            completed = subprocess.run(
                [
                    binary,
                    operation,
                    str(module),
                    "--query",
                    "q",
                    "--case",
                    str(case),
                    "--valid-at",
                    "2026-01-01T00:00:00Z",
                    "--known-at",
                    "2026-01-01T00:00:00Z",
                    *flags,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            assert completed.returncode == 0, completed.stderr
            document = json.loads(completed.stdout)
            validator.validate(document)
            assert document["invocation"]["operation"] == operation
            assert document["traceDocument"]["nodes"], "actual derivation is required"
            assert (
                document["outcomeDocument"]["outcome"]["trace"]
                == document["traceDocument"]["trace"]
            )
            ids = {node["id"] for node in document["traceDocument"]["nodes"]}
            assert document["traceDocument"]["trace"] in ids
            assert all(
                parent in ids
                for node in document["traceDocument"]["nodes"]
                for parent in node.get("parents", [])
            )
            if operation == "explore":
                assert document["searchEvidence"]["examined"] == len(
                    document["searchEvidence"]["branches"]
                )
            results.append(
                {"operation": operation, "mode": document["executionMode"], "accepted": True}
            )
            for field in ["invocation", "invocationId", "engineVersion"]:
                mutant = copy.deepcopy(document)
                del mutant[field]
                assert not validator.is_valid(mutant), f"missing {field} accepted"
            mutant = copy.deepcopy(document)
            mutant["invocation"]["budget"] = -1
            assert not validator.is_valid(mutant), "negative budget accepted"
            mutant = copy.deepcopy(document)
            mutant["invocation"]["semanticsVersion"] = "unknown"
            assert not validator.is_valid(mutant), "unknown semantics accepted"
    print(
        json.dumps(
            {"mode": "actual interpreter reports and schema mutation controls", "reports": results},
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
