"""Exercise the real publication inventory gate with independent package mutations."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
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
