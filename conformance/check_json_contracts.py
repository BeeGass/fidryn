#!/usr/bin/env python3
"""Validate Fidryn case records and manifests against the supplied schemas.

Requires: python -m pip install jsonschema (preferred). Falls back to a
built-in Draft 2020-12 subset checker if jsonschema is not installed.
Usage: python check_json_contracts.py [root] --output validation.json
Exit codes: 0 all checked documents valid; 1 validation failures; 2 setup/read error.
This does not validate artifact authenticity, Rust output, or legal content.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SCHEMA_FILES = {
    "fidryn.case-record/v0.1": "case-record-v0.1.json",
    "fidryn.source-manifest/v0.1": "source-manifest-v0.1.json",
}

JSON_TYPE_NAMES = {
    type(None): "null",
    bool: "boolean",
    int: "integer",
    float: "number",
    str: "string",
    list: "array",
    dict: "object",
}


def repo_root_from_script() -> Path:
    return Path(__file__).resolve().parent.parent


def json_types(instance: Any) -> set[str]:
    names = set()
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


def builtin_iter_errors(
    schema: Any, instance: Any, path: list[Any] | None = None
) -> list[dict[str, Any]]:
    """Tiny Draft 2020-12 subset: type, const, enum, required, properties, items, additionalProperties."""
    location = list(path or [])
    if schema is True:
        return []
    if schema is False:
        return [{"path": location, "message": "False schema does not allow any value"}]
    if not isinstance(schema, dict):
        return []

    errors: list[dict[str, Any]] = []
    if "const" in schema and instance != schema["const"]:
        errors.append(
            {
                "path": location,
                "message": f"{instance!r} was expected to be {schema['const']!r}",
            }
        )

    if "enum" in schema and instance not in schema["enum"]:
        errors.append(
            {
                "path": location,
                "message": f"{instance!r} is not one of {schema['enum']!r}",
            }
        )

    expected = schema.get("type")
    if expected is not None:
        allowed = {expected} if isinstance(expected, str) else set(expected)
        if json_types(instance).isdisjoint(allowed):
            shown = JSON_TYPE_NAMES.get(type(instance), type(instance).__name__)
            errors.append(
                {
                    "path": location,
                    "message": f"{instance!r} is not of type {expected!r} (got {shown})",
                }
            )

    if isinstance(instance, dict):
        required = schema.get("required", [])
        for key in required:
            if key not in instance:
                errors.append(
                    {
                        "path": location,
                        "message": f"{key!r} is a required property",
                    }
                )
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties", True)
        extras = [key for key in instance if key not in properties]
        if additional is False and extras:
            extras_txt = ", ".join(repr(key) for key in extras)
            noun = "property" if len(extras) == 1 else "properties"
            errors.append(
                {
                    "path": location,
                    "message": f"Additional {noun} {extras_txt} {'was' if len(extras) == 1 else 'were'} unexpected",
                }
            )
        for key, value in instance.items():
            if key in properties:
                errors.extend(
                    builtin_iter_errors(properties[key], value, location + [key])
                )
            elif isinstance(additional, dict):
                errors.extend(builtin_iter_errors(additional, value, location + [key]))

    if isinstance(instance, list) and "items" in schema:
        item_schema = schema["items"]
        for index, item in enumerate(instance):
            errors.extend(builtin_iter_errors(item_schema, item, location + [index]))

    return errors


def load_validators(root: Path) -> tuple[dict[str, Any], bool]:
    schemas: dict[str, Any] = {}
    for name, filename in SCHEMA_FILES.items():
        schemas[name] = json.loads(
            (root / "schemas" / filename).read_text(encoding="utf-8")
        )

    try:
        from jsonschema import Draft202012Validator
    except ImportError:
        for schema in schemas.values():
            if not isinstance(schema, dict):
                raise ValueError("schema document must be an object") from None
        return schemas, False

    validators = {}
    for name, schema in schemas.items():
        Draft202012Validator.check_schema(schema)
        validators[name] = Draft202012Validator(schema)
    return validators, True


def document_errors(
    validator: Any, value: dict[str, Any], *, jsonschema_ok: bool
) -> list[dict[str, Any]]:
    if jsonschema_ok:
        return [
            {"path": list(error.path), "message": error.message}
            for error in validator.iter_errors(value)
        ]
    return builtin_iter_errors(validator, value)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "root",
        type=Path,
        nargs="?",
        default=None,
        help="Fidryn repository root (default: parent of this script's directory)",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = (args.root or repo_root_from_script()).resolve()
    try:
        validators, jsonschema_ok = load_validators(root)
        if not jsonschema_ok:
            print(
                "jsonschema not installed; using built-in shape check. "
                "Install jsonschema for Draft 2020-12 validation.",
                file=sys.stderr,
            )
        results = []
        for base in (root / "examples", root / "prelude"):
            if not base.is_dir():
                continue
            for path in sorted(base.rglob("*.json")):
                value = json.loads(path.read_text(encoding="utf-8"))
                if not isinstance(value, dict) or value.get("schema") not in validators:
                    continue
                errors = document_errors(
                    validators[value["schema"]],
                    value,
                    jsonschema_ok=jsonschema_ok,
                )
                results.append(
                    {
                        "path": str(path.relative_to(root)),
                        "valid": not errors,
                        "errors": errors,
                    }
                )
        report = {
            "checked": len(results),
            "failed": sum(not row["valid"] for row in results),
            "documents": results,
        }
        text = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
        if args.output:
            args.output.write_text(text, encoding="utf-8")
        print(text, end="")
        return 1 if report["failed"] else 0
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"Cannot check documents: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
