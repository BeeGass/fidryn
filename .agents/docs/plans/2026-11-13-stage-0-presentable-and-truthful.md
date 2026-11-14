# Stage 0: Presentable and Truthful 0.2.1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make release 0.2.1 say only true things about itself, repair the example modules that fail on typing alone, return the project to the MIT License, and make CI keep all of it true.

**Architecture:** Two new checks live in the existing `xtask` crate and run inside `cargo xtask ci`: one records and enforces which `.fr` files pass the checker and marks the rest as drafts wherever a document names them, one verifies documentation links, complete module samples and every command a document shows. Thirty-four example modules are repaired by typing-only edits. Documentation is then corrected against those checks and against a new set of tests that pin the interpreter's known limits. No interpreter behaviour changes.

**Tech Stack:** Rust 1.98.1 (edition 2024), the `xtask` crate (clap, anyhow, pulldown-cmark, serde_json), `fidryn-cli` and `fidryn-core` as libraries, the site generator in `xtask/src/site/`.

**Spec:** `.agents/docs/specs/2026-11-13-law-monorepo-design.md`, section 8. Sections 2, 12, 13 and 14 give the evidence, constraints and settled decisions this plan relies on.

## Owner decisions of 2026-11-13

The owner reviewed the first version of this plan and decided the following. Each is recorded in the spec's sections 13 and 14.

1. Execution is subagent-driven: one implementer and one reviewer per task.
2. PR #3 is merged as 0.2.0 without this work. Stage 0 follows as release 0.2.1.
3. The examples that fail the checker are labelled, and those that fail for reasons other than date arithmetic are repaired by hand. Planning found 34 that can be repaired without changing legal content (Task 2).
4. `.cargo/config.toml` stays as it is.
5. The roadmap is not published until Stage 1 lands. No reader document states the program's destination.
6. Each task is one signed local commit on `stage0/presentable-and-truthful`. Nothing is pushed.
7. The project is licensed under the MIT License (Task 11).

## Global Constraints

- No change to evaluation semantics. Parser, checker and evaluator behaviour stay as they are at `1a70382`.
- Work on branch `stage0/presentable-and-truthful`, created from `dev`. `dev` is the head of PR #3. The owner merges PR #3 first; this branch reaches `dev` afterwards and is promoted as 0.2.1.
- Pushing, merging PR #3 and changing GitHub settings are the owner's actions. Do not push without the owner's go-ahead.
- `LICENSE`, `LICENSING.md` and licence metadata change only in Task 11.
- Do not edit `.cargo/config.toml`.
- Do not create `docs/roadmap.md`, and do not describe the program's destination or stages in any reader document.
- Prose names the release series, `0.2`, never a patch version. Full version numbers appear only in `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` and the release procedure in `docs/releases.md`.
- An example module or case record is edited only by the repairs listed in Task 2. No rule, threshold, citation or proposition is added, removed or reworded anywhere in `examples/`.
- Commits are GPG-signed (already configured), focused on one task, and do not mention tooling authorship. Commit subjects are imperative sentences without a type prefix, matching `git log` (for example "Require code-scanning findings to pass before promotion").
- Signing uses the owner's hardware token. If a commit fails to sign, stop and report. Never pass `--no-gpg-sign` and never change the signing configuration.
- In this machine's shell, `test` is a user-defined function that switches git branches, not the POSIX command. Never run `test ...` in a shell command; use `[[ ... ]]`.
- The gate is `cargo xtask ci --fmt`. On this machine run it through the scheduler: `lease run --cpus 16 --mem 32G --time 2h -- cargo xtask ci --fmt`. Single named tests may run directly.
- Toolchain: Rust 1.98.1, edition 2024. Python tooling runs through `uv`.
- `docs/*.md` is canonical. Files under `site/` listed as generated in `site/README.md` are never edited by hand; run `cargo xtask site` after changing any guide and commit the result with the change.
- Agent material stays under `.agents/`. Where it disagrees with the human documents, the human documents win.
- No emojis in code, comments or documents.
- Every path written into a generated or committed file uses `/` separators on every host. CI runs on Linux, macOS and Windows.
- Counts for checking work. There are 114 `.fr` files under `examples/`, `packages/`, `prelude/` and `tests/programs/`. At the baseline 20 pass `fidryn check` and 94 do not. After Task 2, 54 pass and 60 do not: 41 of the 101 under `examples/`, and 8 of the 50 state modules. Of the 60 that still fail, 59 need date arithmetic and one (`examples/illinois/bipa/bipa.fr`) declares a money result it never states.
- A verified reference diff for Task 2 is kept outside version control at `.agents/scratchpad/2026-11-13-stage0-repairs/examples-repair.patch`. It is a convenience. The rules in Task 2 decide what is allowed, not the patch.

## Review Focus

Five conditions the spec implies and a person will hit. Each has a test in the task that owns the code.

1. A `.fr` file is added, removed or renamed without regenerating the status. The check fails and names the file. (Task 1, `check_fails_until_update_and_names_files_added_or_removed`)
2. The checks run on Windows. Recorded paths and reported link targets use `/`. (Task 1, `survey_lists_fr_files_sorted_with_forward_slashes_and_records_error_codes`; Task 3, `markdown_files_are_sorted_with_forward_slashes`)
3. A document's status markers are missing, duplicated or reversed. The update refuses and names the document; it never appends a second region. (Task 5, `replace_region_requires_exactly_one_ordered_pair_of_markers`)
4. A document shows a command in a block that is not tagged, that uses a shell operator, or that is meant to fail. The first two are reported; the third is checked against the exit status the block declares. (Task 3, `commands_come_from_sh_blocks_with_the_exit_status_they_declare`, `a_block_that_shows_a_command_must_say_whether_it_is_checked`)
5. A module that does not pass is named by a path relative to its document, or a module that now passes still carries a draft mark. Both are corrected by the update and reported by the check. (Task 5, `draft_marks_follow_the_recorded_status`)

Two further conditions keep their tests in Task 3: link forms that must not be reported (`links_resolve_relative_to_the_file_and_ignore_urls_fragments_and_code`) and CRLF input (`complete_modules_are_fenced_blocks_that_start_with_module_and_crlf_is_normalised`).

## File Structure

| Path | Responsibility |
| --- | --- |
| `xtask/src/examples.rs` (new) | Survey `.fr` files, render and enforce `examples/status.json`, `examples/STATUS.md`, the status regions and the draft marks |
| `xtask/src/docs_check.rs` (new) | Markdown link resolution, complete-module samples, and the commands documents show |
| `xtask/src/workspace.rs` | Gains `fidryn_binary()`, shared by the schema probes and the docs check |
| `xtask/src/ci.rs`, `xtask/src/main.rs` | Wire the two checks into `ci` and expose them as subcommands |
| `tests/integration/repaired_examples.rs` (new) | The 34 repaired modules pass, and the repair idioms behave as documented at run time |
| `tests/integration/known_limits.rs` (new) | Executable statements of what the 0.2 interpreter does not do |
| `docs/internal/` (new directory) | The six internal ledgers, with an index |
| `examples/status.json`, `examples/STATUS.md` (generated) | The recorded check result for every `.fr` file |
| `SECURITY.md` (new) | How to report a vulnerability |
| `LICENSE`, `LICENSING.md`, `deny.toml`, every `Cargo.toml` | MIT terms and matching metadata |

---

### Before Task 1: branch and record the design

- [ ] **Step 1: Create the branch**

```bash
git switch -c stage0/presentable-and-truthful dev
```

- [ ] **Step 2: Commit the approved design and this plan**

```bash
git add .agents/docs/specs/2026-11-13-law-monorepo-design.md .agents/docs/plans/2026-11-13-stage-0-presentable-and-truthful.md
git commit -m "Record the program design and the Stage 0 plan"
```

---

### Task 1: The examples status check

**Files:**
- Create: `xtask/src/examples.rs`
- Create: `xtask/tests/examples_cli.rs`
- Create (generated by the tool): `examples/status.json`, `examples/STATUS.md`
- Modify: `xtask/src/main.rs` (declare `mod examples`, add the `Examples` subcommand)
- Modify: `xtask/src/ci.rs:77` (call the check after the site check)

**Interfaces:**
- Consumes: `fidryn_cli::compile_module(path: &Path) -> Result<(CoreModule, SourceManifest), Vec<Diagnostic>>`; `Diagnostic.code.as_str()`; `crate::workspace::workspace_root()`.
- Produces:

```rust
pub const ROOTS: [&str; 4] = ["examples", "packages", "prelude", "tests/programs"];
pub const STATUS_JSON: &str = "examples/status.json";
pub const STATUS_MD: &str = "examples/STATUS.md";

#[derive(Debug, clap::Args)]
pub struct ExamplesArgs {
    /// Rewrite the recorded status instead of checking it
    #[arg(long)]
    pub update: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStatus {
    /// Relative to the workspace root, `/` separators on every host.
    pub path: String,
    pub ok: bool,
    /// Error code to count. Empty when `ok`.
    pub codes: BTreeMap<String, u32>,
}

pub fn fr_files(root: &Path) -> Result<Vec<String>>;
pub fn survey(root: &Path) -> Result<Vec<FileStatus>>;
pub fn status_json(files: &[FileStatus]) -> String;
pub fn status_markdown(files: &[FileStatus]) -> String;
pub fn check(root: &Path) -> Result<()>;
pub fn update(root: &Path) -> Result<()>;
pub fn run(args: ExamplesArgs) -> Result<()>;
```

- [ ] **Step 1: Write the failing unit tests** at the bottom of `xtask/src/examples.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    const HEADER: &str = "    jurisdiction Test\n    source_snapshot \"scratch\"\n    effective_at 2026-01-01\n    recorded_at 2026-01-01T00:00:00Z\n    outside_scope { everything_else }\n";

    fn module(name: &str, body: &str) -> String {
        format!("module {name} version \"0.1.0\" {{\n{HEADER}    {body}\n}}\n")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("fidryn-xtask-examples-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn ok(path: &str) -> FileStatus {
        FileStatus { path: path.into(), ok: true, codes: BTreeMap::new() }
    }

    fn fails(path: &str, codes: &[(&str, u32)]) -> FileStatus {
        FileStatus {
            path: path.into(),
            ok: false,
            codes: codes.iter().map(|(code, n)| ((*code).to_owned(), *n)).collect(),
        }
    }

    #[test]
    fn survey_lists_fr_files_sorted_with_forward_slashes_and_records_error_codes() {
        let root = scratch("survey");
        write(&root, "examples/b/nested/bad.fr", &module("Scratch.Bad", "query q() -> Int { return true }"));
        write(&root, "examples/a/ok.fr", &module("Scratch.Ok", "query q() -> Int { return 7 }"));
        write(&root, "tests/programs/notes.txt", "not a module");
        assert_eq!(
            survey(&root).unwrap(),
            [ok("examples/a/ok.fr"), fails("examples/b/nested/bad.fr", &[("E210", 1)])]
        );
    }

    #[test]
    fn status_json_is_stable_and_omits_codes_for_passing_files() {
        let files = [ok("examples/a/ok.fr"), fails("examples/b/bad.fr", &[("E210", 2), ("E100", 1)])];
        assert_eq!(
            status_json(&files),
            "{\n  \"schema\": \"fidryn.examples-status/v0.1\",\n  \"files\": [\n    {\n      \"path\": \"examples/a/ok.fr\",\n      \"check\": \"ok\"\n    },\n    {\n      \"path\": \"examples/b/bad.fr\",\n      \"check\": \"fails\",\n      \"codes\": {\n        \"E100\": 1,\n        \"E210\": 2\n      }\n    }\n  ]\n}\n"
        );
    }

    #[test]
    fn status_markdown_has_a_count_line_and_one_row_per_file() {
        let md = status_markdown(&[ok("examples/a/ok.fr"), fails("examples/b/bad.fr", &[("E210", 2), ("E100", 1)])]);
        assert!(md.starts_with("# Example status\n"), "{md}");
        assert!(md.contains("Passing `fidryn check`: 1 of 2.\n"), "{md}");
        assert!(md.contains("| `examples/a/ok.fr` | ok | |\n"), "{md}");
        assert!(md.contains("| `examples/b/bad.fr` | fails | E100 (1), E210 (2) |\n"), "{md}");
    }

    #[test]
    fn check_fails_until_update_and_names_files_added_or_removed() {
        let root = scratch("check");
        write(&root, "examples/a/ok.fr", &module("Scratch.Ok", "query q() -> Int { return 7 }"));
        let missing = check(&root).unwrap_err().to_string();
        assert!(missing.contains("examples/status.json"), "{missing}");
        update(&root).unwrap();
        check(&root).unwrap();

        write(&root, "examples/c/new.fr", &module("Scratch.New", "query q() -> Int { return 7 }"));
        let added = check(&root).unwrap_err().to_string();
        assert!(added.contains("examples/c/new.fr"), "{added}");
        assert!(added.contains("cargo xtask examples --update"), "{added}");

        update(&root).unwrap();
        fs::remove_file(root.join("examples/a/ok.fr")).unwrap();
        let removed = check(&root).unwrap_err().to_string();
        assert!(removed.contains("examples/a/ok.fr"), "{removed}");
    }

    #[test]
    fn a_file_that_is_not_utf8_is_an_error_naming_the_file() {
        let root = scratch("bytes");
        fs::create_dir_all(root.join("examples")).unwrap();
        fs::write(root.join("examples/bytes.fr"), b"module X version \"0\" {\n \xff\xfe }\n").unwrap();
        let error = survey(&root).unwrap_err().to_string();
        assert!(error.contains("examples/bytes.fr"), "{error}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask --offline examples::tests`
