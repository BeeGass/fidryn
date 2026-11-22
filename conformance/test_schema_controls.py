"""Verify that conformance controls detect both directions of schema weakening."""

from __future__ import annotations

import contextlib
import io
import unittest
from unittest.mock import patch

import probe_outcome_schema
import schema_boundary_probes


class SchemaControls(unittest.TestCase):
    def test_outcome_permissive_validator_fails_negative_controls(self) -> None:
        _, failures = probe_outcome_schema.evaluate({})
        self.assertEqual(len(failures), 4)

    def test_outcome_reject_all_fails_positive_control(self) -> None:
        _, failures = probe_outcome_schema.evaluate({"not": {}})
        self.assertEqual(failures, ["valid_determinate"])

    def test_boundary_permissive_validator_fails(self) -> None:
        with (
            patch("sys.argv", ["schema_boundary_probes.py"]),
            patch.object(schema_boundary_probes, "validator_errors", return_value=[]),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(schema_boundary_probes.main(), 1)

    def test_boundary_reject_all_fails(self) -> None:
        with (
            patch("sys.argv", ["schema_boundary_probes.py"]),
            patch.object(schema_boundary_probes, "validator_errors", return_value=["rejected"]),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(schema_boundary_probes.main(), 1)
