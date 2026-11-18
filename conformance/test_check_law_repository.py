"""Exercise `tools/check-law-repository.sh` on copies of the invented demo workspace.

The script checks every workspace at the top of a directory with a release build of
`fidryn`, or with the binary that `FIDRYN_BINARY` names. These tests give it the built
binary that `cargo xtask ci` names in `FIDRYN_BINARY`, so that it builds nothing, and
fail, and do not skip, without it. Each test copies `conformance/fixtures/workspace/demo`
into a temporary directory and gives the script a cache outside that directory, so
that nothing is written into the repository. The two tests of the build put a stand-in
`cargo`, and a stand-in `lease`, first on the search path, and run no build.

The script limits each run of the binary with `timeout` and `prlimit` where they
exist, and says which limit it does not apply where one is missing. The tests run it
on the search path of this process, with whatever of the two it holds, and on a search
path that holds neither. They need `sh`, and skip, saying so, where there is none.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

import blake3

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "tools" / "check-law-repository.sh"
DEMO = ROOT / "conformance" / "fixtures" / "workspace" / "demo"
# The directories of the system's own programs, without `lease` and `cargo`.
SYSTEM_PATH = "/usr/bin:/bin"
# What the script prints for each tool of the limits that the search path lacks.
NO_TIMEOUT = "there is no command timeout, so the limit of 600 s of time is not applied"
NO_PRLIMIT = (
    "there is no command prlimit, so the limits of 900 s of processor time and "
    "8589934592 bytes of address space are not applied"
)


def shell() -> str:
    """The `sh` the script runs with; every test is skipped where there is none."""
    found = shutil.which("sh")
    if found is None:
        raise unittest.SkipTest("the script is a shell script, and there is no command sh")
    return found


def without_the_limits(root: Path) -> str:
    """A search path that holds `sh` and the two other commands the script starts,
    `sed` and `mkdir`, and neither `timeout` nor `prlimit`."""
    bin_dir = root / "without-limits"
    bin_dir.mkdir()
    for tool in ("sh", "sed", "mkdir"):
        found = shutil.which(tool)
        if found is None:
            raise AssertionError(f"there is no command {tool}")
        (bin_dir / tool).symlink_to(found)
    return str(bin_dir)


def built_binary(test: unittest.TestCase) -> str:
    binary = os.environ.get("FIDRYN_BINARY")
    if not binary or not Path(binary).is_file():
        test.fail("FIDRYN_BINARY names no built fidryn; cargo xtask ci sets it")
    return binary


def version() -> str:
    """The version of the toolchain, which the demo workspace records: the test of the
    writer of `fidryn-driver` holds it to what the writer of this version writes."""
    manifest = json.loads((DEMO / "workspace.json").read_text(encoding="utf-8"))
    return str(manifest["toolchain"])


def tree(root: Path) -> list[str]:
    """Every path under `root`, relative to it, sorted."""
    return sorted(str(path.relative_to(root)) for path in root.rglob("*"))


class Run:
    """A temporary directory with a law directory `law/`, which holds a copy of the
    demo workspace, and a cache `cache/` beside it."""

    def __init__(self, test: unittest.TestCase) -> None:
        self.sh = shell()
        self.temporary = tempfile.TemporaryDirectory(prefix="fidryn-check-law-")
        test.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.law = self.root / "law"
        self.law.mkdir()
        shutil.copytree(DEMO, self.law / "demo")
        self.cache = self.root / "cache"

    def script(
        self, *args: str, env: dict[str, str] | None = None, cache: bool = True
    ) -> subprocess.CompletedProcess[str]:
        options = ["--cache-dir", str(self.cache)] if cache else []
        return subprocess.run(
            [self.sh, str(SCRIPT), *options, *args],
            cwd=self.root,
            env=env,
            capture_output=True,
            check=False,
            text=True,
            timeout=900,
        )


def environment(binary: str | None, **extra: str) -> dict[str, str]:
    """The environment of this process without the variables the script reads, with
    `FIDRYN_BINARY` set to `binary` when one is given."""
    env = {
        key: value
        for key, value in os.environ.items()
        if key not in ("FIDRYN_BINARY", "FIDRYN_CACHE_DIR", "CARGO_TARGET_DIR")
        and not key.startswith("FIDRYN_CHECK_")
    }
    if binary is not None:
        env["FIDRYN_BINARY"] = binary
    env.update(extra)
    return env


class CheckLawRepository(unittest.TestCase):
    def test_a_copy_of_the_demo_workspace_passes(self) -> None:
        run = Run(self)
        before = tree(run.law)
        result = run.script(str(run.law), env=environment(built_binary(self)))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(
            f"demo: the manifest records the toolchain {version()}, "
            "which is the version of the binary",
            result.stdout,
        )
        self.assertIn("modules: 5; from cache: 0; errors: 0; warnings: 0\n", result.stdout)
        self.assertIn("demo: the check passed\n", result.stdout)
        self.assertIn(
            "checked 1 workspace: every version is the binary's, and every check passed",
            result.stdout,
        )
        self.assertIn(
            "limits of each run of the binary: 600 s of time, 900 s of processor time, "
            "8589934592 bytes of address space",
            result.stdout,
        )
        self.assertEqual(tree(run.law), before, "the script wrote inside the directory")
        self.assertTrue((run.cache / "v1").is_dir(), "the cache holds no entry")

        again = run.script(str(run.law), env=environment(built_binary(self)))
        self.assertEqual(again.returncode, 0, again.stdout + again.stderr)
        self.assertIn("modules: 5; from cache: 5; errors: 0; warnings: 0\n", again.stdout)

    def test_a_manifest_of_another_version_fails_and_names_both_versions(self) -> None:
        run = Run(self)
        manifest = run.law / "demo" / "workspace.json"
        text = manifest.read_text(encoding="utf-8")
        recorded = f'"toolchain": "{version()}"'
        self.assertIn(recorded, text)
        manifest.write_text(text.replace(recorded, '"toolchain": "0.0.1"'), encoding="utf-8")
        result = run.script(str(run.law), env=environment(built_binary(self)))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(
            f"demo: the manifest records the toolchain 0.0.1, and the binary is {version()}",
            result.stdout,
        )
        self.assertIn("checked 1 workspace; failed: demo", result.stdout)

    def test_a_stale_binding_warns_and_does_not_fail(self) -> None:
        run = Run(self)
        notes = run.law / "notes"
        (notes / "zz" / "notes").mkdir(parents=True)
        digest = blake3.blake3((run.law / "demo" / "law.lock.json").read_bytes()).hexdigest()
        manifest = {
            "schema": "fidryn.workspace/v0.1",
            "name": "notes",
            "kind": "curated",
            "prefix": "/zz/notes",
            "toolchain": version(),
            "dependencies": [
                {"name": "demo", "path": "../demo", "release": "42-7", "lock_digest": digest}
            ],
        }
        (notes / "workspace.json").write_text(json.dumps(manifest, indent=2) + "\n")
        (notes / "zz" / "notes" / "a.fr").write_text(
            "edition 2\n"
            'module "/zz/notes/a" {\n'
            '  citation sizes -> "/zz/demo/t1/s2"\n'
            "  binding sizes_binding {\n"
            "    binds sizes\n"
            '    release "42-7"\n'
            f'    provision_hash "{"0" * 64}"\n'
            '    notes_hash "a396bac23ba81b30cb34dbf808cf06deeaeb4b87e010b66646faadc6accfc6ff"\n'
            "    pending\n"
            "  }\n"
            "}\n"
        )
        result = run.script(str(run.law), env=environment(built_binary(self)))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("W750 StaleBinding", result.stderr)
        self.assertIn("notes: the check passed with warnings, which do not fail it", result.stdout)
        self.assertIn("demo: the check passed\n", result.stdout)
        self.assertIn("checked 2 workspaces: every version is the binary's", result.stdout)

    def test_a_directory_with_no_workspace_at_its_top_is_refused(self) -> None:
        run = Run(self)
        empty = run.root / "empty"
        (empty / "nested" / "demo").mkdir(parents=True)
        (empty / "nested" / "demo" / "workspace.json").write_text("{}\n")
        result = run.script(str(empty), env=environment(built_binary(self)))
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("holds no workspace at its top", result.stderr)
        self.assertFalse(run.cache.exists(), "the cache was made for nothing to check")

    def test_an_exported_cdpath_changes_no_directory_the_script_names(self) -> None:
        run = Run(self)
        elsewhere = run.root / "elsewhere"
        (elsewhere / "law").mkdir(parents=True)
        result = run.script("law", env=environment(built_binary(self), CDPATH=str(elsewhere)))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"demo: fidryn check {run.law.resolve()}/demo --cache-dir", result.stdout)
        self.assertIn("checked 1 workspace: every version is the binary's", result.stdout)
        self.assertNotIn(str(elsewhere), result.stdout + result.stderr)

    def test_without_timeout_and_prlimit_the_check_runs_and_says_what_is_not_applied(
        self,
    ) -> None:
        verdicts: dict[bool, list[str]] = {}
        for limited in (True, False):
            run = Run(self)
            manifest = run.law / "demo" / "workspace.json"
            shutil.copytree(run.law / "demo", run.law / "other")
            other = run.law / "other" / "workspace.json"
            other.write_text(
                manifest.read_text(encoding="utf-8").replace(
                    f'"toolchain": "{version()}"', '"toolchain": "0.0.1"'
                ),
                encoding="utf-8",
            )
            env = environment(built_binary(self))
            if not limited:
                env["PATH"] = without_the_limits(run.root)
            result = run.script(str(run.law), env=env)
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            lines = result.stdout.replace(str(run.root), "ROOT").splitlines()
            if not limited:
                self.assertEqual(lines.count(NO_TIMEOUT), 1, result.stdout)
                self.assertEqual(lines.count(NO_PRLIMIT), 1, result.stdout)
            verdicts[limited] = [line for line in lines if line not in (NO_TIMEOUT, NO_PRLIMIT)]
        self.assertEqual(verdicts[True], verdicts[False])
        self.assertIn("demo: the check passed", verdicts[False])
        self.assertIn("checked 2 workspaces; failed: other", verdicts[False])

    def test_a_cache_inside_the_directory_is_refused_before_anything_is_written(self) -> None:
        run = Run(self)
        before = tree(run.law)
        for cache in (run.law / "demo" / ".cache", run.law, run.root):
            with self.subTest(cache=str(cache)):
                result = run.script(
                    "--cache-dir",
                    str(cache),
                    str(run.law),
                    env=environment(built_binary(self)),
                    cache=False,
                )
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("name a cache outside", result.stderr)
                self.assertEqual(tree(run.law), before)

    def test_the_cache_comes_from_the_environment_or_a_default_the_script_prints(self) -> None:
        run = Run(self)
        named = run.root / "named"
        result = run.script(
            str(run.law),
            env=environment(built_binary(self), FIDRYN_CACHE_DIR=str(named)),
            cache=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"cache: {named} (from FIDRYN_CACHE_DIR)", result.stdout)
        self.assertTrue((named / "v1").is_dir())

        home = run.root / "xdg"
        result = run.script(
            str(run.law), env=environment(built_binary(self), XDG_CACHE_HOME=str(home)), cache=False
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        default = home / "fidryn" / "law-repository"
        self.assertIn(
            f"cache: {default} (the default; --cache-dir or FIDRYN_CACHE_DIR names another)",
            result.stdout,
        )
        self.assertTrue((default / "v1").is_dir())

    def test_limits_of_each_run_can_be_set_and_are_kept(self) -> None:
        run = Run(self)
        result = run.script(
            str(run.law),
            env=environment(
                built_binary(self),
                FIDRYN_CHECK_TIMEOUT="1",
                FIDRYN_CHECK_CPU="900",
                FIDRYN_CHECK_ADDRESS_SPACE="1048576",
            ),
        )
        self.assertIn(
            "limits of each run of the binary: 1 s of time, 900 s of processor time, "
            "1048576 bytes of address space",
            result.stdout,
        )
        if shutil.which("prlimit") is not None:
            # The binary cannot start in 1 MiB of address space.
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertIn(
                "there is no command prlimit, so the limits of 900 s of processor time and "
                "1048576 bytes of address space are not applied",
                result.stdout,
            )
        for name in ("FIDRYN_CHECK_TIMEOUT", "FIDRYN_CHECK_CPU", "FIDRYN_CHECK_ADDRESS_SPACE"):
            for value in ("", "0", "ten", "-1"):
                with self.subTest(name=name, value=value):
                    refused = run.script(
                        str(run.law), env=environment(built_binary(self), **{name: value})
                    )
                    self.assertEqual(refused.returncode, 2, refused.stdout + refused.stderr)
                    self.assertIn(name, refused.stderr)


class Build(unittest.TestCase):
    """Without `FIDRYN_BINARY` the script builds the release binary: through `lease`
    when there is such a command, and with `cargo` directly when there is none."""

    def stand_ins(self, run: Run, lease_exit: int | None) -> Path:
        bin_dir = run.root / "bin"
        bin_dir.mkdir()
        record = run.root / "record"
        cargo = bin_dir / "cargo"
        cargo.write_text(
            "#!/bin/sh\n"
            f'printf "cargo %s in %s\\n" "$*" "$(pwd)" >> "{record}"\n'
            'mkdir -p "$CARGO_TARGET_DIR/release"\n'
            'cp "$STAND_IN_FIDRYN" "$CARGO_TARGET_DIR/release/fidryn"\n'
        )
        cargo.chmod(0o755)
        if lease_exit is not None:
            lease = bin_dir / "lease"
            lease.write_text(
                f'#!/bin/sh\nprintf "lease %s\\n" "$*" >> "{record}"\nexit {lease_exit}\n'
            )
            lease.chmod(0o755)
        return bin_dir

    def test_lease_that_fails_fails_the_script_and_nothing_is_built_without_it(self) -> None:
        run = Run(self)
        bin_dir = self.stand_ins(run, lease_exit=3)
        env = environment(
            None,
            PATH=f"{bin_dir}:{SYSTEM_PATH}",
            CARGO_TARGET_DIR=str(run.root / "target"),
            STAND_IN_FIDRYN=built_binary(self),
        )
        result = run.script(str(run.law), env=env)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn(
            "building the release binary through lease: lease run --cpus 16 --mem 25G "
            "--time 1h -- cargo build --release -p fidryn-cli --locked",
            result.stdout,
        )
        self.assertIn("the build through lease failed (exit 3)", result.stderr)
        self.assertEqual(
            (run.root / "record").read_text(),
            "lease run --cpus 16 --mem 25G --time 1h -- cargo build --release -p fidryn-cli "
            "--locked\n",
        )
        self.assertFalse((run.root / "target").exists())

    def test_without_lease_the_script_builds_with_cargo_and_says_so(self) -> None:
        if shutil.which("lease", path=SYSTEM_PATH) is not None:
            self.fail(f"a command lease stands in {SYSTEM_PATH}")
        run = Run(self)
        bin_dir = self.stand_ins(run, lease_exit=None)
        env = environment(
            None,
            PATH=f"{bin_dir}:{SYSTEM_PATH}",
            CARGO_TARGET_DIR=str(run.root / "target"),
            STAND_IN_FIDRYN=built_binary(self),
        )
        result = run.script(str(run.law), env=env)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(
            "building the release binary directly, since there is no command lease: "
            "cargo build --release -p fidryn-cli --locked",
            result.stdout,
        )
        self.assertEqual(
            (run.root / "record").read_text(),
            f"cargo build --release -p fidryn-cli --locked in {ROOT}\n",
        )
        binary = run.root / "target" / "release" / "fidryn"
        self.assertIn(f"binary: {binary}, fidryn {version()} (built)", result.stdout)
        self.assertIn("demo: the check passed\n", result.stdout)


if __name__ == "__main__":
    unittest.main()