Expected: compile error, `survey`, `status_json`, `status_markdown`, `check` and `update` not found.

- [ ] **Step 3: Implement the items in the Interfaces block in `xtask/src/examples.rs`**

Decisions the tests do not fix:
- `fr_files` walks each entry of `ROOTS` that exists, recursively, keeps files whose extension is `fr`, joins path components with `/`, and sorts by byte order.
- `survey` reads each file's bytes first and returns an error naming the path if they are not UTF-8. It then calls `fidryn_cli::compile_module(&root.join(path))`. `Ok` is `ok: true`. `Err(diagnostics)` is `ok: false` with one count per diagnostic whose code has `Severity::Error`.
- `status_json` builds a `serde_json::Value` with keys in the order `schema`, `files`, and per file `path`, `check`, `codes` (omitted when `ok`), then `serde_json::to_string_pretty` plus a trailing newline. The workspace enables serde_json's `preserve_order`.
- `status_markdown` writes, in order: `# Example status`, a blank line, the sentence `Generated by \`cargo xtask examples --update\`. Do not edit by hand.`, a blank line, `Passing \`fidryn check\`: {ok} of {total}.`, a blank line, the table header `| File | Result | Diagnostics |` with its separator row, then one row per file.
- `check` compares fresh `status_json` and `status_markdown` output with the committed files. On any difference it fails with a message that lists each path whose entry was added, removed or changed and ends with ``run `cargo xtask examples --update` ``. A missing status file is reported by its path.
- `update` writes both files. `run` dispatches on `args.update`, using `workspace_root()`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline examples::tests`
Expected: 5 passed.

- [ ] **Step 5: Register the subcommand and generate the status**

In `xtask/src/main.rs` add `mod examples;`, the variant `/// Check or regenerate the recorded check status of every .fr file` `Examples(examples::ExamplesArgs)`, and the arm `Command::Examples(args) => examples::run(args)`.

Run: `cargo run -p xtask --offline -- examples --update`
Then: `sed -n '5p' examples/STATUS.md`
Expected: `Passing \`fidryn check\`: 20 of 114.`

- [ ] **Step 6: Write the command-line test** in `xtask/tests/examples_cli.rs`

```rust
//! The recorded example status matches a fresh survey of the tree.

use std::process::Command;

#[test]
fn examples_check_passes_on_the_committed_tree() {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("examples")
        .output()
        .expect("spawn xtask examples");
    assert!(
        out.status.success(),
        "run `cargo xtask examples --update`\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
```

Run: `cargo test -p xtask --offline --test examples_cli`
Expected: 1 passed.

- [ ] **Step 7: Add the check to the gate**

In `xtask/src/ci.rs`, directly after `crate::site::check(&crate::workspace::workspace_root())?;`, add `crate::examples::check(&crate::workspace::workspace_root())?;`.

Run: `cargo build -p xtask --offline`
Expected: builds without warnings.

- [ ] **Step 8: Commit**

```bash
git add xtask/src/examples.rs xtask/src/main.rs xtask/src/ci.rs xtask/tests/examples_cli.rs examples/status.json examples/STATUS.md
git commit -m "Record and enforce which example modules pass the checker"
```

---

### Task 2: Repair the examples that fail on typing alone

**Files:**
- Modify: the 34 modules listed in `REPAIRED` below
- Modify: seven case records (Step 3, repair E)
- Create: `tests/integration/repaired_examples.rs`
- Modify: `tests/integration/main.rs` (add `mod repaired_examples;`)
- Regenerate: `examples/status.json`, `examples/STATUS.md`

**Interfaces:**
- Consumes: `fidryn_cli::{CaseInput, RunRequest, compile_module, run_report_text}`; `cargo xtask examples` from Task 1.
- Produces: 54 of 114 modules pass `fidryn check`. Later tasks rely on that count and on the eight state modules that pass.

Why these files fail. The 0.2 checker rejects a query that declares a result type and whose block ends in a `require`, because the block has no typed value. It also rejects a proposition argument that no declaration binds, and a `require` whose condition is a call to another query. None of these needs a language feature. The five repairs below give each construct a type and change nothing else. Repair A is already the examples' own idiom: `income_from_returns` in `examples/us-federal/accredited-investor/accredited-investor.fr` ends its requires with `return true`.

Six files look repairable and are not. Leave them exactly as they are.

| File | Why it stays a draft |
| --- | --- |
| `examples/illinois/bipa/bipa.fr` | `private_right_of_action_damages` declares `Money<USD>` and the module states no amount. Supplying one would add legal content. |
| `examples/us-federal/removal/removal.fr` | `thirty_day_window` adds a duration to a time. |
| `examples/us-federal/medicare/medicare.fr`, `examples/us-federal/naturalization/naturalization.fr` | Each adds a number of years to a time. |
| `examples/states/alaska/security-deposit.fr`, `examples/states/georgia/dispossessory.fr` | A scenario shares its name with another declaration. Behind that error both add durations to times. |

- [ ] **Step 1: Write the failing tests** in `tests/integration/repaired_examples.rs`

```rust
//! Example modules repaired in 0.2.1 by typing-only edits.
//!
//! The first test is the list. The others pin what the repaired constructs
//! do at run time, so the guides can describe them.

use fidryn_cli::{CaseInput, RunRequest, compile_module, run_report_text};
use serde_json::Value;
use std::path::PathBuf;

const REPAIRED: [&str; 34] = [
    "examples/california/abc-test/abc-test.fr",
    "examples/california/final-pay/final-pay.fr",
    "examples/california/lemon/lemon.fr",
    "examples/california/noncompete/noncompete.fr",
    "examples/delaware/dgcl-220/dgcl-220.fr",
    "examples/everyday/final-paycheck/final-paycheck.fr",
    "examples/everyday/minimum-wage/minimum-wage.fr",
    "examples/states/arkansas/homestead.fr",
    "examples/states/colorado/famli.fr",
    "examples/states/florida/homestead.fr",
    "examples/states/idaho/noncompete.fr",
    "examples/states/kansas/homestead.fr",
    "examples/states/louisiana/community-property.fr",
    "examples/states/michigan/no-fault-pip.fr",
    "examples/states/south-dakota/wage-payment.fr",
    "examples/texas/homestead/homestead.fr",
    "examples/ucc/battle-of-forms/battle-of-forms.fr",
    "examples/us-federal/accredited-investor/accredited-investor.fr",
    "examples/us-federal/ada-service-animal/ada-service-animal.fr",
    "examples/us-federal/bankruptcy-stay/bankruptcy-stay.fr",
    "examples/us-federal/diversity/diversity.fr",
    "examples/us-federal/dmca/dmca.fr",
    "examples/us-federal/fair-housing/fair-housing.fr",
    "examples/us-federal/flsa/flsa.fr",
    "examples/us-federal/fmla/fmla.fr",
    "examples/us-federal/garnishment/garnishment.fr",
    "examples/us-federal/hearsay/hearsay.fr",
    "examples/us-federal/means-test/means-test.fr",
    "examples/us-federal/pslf/pslf.fr",
    "examples/us-federal/scra/scra.fr",
    "examples/us-federal/ssa-fra/ssa-fra.fr",
    "examples/us-federal/ssi/ssi.fr",
    "examples/us-federal/tcpa/tcpa.fr",
    "examples/us-federal/unlawful-presence/unlawful-presence.fr",
];

const AT: &str = "2026-09-17T19:00:00Z";
const NO_FACTS: &str =
    r#"{"schema":"fidryn.case-record/v0.1","facts":{},"admissibleCompletions":{}}"#;
const FINAL_PAY: &str = "examples/california/final-pay/final-pay.fr";
const BATTLE_OF_FORMS: &str = "examples/ucc/battle-of-forms/battle-of-forms.fr";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The outcome object of a run, or the text `fidryn run` writes to stderr.
fn outcome(module: &str, query: &str, case: &str, args: &[String]) -> Result<Value, String> {
    let path = root().join(module);
    let text = run_report_text(&RunRequest {
        path: &path,
        query,
        case: CaseInput::Json(case),
        valid_at: AT,
        known_at: AT,
        args,
        scenario: false,
    })?;
    let report: Value = serde_json::from_str(&text).expect("report is JSON");
    Ok(report["outcomeDocument"]["outcome"].clone())
}

fn final_pay_case(determinations: &str) -> String {
    format!(
        r#"{{"schema":"fidryn.case-record/v0.1","facts":{{"discharged":true}},"determinations":{determinations},"admissibleCompletions":{{}}}}"#
    )
}

fn willful(established: bool) -> String {
    final_pay_case(&format!(
        r#"[{{"issue":"WillfulFailureToPay(Employer)","protocol":"Determine","established":{established},"decider":"Court"}}]"#
    ))
}

#[test]
fn every_repaired_module_passes_the_checker() {
    for module in REPAIRED {
        if let Err(diagnostics) = compile_module(&root().join(module)) {
            panic!("{module}: {diagnostics:?}");
        }
    }
}

#[test]
fn a_judgment_query_suspends_until_a_determination_is_on_file() {
    let out = outcome(FINAL_PAY, "waiting_time_penalty", &final_pay_case("[]"), &[]).unwrap();
    assert_eq!(out["kind"], "suspended");
    assert_eq!(out["requests"][0]["kind"], "needJudgment");
    assert_eq!(out["requests"][0]["issue"]["predicate"], "WillfulFailureToPay");
}

#[test]
fn an_established_determination_makes_the_judgment_query_true() {
    let out = outcome(FINAL_PAY, "waiting_time_penalty", &willful(true), &[]).unwrap();
    assert_eq!(out["kind"], "determinate");
    assert_eq!(out["value"]["data"], true);
}

#[test]
fn a_negative_determination_suspends_and_never_answers_false() {
    let out = outcome(FINAL_PAY, "waiting_time_penalty", &willful(false), &[]).unwrap();
    assert_eq!(out["kind"], "suspended");
    assert_eq!(out["requests"][0]["payload"], "requirement failed");
}

#[test]
fn a_query_with_a_bound_parameter_asks_for_its_argument() {
    assert_eq!(
        outcome(BATTLE_OF_FORMS, "material_alteration", NO_FACTS, &[]).unwrap_err(),
        "InvalidInput: invalid input: missing query argument `term`"
    );
    let args = ["term=arbitration".to_owned()];
    let out = outcome(BATTLE_OF_FORMS, "material_alteration", NO_FACTS, &args).unwrap();
    assert_eq!(out["kind"], "suspended");
    assert_eq!(out["requests"][0]["kind"], "needJudgment");
    assert_eq!(out["requests"][0]["issue"]["arguments"][0]["String"], "arbitration");
}

#[test]
fn money_facts_in_the_repaired_case_records_are_accepted() {
    for (module, query, case) in [
        (
            "examples/us-federal/accredited-investor/accredited-investor.fr",
            "individual_income_met",
            "examples/us-federal/accredited-investor/cases/income-met.json",
        ),
        (
            "examples/us-federal/diversity/diversity.fr",
            "amount_in_controversy_met",
            "examples/us-federal/diversity/cases/amount-met-diversity-open.json",
        ),
        (
            "examples/us-federal/fair-housing/fair-housing.fr",
            "extra_deposit_not_required",
            "examples/us-federal/fair-housing/cases/no-extra-deposit.json",
        ),
        (
            "examples/us-federal/garnishment/garnishment.fr",
            "within_twenty_five_percent",
            "examples/us-federal/garnishment/cases/under-cap.json",
        ),
        (
            "examples/us-federal/means-test/means-test.fr",
            "above_median",
            "examples/us-federal/means-test/cases/above-median.json",
        ),
        (
            "examples/us-federal/ssi/ssi.fr",
            "resources_within_cap",
            "examples/us-federal/ssi/cases/under-cap.json",
        ),
        (
            "examples/us-federal/tcpa/tcpa.fr",
            "at_least_statutory_floor",
            "examples/us-federal/tcpa/cases/statutory-floor.json",
        ),
    ] {
        let record = std::fs::read_to_string(root().join(case)).expect(case);
        let out = outcome(module, query, &record, &[])
            .unwrap_or_else(|error| panic!("{module}: {error}"));
        assert_eq!(out["kind"], "determinate", "{module}");
        assert_eq!(out["value"]["data"], true, "{module}");
    }
}

#[test]
fn a_module_that_fails_the_checker_cannot_be_run() {
    let error = outcome(
        "examples/us-federal/removal/removal.fr",
        "thirty_day_window",
        NO_FACTS,
        &[],
    )
    .unwrap_err();
    assert!(error.contains("E210"), "{error}");
}
```

