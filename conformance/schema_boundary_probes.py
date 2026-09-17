#!/usr/bin/env python3
"""Probe Fidryn evaluation-report and mill-transport JSON Schemas.

Does not execute Rust. Prefers jsonschema + referencing when installed.
Without those packages, uses a local Draft 2020-12 subset (no network).
Reports observed acceptance, not semantic verification of an evaluation.

Usage: python3 conformance/schema_boundary_probes.py [/path/to/fidryn]
"""

from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
from typing import Any
from urllib.parse import urljoin


def repo_root_from_script() -> Path:
    return Path(__file__).resolve().parent.parent


JSON_TYPE_NAMES = {
    type(None): "null",
    bool: "boolean",
    int: "integer",
    float: "number",
    str: "string",
    list: "array",
    dict: "object",
}


def json_types(instance: Any) -> set[str]:
    names: set[str] = set()
    if instance is None:
        names.add("null")
    elif isinstance(instance, bool):
        names.add("boolean")
    elif isinstance(instance, int):
        names.update({"integer", "number"})
    elif isinstance(instance, float):
        names.add("number")
    elif isinstance(instance, str):
        names.add("string")
    elif isinstance(instance, list):
        names.add("array")
    elif isinstance(instance, dict):
        names.add("object")
    return names


def json_pointer(schema: Any, pointer: str) -> Any:
    if not pointer:
        return schema
    current = schema
    for part in pointer.lstrip("/").split("/"):
        part = part.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict):
            current = current[part]
        else:
            raise KeyError(pointer)
    return current


def resolve_ref(ref: str, base_id: str, by_id: dict[str, Any]) -> Any:
    uri = urljoin(base_id, ref)
    doc_uri, _, fragment = uri.partition("#")
    schema = by_id.get(doc_uri) or by_id.get(uri)
    if schema is None:
        name = Path(doc_uri).name or Path(ref.split("#", 1)[0]).name
        for key, candidate in by_id.items():
            if key.endswith("/" + name) or key.endswith(name):
                schema = candidate
                break
    if schema is None:
        raise KeyError(f"unresolved $ref {ref!r} from {base_id!r}")
    if fragment:
        return json_pointer(schema, fragment)
    return schema


def builtin_iter_errors(
    schema: Any,
    instance: Any,
    by_id: dict[str, Any],
    base_id: str,
    path: list[Any] | None = None,
) -> list[str]:
    location = list(path or [])
    if schema is True:
        return []
    if schema is False:
        return ["False schema does not allow any value"]
    if not isinstance(schema, dict):
        return []

    errors: list[str] = []
    current_id = schema.get("$id", base_id)
    if "$ref" in schema:
        target = resolve_ref(schema["$ref"], current_id, by_id)
        rest = {k: v for k, v in schema.items() if k != "$ref"}
        errors.extend(
            builtin_iter_errors(target, instance, by_id, current_id, location)
        )
        if rest:
            errors.extend(
                builtin_iter_errors(rest, instance, by_id, current_id, location)
            )
        return errors

    for sub in schema.get("allOf", []):
        errors.extend(builtin_iter_errors(sub, instance, by_id, current_id, location))

    if "if" in schema:
        if_errors = builtin_iter_errors(
            schema["if"], instance, by_id, current_id, location
        )
        if not if_errors and "then" in schema:
            errors.extend(
                builtin_iter_errors(
                    schema["then"], instance, by_id, current_id, location
                )
            )
        if if_errors and "else" in schema:
            errors.extend(
                builtin_iter_errors(
                    schema["else"], instance, by_id, current_id, location
                )
            )

    if "const" in schema and instance != schema["const"]:
        errors.append(f"{instance!r} was expected to be {schema['const']!r}")

    if "enum" in schema and instance not in schema["enum"]:
        errors.append(f"{instance!r} is not one of {schema['enum']!r}")

    expected = schema.get("type")
    if expected is not None:
        allowed = {expected} if isinstance(expected, str) else set(expected)
        if json_types(instance).isdisjoint(allowed):
            shown = JSON_TYPE_NAMES.get(type(instance), type(instance).__name__)
            errors.append(f"{instance!r} is not of type {expected!r} (got {shown})")

    if isinstance(instance, list):
        if "maxItems" in schema and len(instance) > schema["maxItems"]:
            errors.append(
                f"{instance!r} has too many items (maxItems {schema['maxItems']})"
            )
        if "minItems" in schema and len(instance) < schema["minItems"]:
            errors.append(
                f"{instance!r} has too few items (minItems {schema['minItems']})"
            )
        if "items" in schema:
            for index, item in enumerate(instance):
                errors.extend(
                    builtin_iter_errors(
                        schema["items"], item, by_id, current_id, location + [index]
                    )
                )

    if isinstance(instance, dict):
        required = schema.get("required", [])
        for key in required:
            if key not in instance:
                errors.append(f"{key!r} is a required property")
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties", True)
        extras = [key for key in instance if key not in properties]
        if additional is False and extras:
            extras_txt = ", ".join(repr(key) for key in extras)
            noun = "property" if len(extras) == 1 else "properties"
            verb = "was" if len(extras) == 1 else "were"
            errors.append(f"Additional {noun} {extras_txt} {verb} unexpected")
        for key, value in instance.items():
            if key in properties:
                errors.extend(
                    builtin_iter_errors(
                        properties[key], value, by_id, current_id, location + [key]
                    )
                )
            elif isinstance(additional, dict):
                errors.extend(
                    builtin_iter_errors(
                        additional, value, by_id, current_id, location + [key]
                    )
                )

    return errors


