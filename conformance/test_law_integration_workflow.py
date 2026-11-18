"""Hold `.github/workflows/law-integration.yml` and `law-repository.pin.json` to what
they are for.

The workflow checks the law repository that the pin file names: one job at the pinned
commit with the toolchain of the commit under test, one with the newest commit of each.
Its only trigger is a manual run, and a comment at its top names the two lines that
switch on the others. The tests read the workflow as YAML with the small reader below,
which knows the block mappings, block sequences, literal blocks, quoted and plain
scalars and flow collections the workflow is written in, and refuses anything else, so
that a typing mistake that changes what the file means fails here as well as in the
"Workflow lint" job of CI. They also run steps of the workflow: each written to a file
and run with `bash -e FILE`, as the runner runs a step that names no `shell`
(`bash -e {0}`). The step that reads the pin runs against the committed pin; the last
step of each job runs with stand-ins for `sh` and `git` on the search path, so that
nothing is built or checked.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "law-integration.yml"
PIN = ROOT / "law-repository.pin.json"
OTHER_WORKFLOWS = (ROOT / ".github" / "workflows" / "ci.yml", ROOT / ".github/workflows/codeql.yml")

KEY = re.compile(r"([A-Za-z_][A-Za-z0-9_.-]*):(?: (.*))?$")


class YamlError(ValueError):
    pass


def strip_comment(text: str) -> str:
    """`text` without its comment: from a `#` that begins it or follows a space,
    outside a quoted scalar."""
    quote = ""
    at = 0
    while at < len(text):
        char = text[at]
        if quote:
            if quote == '"' and char == "\\":
                at += 1
            elif char == quote:
                quote = ""
        elif char in "'\"" and (at == 0 or text[at - 1] in " [{,:"):
            quote = char
        elif char == "#" and (at == 0 or text[at - 1] == " "):
            return text[:at].rstrip()
        at += 1
    if quote:
        raise YamlError(f"an unclosed quotation mark in {text!r}")
    return text.rstrip()


def quoted(text: str) -> tuple[str, int]:
    """The scalar quoted at the start of `text`, and where it ends."""
    mark = text[0]
    out = []
    at = 1
    while at < len(text):
        char = text[at]
        if mark == "'" and char == "'":
            if text[at + 1 : at + 2] == "'":
                out.append("'")
                at += 2
                continue
            return "".join(out), at + 1
        if mark == '"' and char == '"':
            return "".join(out), at + 1
        if mark == '"' and char == "\\":
            escapes = {'"': '"', "\\": "\\", "n": "\n", "t": "\t"}
            following = text[at + 1 : at + 2]
            if following not in escapes:
                raise YamlError(f"an escape this reader does not know in {text!r}")
            out.append(escapes[following])
            at += 2
            continue
        out.append(char)
        at += 1
    raise YamlError(f"an unclosed quotation mark in {text!r}")


class Flow:
    """A flow collection or scalar: `[a, b]`, `{k: v}`."""

    def __init__(self, text: str) -> None:
        self.text = text
        self.at = 0

    def space(self) -> None:
        while self.at < len(self.text) and self.text[self.at] == " ":
            self.at += 1

    def value(self, stops: str) -> object:
        self.space()
        rest = self.text[self.at :]
        if rest.startswith("["):
            self.at += 1
            items: list[object] = []
            while not self.closes("]"):
                items.append(self.value(",]"))
                self.separator("]")
            return items
        if rest.startswith("{"):
            self.at += 1
            mapping: dict[str, object] = {}
            while not self.closes("}"):
                key = self.value(":")
                if not isinstance(key, str) or key in mapping:
                    raise YamlError(f"a key that is no new string in {self.text!r}")
                self.space()
                if not self.text.startswith(": ", self.at):
                    raise YamlError(f"a key without `: ` in {self.text!r}")
                self.at += 2
                mapping[key] = self.value(",}")
                self.separator("}")
            return mapping
        if rest[:1] in ("'", '"'):
            scalar, length = quoted(rest)
            self.at += length
            return scalar
        end = self.at
        while end < len(self.text) and self.text[end] not in stops + "[]{},":
            end += 1
        scalar = self.text[self.at : end].strip()
        if not scalar:
            raise YamlError(f"an empty flow scalar in {self.text!r}")
        self.at = end
        return scalar

    def closes(self, mark: str) -> bool:
        self.space()
        if self.text.startswith(mark, self.at):
            self.at += 1
            return True
        return False

    def separator(self, mark: str) -> None:
        self.space()
        if self.text.startswith(",", self.at):
            self.at += 1
        elif not self.text.startswith(mark, self.at):
            raise YamlError(f"`,` or `{mark}` expected in {self.text!r}")


def scalar(text: str) -> object:
    """The value written on one line after a key or a `- `."""
    if text[0] in "[{":
        flow = Flow(text)
        value = flow.value("")
        flow.space()
        if flow.at != len(text):
            raise YamlError(f"text after a flow collection in {text!r}")
        return value
    if text[0] in "'\"":
        value, length = quoted(text)
        if length != len(text):
            raise YamlError(f"text after a quoted scalar in {text!r}")
        return value
    if text[0] in "&*!|>%@`" or ": " in text:
        raise YamlError(f"a scalar this reader does not know: {text!r}")
    return text


class Reader:
    """Block YAML, read line by line by indentation."""

    def __init__(self, text: str) -> None:
        self.lines = text.split("\n")
        self.at = 0

    def indent(self) -> int | None:
        """The indentation of the next line that is neither blank nor a comment, or
        `None` at the end."""
        while self.at < len(self.lines):
            line = self.lines[self.at]
            stripped = line.strip()
            if stripped and not stripped.startswith("#"):
                if "\t" in line[: len(line) - len(line.lstrip())]:
                    raise YamlError(f"line {self.at + 1} is indented with a tab")
                return len(line) - len(line.lstrip(" "))
            self.at += 1
        return None

    def node(self, indent: int) -> object:
        text = self.lines[self.at][indent:]
        if text == "-" or text.startswith("- "):
            return self.sequence(indent)
        return self.mapping(indent)

    def mapping(self, indent: int) -> dict[str, object]:
        out: dict[str, object] = {}
        while (here := self.indent()) is not None and here >= indent:
            if here > indent:
                raise YamlError(f"line {self.at + 1} is indented more than its mapping")
            text = strip_comment(self.lines[self.at][indent:])
            match = KEY.fullmatch(text)
            if match is None:
                raise YamlError(f"line {self.at + 1} is no `key: value`: {text!r}")
            key, rest = match.group(1), match.group(2) or ""
            if key in out:
                raise YamlError(f"line {self.at + 1} repeats the key `{key}`")
            self.at += 1
            out[key] = self.value(rest, indent)
        return out

    def value(self, rest: str, indent: int) -> object:
        if rest in ("|", "|-"):
            return self.literal(indent, rest == "|")
        if rest:
            return scalar(rest)
        here = self.indent()
        if here is not None and here > indent:
            return self.node(here)
        return None

    def sequence(self, indent: int) -> list[object]:
        out: list[object] = []
        while (here := self.indent()) is not None and here == indent:
            text = self.lines[self.at][indent:]
            if not (text == "-" or text.startswith("- ")):
                raise YamlError(f"line {self.at + 1} ends a sequence where none may end")
            rest = strip_comment(text[2:])
            if KEY.fullmatch(rest):
                self.lines[self.at] = " " * (indent + 2) + text[2:]
                out.append(self.mapping(indent + 2))
            else:
                self.at += 1
                out.append(self.value(rest, indent))
        if here is not None and here > indent:
            raise YamlError(f"line {self.at + 1} is indented more than its sequence")
        return out

    def literal(self, indent: int, keep: bool) -> str:
        lines: list[str] = []
        block: int | None = None
        while self.at < len(self.lines):
            line = self.lines[self.at]
            if line.strip():
                here = len(line) - len(line.lstrip(" "))
                if block is None:
                    if here <= indent:
                        break
                    block = here
                elif here < block:
                    break
                lines.append(line[block:])
            else:
                lines.append("")
            self.at += 1
        while lines and not lines[-1]:
            lines.pop()
        if not lines:
            raise YamlError(f"an empty literal block before line {self.at + 1}")
        return "\n".join(lines) + ("\n" if keep else "")


def read_yaml(text: str) -> dict[str, object]:
    reader = Reader(text)
    if reader.indent() != 0:
        raise YamlError("the document does not begin at the left margin")
    document = reader.mapping(0)
    if reader.indent() is not None:
        raise YamlError(f"line {reader.at + 1} stands outside the document")
    return document


def mapping(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        raise AssertionError(f"a mapping expected, not {value!r}")
    return value


def sequence(value: object) -> list[object]:
    if not isinstance(value, list):
        raise AssertionError(f"a sequence expected, not {value!r}")
    return value


# The jobs of the workflow, by id: the name each has, and the event on which it never
# runs once the other triggers are on.
JOBS = {
    "pinned": ("Law repository at its pin", "schedule"),
    "drift": ("Drift of the newest law repository", "pull_request"),
}
SWITCHES = ("  # pull_request: ", "  # schedule: ")


class Reading(unittest.TestCase):
    def test_the_reader_reads_what_the_workflow_is_written_in_and_refuses_the_rest(self) -> None:
        text = (
            "# a comment\n"
            "name: Example # a comment after a value\n"
            "on:\n"
            "  workflow_dispatch:\n"
            "  push: {branches: [main, dev]}\n"
            "jobs:\n"
            "  one:\n"
            "    if: github.event_name != 'schedule'\n"
            "    steps:\n"
            "      - uses: actions/checkout@abc # v7\n"
            "        with:\n"
            "          path: 'it''s'\n"
            "      - run: |\n"
            '          echo "#1"\n'
            "\n"
            "          # a line of the script\n"
            '      - schedule: [{cron: "1 2 * * *"}]\n'
        )
        self.assertEqual(
            read_yaml(text),
            {
                "name": "Example",
                "on": {"workflow_dispatch": None, "push": {"branches": ["main", "dev"]}},
                "jobs": {
                    "one": {
                        "if": "github.event_name != 'schedule'",
                        "steps": [
                            {"uses": "actions/checkout@abc", "with": {"path": "it's"}},
                            {"run": 'echo "#1"\n\n# a line of the script\n'},
                            {"schedule": [{"cron": "1 2 * * *"}]},
                        ],
                    }
                },
            },
        )
        for broken in (
            "on:\n  push:\n workflow_dispatch:\n",
            "on:\n  workflow_dispatch\n",
            "name: a\nname: b\n",
            "on: {branches: [main, dev}\n",
            "steps:\n  - run: x\n    - run: y\n",
            "name: 'open\n",
            "name: a: b\n",
            "  name: a\n",
            "name: &anchor a\n",
            "run: |\nname: x\n",
            "on:\n\tpush:\n",
        ):
            with self.subTest(broken=broken), self.assertRaises(YamlError):
                read_yaml(broken)


class Workflow(unittest.TestCase):
    def setUp(self) -> None:
        self.text = WORKFLOW.read_text(encoding="utf-8")
        self.document = read_yaml(self.text)

    def jobs(self) -> dict[str, object]:
        return mapping(self.document["jobs"])

    def steps(self, job: str) -> list[dict[str, object]]:
        return [mapping(step) for step in sequence(mapping(self.jobs()[job])["steps"])]

    def test_its_only_trigger_is_a_manual_run(self) -> None:
        self.assertEqual(self.document["on"], {"workflow_dispatch": None})
        self.assertEqual(self.document["permissions"], {"contents": "read"})

    def test_the_two_lines_its_comment_names_switch_on_the_other_triggers(self) -> None:
        comment = self.text.split("\nname:", 1)[0]
        self.assertTrue(all(line.startswith("#") for line in comment.splitlines()))
        self.assertIn('remove the "# " at the start of the two lines', comment)
        lines = self.text.splitlines(keepends=True)
        under = lines.index("  workflow_dispatch:\n")
        self.assertEqual(
            [
                line.startswith(switch)
                for line, switch in zip(lines[under + 1 : under + 3], SWITCHES, strict=True)
            ],
            [True, True],
            "the two lines under `workflow_dispatch:` are not the two switches",
        )
        for switch in SWITCHES:
            self.assertEqual(self.text.count(switch), 1, switch)
        switched = self.text
        for switch in SWITCHES:
            switched = switched.replace(switch, switch.replace("# ", "", 1))
        triggers = mapping(read_yaml(switched)["on"])
        self.assertEqual(list(triggers), ["workflow_dispatch", "pull_request", "schedule"])
        self.assertEqual(triggers["pull_request"], {"branches": ["main", "dev"]})
        schedule = sequence(triggers["schedule"])
        self.assertEqual(len(schedule), 1)
        cron = mapping(schedule[0])["cron"]
        self.assertIsInstance(cron, str)
        self.assertEqual(len(str(cron).split()), 5, cron)

    def test_it_has_the_two_jobs_and_each_reads_the_pin_before_anything_else(self) -> None:
        self.assertEqual(list(self.jobs()), list(JOBS))
        for job, (name, never) in JOBS.items():
            with self.subTest(job=job):
                settings = mapping(self.jobs()[job])
                self.assertEqual(settings["name"], name)
                self.assertEqual(settings["if"], f"github.event_name != '{never}'")
                steps = self.steps(job)
                self.assertTrue(str(steps[0]["uses"]).startswith("actions/checkout@"))
                self.assertNotIn("with", steps[0])
                self.assertEqual(steps[1]["id"], "pin")
                self.assertIn("law-repository.pin.json", str(steps[1]["run"]))
                for step in steps[2:]:
                    self.assertEqual(step.get("if"), "steps.pin.outputs.check == 'true'", step)
                runs = "".join(str(step.get("run", "")) for step in steps)
                self.assertIn("sh tools/check-law-repository.sh", runs)

    def run_step(
        self, step: dict[str, object], directory: Path, env: dict[str, str]
    ) -> subprocess.CompletedProcess[str]:
        """`step` run as the runner runs a step that names no `shell`: `bash -e FILE`."""
        bash = shutil.which("bash")
        if bash is None:
            self.fail("the steps of the workflow run with bash")
        script = directory / "step.sh"
        script.write_text(str(step["run"]))
        return subprocess.run(
            [bash, "-e", str(script)],
            cwd=ROOT,
            env={**os.environ, **env},
            capture_output=True,
            check=False,
            text=True,
        )

    def test_while_the_pin_names_no_repository_each_job_says_so_and_ends(self) -> None:
        if shutil.which("jq") is None:
            self.fail("the step that reads the pin needs jq")
        for job in JOBS:
            with self.subTest(job=job), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "output"
                result = self.run_step(
                    self.steps(job)[1], Path(directory), {"GITHUB_OUTPUT": str(output)}
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("names no law repository", result.stdout)
                self.assertEqual(output.read_text(), "check=false\n")

    def test_the_pinned_job_checks_the_pinned_commit_and_the_drift_job_the_newest(
        self,
    ) -> None:
        for job, ref in (("pinned", "${{ steps.pin.outputs.commit }}"), ("drift", None)):
            with self.subTest(job=job):
                steps = self.steps(job)
                checkout = steps[2]
                self.assertTrue(str(checkout["uses"]).startswith("actions/checkout@"))
                settings = mapping(checkout["with"])
                self.assertEqual(settings["repository"], "${{ steps.pin.outputs.repository }}")
                self.assertEqual(settings.get("ref"), ref)
                command = str(steps[-1]["run"])
                checked = re.findall(
                    r'sh tools/check-law-repository\.sh --cache-dir "\$RUNNER_TEMP/fidryn-cache" '
                    r"([^\s;]+)",
                    command,
                )
                self.assertEqual(checked, [settings["path"]], command)
                self.assertEqual(
                    [s for s in steps if "tools/check-law-repository.sh" in str(s.get("run"))],
                    [steps[-1]],
                )

    def test_the_last_step_of_each_job_fails_when_the_check_fails(self) -> None:
        newest = "b" * 40
        for job in JOBS:
            for check_exit, pinned in ((0, newest), (0, "a" * 40), (1, newest)):
                with (
                    self.subTest(job=job, check_exit=check_exit, pinned=pinned),
                    tempfile.TemporaryDirectory() as name,
                ):
                    directory = Path(name)
                    stand_ins = directory / "bin"
                    stand_ins.mkdir()
                    record = directory / "record"
                    for tool, out, code in (("sh", "", check_exit), ("git", newest, 0)):
                        stand_in = stand_ins / tool
                        stand_in.write_text(
                            "#!/bin/bash\n"
                            f'printf "{tool} %s\\n" "$*" >> "{record}"\n'
                            f'printf "%s" "{out}"\n'
                            f"exit {code}\n"
                        )
                        stand_in.chmod(0o755)
                    summary = directory / "summary"
                    result = self.run_step(
                        self.steps(job)[-1],
                        directory,
                        {
                            "PATH": f"{stand_ins}:{os.environ.get('PATH', '')}",
                            "RUNNER_TEMP": str(directory),
                            "GITHUB_SHA": "c" * 40,
                            "GITHUB_STEP_SUMMARY": str(summary),
                            "PINNED": pinned,
                        },
                    )
                    self.assertEqual(result.returncode, check_exit, result.stdout + result.stderr)
                    self.assertIn(
                        f"sh tools/check-law-repository.sh --cache-dir {directory}/fidryn-cache "
                        "law-repository\n",
                        record.read_text(),
                    )
                    if job != "drift":
                        continue
                    written = summary.read_text()
                    if check_exit:
                        self.assertTrue(
                            written.startswith(f"Drift: the newest law repository, {newest}")
                        )
                        self.assertIn("Drift:", result.stderr)
                    elif pinned == newest:
                        self.assertIn("is the pinned one", written)
                    else:
                        self.assertIn(f"the pin is at {pinned}", written)

    def test_every_action_it_uses_is_one_the_other_workflows_use_at_the_same_commit(
        self,
    ) -> None:
        known: set[str] = set()
        for path in OTHER_WORKFLOWS:
            known.update(re.findall(r"uses: (\S+@[0-9a-f]{40})", path.read_text(encoding="utf-8")))
        used = [str(step["uses"]) for job in JOBS for step in self.steps(job) if "uses" in step]
        self.assertTrue(used)
        for action in used:
            self.assertIn(action, known)
        for line in self.text.splitlines():
            if "uses: " in line:
                pinned = line.split("uses: ", 1)[1]
                self.assertTrue(
                    any(other.startswith(pinned.split(" # ")[0]) for other in known), line
                )
                self.assertRegex(pinned, r"@[0-9a-f]{40} # v\d")


class Pin(unittest.TestCase):
    def test_the_pin_names_no_repository_and_no_commit(self) -> None:
        self.assertEqual(PIN.read_bytes(), b'{"repository": "", "commit": ""}\n')


if __name__ == "__main__":
    unittest.main()