Add `mod repaired_examples;` to `tests/integration/main.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p fidryn-integration-tests --offline repaired_examples`
Expected: `a_module_that_fails_the_checker_cannot_be_run` passes. The other six fail: the modules do not compile yet.

- [ ] **Step 3: Apply the five repairs**

These are the only edits allowed under `examples/`.

**A. A typed result.** In each of the 34 modules, every query that declares `-> Bool` and whose `goal Evaluate { ... }` block ends with a `require` statement gains one line, `return true`, as the last statement of that block, indented like the `require` above it. This adds 51 lines. The query then answers `true` once every requirement holds and suspends otherwise.

```
        goal Evaluate {
            require determined WillfulFailureToPay(Employer)
                using Determine
            return true
        }
```

Before adding the line to a query, read its name and the proposition it requires. Both must state the same thing in the same direction (`serious_health_condition` requires `SeriousHealthCondition`). Planning found no query whose name negates its proposition. If you find one, do not add the line: stop and report it.

**B. A typed parameter for a free variable.** Seven query headers gain the parameter their proposition argument already uses:

| File | Header after the repair |
| --- | --- |
| `examples/ucc/battle-of-forms/battle-of-forms.fr` | `query material_alteration(term: String)` |
| `examples/us-federal/dmca/dmca.fr` | `query counter_notice_good_faith(notice: RecordItem)` |
| `examples/us-federal/garnishment/garnishment.fr` | `query support_order_priority(order: RecordItem)` |
| `examples/us-federal/hearsay/hearsay.fr` | `query is_hearsay(text: String)` and `query business_records_exception(text: String)` |
| `examples/us-federal/diversity/diversity.fr` | `query complete_diversity(parties: FiniteSet<LegalPerson>)` and `query diversity_jurisdiction(amount: Money<USD>, parties: FiniteSet<LegalPerson>)` |

**C. A condition written out instead of a call to another query.** Two `require` lines:

| File | Before | After |
| --- | --- | --- |
| `examples/us-federal/accredited-investor/accredited-investor.fr` | `require individual_income_met(year1, year2)` | `require year1 > income_threshold() and year2 > income_threshold()` |
| `examples/us-federal/diversity/diversity.fr` | `require amount_in_controversy_met(amount)` | `require amount > amount_threshold()` |

Each replacement is the body of the query it replaces, copied from the same file. Leave the called query in place.

**D. The record type an observation already uses.** `examples/states/florida/homestead.fr` uses `ResidenceRecord` without declaring it. Insert, immediately before `observation OccupancyRecord(record: ResidenceRecord)` and followed by one blank line, the declaration that `examples/states/arkansas/homestead.fr` and `examples/states/kansas/homestead.fr` already carry:

```
    record_type ResidenceRecord {
        person: NaturalPerson
        property: RealProperty
    }
```

**E. Money facts in the form the runtime accepts.** In these seven case records, each fact written as the string `"USD(N)"` becomes the typed value below, with `N` copied character for character. Change nothing else in the file and keep two-space indentation.

`examples/us-federal/accredited-investor/cases/income-met.json`, `examples/us-federal/diversity/cases/amount-met-diversity-open.json`, `examples/us-federal/fair-housing/cases/no-extra-deposit.json`, `examples/us-federal/garnishment/cases/under-cap.json`, `examples/us-federal/means-test/cases/above-median.json`, `examples/us-federal/ssi/cases/under-cap.json`, `examples/us-federal/tcpa/cases/statutory-floor.json`

```json
    "resources": {
      "kind": "ctor",
      "data": {
        "name": "USD",
        "fields": {
          "_0": {
            "kind": "decimal",
            "data": "1500.00"
          }
        }
      }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p fidryn-integration-tests --offline repaired_examples`
Expected: 7 passed.

- [ ] **Step 5: Confirm the edits are exactly the repairs**

Run: `git diff --shortstat -- examples ':!examples/status.json' ':!examples/STATUS.md'`
Expected: `41 files changed, 197 insertions(+), 20 deletions(-)`.
Run: `git diff -U0 -- 'examples/**/*.fr' | grep -c '^+ *return true$'`
Expected: `51`.
Run: `git diff -U0 -- 'examples/**/*.fr' | grep -c '^-[^-]'`
Expected: `9` (the seven headers and two `require` lines replaced).

If a count differs, find the difference before going on. An extra changed line is a change this task does not allow.

- [ ] **Step 6: Record the new status**

Run: `cargo run -p xtask --offline -- examples`
Expected: fails, naming the 34 modules whose recorded status no longer matches.
Run: `cargo run -p xtask --offline -- examples --update`
Run: `sed -n '5p' examples/STATUS.md`
Expected: ``Passing `fidryn check`: 54 of 114.``
Run: `cargo run -p xtask --offline -- examples`
Expected: passes.

- [ ] **Step 7: Commit**

```bash
git add tests/integration/repaired_examples.rs tests/integration/main.rs examples
git commit -m "Repair the example modules that fail on typing alone"
```

---

### Task 3: The documentation check

**Files:**
- Create: `xtask/src/docs_check.rs`
- Create: `xtask/tests/docs_cli.rs`
- Modify: `xtask/Cargo.toml` (add `fidryn-core.workspace = true`), `Cargo.lock`
- Modify: `xtask/src/workspace.rs:142-161` (extract `fidryn_binary`)
- Modify: `xtask/src/main.rs`, `xtask/src/ci.rs`

**Interfaces:**
- Consumes: `fidryn_cli::compile_source(src: &str, manifest: &SourceManifest) -> Result<CoreModule, Vec<Diagnostic>>`; `fidryn_core::SourceManifest::default()`; `pulldown_cmark::Parser`.
- Produces:

```rust
// xtask/src/workspace.rs
/// Build the debug `fidryn` binary and return its path.
pub fn fidryn_binary() -> Result<PathBuf>;

// xtask/src/docs_check.rs
pub const MARKDOWN_ROOTS: [&str; 9] = [
    "README.md", "LICENSING.md", "CHANGELOG.md", "SECURITY.md",
    "docs", "examples", "packages", "conformance", "site/README.md",
];

#[derive(Debug, clap::Args)]
pub struct DocsArgs {
    /// Check links and module samples only; do not build or run the binary
    #[arg(long)]
    pub skip_commands: bool,
}

pub fn markdown_files(root: &Path) -> Result<Vec<String>>;
pub fn broken_links(root: &Path, file: &str, markdown: &str) -> Vec<String>;
pub fn complete_modules(markdown: &str) -> Vec<(usize, String)>;
pub fn module_problems(file: &str, markdown: &str) -> Vec<String>;

/// One `fidryn` invocation shown in a checked block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocCommand {
    /// 1-based line of the block's opening fence.
    pub line: usize,
    /// The words after `fidryn`.
    pub args: Vec<String>,
    /// The exit status the block declares: 0 for `sh`, N for `sh exit=N`.
    pub exit: i32,
}

/// Documents whose command blocks are not yet tagged. Task 6 empties it.
pub const COMMAND_RULE_PENDING: &[&str] = &["docs/examples.md"];

pub fn doc_commands(markdown: &str) -> Vec<DocCommand>;
pub fn command_block_problems(file: &str, markdown: &str) -> Vec<String>;
pub fn check_static(root: &Path) -> Result<()>;
pub fn check_commands(root: &Path, fidryn: &Path) -> Result<()>;
pub fn run(args: DocsArgs) -> Result<()>;
```

- [ ] **Step 1: Write the failing unit tests** at the bottom of `xtask/src/docs_check.rs`

Reuse the `HEADER`, `module`, `scratch` and `write` helpers from Task 1's test module by copying them into this test module with the scratch prefix `fidryn-xtask-docs-`.

```rust
#[test]
fn markdown_files_are_sorted_with_forward_slashes() {
    let root = scratch("files");
    write(&root, "README.md", "x");
    write(&root, "docs/internal/b.md", "x");
    write(&root, "docs/a.md", "x");
    write(&root, "docs/notes.txt", "x");
    write(&root, "site/README.md", "x");
    write(&root, "site/docs/generated.md", "x");
    assert_eq!(
        markdown_files(&root).unwrap(),
        ["README.md", "docs/a.md", "docs/internal/b.md", "site/README.md"]
    );
}

#[test]
fn links_resolve_relative_to_the_file_and_ignore_urls_fragments_and_code() {
    let root = scratch("links");
    write(&root, "docs/a.md", "x");
    write(&root, "examples/trust/x.fr", "x");
    let md = "[ok](a.md) [frag](a.md#part) [dir](../examples/trust/) [up](../docs/a.md) [ref]\n\
              [web](https://example.com/x.md) [mail](mailto:a@b.c) [anchor](#here) [site](/docs/cli)\n\
              `[code](missing-in-code.md)`\n\n```\n[fenced](missing-in-fence.md)\n```\n\n\
              [gone](missing.md) ![img](missing.png) [sub](internal/missing.md#x)\n\n\
              [ref]: missing-ref.md\n";
    assert_eq!(
        broken_links(&root, "docs/b.md", md),
        [
            "docs/b.md: link target `missing-ref.md` does not exist",
            "docs/b.md: link target `missing.md` does not exist",
            "docs/b.md: link target `missing.png` does not exist",
            "docs/b.md: link target `internal/missing.md#x` does not exist",
        ]
    );
}

#[test]
fn complete_modules_are_fenced_blocks_that_start_with_module_and_crlf_is_normalised() {
    let md = "intro\r\n\r\n```\r\nmodule A version \"1\" {\r\n}\r\n```\r\n\r\n```text\r\nmodule NotChecked version \"1\" {\r\n```\r\n\r\n```\r\nquery q() -> Int { return 7 }\r\n```\r\n";
    assert_eq!(complete_modules(md), [(3, "module A version \"1\" {\n}\n".to_owned())]);
}

#[test]
fn a_module_that_does_not_check_is_reported_with_file_line_and_code() {
    let bad = format!("text\n\n```\n{}```\n", module("Docs.Bad", "query q() -> Int { return true }"));
    let problems = module_problems("docs/x.md", &bad);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with("docs/x.md:3: module does not pass `fidryn check`: E210"),
        "{}",
        problems[0]
    );
    let good = format!("```\n{}```\n", module("Docs.Ok", "query q() -> Int { return 7 }"));
    assert!(module_problems("docs/x.md", &good).is_empty());
}

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn commands_come_from_sh_blocks_with_the_exit_status_they_declare() {
    let md = "```sh\nfidryn check a.fr\nfidryn run b.fr \\\n  --query q \\\n  --arg 'note=two words'\ncargo test\nfidryn ui --no-open\n```\n\n```sh exit=1\nfidryn check broken.fr\n```\n\n```text\nfidryn fmt PATH\n```\n";
    assert_eq!(
        doc_commands(md),
        [
            DocCommand { line: 1, args: words(&["check", "a.fr"]), exit: 0 },
            DocCommand {
                line: 1,
                args: words(&["run", "b.fr", "--query", "q", "--arg", "note=two words"]),
                exit: 0,
            },
            DocCommand { line: 10, args: words(&["check", "broken.fr"]), exit: 1 },
        ]
    );
}