def load_schemas(schema_dir: Path) -> dict[str, Any]:
    by_id: dict[str, Any] = {}
    for path in sorted(schema_dir.glob("*.json")):
        schema = json.loads(path.read_text(encoding="utf-8"))
        by_id[schema["$id"]] = schema
        by_id[path.name] = schema
    return by_id


def validator_errors(
    schema: dict[str, Any],
    document: dict[str, Any],
    by_id: dict[str, Any],
    jsonschema_ok: bool,
    jsonschema_validator: Any,
) -> list[str]:
    if jsonschema_ok:
        return [error.message for error in jsonschema_validator.iter_errors(document)]
    return builtin_iter_errors(schema, document, by_id, schema.get("$id", ""))


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
    schema_dir = root / "schemas"
    by_id = load_schemas(schema_dir)
    report_schema = json.loads((schema_dir / "evaluation-report-v0.1.json").read_text())
    mill_schema = json.loads(
        (schema_dir / "mill-evaluation-response-v0.1.json").read_text()
    )
    jsonschema_ok = False
    report_js = None
    mill_js = None
    try:
        from jsonschema import Draft202012Validator
        from referencing import Registry, Resource

        registry: Registry = Registry()
        for path in sorted(schema_dir.glob("*.json")):
            schema = json.loads(path.read_text(encoding="utf-8"))
            Draft202012Validator.check_schema(schema)
            registry = registry.with_resource(
                schema["$id"], Resource.from_contents(schema)
            )
        report_js = Draft202012Validator(report_schema, registry=registry)
        mill_js = Draft202012Validator(mill_schema, registry=registry)
        jsonschema_ok = True
    except ImportError:
        pass

    report: dict[str, Any] = {
        "schema": "fidryn.evaluation-report/v0.1",
        "executionMode": "operative",
        "sourceTrust": "unauthenticated",
        "verificationMethod": "none",
        "assumptions": [],
        "coverage": None,
        "outcomeDocument": {
            "schema": "fidryn.outcome/v0.1",
            "module": "Review@0.1.0",
            "sourceSnapshot": "0" * 32,
            "query": "q",
            "asOf": {
                "validTime": "2033-01-01T00:00:00Z",
                "recordTime": "2033-01-01T00:00:00Z",
            },
            "modelBoundary": {"outsideScope": [], "admissibleCompletions": {}},
            "outcome": {
                "kind": "determinate",
                "value": {"kind": "bool", "data": True},
                "trace": "1" * 32,
                "ignoredOpenIssues": [],
            },
        },
    }
    probes: dict[str, Any] = {"baseline_report": report}
    mill = copy.deepcopy(report)
    mill["ok"] = True
    probes["mill_flattened_transport_ok"] = mill
    scenario = copy.deepcopy(report)
    scenario["executionMode"] = "scenario"
    del scenario["assumptions"]
    probes["scenario_missing_assumptions_field"] = scenario
    operative = copy.deepcopy(report)
    operative["assumptions"] = [{"id": "hypothesis", "payload": True}]
    probes["operative_with_nonempty_assumptions"] = operative
    finite = copy.deepcopy(report)
    finite["verificationMethod"] = "finiteReplay"
    probes["finite_replay_with_no_coverage_or_certificate"] = finite
    rows = []
    for name, document in probes.items():
        errors = validator_errors(
            report_schema, document, by_id, jsonschema_ok, report_js
        )
        rows.append({"name": name, "accepted_by_schema": not errors, "errors": errors})
    wrapper = {"ok": True, "report": copy.deepcopy(report)}
    wrapper_errors = validator_errors(
        mill_schema, wrapper, by_id, jsonschema_ok, mill_js
    )
    rows.append(
        {
            "name": "mill_transport_wrapper",
            "accepted_by_schema": not wrapper_errors,
            "errors": wrapper_errors,
        }
    )
    flattened_mill_errors = validator_errors(
        mill_schema, mill, by_id, jsonschema_ok, mill_js
    )
    rows.append(
        {
            "name": "mill_flattened_rejected_by_transport_schema",
            "accepted_by_schema": not flattened_mill_errors,
            "errors": flattened_mill_errors,
        }
    )
    print(
        json.dumps(
            {
                "mode": (
                    "actual JSON Schema evaluation; constructed inputs; no Rust execution"
                    if jsonschema_ok
                    else "built-in Draft 2020-12 subset; constructed inputs; no Rust execution"
                ),
                "probes": rows,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
