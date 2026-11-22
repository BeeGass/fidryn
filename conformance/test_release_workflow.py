"""Exercise publication gates with independent findings and package mutations."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMMIT = "a" * 40


def inventory_script() -> str:
    workflow = (ROOT / ".github/workflows/ci.yml").read_text()
    step = workflow.split("      - name: Verify package identity and checksums\n", 1)[1]
    step = step.split("      - name: Prepare release notes\n", 1)[0]
    return textwrap.dedent(step.split("        run: |\n", 1)[1])


def codeql_script() -> str:
    workflow = (ROOT / ".github/workflows/ci.yml").read_text()
    step = workflow.split(
        "      - name: Refuse unresolved CodeQL errors and high-severity findings\n", 1
    )[1]
    step = step.split("      - name: Verify package identity and checksums\n", 1)[0]
    return textwrap.dedent(step.split("        run: |\n", 1)[1])


class ReleaseCodeQL(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="fidryn-codeql-control-")
        self.root = Path(self.temporary.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for tool in ["bash", "jq"]:
            executable = shutil.which(tool)
            if executable is None:
                self.fail(f"release control requires {tool}")
            (self.bin / tool).symlink_to(executable)
        stub = self.bin / "gh"
        stub.write_text(
            f"#!{sys.executable}\n"
            + textwrap.dedent(
                """\
                import json
                import os
                import sys
                from pathlib import Path

                args = sys.argv[1:]
                endpoint = (
                    "repos/fixture/project/code-scanning/alerts?ref=refs/heads/main"
                    "&state=open&tool_name=CodeQL&per_page=100"
                )
                if args[:1] != ["api"] or endpoint not in args:
                    sys.exit("unexpected GitHub operation")
                if "--jq" not in args or args[args.index("--jq") + 1] != ".[]":
                    sys.exit("expected alert records")
                response = Path(os.environ["STUB_GH_RESPONSE"]).read_text()
                if os.environ["STUB_GH_RAW"] == "1":
                    sys.stdout.write(response)
                else:
                    pages = json.loads(response)
                    if "--paginate" not in args:
                        pages = pages[:1]
                    for page in pages:
                        for alert in page:
                            print(json.dumps(alert))
                sys.exit(int(os.environ["STUB_GH_EXIT"]))
                """
            )
        )
        stub.chmod(0o700)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def alert(severity: str = "warning", security: str | None = "low") -> dict[str, object]:
        return {"rule": {"severity": severity, "security_severity_level": security}}

    def run_gate(
        self,
        pages: list[list[dict[str, object]]],
        *,
        api_exit: int = 0,
        raw: str | None = None,
    ) -> subprocess.CompletedProcess[str]:
        response = self.root / "response.json"
        response.write_text(json.dumps(pages) if raw is None else raw)
        return subprocess.run(
            [str(self.bin / "bash"), "-e", "-o", "pipefail", "-c", codeql_script()],
            cwd=self.root,
            env={
                **os.environ,
                "PATH": str(self.bin),
                "GITHUB_REPOSITORY": "fixture/project",
                "STUB_GH_RESPONSE": str(response),
                "STUB_GH_RAW": "0" if raw is None else "1",
                "STUB_GH_EXIT": str(api_exit),
            },
            capture_output=True,
            check=False,
            text=True,
        )

    def test_no_findings_are_accepted(self) -> None:
        result = self.run_gate([[]])
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_benign_findings_are_accepted(self) -> None:
        result = self.run_gate(
            [
                [
                    self.alert(),
                    self.alert(severity="note", security="medium"),
                    self.alert(security=None),
                ]
            ]
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_high_and_critical_security_findings_are_rejected(self) -> None:
        for security in ["high", "critical"]:
            with self.subTest(security=security):
                result = self.run_gate([[self.alert(security=security)]])
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_error_findings_are_rejected_without_security_severity(self) -> None:
        result = self.run_gate([[self.alert(severity="error", security=None)]])
        self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_high_finding_after_first_full_page_is_rejected(self) -> None:
        result = self.run_gate([[self.alert()] * 100, [self.alert(security="high")]])
        self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_api_failure_is_rejected_after_benign_output(self) -> None:
        result = self.run_gate([[self.alert()]], api_exit=7)
        self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_invalid_json_is_rejected(self) -> None:
        for raw in ["not-json", '{"rule":', json.dumps(self.alert()) + "\n{invalid}"]:
            with self.subTest(raw=raw):
                result = self.run_gate([[]], raw=raw)
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_malformed_alert_records_are_rejected(self) -> None:
        records: list[dict[str, object]] = [
            {},
            {"rule": None},
            {"rule": []},
            {"rule": {"severity": "warning"}},
            {"rule": {"security_severity_level": None}},
            {"rule": {"severity": "warning", "security_severity_level": 42}},
        ]
        for record in records:
            with self.subTest(record=record):
                result = self.run_gate([[record]])
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_unknown_rule_or_security_severity_is_rejected(self) -> None:
        records = [
            self.alert(severity="unknown", security=None),
            self.alert(severity="Warning", security=None),
            self.alert(security="unknown"),
            self.alert(security="HIGH"),
        ]
        for record in records:
            with self.subTest(record=record):
                result = self.run_gate([[record]])
                self.assertNotEqual(result.returncode, 0, result.stdout)


class ReleaseInventory(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="fidryn-release-control-")
        self.root = Path(self.temporary.name)
        self.assets = self.root / "release-assets"
        self.assets.mkdir()
        self.hosts = [
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ]
        self.write_packages(self.hosts)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_packages(self, hosts: list[str]) -> None:
        for path in self.assets.iterdir():
            path.unlink()
        for host in hosts:
            stem = f"fidryn-0.2.0-{host}"
            archive = stem + (".zip" if "windows" in host else ".tar.gz")
            content = ("independent archive fixture " + host).encode()
            (self.assets / archive).write_bytes(content)
            (self.assets / (archive + ".sha256")).write_text(
                hashlib.sha256(content).hexdigest() + "  " + archive + "\n"
            )
            (self.assets / (archive + ".blake3")).write_text("b" * 64 + "  " + archive + "\n")
            (self.assets / (stem + ".manifest.json")).write_text(
                json.dumps(
                    {
                        "schema": "fidryn.package/v0.1",
                        "version": "0.2.0",
                        "sourceCommit": COMMIT,
                        "dirty": False,
                        "host": host,
                    }
                )
            )

    def accepted(self) -> bool:
        result = subprocess.run(
            ["bash", "-e", "-o", "pipefail", "-c", inventory_script()],
            cwd=self.root,
            env={
                **os.environ,
                "RELEASE_VERSION": "0.2.0",
                "GITHUB_SHA": COMMIT,
                "RUNNER_TEMP": str(self.root),
            },
            capture_output=True,
            check=False,
        )
        return result.returncode == 0

    def manifest(self) -> Path:
        return next(self.assets.glob("*.manifest.json"))

    def test_complete_native_inventory_is_accepted(self) -> None:
        self.assertTrue(self.accepted())

    def test_missing_archive_is_rejected(self) -> None:
        next(self.assets.glob("*.tar.gz")).unlink()
        self.assertFalse(self.accepted())

    def test_extra_archive_without_checksum_is_rejected(self) -> None:
        (self.assets / "unqualified.tar.gz").write_bytes(b"extra")
        self.assertFalse(self.accepted())

    def test_three_manifests_do_not_replace_three_platform_families(self) -> None:
        self.write_packages(
            ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "riscv64-unknown-linux-gnu"]
        )
        self.assertFalse(self.accepted())

    def test_wrong_version_commit_or_dirty_source_is_rejected(self) -> None:
        path = self.manifest()
        original = json.loads(path.read_text())
        for field, value in [("version", "0.1.0"), ("sourceCommit", "c" * 40), ("dirty", True)]:
            with self.subTest(field=field):
                path.write_text(json.dumps({**original, field: value}))
                self.assertFalse(self.accepted())

    def test_archive_tampering_is_rejected(self) -> None:
        next(self.assets.glob("*.tar.gz")).write_bytes(b"changed after packaging")
        self.assertFalse(self.accepted())

    def test_checksum_cannot_name_an_unrelated_path(self) -> None:
        next(self.assets.glob("*.sha256")).write_text("d" * 64 + "  ../unrelated\n")
        self.assertFalse(self.accepted())

    def test_additional_checksum_record_cannot_read_outside_assets(self) -> None:
        content = b"outside the release asset inventory"
        (self.root / "outside").write_bytes(content)
        path = next(self.assets.glob("*.sha256"))
        with path.open("a") as stream:
            stream.write(hashlib.sha256(content).hexdigest() + "  ../outside\n")
        self.assertFalse(self.accepted())

    def test_blake3_sidecar_cannot_contain_additional_records(self) -> None:
        path = next(self.assets.glob("*.blake3"))
        with path.open("a") as stream:
            stream.write("c" * 64 + "  ../outside\n")
        self.assertFalse(self.accepted())

    def test_manifest_host_must_match_its_archive_stem(self) -> None:
        path = self.manifest()
        document = json.loads(path.read_text())
        document["host"] = "other-unknown-linux-gnu"
        path.write_text(json.dumps(document))
        self.assertFalse(self.accepted())


if __name__ == "__main__":
    unittest.main()