#[test]
fn a_block_that_shows_a_command_must_say_whether_it_is_checked() {
    let md = "```\nfidryn check untagged.fr\n```\n\n```sh\nfidryn run a.fr --query q > out.json\n```\n\n```text\nfidryn check PATH\n```\n\n```\nmodule A version \"1\" {\n}\n```\n\n```console\n$ cargo test\n```\n";
    assert_eq!(
        command_block_problems("docs/x.md", md),
        [
            "docs/x.md:1: block shows a `fidryn` command; tag it `sh` to have the check run it or `text` to show it unchecked",
            "docs/x.md:5: `fidryn run a.fr --query q > out.json` uses the shell operator `>`; the check runs `fidryn` directly, so split the block or tag it `text`",
        ]
    );
    assert!(command_block_problems("docs/examples.md", md).is_empty());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask --offline docs_check::tests`
Expected: compile error, the functions are not found.

- [ ] **Step 3: Implement `xtask/src/docs_check.rs` and `fidryn_binary`**

Decisions the tests do not fix:
- `markdown_files`: each entry of `MARKDOWN_ROOTS` that exists. A file entry is taken as is. A directory entry is walked recursively for `*.md`. Nothing under `site/` other than `site/README.md` is included. Sorted by byte order.
- `broken_links`: parse with `pulldown_cmark::Parser::new_ext(md, Options::ENABLE_TABLES)`. Take the destination of every link and image start tag. Skip destinations that are empty, start with `#` or `/`, or carry a URL scheme. Strip the fragment and any query, resolve against the directory of `file` under `root`, and report when neither a file nor a directory exists there. Report in document order, with the destination exactly as written.
- `complete_modules`: a fenced block is examined when its info string is empty or `fidryn`, and its first non-blank line starts with `module `. Line endings are normalised to `\n`. The number is the 1-based line of the opening fence. An author opts a block out with the info string `text`.
- `module_problems`: compile each complete module with `compile_source(src, &SourceManifest::default())`. On error, report `{file}:{line}: module does not pass \`fidryn check\`: ` followed by the diagnostics' `Display` text joined with `; `.
- `doc_commands`: only fenced blocks whose info string's first word is `sh`. A second word `exit=N` sets the expected exit status; anything else in the info string is ignored. A line ending in `\` continues on the next line. Words are split on ASCII whitespace; single or double quotes group a word and are removed; there is no escape processing. Keep commands whose first word is `fidryn` and whose second is not `ui` (the server does not exit); return the words after `fidryn`. Lines that start with anything else are ignored.
- `command_block_problems`: empty for a document listed in `COMMAND_RULE_PENDING`. Otherwise two problems are reported, in document order. First, a fenced block whose info string's first word is neither `sh` nor `text` and that contains a line whose first word is `fidryn`. Second, inside an `sh` block, a `fidryn` command with a word that is one of `>`, `>>`, `<`, `|`, `&&`, `||` or `;`; the message quotes the joined command and the first such operator. The line is that of the opening fence.
- `check_static`: for every Markdown file, collect `broken_links`, `module_problems` and `command_block_problems`; fail with all problems, one per line.
- `check_commands`: for every Markdown file and every `DocCommand` in it, run `fidryn` with those arguments from `root` and compare the exit status with `exit` (a process ended by a signal counts as `-1`). Fail listing each mismatch as ``{file}:{line}: `fidryn {args}` exited with status {actual}, expected {expected}`` followed by the first line the command wrote to stderr.
- `fidryn_binary`: move the build and path computation out of `run_schema_probes` (`cargo build -p fidryn-cli --locked --offline`, then `target_directory/debug/fidryn{EXE_SUFFIX}`) and have `run_schema_probes` call it.
- `run`: `check_static`, then unless `skip_commands`, `check_commands(root, &fidryn_binary()?)`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline docs_check::tests`
Expected: 6 passed.

- [ ] **Step 5: Register the subcommand, then fix what it finds**

In `xtask/src/main.rs` add `mod docs_check;`, the variant `/// Check documentation links, module samples and the commands documents show` `Docs(docs_check::DocsArgs)`, and its arm.

Run: `cargo run -p xtask --offline -- docs --skip-commands`
Expected: it fails, listing every untagged command block outside `docs/examples.md`. A scan at planning time found no broken relative link in the 28 Markdown files, and both complete modules pass. If the check reports a link, fix it in the document that contains it so it points at the file that exists. Do not delete a link to silence it.

Tag each reported block. This step changes info strings only, not what a document says.

- `sh`: the commands run as written from the repository root, exit 0, need no network, and write nothing into the repository. `fidryn fmt` prints to standard output and may be tagged `sh`.
- `sh exit=1`: the document shows a failure on purpose. Two blocks in `docs/cli.md` do: `fidryn check tests/diagnostics/e310-prop-as-guard.fr`, and `fidryn verify examples/trust/bryan-revocable-trust.fr --property TrusteeContinuity`, which reports `unknown`.
- `text`: a synopsis with placeholders (`README.md`, `docs/ARCHITECTURE.md`, `docs/cases-and-time.md`, `docs/mill.md`); program output; a block that needs shell redirection or a path outside the repository (the `run ... > /tmp/fidryn-outcome.json` and `explain` pair in `docs/cli.md`, the block in `docs/getting-started.md` that Task 8 rewrites); and `fidryn file`.

Run: `cargo run -p xtask --offline -- docs`
Expected: `ok`. If a block tagged `sh` fails, read what the command printed and choose the tag that describes it truthfully. Do not edit a module or a case record to make a command pass.

- [ ] **Step 6: Write the command-line test** in `xtask/tests/docs_cli.rs`

```rust
//! Documentation links resolve and complete module samples pass the checker.

use std::process::Command;

#[test]
fn docs_static_check_passes_on_the_committed_tree() {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["docs", "--skip-commands"])
        .output()
        .expect("spawn xtask docs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
```

Run: `cargo test -p xtask --offline --test docs_cli`
Expected: 1 passed.

- [ ] **Step 7: Add the check to the gate**

In `xtask/src/ci.rs`, leave `run_schema_probes()?;` as it is; it now calls `fidryn_binary` internally. After the examples check from Task 1 add:

```rust
let root = crate::workspace::workspace_root();
crate::docs_check::check_static(&root)?;
crate::docs_check::check_commands(&root, &crate::workspace::fidryn_binary()?)?;
```

Run: `cargo run -p xtask --offline -- docs`
Expected: `ok`, after running the commands tagged in Step 5.

- [ ] **Step 8: Commit**

```bash
git add xtask/Cargo.toml Cargo.lock xtask/src/docs_check.rs xtask/src/workspace.rs xtask/src/main.rs xtask/src/ci.rs xtask/tests/docs_cli.rs
git add -u docs README.md examples
git commit -m "Check documentation links, module samples and shown commands in CI"
```

---

### Task 4: Move the internal ledgers out of the reader's path

**Files:**
- Move with `git mv` into `docs/internal/`: `docs/WORKSTREAM-CONTRACT.md`, `docs/REVIEW-FIX-CONTRACT.md`, `docs/INTEGRATION-CONTRACT.md`, `docs/INTEGRATION-SUITE.md`, `docs/FULL-IMPLEMENTATION.md`, `docs/OBLIGATIONS.md`
- Create: `docs/internal/README.md`
- Modify: `docs/README.md:44-48`, `docs/implementation-status.md:8,19-20,61`, `docs/contributing.md:143-145`, `docs/ARCHITECTURE.md:149`, links between the six moved files and from them to `../ARCHITECTURE.md` and `../implementation-status.md`
- Modify: `xtask/src/site/guides.rs:111-128` (`IMPLEMENTER_DOCS` paths), `tests/integration/integration_suite.rs:5` (comment path), `site/README.md:32`
- Regenerate: `site/`
- Test: `xtask/src/site/guides.rs` (new unit test)

**Interfaces:**
- Consumes: `cargo xtask docs --skip-commands` from Task 3.
- Produces: the paths `docs/internal/<NAME>.md` for the six files. `IMPLEMENTER_DOCS` entries for Obligations and Integration contract become `"internal/OBLIGATIONS.md"` and `"internal/INTEGRATION-CONTRACT.md"`.

- [ ] **Step 1: Write the failing test** in the `tests` module of `xtask/src/site/guides.rs`

```rust
#[test]
fn internal_ledgers_live_under_docs_internal_only() {
    let docs = workspace_root().join("docs");
    for name in [
        "WORKSTREAM-CONTRACT.md",
        "REVIEW-FIX-CONTRACT.md",
        "INTEGRATION-CONTRACT.md",
        "INTEGRATION-SUITE.md",
        "FULL-IMPLEMENTATION.md",
        "OBLIGATIONS.md",
    ] {
        assert!(docs.join("internal").join(name).is_file(), "docs/internal/{name} is missing");
        assert!(!docs.join(name).exists(), "docs/{name} should have moved");
    }
    assert!(docs.join("internal/README.md").is_file());
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p xtask --offline internal_ledgers_live_under_docs_internal_only`
Expected: FAIL, `docs/internal/WORKSTREAM-CONTRACT.md is missing`.

- [ ] **Step 3: Move the files and write the index**

`git mv` each of the six files into `docs/internal/`. Write `docs/internal/README.md` with this content:

```markdown
# Internal ledgers

These documents are working records of how the 0.2 interpreter was
reviewed and repaired. They are kept for history. They are not user
documentation, and several describe states of the code that no longer
exist. For what the interpreter does today, read
[Implementation status](../implementation-status.md).

| Document | What it records |
| --- | --- |
| [Obligations](OBLIGATIONS.md) | Review obligations and the tests that guard them |
| [Workstream contract](WORKSTREAM-CONTRACT.md) | Shared types and acceptance for certificates, `require`, tax and trust profiles |
| [Integration contract](INTEGRATION-CONTRACT.md) | How existing machinery was to be wired into ordinary paths |
| [Integration suite](INTEGRATION-SUITE.md) | The public-API boundaries the integration tests close |
| [Review-fix contract](REVIEW-FIX-CONTRACT.md) | The repair brief for an earlier review pass |
| [Full implementation](FULL-IMPLEMENTATION.md) | An older list of required subsystems |
```

- [ ] **Step 4: Fix every reference**

Update each location listed under Files so it points at the new path. In `docs/README.md`, replace the five rows at lines 44-48 with one row: `| [Internal ledgers](internal/README.md) | Review and repair records kept for history |`. Inside the moved files, a link to a sibling ledger stays bare and a link to a document that stayed in `docs/` gains `../`.

Run: `cargo run -p xtask --offline -- docs --skip-commands`
Expected: `ok`.

- [ ] **Step 5: Regenerate the site and run the tests**

Run: `cargo run -p xtask --offline -- site`
Run: `cargo test -p xtask --offline`
Expected: all xtask tests pass, including `internal_ledgers_live_under_docs_internal_only`, `every_source_file_exists_under_docs` and `every_internal_link_and_anchor_resolves`.

- [ ] **Step 6: Commit**

```bash
git add -A docs site xtask/src/site/guides.rs tests/integration/integration_suite.rs
git commit -m "Separate internal review ledgers from reader documentation"
```

---

### Task 5: Status regions and draft marks

**Files:**
- Modify: `xtask/src/examples.rs` (regions and draft marks)
- Modify: `README.md` (Examples section), `examples/README.md:69`, `examples/states/README.md:21`, `docs/examples.md` (after the first paragraph of "How to use an example")
- Modify: `docs/examples.md` and `examples/states/README.md` again, by `--update`, which places the draft marks
- Regenerate: `site/`

**Interfaces:**
- Consumes: `FileStatus`, `survey`, `check`, `update` from Task 1; the counts Task 2 produced.
- Produces:

```rust
pub const BEGIN: &str = "<!-- examples-status:begin -->";
pub const END: &str = "<!-- examples-status:end -->";

pub struct Region {
    pub file: &'static str,
    /// Path prefix of the files counted; empty counts every file.
    pub scope: &'static str,
    /// What the counted files are called in the sentence.
    pub noun: &'static str,
    pub status_link: &'static str,
    /// Link to the implementation status guide from this document.
    pub limits_link: &'static str,
}

pub const REGIONS: [Region; 4] = [
    Region { file: "README.md", scope: "", noun: "`.fr` files in the example and test trees", status_link: "examples/STATUS.md", limits_link: "docs/implementation-status.md" },
    Region { file: "examples/README.md", scope: "", noun: "`.fr` files in the example and test trees", status_link: "STATUS.md", limits_link: "../docs/implementation-status.md" },
    Region { file: "examples/states/README.md", scope: "examples/states/", noun: "state modules", status_link: "../STATUS.md", limits_link: "../../docs/implementation-status.md" },
    Region { file: "docs/examples.md", scope: "", noun: "`.fr` files in the example and test trees", status_link: "../examples/STATUS.md", limits_link: "implementation-status.md" },
];

pub fn region_text(files: &[FileStatus], region: &Region) -> String;
pub fn replace_region(file: &str, document: &str, body: &str) -> Result<String>;

/// Text placed directly after the name of a module that does not pass.
pub const DRAFT_MARK: &str = " (draft)";

/// Reader documents in which draft marks are kept: `README.md`,
/// `examples/README.md`, `examples/states/README.md` and the files directly
/// in `docs/` (not `docs/internal/`). Sorted, with `/` separators.
pub fn marked_documents(root: &Path) -> Result<Vec<String>>;

/// `markdown` with the mark present after every mention of a module that
/// does not pass and absent after every mention of one that does.
pub fn apply_draft_marks(files: &[FileStatus], document: &str, markdown: &str) -> String;
```

`check` and `update` from Task 1 are extended to cover every entry of `REGIONS` and the draft marks in every document of `marked_documents`.

- [ ] **Step 1: Write the failing tests** in the `tests` module of `xtask/src/examples.rs`

```rust
#[test]
fn region_text_counts_only_its_scope() {
    let files = [
        ok("examples/states/a/x.fr"),
        fails("examples/states/b/y.fr", &[("E210", 1)]),
        ok("tests/programs/z.fr"),
    ];
    assert_eq!(
        region_text(&files, &REGIONS[0]),
        "`fidryn check` passes for 2 of the 3 `.fr` files in the example and test trees with this version. The [per-file list](examples/STATUS.md) is generated by `cargo xtask examples`. A file marked (draft) does not pass and cannot be run; [implementation status](docs/implementation-status.md) lists what the language cannot yet express.\n"
    );
    assert!(region_text(&files, &REGIONS[2]).starts_with(
        "`fidryn check` passes for 1 of the 2 state modules with this version. The [per-file list](../STATUS.md)"
    ));
}

#[test]
fn replace_region_requires_exactly_one_ordered_pair_of_markers() {
    let doc = format!("intro\n\n{BEGIN}\nold\n{END}\n\noutro\n");
    assert_eq!(
        replace_region("a.md", &doc, "new\n").unwrap(),
        format!("intro\n\n{BEGIN}\nnew\n{END}\n\noutro\n")
    );
    for broken in [
        "no markers at all\n".to_owned(),
        format!("{BEGIN}\nonly a beginning\n"),
        format!("{END}\nreversed\n{BEGIN}\n"),
        format!("{BEGIN}\na\n{END}\n{BEGIN}\nb\n{END}\n"),
    ] {
        let error = replace_region("a.md", &broken, "new\n").unwrap_err().to_string();
        assert!(error.contains("a.md"), "{error}");
        assert!(error.contains("examples-status"), "{error}");
    }
}

#[test]
fn check_covers_regions_and_update_rewrites_them() {
    let root = scratch("regions");
    write(&root, "examples/a/ok.fr", &module("Scratch.Ok", "query q() -> Int { return 7 }"));
    for region in &REGIONS {
        write(&root, region.file, &format!("text\n\n{BEGIN}\nstale\n{END}\n"));
    }
    update(&root).unwrap();
    check(&root).unwrap();
    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.contains("`fidryn check` passes for 1 of the 1 `.fr` files"), "{readme}");
    write(&root, "README.md", &format!("text\n\n{BEGIN}\nedited by hand\n{END}\n"));
    let error = check(&root).unwrap_err().to_string();
    assert!(error.contains("README.md"), "{error}");
}
```

```rust
#[test]
fn draft_marks_follow_the_recorded_status() {
    let files = [
        ok("examples/a/pass.fr"),
        fails("examples/states/b/fail.fr", &[("E210", 1)]),
    ];
    let md = "| `examples/a/pass.fr` (draft) | `examples/states/b/fail.fr` |\nSee `b/fail.fr`, examples/states/b/fail.fr and `elsewhere/unknown.fr`.\n\n```sh\nfidryn check examples/states/b/fail.fr\n```\n";
    let marked = "| `examples/a/pass.fr` | `examples/states/b/fail.fr` (draft) |\nSee `b/fail.fr` (draft), examples/states/b/fail.fr (draft) and `elsewhere/unknown.fr`.\n\n```sh\nfidryn check examples/states/b/fail.fr\n```\n";
    assert_eq!(apply_draft_marks(&files, "examples/states/README.md", md), marked);
    assert_eq!(apply_draft_marks(&files, "examples/states/README.md", marked), marked);
}
```

The sentence is worded as "passes for N of the M" so that no count needs singular or plural agreement; do not add plural logic.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p xtask --offline examples::tests`
Expected: the four new tests fail to compile.

- [ ] **Step 3: Implement the regions and the draft marks, and extend `check` and `update`**

`replace_region` keeps both marker lines and replaces everything between them with `body`. `check` reports each document whose region differs from `region_text`. `update` rewrites each document. A document listed in `REGIONS` that does not exist is an error naming it.

Draft marks, where the test leaves a choice:
- A mention is the longest run of the characters `A-Z a-z 0-9 _ . / -` that ends in `.fr` and is not followed by a letter, digit or underscore. Mentions inside fenced code blocks are ignored; commands are verified by running them.
- A mention names a surveyed file when it equals a recorded path, or when joining it to the directory of `document` gives one. Any other mention is left alone.
- The mark goes after the mention, and after a closing backtick when one follows it directly.
- `update` applies the marks after the regions and writes a document only when its text changes. `check` reports ``{document}: draft marks are out of date; run `cargo xtask examples --update` `` for each document `apply_draft_marks` would change.
- Line endings are preserved.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline examples::tests`
Expected: 9 passed.

- [ ] **Step 5: Put the markers in the four documents and remove the false sentences**

In each document insert an empty region (`BEGIN`, a blank line, `END`, each on its own line) at the place named under Files. Then:
- `examples/README.md:69`: delete the sentence "All of these `fidryn check` clean." Keep the rest of that paragraph.
- `examples/states/README.md:21`: delete the line "All fifty `fidryn check` clean on the 2026-09-17 snapshot."
- `README.md`: put the region directly under the heading `## Examples`.

Run: `cargo run -p xtask --offline -- examples --update`
Run: `grep -c "examples-status:begin" README.md examples/README.md examples/states/README.md docs/examples.md`
Expected: `1` for each file.
Run: `grep -n "passes for" README.md examples/states/README.md`
Expected: `README.md` says `passes for 54 of the 114`; `examples/states/README.md` says `passes for 8 of the 50 state modules`.
Run: `grep -o '(draft)' docs/examples.md | wc -l; grep -o '(draft)' examples/states/README.md | wc -l`
Expected: `64` and `43`: 63 and 42 marks, plus the one occurrence in each file's status sentence. No other document gains a mark.
Run: `cargo run -p xtask --offline -- examples`
Expected: passes.

- [ ] **Step 6: Regenerate the site and confirm the markers are not visible text**

Run: `cargo run -p xtask --offline -- site`
Run: `grep -c "&lt;!-- examples-status" site/docs/examples.html`
Expected: `0`. If it is not `0`, the Markdown renderer escaped the comment; stop and report, do not work around it in the document.
Run: `cargo test -p xtask --offline`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add xtask/src/examples.rs README.md examples/README.md examples/states/README.md docs/examples.md site
git commit -m "State example status in the documents from the recorded check"
```

---

### Task 6: An examples guide whose commands all run

**Files:**
- Modify: `docs/examples.md`
- Modify: `xtask/src/docs_check.rs` (`COMMAND_RULE_PENDING` and one test assertion)
- Regenerate: `site/`

**Interfaces:**
- Consumes: the block rules of Task 3 (`sh`, `sh exit=N`, `text`); the draft marks and status region of Task 5; the repaired modules and case records of Task 2.
- Produces: `COMMAND_RULE_PENDING` is empty. Every command any reader document shows is either run by `cargo xtask docs` or sits in a `text` block.

At the baseline the guide shows 77 commands and 16 of them work. After Task 2, 48 can work. The other 29 name drafts, use placeholders, or claim something the interpreter does not do.

- [ ] **Step 1: Remove the exemption and watch the check fail**

In `xtask/src/docs_check.rs` set `COMMAND_RULE_PENDING` to `&[]` and delete the last assertion of `a_block_that_shows_a_command_must_say_whether_it_is_checked` (the one about `docs/examples.md`).

Run: `cargo run -p xtask --offline -- docs --skip-commands`
Expected: fails, listing the untagged command blocks of `docs/examples.md`.

- [ ] **Step 2: Edit `docs/examples.md`**

Rules:

1. A block of real commands is tagged `sh` and keeps only commands whose module is not marked `(draft)`. A block left with no command is deleted, together with a sentence whose only job was to introduce it.
2. A synopsis with placeholders (`PATH`, `QUERY`, `CASE`) is tagged `text`.
3. Table rows stay, with the draft marks Task 5 put on them. Do not rewrite what a row says a module encodes.
4. A sentence that says or implies every module checks or runs is reworded to agree with the status region. Search for "clean", "all", "every", "eleven" and "fifty".

Corrections found at planning. Confirm each before editing:

- "How to use an example" shows `fidryn verify examples/states/florida/homestead.fr --property HomesteadHonesty` as the way to check the honesty assertions. It exits 1 and prints `unknown HomesteadHonesty: unsupported verification operation`. Keep the `check` command in an `sh` block. Replace the sentence about verifying with one that says `verify` cannot yet discharge these assertions and reports `unknown`, and show the `verify` command in its own block tagged `sh exit=1`. The same pair appears again in the state section: there keep only the `check`.
- "Start here" counts eleven modules and gives each a command. `examples/california/security-deposit/security-deposit.fr` is a draft. Remove its `run` command and reword the count so it does not say all eleven run.
- The catalog section runs `--query encoding_is_bounded`, which the interpreter reports as an unknown query, and verifies `CatalogDoesNotClaimCompleteness`, which is reported `unknown`. Use `--query complete_code_is_outside_scope` and delete the `verify` line. Task 7 records the cause.
- "Check any row the same way" lists six state modules, four of them drafts. List the eight that pass instead, and change the lead-in to "The rows not marked (draft) check clean:". The eight are `arkansas/homestead.fr`, `colorado/famli.fr`, `florida/homestead.fr`, `idaho/noncompete.fr`, `kansas/homestead.fr`, `louisiana/community-property.fr`, `michigan/no-fault-pip.fr` and `south-dakota/wage-payment.fr` under `examples/states/`.

- [ ] **Step 3: Run the documentation check, commands included**

Run: `cargo run -p xtask --offline -- docs`
Expected: `ok`. If a command exits with an unexpected status, read what it printed. Fix the document so it shows a command that works, or remove the command. Do not edit a module or a case record to make a command pass, and do not tag a failing command `text` to hide it.

Run: `cargo run -p xtask --offline -- examples`
Expected: passes (the status region and draft marks are intact).

- [ ] **Step 4: Regenerate the site and run the tests**

Run: `cargo run -p xtask --offline -- site`
Run: `cargo test -p xtask --offline`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add docs/examples.md xtask/src/docs_check.rs site
git commit -m "Show only commands that run in the examples guide"
```

---

### Task 7: Known limits as tests, and documents that match them

**Files:**
- Create: `tests/integration/known_limits.rs`
- Modify: `tests/integration/main.rs` (add `mod known_limits;`)
- Modify: `docs/implementation-status.md` (rewrite), `docs/language.md:13,27,101-117,238-243,276-298,409-424`, `grammar.ebnf:1`
- Regenerate: `site/`

**Interfaces:**
- Consumes: `fidryn_cli::compile_source`, `fidryn_core::{CoreDecl, CoreModule, DiagnosticCode, SourceManifest}`, `fidryn_core::ir::Consequence`.
- Produces: the test names below, which `docs/implementation-status.md` cites.

These tests describe what the interpreter does today. They pass as soon as they are written. That is intended: they are evidence for the status document, and Stages 2 and 3 replace each one when its limit is lifted.

- [ ] **Step 1: Write `tests/integration/known_limits.rs`**

```rust
//! Known limits of the 0.2 interpreter, pinned as executable facts.
//!
//! Each test states what the implementation does today, not what it should
//! do. `docs/implementation-status.md` cites these tests by name. When a
//! limit is lifted the test is replaced in the same change, never deleted
//! on its own.

use fidryn_cli::compile_source;
use fidryn_core::ir::Consequence;
use fidryn_core::{CoreDecl, CoreModule, DiagnosticCode, SourceManifest};

fn source(name: &str, body: &str) -> String {
    format!(
        "module {name} version \"0.1.0\" {{\n    jurisdiction Test\n    source_snapshot \"limits\"\n    effective_at 2026-01-01\n    recorded_at 2026-01-01T00:00:00Z\n    outside_scope {{ everything_else }}\n{body}\n}}\n"
    )
}

fn compile(name: &str, body: &str) -> CoreModule {
    compile_source(&source(name, body), &SourceManifest::default()).expect("limit program compiles")
}

fn codes(name: &str, body: &str) -> Vec<DiagnosticCode> {
    compile_source(&source(name, body), &SourceManifest::default())
        .expect_err("limit program is rejected")
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn a_duration_cannot_be_added_to_a_time_in_an_expression() {
    assert_eq!(
        codes(
            "Limits.TimePlusDuration",
            "    query automatic due(start: Time) -> Time { return start + 30 days }"
        ),
        [DiagnosticCode::E210]
    );
}

#[test]
fn a_date_literal_is_untyped_in_an_expression() {
    assert_eq!(
        codes(
            "Limits.DateLiteral",
            "    query automatic before(d: Date) -> Bool { return d <= 2026-12-31 }"
        ),
        [DiagnosticCode::E210]
    );
}

#[test]
fn an_unrecognised_rule_verb_is_lowered_as_derive() {
    let module = compile(
        "Limits.RuleVerb",
        "    entity Acme : LegalPerson\n    proposition Registered(who: LegalPerson)\n    proposition Flagged(who: LegalPerson)\n    rule Flag : constitutive {\n        when operative Registered(Acme)\n        then activate Flagged(Acme)\n    }\n    query q() -> Int { return 7 }",
    );
    let rule = module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Rule(rule) if rule.name == "Flag" => Some(rule),
            _ => None,
        })
        .expect("rule Flag");
    assert!(matches!(
        &rule.consequences[0].consequence,
        Consequence::Derive(prop) if prop.predicate == "Flagged"
    ));
}

#[test]
fn an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored() {
    compile(
        "Limits.Import",
        "    import Nowhere.DoesNotExist version \"9.9.9\" {\n        alias Gone\n    }\n    query q() -> Int { return 7 }",
    );
}

#[test]
fn power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass() {
    let module = compile(
        "Limits.PowerAct",
        "    entity Acme : LegalPerson\n    proposition Adopted(who: LegalPerson)\n    power AmendPower {\n        holder Acme\n        subject Acme\n        effect Adopted(Acme)\n        this_field_does_not_exist 42\n    }\n    legal_act Amend(who: LegalPerson) {\n        performer who\n        nonsense_field also_ignored\n    }\n    query q() -> Int { return 7 }",
    );
    assert!(
        !module
            .declarations
            .iter()
            .any(|decl| matches!(decl, CoreDecl::Power(_) | CoreDecl::LegalAct(_)))
    );
}

#[test]
fn a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read() {
    let module = compile(
        "Limits.Source",
        "    source Statute {\n        kind statute\n        authority Congress\n        citation \"1 U.S.C. 1\"\n        artifact \"sources/none.txt\"\n        digest fixture\n        effective [2026-01-01, +inf)\n    }\n    query q() -> Int { return 7 }",
    );
    assert!(
        !module
            .declarations
            .iter()
            .any(|decl| matches!(decl, CoreDecl::Source(_)))
    );
}

#[test]
fn an_office_keeps_none_of_its_declared_fields() {
    let module = compile(
        "Limits.Office",
        "    office Board(of: LegalPerson) occupied_by NaturalPerson {\n        cardinality 3..9\n        competence {Manage}\n    }\n    query q() -> Int { return 7 }",
    );
    let office = module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Office(office) => Some(office),
            _ => None,
        })
        .expect("office");
    assert_eq!(
        (office.cardinality_min, office.cardinality_max, office.competence.len()),
        (0, Some(1), 0)
    );
}

#[test]
fn a_misspelled_type_name_becomes_a_new_sort_without_a_diagnostic() {
    compile(
        "Limits.Misspelled",
        "    proposition Holds(who: NaturalPersn)\n    query q() -> Int { return 7 }",
    );
}

#[test]
fn a_query_that_ends_in_a_require_has_an_untyped_result() {
    assert_eq!(
        codes(
            "Limits.TrailingRequire",
            "    entity Acme : LegalPerson\n    proposition Sound(who: LegalPerson)\n    query q() -> Bool ! {Determine} { goal Evaluate { require determined Sound(Acme) using Determine } }"
        ),
        [DiagnosticCode::E210]
    );
}

#[test]
fn a_rule_whose_basis_is_a_bare_name_is_not_checked_and_swallows_the_next_declaration() {
    let module = compile(
        "Limits.BareBasis",
        "    proposition P(name: String)\n    proposition Q(name: String)\n    rule R : constitutive from Basis {\n        when operative P(undeclared)\n        then derive Q(\"x\")\n    }\n    query a() -> Int { return 1 }\n    query b() -> Int { return 2 }",
    );
    let names: Vec<&str> = module.queries.iter().map(|query| query.name.as_str()).collect();
    assert_eq!(names, ["b"]);
}
```

Add `mod known_limits;` to `tests/integration/main.rs`.

- [ ] **Step 2: Run the tests**

Run: `cargo test -p fidryn-integration-tests --offline known_limits`
Expected: 10 passed. If a test fails, the interpreter does not behave as this plan recorded. Do not change the interpreter and do not weaken the assertion: report the difference, because the status document must say what the code does.

- [ ] **Step 3: Rewrite `docs/implementation-status.md`**

Keep the document's first two paragraphs' definition of "implemented". Replace the matrix and everything after it with these sections, in order. Before writing a row, open the cited location and confirm it; where the code differs, write what the code does.

`## Declarations`, a table with columns Kind, Body parsed into structure, Reaches the intermediate representation, Executed, Evidence:

| Kind | Parsed into structure | Reaches the IR | Executed | Evidence to confirm |
| --- | --- | --- | --- | --- |
| `query`, `fn`, `calc` | Yes | Yes | Yes | `crates/fidryn-syntax/src/parser.rs:1546`, `:1846` |
| `rule` | Yes | Guard, selection, consequences. The `from` basis is dropped. Verbs other than establish, constitute, terminate, suspend and create become derive | Derive, establish, terminate, suspend. `create` is refused | `crates/fidryn-check/src/lib.rs:3634`; test `an_unrecognised_rule_verb_is_lowered_as_derive` |
| `judgment` | Yes | Yes | Concurrence policies only | `crates/fidryn-eval/src/judgment.rs:44` |
| `observation` | Yes | Yes | Yes | `crates/fidryn-eval/src/judgment.rs:486` |
| `type`, `record_type`, `evidence_type`, `entity`, `proposition` | Yes | Yes | Not applicable | `crates/fidryn-check/src/lib.rs:3277` |
| `duty` | Re-read from the declaration text | Bearer, claimant, attachment, content, due | Status and step, on one specialised path | `crates/fidryn-hir/src/body.rs:422`; `crates/fidryn-eval/src/duty.rs` |
| `decision` | Re-read from the declaration text | Binders, record requirements, first `returns` | Through `RunDecision` | `crates/fidryn-hir/src/body.rs:304` |
| `interpretation_family`, `nomination` | By splitting the text on keywords | Alternatives; candidate, office text, rank | Only by trustee-style succession | `crates/fidryn-hir/src/lib.rs:914`, `:940` |
| `office` | Name and parameters only | Fixed defaults: one occupant at most, no competence | Not read at run time | test `an_office_keeps_none_of_its_declared_fields` |
| `clause` | Name and parameters only | Body kept as text | Two clause names are known to the evaluator | `crates/fidryn-eval/src/lib.rs:4791`, `:4986` |
| `power`, `legal_act`, `conflict_doctrine`, `scenario` | No. Unknown fields are accepted | Nothing | Nothing | `crates/fidryn-syntax/src/parser.rs:882`; test `power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass` |
| `source` | Fields parsed, then dropped | Nothing | Nothing. `digest fixture` never reads the artifact | test `a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read` |
| `import` | Name, version, digest | Only a package under `packages/` is linked | Not applicable. `alias` is ignored; with no digest and no package the import does nothing | `crates/fidryn-check/src/lib.rs:2232`; test `an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored` |
| `verify` | Assertions kept | Yes | Closed formulas over synthetic entities only | `crates/fidryn-verify/src/property.rs` |

`## Expressions and types`, a short list: integer, decimal and same-currency money arithmetic work; a duration cannot be added to a time and date literals are untyped (cite the two tests); there are no month or year units and every instant is UTC; there are no lists, no aggregation, no string operations and no matching on variants; a misspelled type name creates a new sort (cite the test); a query that declares a result type must end in a typed value, so one that ends in `require` is rejected (cite `a_query_that_ends_in_a_require_has_an_untyped_result`); a module that does not pass the checker cannot be run (cite `a_module_that_fails_the_checker_cannot_be_run` in `tests/integration/repaired_examples.rs`).

`## Declarations that are lost`, one paragraph: a rule whose `from` basis is a bare name, as in `from Basis`, is not checked, and the declaration that follows it is dropped without a diagnostic (cite `a_rule_whose_basis_is_a_bare_name_is_not_checked_and_swallows_the_next_declaration`). Among the modules that pass, this hides one query: `encoding_is_bounded` in `examples/us-federal/catalog/usc-titles.fr`.

`## Names the evaluator knows`, a list with file references: `Incapacitated`, `Eligible`, `AcceptOffice`, `OfficialFilingRecord`, `FormedLLC`, `SpousalSupportWaiver`, `ChildSupportWaiver` in `crates/fidryn-eval/src/lib.rs` at `:3613`, `:4088`, `:4458`, `:4504`, `:4548`, `:4791`, `:4986`; `PaymentRecord` and `invoice_date` in `crates/fidryn-eval/src/duty.rs` at `:701`, `:944`; and the four-bracket `ordinary_income_tax` at `crates/fidryn-eval/src/lib.rs:3174`. Say plainly that programs using other names for the same ideas do not get this behaviour.

`## Known limits pinned by tests`, a two-column table of each test name in `tests/integration/known_limits.rs` and the sentence it proves.

`## Budgets`, keeping the existing figures for operations, depth, worklist rounds and trace size.

Link the internal ledgers only through `internal/README.md`. Say nothing about when or how a limit will be lifted.

- [ ] **Step 4: Correct `docs/language.md` and `grammar.ebnf`**

- `docs/language.md:13`: replace "The grammar is closed: unknown body fields are parse errors." with "The grammar in `grammar.ebnf` is closed, but the 0.2 parser enforces that only inside `source` blocks. In most other declarations an unknown field is skipped without a diagnostic; see [implementation status](implementation-status.md)."
- `docs/language.md:27`: change the import row's status note to "Parsed. `alias` is accepted and ignored. An import with no digest and no matching package does nothing."
- The Imports section (`:101-117`): remove `alias FOIARegs` from the sample and say in one sentence that `alias` is ignored in 0.2.
- The deadline sample (`:276-298`): keep `calc return_deadline()`. Remove the `deadline_running` query. In its place state that 0.2 cannot add a duration to a time in an expression (it is `E210`), and that a deadline is computed only by a duty's `due` clause, pointing to the Duties section.
- The section "Powers, legal acts, clauses, and conflict doctrines" (`:409-424`): state that 0.2 does not interpret the bodies of `power`, `legal_act`, `conflict_doctrine` or `scenario`, and that `clause` bodies are kept as text.
- The judgment-query sample under "Automatic versus judgment" (`:238-243`) is the form the checker rejects. Add `return true` as the last statement of its `goal` block, as Task 2 did in the examples, and add one sentence after the sample: the query answers `true` once a competent determination establishes the proposition and suspends otherwise, including when the determination is negative (tests `an_established_determination_makes_the_judgment_query_true` and `a_negative_determination_suspends_and_never_answers_false`).
- `grammar.ebnf:1`: change the comment to `(* Fidryn 0.2 grammar. The parser enforces closed field sets only for source blocks; see docs/implementation-status.md. *)`

- [ ] **Step 5: Regenerate and run the checks**

Run: `cargo run -p xtask --offline -- site`
Run: `cargo run -p xtask --offline -- docs --skip-commands`
Expected: `ok`.
Run: `cargo test -p xtask --offline && cargo test -p fidryn-integration-tests --offline`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add tests/integration/known_limits.rs tests/integration/main.rs docs/implementation-status.md docs/language.md grammar.ebnf site
git commit -m "Pin known limits as tests and correct the status and language guides"
```

---

### Task 8: The displayed version and other stale text

**Files:**
- Modify: `xtask/src/site/templates/base.html:42`, `xtask/src/site/pages.rs:252-266`, `xtask/src/site/mod.rs` (test)
- Modify: `crates/fidryn-core/src/request.rs:154`, `crates/fidryn-core/src/outcome.rs:645,1089`, `crates/fidryn-core/src/lib.rs` (re-export), `crates/fidryn-cli/src/engine.rs:44`, and the two further literals reported at `crates/fidryn-trace/src/lib.rs:315` and `crates/fidryn-cli/src/review.rs:254`
- Modify: `crates/fidryn-trace/src/lib.rs:343-346,378-382` (comments only)
- Create: `tests/programs/cases/empty.json`
- Modify: `xtask/src/site/specimen.rs:37,388-389`, `docs/getting-started.md:95-112`
- Modify: `docs/cli.md:468-481`, `conformance/README.md:27-30`, `.agents/AGENTS.md:29`, `.agents/ARCHITECTURE.md:5`
- Regenerate: `site/`

**Interfaces:**
- Produces: `pub const SEMANTICS_VERSION: &str = "0.2";` in `crates/fidryn-core/src/request.rs`, re-exported from the crate root. The template slot `{{version}}`.

- [ ] **Step 1: Write the failing test** in `build_tests` of `xtask/src/site/mod.rs`

```rust
#[test]
fn every_page_footer_shows_the_workspace_version() {
    let files = build(&workspace_root()).expect("build");
    let footer = format!(
        "<p>Fidryn {} &middot; a research fixture, not legal advice &middot; Bryan Gass</p>",
        env!("CARGO_PKG_VERSION")
    );
    let mut pages = 0;
    for file in files.iter().filter(|f| f.path.extension().is_some_and(|e| e == "html")) {
        let html = String::from_utf8(file.bytes.clone()).expect("utf-8");
        assert!(html.contains(&footer), "{}", file.path.display());
        assert!(!html.contains("Fidryn v0.1"), "{}", file.path.display());
        pages += 1;
    }
    assert!(pages >= 10, "only {pages} pages");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p xtask --offline every_page_footer_shows_the_workspace_version`
Expected: FAIL on `index.html`.

- [ ] **Step 3: Take the footer version from the workspace**

In `base.html` change `Fidryn v0.1` to `Fidryn {{version}}`. In `shell` in `pages.rs`, add `("version", env!("CARGO_PKG_VERSION"))` to the slot list. The `xtask` crate inherits the workspace version.

Run: `cargo test -p xtask --offline every_page_footer_shows_the_workspace_version`
Expected: PASS.

- [ ] **Step 4: One source for the semantics version**

Add `pub const SEMANTICS_VERSION: &str = "0.2";` to `crates/fidryn-core/src/request.rs` and re-export it in `crates/fidryn-core/src/lib.rs`. Replace each non-test string literal `"0.2"` that denotes the semantics version at the locations under Files with the constant. Confirm the two reported locations before editing them; leave any that turn out to mean something else. Test expectations keep their literals.

Run: `grep -rn '"0\.2"' crates --include='*.rs' | grep -v -E 'assert|#\[test\]|tests/'`
Expected: only the definition of `SEMANTICS_VERSION`.
Run: `cargo test -p fidryn-core -p fidryn-cli -p fidryn-trace --offline`
Expected: all pass.

- [ ] **Step 5: A case file the guides can name on every platform**

Create `tests/programs/cases/empty.json` containing exactly:

```json
{
  "schema": "fidryn.case-record/v0.1",
  "admissibleCompletions": {}
}
```

In `xtask/src/site/specimen.rs` set `EMPTY_CASE_PATH` to `"tests/programs/cases/empty.json"` and update the two expected command strings in its test at `:388-389`. In `docs/getting-started.md`, remove the step that writes `/tmp/fidryn-empty-case.json` with a heredoc, say that the empty case record is `tests/programs/cases/empty.json`, show its three lines, and use that path in every command. Then retag the blocks Task 3 marked `text` only because of that path: a command that now runs as written from the repository root is tagged `sh`. A command that redirects its output stays in a `text` block.

Run: `grep -rn "/tmp/fidryn-empty-case" docs xtask/src README.md`
Expected: no output.

- [ ] **Step 6: The remaining stale text**

- `docs/cli.md`: move the `## See also` section so it is the last section, after `inputs` and `review`.
- `conformance/README.md:27-30`: the outcome probe uses four malformed documents; list all four (`determinate_without_value`, `invalid_trace`, `ignored_issue_without_certificate`, `ill_typed_tagged_value`).
- `crates/fidryn-trace/src/lib.rs`: correct the two doc comments. `provenance_root` is the invocation id when the CLI builds the document; the CLI attaches the evaluator's real derivation steps.
- `.agents/AGENTS.md:29` and `.agents/ARCHITECTURE.md:5`: say the README describes the 0.2 reference interpreter.
- Prose names the release series, not a patch version. Change `0.2.0` to `0.2` at `docs/ARCHITECTURE.md:1,36,43`, `docs/README.md:8`, `docs/cli.md:3`, `docs/contributing.md:3`, `docs/getting-started.md:14` and `docs/language.md:9,22,496`. Reword `docs/releases.md:7` to begin "Release 0.2 therefore still loads the repository's 0.1.0 instrument fixtures". Leave `docs/internal/` and `README.md` alone; Task 10 rewrites the README's statement.

Run: `git grep -n '0\.2\.0' -- docs ':!docs/internal'`
Expected: no output.

- [ ] **Step 7: Regenerate and run the tests**

Run: `cargo run -p xtask --offline -- site`
Run: `grep -rc "Fidryn v0.1" site | grep -v ":0"`
Expected: no output.
Run: `cargo test -p xtask --offline && cargo run -p xtask --offline -- docs`
Expected: all pass, then `ok`.

- [ ] **Step 8: Commit**

```bash
git add -A xtask/src/site crates/fidryn-core crates/fidryn-cli crates/fidryn-trace tests/programs/cases docs conformance/README.md .agents/AGENTS.md .agents/ARCHITECTURE.md site
git commit -m "Derive the displayed version and remove stale statements"
```

---

### Task 9: Repository hygiene

**Files:**
- Modify: `.gitignore:4-6`
- Create: `SECURITY.md`
- Create: `xtask/tests/repo_hygiene.rs`
- Regenerate: `site/` (only if a generated file changes)

**Interfaces:** none consumed or produced.

`.cargo/config.toml` is not part of this task. The owner decided it stays as it is.

- [ ] **Step 1: Write the failing test** in `xtask/tests/repo_hygiene.rs`

```rust
//! Files a newcomer looks for exist and say what they should.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_owned()
}

#[test]
fn a_security_policy_exists_and_names_the_reporting_route() {
    let policy = fs::read_to_string(root().join("SECURITY.md")).expect("SECURITY.md");
    assert!(policy.contains("Report a vulnerability"), "{policy}");
    assert!(!policy.contains('@'), "do not publish an address the owner has not chosen");
}

#[test]
fn the_lockfile_is_tracked_and_nothing_ignores_it() {
    let ignore = fs::read_to_string(root().join(".gitignore")).expect(".gitignore");
    assert!(!ignore.contains("Cargo.lock"), "{ignore}");
    assert!(root().join("Cargo.lock").is_file());
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p xtask --offline --test repo_hygiene`
Expected: both FAIL.

- [ ] **Step 3: Tidy `.gitignore`**

Remove the three lines `/Cargo.lock`, `# Keep lockfile for the CLI binary reproducibility.` and `!/Cargo.lock`. The lockfile is tracked and nothing ignores it.

Run: `git check-ignore -q Cargo.lock; echo $?`
Expected: `1`.

- [ ] **Step 4: Write `SECURITY.md`**

```markdown
# Security policy

## Reporting a vulnerability

Use GitHub's private reporting: on the repository page open the
**Security** tab and choose **Report a vulnerability**. Please do not open
a public issue for a suspected vulnerability.

Include the version (`fidryn --version`), the platform, and the smallest
module, case record or command that shows the problem.

## What is in scope

- The `fidryn` binary and the crates in this repository.
- The local web interface started by `fidryn ui`. It binds to `127.0.0.1`
  only and is not meant to be exposed to a network.
- Handling of untrusted input: module source, case records, source
  manifests, templates and review bundles.

## What is not a vulnerability

- A legal conclusion you disagree with. Modules in this repository are
  research fixtures, not legal advice.
- Live filing. It is off unless both `--live` and
  `FIDRYN_ALLOW_LIVE_FILING=1` are set by the operator.

## Supported versions

Only the latest release receives fixes.
```

- [ ] **Step 5: Run the tests and checks**

Run: `cargo test -p xtask --offline --test repo_hygiene`
Expected: 2 passed.
Run: `cargo run -p xtask --offline -- docs --skip-commands`
Expected: `ok`.

- [ ] **Step 6: Commit**

```bash
git add .gitignore SECURITY.md xtask/tests/repo_hygiene.rs
git commit -m "Add a security policy and stop ignoring the tracked lockfile"
```

Private vulnerability reporting must also be switched on in the repository's GitHub settings. That is the owner's action; note it in the final report.

---

### Task 10: A README an outside reader can follow

**Files:**
- Modify: `README.md` (everything above `## Crates`)

**Interfaces:**
- Consumes: the block rules of Task 3 (`fidryn` commands in `sh` blocks are run by `cargo xtask docs`; a synopsis is tagged `text`); the status region of Task 5, which must survive unchanged between its markers.

- [ ] **Step 1: Rewrite the top of `README.md`**

Keep `## Crates`, `## Website` and the licensing paragraph. Replace everything else with these sections in this order.

1. `# Fidryn` and the existing one-sentence description.
2. `## Status`: Fidryn 0.2 is a research interpreter. In three or four sentences say what works (checking and running modules built from queries, calculations, rules, judgments, observations and one duty path; reports that record their inputs) and what does not (most other declaration bodies are not interpreted; no date arithmetic in expressions; no imports between modules), linking `docs/implementation-status.md`. State that nothing here is legal advice.
3. `## Quick start`, with every runnable block tagged `sh`:

````markdown
Rust 1.98 is required; `rust-toolchain.toml` selects it.

```sh
cargo install --path crates/fidryn-cli --locked
fidryn check tests/programs/require-gate.fr
```

`check` prints `ok`. Run a query against a case record:

```sh
fidryn run tests/programs/require-gate.fr \
  --query q \
  --case tests/programs/cases/empty.json \
  --valid-at 2026-09-17T12:00:00Z \
  --known-at 2026-09-17T12:00:00Z
```

The report's outcome is determinate `7`. Query `r` in the same file is
suspended, because its requirement fails:

```sh
fidryn run tests/programs/require-gate.fr \
  --query r \
  --case tests/programs/cases/empty.json \
  --valid-at 2026-09-17T12:00:00Z \
  --known-at 2026-09-17T12:00:00Z
```

Run the tests:

```sh
cargo test --workspace --locked
```
````

4. `## Documentation`: the existing "What to read next" table, unchanged.
5. `## CLI at a glance`: the existing synopsis block, tagged `text` so it is not executed.
6. `## Examples`: the heading, the status region exactly as Task 5 left it, then the existing list.

Do not say where the project is going. The owner is holding that until Stage 1 lands.

- [ ] **Step 2: Run the documentation check, commands included**

Run: `cargo run -p xtask --offline -- docs`
Expected: `ok`. The three README commands (`check`, `run --query q`, `run --query r`) are among those it runs. A suspended outcome exits zero.
Run: `cargo run -p xtask --offline -- examples`
Expected: passes, which shows the status region is intact.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "Rewrite the README around what works and how to run it"
```

---

### Task 11: Return the project to the MIT License

**Files:**
- Modify: `LICENSE` (replace the whole file), `LICENSING.md` (rewrite)
- Delete: `LICENSES/prior-project-LICENSE.txt`
- Modify: `Cargo.toml:26`, the 17 member manifests (`crates/*/Cargo.toml`, `xtask/Cargo.toml`, `tests/Cargo.toml`, `benches/Cargo.toml`), `Cargo.lock` if Cargo rewrites it
- Modify: `deny.toml` (remove every first-party block)
- Modify: `README.md` (the licensing sentence), `docs/README.md:40`, `docs/releases.md:3-5,71-74`
- Create: `xtask/tests/licensing.rs`
- Regenerate: `site/`

**Interfaces:** none consumed or produced.

The owner decided on 2026-11-13 that Fidryn is licensed under the MIT License. The project began under MIT: `main` still carries that text, and `LICENSES/prior-project-LICENSE.txt` preserves it byte for byte (1,067 bytes, SHA-256 `a1211fd38230055cf76a2a282da537b3e0d6f87d5cd0ee2ad454c30eecea16c9`). The proprietary text exists only on `dev`. This task restores the original file and makes every statement of the licence agree with it.

Leave these alone: `LICENSES/crates/`, `LICENSES/fonts/`, `site/fonts/LICENSE.md`, `xtask/src/licenses.rs`, the fixture names in the tests of `xtask/src/package.rs`, `pyproject.toml` (it points at `LICENSE`), the `[0.2.0]` changelog entry, and everything under `.agents/docs/reviews/`. The contribution terms in `LICENSING.md` are kept as they are; changing them is a separate decision for the owner.

- [ ] **Step 1: Write the failing tests** in `xtask/tests/licensing.rs`

```rust
//! The licence text, the package metadata and the documents agree.

use std::fs;
use std::path::{Path, PathBuf};

const MIT: &str = "MIT License

Copyright (c) 2026 Bryan Gass

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_owned()
}

#[test]
fn the_licence_file_is_the_mit_text() {
    let text = fs::read_to_string(root().join("LICENSE")).expect("LICENSE");
    assert_eq!(text.replace("\r\n", "\n"), MIT);
}

#[test]
fn every_workspace_package_declares_mit() {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root().join("Cargo.toml"))
        .no_deps()
        .exec()
        .expect("cargo metadata");
    let packages = metadata.workspace_packages();
    assert!(packages.len() >= 17, "only {} packages", packages.len());
    for package in packages {
        assert_eq!(package.license.as_deref(), Some("MIT"), "{}", package.name);
        assert!(package.license_file.is_none(), "{}", package.name);
    }
}

#[test]
fn nothing_still_states_the_proprietary_terms() {
    for file in [
        "LICENSE",
        "LICENSING.md",
        "README.md",
        "deny.toml",
        "docs/README.md",
        "docs/contributing.md",
        "docs/releases.md",
    ] {
        let text = fs::read_to_string(root().join(file)).expect(file);
        for needle in ["roprietary", "PROPRIETARY", "LicenseRef-", "All Rights Reserved", "freectrl"] {
            assert!(!text.contains(needle), "{file} still contains {needle}");
        }
    }
    assert!(!root().join("LICENSES/prior-project-LICENSE.txt").exists());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p xtask --offline --test licensing`
Expected: all three FAIL.

- [ ] **Step 3: Restore the licence text**

```bash
cp LICENSES/prior-project-LICENSE.txt LICENSE
git rm --quiet LICENSES/prior-project-LICENSE.txt
sha256sum LICENSE
```

Expected: `a1211fd38230055cf76a2a282da537b3e0d6f87d5cd0ee2ad454c30eecea16c9  LICENSE`. The restored file is the authority. If `the_licence_file_is_the_mit_text` still fails after this step, the constant in the test was mistyped: correct the constant, never the file.

- [ ] **Step 4: Make the metadata agree**

In `Cargo.toml`, replace `license-file = "LICENSE"` under `[workspace.package]` with `license = "MIT"`. In each of the 17 member manifests replace `license-file.workspace = true` with `license.workspace = true`.

Reduce `deny.toml` to exactly:

```toml
[advisories]
version = 2
yanked = "deny"

[licenses]
version = 2
confidence-threshold = 0.95
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Zlib", "CC0-1.0", "0BSD", "MPL-2.0", "CDLA-Permissive-2.0"]

[bans]
multiple-versions = "allow"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

Run: `cargo deny --offline check licenses bans sources`
Expected: the last line is `bans ok, licenses ok, sources ok`. A warning about an unmatched licence allowance is already present at the baseline and is not an error. This check ran clean on a scratch copy of the tree during planning.

- [ ] **Step 5: Rewrite `LICENSING.md`**

Replace the whole file with:

```markdown
# Licensing

Fidryn is released under the MIT License. The full text is in
[LICENSE](LICENSE). It covers the source code, documentation, example
modules and fixtures in this repository, except the third-party material
described below.

## Third-party material

Dependencies, fonts and other third-party material keep their own
licences and copyright notices. `LICENSES/` holds the notices this
repository retains: `LICENSES/fonts/` for the fonts used by the site and
the local interface, and `LICENSES/crates/` for crates whose published
archives omit their licence files. Release archives also carry
`THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES/`, collected from the
locked dependency graph. Do not remove an attribution or present upstream
material as this project's own.

## Contributions

Contributions require an appropriate separate rights agreement before
acceptance. Repository access or a contribution does not automatically
grant equity or management.
```

- [ ] **Step 6: Correct the documents that describe the licence**

- `README.md`: replace the sentence "Current licensing terms and retained prior grants are described in [Licensing](LICENSING.md)." with "Fidryn is released under the MIT License: see [LICENSE](LICENSE). [Licensing](LICENSING.md) covers third-party notices."
- `docs/README.md:40`: the row's description becomes "MIT terms and third-party notices".
- `docs/releases.md:3-5`: replace "Current project terms and retained prior/third-party grants are documented in [Licensing](../LICENSING.md)." with "The project is under the MIT License; [Licensing](../LICENSING.md) describes it and the third-party notices."
- `docs/releases.md:71-74`: replace the paragraph that begins "First-party custom license checks" with: "`deny.toml` allows a fixed list of licences for dependencies. Workspace crates declare `MIT`, which is on that list, so a release needs no licence entries of its own."

Run: `git grep -n -i -E 'proprietary|LicenseRef|prior-project' -- . ':!LICENSES' ':!.agents' ':!CHANGELOG.md' ':!site' ':!xtask/src/package.rs' ':!xtask/tests/licensing.rs'`
Expected: one line, the Idaho statute excerpt at `examples/states/idaho/sources/id-code-44-2704.txt:14`, which is quoted law and stays.

- [ ] **Step 7: Run the tests and the checks**

Run: `cargo test -p xtask --offline --test licensing`
Expected: 3 passed.
Run: `cargo run -p xtask --offline -- site && cargo run -p xtask --offline -- docs --skip-commands`
Expected: `ok`.
Run: `cargo test -p xtask --offline`
Expected: all pass. The packaging tests build their own fixture trees and do not read the files this task changed.

- [ ] **Step 8: Commit**

```bash
git add LICENSE LICENSING.md Cargo.toml Cargo.lock deny.toml README.md docs site xtask/tests/licensing.rs
git add crates/*/Cargo.toml xtask/Cargo.toml tests/Cargo.toml benches/Cargo.toml
git add -u LICENSES
git commit -m "Return the project to the MIT License"
```

---

### Task 12: Release 0.2.1, the full gate, and hand-off

**Files:**
- Modify: `Cargo.toml:24,32-45`, `Cargo.lock`
- Modify: `CHANGELOG.md` (new `[0.2.1]` entry)
- Regenerate: `site/`

**Interfaces:** none.

The procedure is the one in `docs/releases.md`, "Prepare a release". The owner merges PR #3 as 0.2.0 before this branch reaches `dev`, so the base for the version check is the head of that pull request, `1a703828641e094d4179ab0265a98a75f8aee43c`.

- [ ] **Step 1: Raise the version**

In `Cargo.toml` change `version = "0.2.0"` under `[workspace.package]` to `"0.2.1"`, and change the `version` of each of the 14 internal dependencies on lines 32 to 45 from `"0.2.0"` to `"0.2.1"`.

Run: `cargo update --workspace --offline`
Run: `grep -c 'version = "0.2.1"' Cargo.lock`
Expected: `17`.
Run: `grep -B1 'version = "0.2.0"' Cargo.lock | grep name`
Expected: exactly `name = "foldhash"` and `name = "powerfmt"`. Those are third-party crates that happen to share the number. Do not edit `Cargo.lock` by hand.

- [ ] **Step 2: Write the changelog entry**

Leave `## [Unreleased]` in place. Directly under it add `## [0.2.1] - YYYY-MM-DD` with the day this task runs, and these classified notes:

```markdown
### Changed

- The project is licensed under the MIT License again. `LICENSE` holds the
  MIT text and every workspace package declares `MIT`.
- Thirty-four example modules and seven case records were repaired with
  typing-only edits. 54 of the 114 example and test modules now pass
  `fidryn check`; the rest are marked as drafts wherever a document names
  them.
- The implementation status and language guides state what the 0.2
  interpreter interprets and what it does not.
- Internal review ledgers moved under `docs/internal/`.

### Added

- A recorded check status for every example module, enforced in CI, with
  the counts stated in the README and example guides.
- A documentation check that follows links, compiles complete module
  samples and runs every command the documents show.
- Tests that pin known limits of the interpreter, cited by the
  implementation status guide.
- A security policy.

### Fixed

- The site footer shows the workspace version.
- Guides no longer rely on a POSIX-only temporary path.
- The examples guide no longer shows commands that fail.
```

Run: `cargo run -p xtask --offline -- release-check --base 1a703828641e094d4179ab0265a98a75f8aee43c`
Expected: passes, reporting version 0.2.1.

- [ ] **Step 3: Regenerate what carries the version**

Run: `cargo run -p xtask --offline -- site`
Run: `cargo run -p xtask --offline -- examples`
Expected: the site footer now reads `Fidryn 0.2.1`; the examples check passes without an update.

- [ ] **Step 4: Run the full gate**

Run: `lease run --cpus 16 --mem 32G --time 2h -- cargo xtask ci --fmt`
Expected: exit code 0. If formatting fails, run `cargo fmt --all` and include the result in this task's commit. If the job waits as PENDING, wait; do not shrink the request or submit it again.

- [ ] **Step 5: Confirm the acceptance criteria of spec section 8.2**

Run: `cargo run -p xtask --offline -- site --check`
Expected: `site/ is up to date`.
Run: `cargo deny --offline check licenses bans sources`
Expected: the last line is `bans ok, licenses ok, sources ok`.
Run: `grep -rn 'check. clean\|All fifty' README.md docs examples --include='*.md'`
Expected: no output.
Run: `grep -rc "Fidryn v0.1" site | grep -v ":0"`
Expected: no output.
Run: `git grep -n '0\.2\.0' -- README.md docs ':!docs/internal'`
Expected: no output.
Run: `git grep -n -i 'roadmap' -- README.md docs examples ':!docs/internal'`
Expected: no output.
Run: `git diff --stat dev -- .cargo/config.toml`
Expected: no output.
Run: `grep -n 'COMMAND_RULE_PENDING' xtask/src/docs_check.rs`
Expected: the definition is `&[]`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md site
git add -u
git commit -m "Prepare release 0.2.1"
```

- [ ] **Step 7: Report and stop**

Report to the owner: the branch name and its commits; the gate result; and the actions that are theirs. Merge PR #3 first, then put this branch onto `dev` and promote it as 0.2.1. Enable private vulnerability reporting in the repository settings. Decide whether contributions still need a separate rights agreement now that the project is MIT. Do not push.
