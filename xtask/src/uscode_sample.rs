//! Checks the pinned sample of the Code under `samples/uscode/`, eight
//! things in this order, and fails with a message that names the first
//! difference when one of them does not hold:
//!
//! 1. every archive in `archives/` has the size, the SHA-256 and the BLAKE3
//!    that the retrieval record beside it gives, and the record names every
//!    archive and no other;
//! 2. `fidryn ingest uscode samples/uscode/archives --out DIR` writes into a
//!    temporary directory, byte for byte, every directory and file of the
//!    workspace committed at `workspace/`, its coverage ledger
//!    `coverage.json` among them, and nothing else; `--update` writes the
//!    committed workspace with the same command first;
//! 3. an ingest on one worker writes the same bytes as one on eight;
//! 4. `fidryn check samples/uscode/workspace --no-cache` exits 0 with no
//!    error, and `fidryn coverage samples/uscode/workspace` prints the
//!    committed `coverage.json`, byte for byte;
//! 5. `profile.json` is, byte for byte, what `conformance/usc_profile.py`
//!    prints for the archives; its counts of references by layer and
//!    outcome are those of the check report of step 4; and every key that
//!    the profile's `import` and the ledger's `totals` share has one value
//!    in both, the keys that only one of them has being named here;
//! 6. what `conformance/uslm_text.py` prints for the archives is, line for
//!    line, what `fidryn text samples/uscode/workspace --json` prints;
//! 7. the check of the curated sample, `samples/uscode/curated`, which
//!    depends on the generated one at a pinned release, finds no error and
//!    no warning, a stale binding (`W750`) among the warnings, counts at
//!    least one citation, and gives every one of its citations the outcome
//!    `exact`;
//!    `--update` writes, after the workspace, the release and the lock
//!    digest of that dependency and the toolchain of the curated manifest,
//!    and nothing else of it;
//! 8. the table of `docs/coverage.md` between its two markers is the one
//!    that the committed `coverage.json` gives; `--update` writes it, and
//!    nothing else of the page.
//!
//! The `fidryn` commands run in this process, through `fidryn_cli`, as the
//! binary runs them, and the check of the curated sample through the check
//! of a workspace that `fidryn check` runs, so that its diagnostics can be
//! named. Their temporary directories are made under the system's directory
//! for temporary files and removed whether the step passes or fails, and the
//! checks keep no cache, so the step leaves the repository as it found it;
//! only `--update` writes, and only the committed workspace, those three
//! values of the curated manifest and the table of the page.

use crate::workspace::workspace_root;
use anyhow::{Context, Result, anyhow, bail};
use clap::Parser as _;
use fidryn_core::DiagnosticCode;
use fidryn_driver::workspace::{CheckOptions, check_workspace};
use fidryn_provision::layout::module_path;
use fidryn_provision::workspace::{Lock, Manifest, RetrievalRecord};
use fidryn_provision::{Hash32, Outcome};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The directory of the archives and their retrieval record, which holds
/// nothing else.
pub const ARCHIVES: &str = "samples/uscode/archives";
/// The retrieval record, beside the archives.
pub const RECORD: &str = "retrieval.json";
/// The independent profile of the archives.
pub const PROFILE: &str = "samples/uscode/profile.json";
/// The script that prints the profile.
pub const PROFILE_SCRIPT: &str = "conformance/usc_profile.py";
/// The workspace that `fidryn ingest uscode` writes from the archives.
pub const WORKSPACE: &str = "samples/uscode/workspace";
/// The script that prints the text records of the archives, independently.
pub const TEXT_SCRIPT: &str = "conformance/uslm_text.py";

/// The curated sample, which depends on the generated sample.
pub const CURATED: &str = "samples/uscode/curated";
/// The path at which the curated manifest names the generated sample.
const DEPENDENCY_PATH: &str = "../workspace";
/// The manifest of a workspace, at its top.
const MANIFEST: &str = "workspace.json";
/// The lock of a generated workspace, at its top.
const LOCK: &str = "law.lock.json";
/// The version of the running toolchain: that of the crates of the
/// repository, which `fidryn` and this task share.
const TOOLCHAIN: &str = env!("CARGO_PKG_VERSION");

/// The layers whose references the profile and the check report count.
const LAYERS: [&str; 3] = ["operative", "enacted", "editorial"];

/// The coverage ledger of a generated workspace, at its top.
const LEDGER: &str = "coverage.json";
/// The page of the site that shows the ledger of the sample.
const PAGE: &str = "docs/coverage.md";
/// The markers between which the page holds the table of the ledger.
const TABLE_BEGIN: &str = "<!-- coverage-table:begin -->";
const TABLE_END: &str = "<!-- coverage-table:end -->";

/// The keys of the ledger's `totals` that the profile's `import` does not
/// have: the independent reading extracts no definitions.
const LEDGER_ONLY: [&str; 2] = ["definitions", "definitions_by_kind"];
/// The keys of the profile's `import` that the ledger's `totals` does not
/// have: the ledger has them at its top, where they are compared.
const PROFILE_ONLY: [&str; 2] = ["prefix", "release"];

#[derive(Debug, clap::Args)]
pub struct UscodeSampleArgs {
    /// Write samples/uscode/workspace with what the ingester writes from the archives, the pin and the toolchain of samples/uscode/curated, and the table of docs/coverage.md, then check them
    #[arg(long)]
    pub update: bool,
}

pub fn run(args: UscodeSampleArgs) -> Result<()> {
    check(&workspace_root(), args.update)
}

/// Runs the eight steps on the sample under `root`; with `update`, step 2
/// first writes the committed workspace, and then the pin and the toolchain
/// of the curated manifest and the table of the page.
pub fn check(root: &Path, update: bool) -> Result<()> {
    let problems = archive_problems(&root.join(ARCHIVES))?;
    if !problems.is_empty() {
        bail!(
            "{ARCHIVES} differs from its record:\n{}",
            problems.join("\n")
        );
    }
    let scratch = Scratch::new()?;
    let archives = root.join(ARCHIVES);
    let committed = root.join(WORKSPACE);

    let eight = scratch.join("jobs-8");
    ingest(&archives, &eight, 8)?;
    if update {
        ingest(&archives, &committed, 8)?;
        eprintln!("wrote {WORKSPACE}");
        update_curated(root)?;
        eprintln!("wrote the pin and the toolchain of {CURATED}/{MANIFEST}");
        update_page(root)?;
        eprintln!("wrote the table of {PAGE}");
    }
    if !committed.is_dir() {
        bail!("{WORKSPACE} does not exist; write it with `cargo xtask uscode-sample --update`");
    }
    if let Some(difference) = first_difference(&eight, &committed)? {
        bail!(
            "{WORKSPACE} is not what `fidryn ingest uscode {ARCHIVES}` writes: {difference}; \
             write it with `cargo xtask uscode-sample --update`"
        );
    }

    let one = scratch.join("jobs-1");
    ingest(&archives, &one, 1)?;
    if let Some(difference) = first_difference(&eight, &one)? {
        bail!(
            "an ingest on one worker does not write the bytes of an ingest on eight: in the \
             output of one, {difference}"
        );
    }

    let report_file = scratch.join("report.json");
    let checked = fidryn(&[
        OsStr::new("check"),
        committed.as_os_str(),
        OsStr::new("--no-cache"),
        OsStr::new("--report"),
        report_file.as_os_str(),
    ])?;
    let report = read_json(&report_file)?;
    if !checked || report["errors"].as_u64() != Some(0) {
        bail!("`fidryn check {WORKSPACE} --no-cache` found errors, which it printed above");
    }
    eprintln!("+ fidryn coverage {WORKSPACE}");
    let mut counted = Vec::new();
    fidryn_cli::law::write_coverage(&committed, &mut counted)
        .map_err(|error| anyhow!("`fidryn coverage {WORKSPACE}` failed: {error}"))?;
    let committed_ledger =
        fs::read(committed.join(LEDGER)).with_context(|| format!("read {WORKSPACE}/{LEDGER}"))?;
    if counted != committed_ledger {
        bail!(
            "{WORKSPACE}/{LEDGER} is not what `fidryn coverage {WORKSPACE}` prints: they differ \
             first at line {}",
            first_differing_line(&committed_ledger, &counted)
        );
    }
    let ledger: Value = serde_json::from_slice(&committed_ledger)
        .with_context(|| format!("{WORKSPACE}/{LEDGER} holds no JSON"))?;

    let printed = printed_output(root, PROFILE_SCRIPT, profile_command(root))?;
    let committed_profile =
        fs::read(root.join(PROFILE)).with_context(|| format!("read {PROFILE}"))?;
    if let Some(problem) = profile_problem(&committed_profile, &printed) {
        bail!("{problem}");
    }
    let profile: Value = serde_json::from_slice(&printed)
        .with_context(|| format!("{PROFILE_SCRIPT} printed no JSON"))?;
    if let Some(problem) =
        references_problem(&profile["import"]["references"], &report["references"])
    {
        bail!("{problem}");
    }
    if let Some(problem) = ledger_problem(&profile["import"], &ledger) {
        bail!("{problem}");
    }

    let script = printed_output(root, TEXT_SCRIPT, text_command(root))?;
    let mut text = Vec::new();
    fidryn_cli::law::write_text(&committed, None, true, &mut text)
        .map_err(|error| anyhow!("`fidryn text {WORKSPACE} --json` failed: {error}"))?;
    if let Some(problem) = text_problem(&script, &text) {
        bail!("{problem}");
    }
    let records = text.iter().filter(|&&byte| byte == b'\n').count();

    let citations = check_curated(root)?;

    let page = fs::read_to_string(root.join(PAGE)).with_context(|| format!("read {PAGE}"))?;
    if page_region(&page)? != coverage_table(&ledger)? {
        bail!(
            "{PAGE}: the table between `{TABLE_BEGIN}` and `{TABLE_END}` is not what \
             {WORKSPACE}/{LEDGER} gives; write it with `cargo xtask uscode-sample --update`"
        );
    }
    eprintln!(
        "{ARCHIVES} matches {RECORD}; {WORKSPACE} is what the ingester writes from it, on one \
         worker and on eight, checks with no error, and holds the ledger `fidryn coverage` \
         prints; {PROFILE} is current and counts the references the check counts and what the \
         ledger counts; the text of its {records} records is what {TEXT_SCRIPT} prints; \
         {CURATED} checks against it with no error and no warning, each of its {citations} \
         citations `exact`; and the table of {PAGE} is what the ledger gives"
    );
    Ok(())
}

/// The number, from 1, of the first line at which `a` and `b` differ, or
/// the line after the shorter when one is the beginning of the other.
fn first_differing_line(a: &[u8], b: &[u8]) -> usize {
    let a_lines: Vec<&[u8]> = a.split_inclusive(|&byte| byte == b'\n').collect();
    let b_lines: Vec<&[u8]> = b.split_inclusive(|&byte| byte == b'\n').collect();
    a_lines
        .iter()
        .zip(&b_lines)
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| a_lines.len().min(b_lines.len()))
        + 1
}

/// Step 5, against the ledger: `None` when every key that `import`, the
/// profile's object of that name, and the `totals` of `ledger` share has one
/// value in both, and the keys only one of them has are those of
/// [`PROFILE_ONLY`], which are compared with the keys of the ledger's top,
/// and of [`LEDGER_ONLY`]. Each key those two name must be on its side: a
/// key named as the ledger's alone that the ledger lacks, or as the
/// profile's alone that the profile lacks, is a difference too. Otherwise
/// what differs, naming the first key, in the byte order of their paths,
/// with the value of each side.
fn ledger_problem(import: &Value, ledger: &Value) -> Option<String> {
    let shown = format!("{WORKSPACE}/{LEDGER}");
    let (Some(import), Some(totals)) = (import.as_object(), ledger["totals"].as_object()) else {
        return Some(format!(
            "{PROFILE} has no object `import`, or {shown} no object `totals`"
        ));
    };
    for key in LEDGER_ONLY {
        if !totals.contains_key(key) {
            return Some(format!(
                "the step names `totals.{key}` as a key of the ledger alone, and {shown} has no \
                 `totals.{key}`"
            ));
        }
    }
    for key in PROFILE_ONLY {
        if !import.contains_key(key) {
            return Some(format!(
                "the step names `import.{key}` as a key of the profile alone, and {PROFILE} has \
                 no `import.{key}`"
            ));
        }
    }
    for key in import.keys() {
        if PROFILE_ONLY.contains(&key.as_str()) {
            if totals.contains_key(key) {
                return Some(format!(
                    "{shown} has `totals.{key}`, which the step takes to stand at its top"
                ));
            }
        } else if !totals.contains_key(key) {
            return Some(format!(
                "{PROFILE} has `import.{key}`, and {shown} has no `totals.{key}`"
            ));
        }
    }
    for key in totals.keys() {
        if !import.contains_key(key) && !LEDGER_ONLY.contains(&key.as_str()) {
            return Some(format!(
                "{shown} has `totals.{key}`, and {PROFILE} has no `import.{key}`"
            ));
        }
    }
    for key in PROFILE_ONLY {
        if let Some(problem) = value_difference("", import.get(key), ledger.get(key)) {
            return Some(problem.shown(&format!("import.{key}"), key, &shown));
        }
    }
    let mut keys: Vec<&String> = import
        .keys()
        .filter(|key| totals.contains_key(*key))
        .collect();
    keys.sort();
    for key in keys {
        if let Some(problem) = value_difference("", import.get(key), totals.get(key)) {
            return Some(problem.shown(&format!("import.{key}"), &format!("totals.{key}"), &shown));
        }
    }
    None
}

/// Where two JSON values first differ: the path below the two compared,
/// and the value of each there, `None` where it has none.
struct ValueDifference {
    below: String,
    profiled: Option<Value>,
    counted: Option<Value>,
}

impl ValueDifference {
    /// The message of the difference of the profile's value at `profile_at`
    /// and the ledger's, `shown`, at `ledger_at`.
    fn shown(&self, profile_at: &str, ledger_at: &str, shown: &str) -> String {
        let value = |value: &Option<Value>| {
            value
                .as_ref()
                .map_or("nothing".to_owned(), |value| format!("`{value}`"))
        };
        format!(
            "{shown} has {} at `{ledger_at}{below}`, and {PROFILE} has {} at \
             `{profile_at}{below}`",
            value(&self.counted),
            value(&self.profiled),
            below = self.below
        )
    }
}

/// The first place, in the byte order of the keys, at which `profiled` and
/// `counted` differ; the keys of an object are compared whatever their
/// order. `None` when the two are equal.
fn value_difference(
    below: &str,
    profiled: Option<&Value>,
    counted: Option<&Value>,
) -> Option<ValueDifference> {
    if let (Some(Value::Object(profiled)), Some(Value::Object(counted))) = (profiled, counted) {
        let keys: BTreeSet<&String> = profiled.keys().chain(counted.keys()).collect();
        return keys.into_iter().find_map(|key| {
            value_difference(
                &format!("{below}.{key}"),
                profiled.get(key),
                counted.get(key),
            )
        });
    }
    (profiled != counted).then(|| ValueDifference {
        below: below.to_owned(),
        profiled: profiled.cloned(),
        counted: counted.cloned(),
    })
}

/// What `--update` writes of the page under `root`: the table between its
/// markers, what the committed ledger gives. Nothing else of the page
/// changes.
fn update_page(root: &Path) -> Result<()> {
    let ledger = read_json(&root.join(WORKSPACE).join(LEDGER))?;
    let path = root.join(PAGE);
    let page = fs::read_to_string(&path).with_context(|| format!("read {PAGE}"))?;
    let updated = with_region(&page, &coverage_table(&ledger)?)?;
    fs::write(&path, updated).with_context(|| format!("write {PAGE}"))
}

/// The positions in `page` of the end of its one begin marker and of the
/// start of its one end marker, which follows it.
fn region_bounds(page: &str) -> Result<(usize, usize)> {
    let begins: Vec<usize> = page.match_indices(TABLE_BEGIN).map(|(at, _)| at).collect();
    let ends: Vec<usize> = page.match_indices(TABLE_END).map(|(at, _)| at).collect();
    match (begins.as_slice(), ends.as_slice()) {
        ([begin], [end]) if begin + TABLE_BEGIN.len() <= *end => {
            Ok((begin + TABLE_BEGIN.len(), *end))
        }
        _ => bail!("{PAGE} must hold exactly one `{TABLE_BEGIN}` followed by one `{TABLE_END}`"),
    }
}

/// What `page` holds between its markers.
fn page_region(page: &str) -> Result<&str> {
    let (start, end) = region_bounds(page)?;
    Ok(&page[start..end])
}

/// `page` with `region` between its markers in place of what stood there.
fn with_region(page: &str, region: &str) -> Result<String> {
    let (start, end) = region_bounds(page)?;
    Ok(format!("{}{region}{}", &page[..start], &page[end..]))
}

/// `count` with a comma between each group of three digits.
fn grouped(count: u64) -> String {
    let digits = count.to_string();
    let mut shown = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            shown.push(',');
        }
        shown.push(digit);
    }
    shown
}

/// The count at `key` of the object `value`, which names `at`.
fn count_at(value: &Value, key: &str, at: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER} has no count at `{at}.{key}`"))
}

/// The counts of the object at `key` of `value`, each before its key:
/// `229 `courtRule`, 1 `level``, in the order of the object, or `none`.
fn counts_at(value: &Value, key: &str, at: &str) -> Result<String> {
    let object = value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER} has no object at `{at}.{key}`"))?;
    if object.is_empty() {
        return Ok("none".to_owned());
    }
    object
        .iter()
        .map(|(word, count)| {
            let count = count
                .as_u64()
                .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER}: `{at}.{key}.{word}` is no count"))?;
            Ok(format!("{} `{word}`", grouped(count)))
        })
        .collect::<Result<Vec<String>>>()
        .map(|counts| counts.join(", "))
}

/// The table of the page: one row for each key of the ledger `ledger`, in
/// its order, and one for each source file, each with what it counts, the
/// key it stands at and the figure or the figures.
fn coverage_table(ledger: &Value) -> Result<String> {
    let text = |key: &str| {
        ledger
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER} has no string at `{key}`"))
    };
    let totals = ledger
        .get("totals")
        .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER} has no `totals`"))?;
    let references = totals
        .get("references")
        .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER} has no `totals.references`"))?;
    let mut rows: Vec<(String, String, String)> = vec![
        (
            "The release".to_owned(),
            "release".to_owned(),
            format!("`{}`", text("release")?),
        ),
        (
            "The prefix of every address".to_owned(),
            "prefix".to_owned(),
            format!("`{}`", text("prefix")?),
        ),
    ];
    let mut row = |what: &str, key: &str, figure: String| {
        rows.push((what.to_owned(), format!("totals.{key}"), figure));
    };
    let count = |key: &str| count_at(totals, key, "totals").map(grouped);
    let counts = |key: &str| counts_at(totals, key, "totals");
    row("Units imported", "units", count("units")?);
    row("Units, by kind", "units_by_kind", counts("units_by_kind")?);
    row(
        "Units, by status (`in-force` for a unit with none)",
        "units_by_status",
        counts("units_by_status")?,
    );
    row("Containers imported", "containers", count("containers")?);
    row(
        "Containers, by kind",
        "containers_by_kind",
        counts("containers_by_kind")?,
    );
    row(
        "Provisions imported and addressed, the top provision of each unit included",
        "provisions",
        count("provisions")?,
    );
    row(
        "Addresses: aliases kept, and addresses disambiguated and assigned by the importer",
        "addresses",
        counts("addresses")?,
    );
    row("Notes imported, by layer", "notes", counts("notes")?);
    row(
        "Notes, by group",
        "notes_by_group",
        counts("notes_by_group")?,
    );
    row("Footnotes imported", "footnotes", count("footnotes")?);
    row(
        "Provisions and notes that hold an image, whose picture is not in the archive",
        "with_images",
        count("with_images")?,
    );
    for (layer, what) in [
        ("operative", "References in operative text, by outcome"),
        ("enacted", "References in enacted notes, by outcome"),
        ("editorial", "References in editorial matter, by outcome"),
    ] {
        row(
            what,
            &format!("references.{layer}"),
            counts_at(references, layer, "totals.references")?,
        );
    }
    row(
        "References of the outcome `outside`, by the body of law they name",
        "outside_by_body",
        counts("outside_by_body")?,
    );
    row(
        "Definitions extracted, by basis",
        "definitions",
        counts("definitions")?,
    );
    row(
        "Definitions extracted, by kind",
        "definitions_by_kind",
        counts("definitions_by_kind")?,
    );
    row(
        "Counted and not imported",
        "not_imported",
        counts("not_imported")?,
    );
    let sources = ledger
        .get("by_source")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER} has no array at `by_source`"))?;
    for (place, source) in sources.iter().enumerate() {
        let at = format!("by_source.{place}");
        let name = source
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("{WORKSPACE}/{LEDGER} has no string at `{at}.source`"))?;
        let flag = match source.get("positive_law") {
            Some(Value::Bool(true)) => "positive law",
            Some(Value::Bool(false)) => "not positive law",
            Some(Value::Null) => "no positive-law flag",
            _ => bail!("{WORKSPACE}/{LEDGER} has no flag at `{at}.positive_law`"),
        };
        rows.push((
            format!("The file `{name}` ({flag})"),
            at.clone(),
            format!(
                "{} units, {} provisions, {} definitions",
                grouped(count_at(source, "units", &at)?),
                grouped(count_at(source, "provisions", &at)?),
                grouped(count_at(source, "definitions", &at)?)
            ),
        ));
    }
    let mut table = String::from("\n\n| Counted | Key | Figure |\n| --- | --- | --- |\n");
    for (what, key, figure) in rows {
        table.push_str(&format!("| {what} | `{key}` | {figure} |\n"));
    }
    table.push('\n');
    Ok(table)
}

/// A directory of its own under the system's directory for temporary
/// files, removed when it is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "fidryn-uscode-sample-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
        Ok(Scratch(path))
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs `fidryn ARGS...` in this process, as the binary runs it, and says
/// whether it exited 0. What it prints goes to this process's stdout and
/// stderr.
fn fidryn(args: &[&OsStr]) -> Result<bool> {
    let words: Vec<OsString> = std::iter::once(OsStr::new("fidryn"))
        .chain(args.iter().copied())
        .map(OsStr::to_owned)
        .collect();
    let shown: Vec<String> = words
        .iter()
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    eprintln!("+ {}", shown.join(" "));
    let cli = fidryn_cli::Cli::try_parse_from(&words)
        .map_err(|error| anyhow!("`{}`: {error}", shown.join(" ")))?;
    Ok(fidryn_cli::run(cli) == ExitCode::SUCCESS)
}

/// `fidryn ingest uscode ARCHIVES --out OUT --jobs JOBS`, which must succeed.
fn ingest(archives: &Path, out: &Path, jobs: u32) -> Result<()> {
    let jobs = jobs.to_string();
    let ingested = fidryn(&[
        OsStr::new("ingest"),
        OsStr::new("uscode"),
        archives.as_os_str(),
        OsStr::new("--out"),
        out.as_os_str(),
        OsStr::new("--jobs"),
        OsStr::new(&jobs),
    ])?;
    if !ingested {
        bail!(
            "`fidryn ingest uscode {ARCHIVES} --out {} --jobs {jobs}` failed, as it printed above",
            out.display()
        );
    }
    Ok(())
}

/// `count` and `noun`, the noun in the plural unless the count is 1.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Step 7: the check of the curated sample under `root`, in this process
/// and with no cache, as `fidryn check samples/uscode/curated --no-cache`
/// runs it. It gives the number of the sample's citations, and fails,
/// naming the file and the first thing that is wrong, on an error, on a
/// warning, such as a stale binding, on a citation whose outcome is not
/// `exact`, and, naming the curated sample, when the check counts no
/// citation at all.
fn check_curated(root: &Path) -> Result<u64> {
    eprintln!("+ fidryn check {CURATED} --no-cache");
    let report = check_workspace(&root.join(CURATED), &curated_options());
    if let Some(first) = report.diagnostics.first() {
        let mut shown = first.clone();
        shown.source_file = Some(match &first.source_file {
            Some(file) => format!("{CURATED}/{file}"),
            None => CURATED.to_owned(),
        });
        let mut problem = format!(
            "{shown}; `fidryn check {CURATED} --no-cache` finds {} and {}, and the curated sample \
             checks with none",
            counted(report.errors(), "error"),
            counted(report.warnings(), "warning")
        );
        if first.code == DiagnosticCode::E740 {
            problem.push_str(
                "; when the generated sample was written again, write the pin with `cargo xtask \
                 uscode-sample --update`",
            );
        }
        bail!("{problem}");
    }
    if let Some(citation) = report.unresolved_citations.first() {
        bail!(
            "{CURATED}/{}: the citation `{}` of `{}` names `{}`, whose outcome is `{}`, and every \
             citation of the curated sample is `exact`; {} of its citations are not",
            module_path(&citation.from),
            citation.citation,
            citation.from,
            citation.target,
            citation.outcome.name(),
            report.unresolved_citations.len()
        );
    }
    let citations = report.citations.values().sum();
    if citations == 0 {
        bail!(
            "{CURATED}: `fidryn check {CURATED} --no-cache` counts no citation, and the curated \
             sample cites provisions of the generated one"
        );
    }
    Ok(citations)
}

/// How step 7 checks the curated sample: as `--no-cache` does, with no
/// cache, on as many workers as are available.
fn curated_options() -> CheckOptions {
    CheckOptions {
        jobs: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
        cache_dir: None,
        toolchain: TOOLCHAIN.to_owned(),
        executable: String::new(),
    }
}

/// What `--update` writes of the curated manifest under `root`, after the
/// generated sample: the release and the lock digest of the dependency at
/// `../workspace`, those of the generated sample's lock, and the manifest's
/// toolchain, the running one. Nothing else of the file changes, and it is
/// written in the canonical form of a manifest. The curated module is not
/// touched.
fn update_curated(root: &Path) -> Result<()> {
    let shown = format!("{CURATED}/{MANIFEST}");
    let path = root.join(CURATED).join(MANIFEST);
    let text = fs::read_to_string(&path).with_context(|| format!("read {shown}"))?;
    let mut manifest = Manifest::from_json(&text).map_err(|error| anyhow!("{shown}: {error}"))?;
    let lock_shown = format!("{WORKSPACE}/{LOCK}");
    let lock_text = fs::read_to_string(root.join(WORKSPACE).join(LOCK))
        .with_context(|| format!("read {lock_shown}"))?;
    let lock = Lock::from_json(&lock_text).map_err(|error| anyhow!("{lock_shown}: {error}"))?;
    let Some(entry) = manifest
        .dependencies
        .iter_mut()
        .find(|entry| entry.path == DEPENDENCY_PATH)
    else {
        bail!("{shown} names no dependency at `{DEPENDENCY_PATH}`");
    };
    entry.release = lock.release;
    entry.lock_digest = Lock::digest(&lock_text);
    manifest.toolchain = TOOLCHAIN.to_owned();
    fs::write(&path, manifest.to_json()).with_context(|| format!("write {shown}"))
}

fn read_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} holds no JSON", path.display()))
}

/// What a tree holds at a path.
#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    /// Neither a file nor a directory, such as a symbolic link, which is not
    /// followed.
    Other,
}

/// Every entry below `root`, by its path relative to `root` with `/`
/// between its segments, but for an entry whose name begins with a dot and
/// what lies below such a directory, which are not a workspace's.
fn tree(root: &Path) -> Result<BTreeMap<String, Entry>> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![(root.to_owned(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let listed = fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))?;
        for entry in listed {
            let entry = entry.with_context(|| format!("read {}", dir.display()))?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|name| anyhow!("{}: the name {name:?} is not UTF-8", dir.display()))?;
            if name.starts_with('.') {
                continue;
            }
            let rel = format!("{prefix}{name}");
            let kind = entry
                .file_type()
                .with_context(|| format!("read {}", entry.path().display()))?;
            if kind.is_dir() {
                pending.push((entry.path(), format!("{rel}/")));
                entries.insert(rel, Entry::Directory);
            } else if kind.is_file() {
                let bytes = fs::read(entry.path())
                    .with_context(|| format!("read {}", entry.path().display()))?;
                entries.insert(rel, Entry::File(bytes));
            } else {
                entries.insert(rel, Entry::Other);
            }
        }
    }
    Ok(entries)
}

/// The first path, in the byte order of the paths, at which the tree
/// `other` differs from the tree `expected`, with how and with the number of
/// paths that differ: `missing` when `other` lacks what `expected` holds,
/// `extra` when it holds what `expected` lacks, and `different` when the two
/// hold different bytes or different kinds of entry. `None` when every
/// directory and file of each is in the other with the same bytes.
fn first_difference(expected: &Path, other: &Path) -> Result<Option<String>> {
    let expected = tree(expected)?;
    let other = tree(other)?;
    let paths: BTreeSet<&String> = expected.keys().chain(other.keys()).collect();
    let mut differing = paths.into_iter().filter_map(|path| {
        let word = match (expected.get(path), other.get(path)) {
            (Some(a), Some(b)) if a == b => return None,
            (Some(_), Some(_)) => "different",
            (Some(_), None) => "missing",
            (None, _) => "extra",
        };
        Some(format!("{word} {path}"))
    });
    let Some(first) = differing.next() else {
        return Ok(None);
    };
    let count = 1 + differing.count();
    let noun = if count == 1 {
        "path differs"
    } else {
        "paths differ"
    };
    Ok(Some(format!("{first} ({count} {noun})")))
}

/// What differs between the counts of references by layer and outcome of
/// the profile and of the check report, naming the first count that
/// differs; `None` when every one is the same.
fn references_problem(profile: &Value, report: &Value) -> Option<String> {
    for layer in LAYERS {
        for outcome in Outcome::NAMES {
            let count = |counts: &Value| counts.get(layer)?.get(outcome)?.as_u64();
            let (profiled, reported) = (count(profile), count(report));
            if profiled != reported {
                let shown = |count: Option<u64>| count.map_or("no".to_owned(), |n| n.to_string());
                return Some(format!(
                    "the check report of {WORKSPACE} counts {} references of the {layer} layer \
                     with the outcome `{outcome}`, and {PROFILE} counts {}",
                    shown(reported),
                    shown(profiled)
                ));
            }
        }
    }
    None
}

/// What differs between the lines the script prints and those `fidryn
/// text` prints, naming the first line that differs, the address of its
/// record and the bytes around the first that differs; `None` when the two
/// are the same.
fn text_problem(script: &[u8], text: &[u8]) -> Option<String> {
    if script == text {
        return None;
    }
    let script_lines: Vec<&[u8]> = script.split_inclusive(|&byte| byte == b'\n').collect();
    let text_lines: Vec<&[u8]> = text.split_inclusive(|&byte| byte == b'\n').collect();
    let first = script_lines
        .iter()
        .zip(&text_lines)
        .position(|(script, text)| script != text)
        .unwrap_or_else(|| script_lines.len().min(text_lines.len()));
    let line = first + 1;
    let what = format!("{TEXT_SCRIPT} on {ARCHIVES} and `fidryn text {WORKSPACE} --json`");
    match (script_lines.get(first), text_lines.get(first)) {
        (Some(script), Some(text)) => {
            let at = script
                .iter()
                .zip(text.iter())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| script.len().min(text.len()));
            Some(format!(
                "{what} differ first at line {line}, the record of `{}`, at byte {at}: the \
                 script prints `{}` and fidryn text `{}`",
                address_of(script),
                around(script, at),
                around(text, at)
            ))
        }
        (Some(script), None) => Some(format!(
            "{what} differ: the script prints {} lines and fidryn text {}; the first it does not \
             print is line {line}, the record of `{}`",
            script_lines.len(),
            text_lines.len(),
            address_of(script)
        )),
        (None, Some(text)) => Some(format!(
            "{what} differ: the script prints {} lines and fidryn text {}; the first the script \
             does not print is line {line}, the record of `{}`",
            script_lines.len(),
            text_lines.len(),
            address_of(text)
        )),
        (None, None) => Some(format!("{what} differ")),
    }
}

/// The address of the record of `line`, or `?` when it holds none.
fn address_of(line: &[u8]) -> String {
    serde_json::from_slice::<Value>(line)
        .ok()
        .and_then(|record| record.get("address")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| "?".to_owned())
}

/// The bytes of `line` from up to 40 before `at` to up to 40 after it, line
/// end left out.
fn around(line: &[u8], at: usize) -> String {
    let start = at.saturating_sub(40);
    let end = (at + 40).min(line.len());
    String::from_utf8_lossy(&line[start.min(end)..end])
        .trim_end_matches('\n')
        .to_owned()
}

/// What differs between the archives in `dir` and the retrieval record
/// beside them, one line for each, naming the archive. An error when the
/// directory or the record cannot be read.
fn archive_problems(dir: &Path) -> Result<Vec<String>> {
    let shown = dir.display();
    let text = fs::read_to_string(dir.join(RECORD))
        .with_context(|| format!("read {RECORD} in {shown}"))?;
    let record = RetrievalRecord::from_json(&text).map_err(|error| anyhow!("{RECORD}: {error}"))?;
    let mut problems = Vec::new();
    if record.to_json() != text {
        problems.push(format!(
            "{RECORD} is not in the layout of a retrieval record"
        ));
    }
    let mut recorded = BTreeMap::new();
    for archive in &record.archives {
        if recorded.insert(archive.file.as_str(), archive).is_some() {
            problems.push(format!("{}: recorded twice in {RECORD}", archive.file));
        }
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("read directory {shown}"))? {
        let entry = entry.with_context(|| format!("read directory {shown}"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| anyhow!("{shown}: file name {name:?} is not UTF-8"))?;
        let is_file = entry
            .file_type()
            .with_context(|| format!("read {name}"))?
            .is_file();
        names.push((name, is_file));
    }
    names.sort();
    let mut present = Vec::new();
    for (name, is_file) in names {
        if name == RECORD && is_file {
            continue;
        }
        if !is_file || !name.ends_with(".zip") {
            problems.push(format!("{name}: neither an archive nor {RECORD}"));
            continue;
        }
        let Some(archive) = recorded.get(name.as_str()) else {
            problems.push(format!("{name}: an archive with no record in {RECORD}"));
            continue;
        };
        let bytes = fs::read(dir.join(&name)).with_context(|| format!("read {name}"))?;
        let size = bytes.len() as u64;
        if size != archive.bytes {
            problems.push(format!(
                "{name}: {size} bytes, the record gives {}",
                archive.bytes
            ));
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        if sha256 != archive.sha256 {
            problems.push(format!(
                "{name}: sha256 is {sha256}, the record gives {}",
                archive.sha256
            ));
        }
        let blake3 = Hash32(*blake3::hash(&bytes).as_bytes());
        if blake3 != archive.blake3 {
            problems.push(format!(
                "{name}: blake3 is {blake3}, the record gives {}",
                archive.blake3
            ));
        }
        present.push(name);
    }
    for file in recorded.keys() {
        if !present.iter().any(|name| name == file) {
            problems.push(format!(
                "{file}: recorded in {RECORD}, and not in the directory"
            ));
        }
    }
    Ok(problems)
}

/// `uv run --locked python -I SCRIPT samples/uscode/archives`, from `root`:
/// the downloaded archives are read by isolated Python.
fn script_command(root: &Path, script: &str) -> Command {
    let mut cmd = Command::new("uv");
    cmd.current_dir(root);
    cmd.args(["run", "--locked", "python", "-I", script, ARCHIVES]);
    cmd
}

/// The command that prints the profile of the archives.
fn profile_command(root: &Path) -> Command {
    script_command(root, PROFILE_SCRIPT)
}

/// The command that prints the text records of the archives.
fn text_command(root: &Path) -> Command {
    script_command(root, TEXT_SCRIPT)
}

/// What `command`, which runs `script` on the archives, prints to stdout;
/// an error when it fails.
fn printed_output(root: &Path, script: &str, mut command: Command) -> Result<Vec<u8>> {
    let shown: Vec<String> = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    eprintln!("+ {}", shown.join(" "));
    let output = command
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to spawn uv in {}", root.display()))?;
    if !output.status.success() {
        bail!("{script} failed on {ARCHIVES}: {}", output.status);
    }
    Ok(output.stdout)
}

/// `None` when the committed profile is the printed one byte for byte;
/// otherwise what differs, with the first line that differs.
fn profile_problem(committed: &[u8], printed: &[u8]) -> Option<String> {
    if committed == printed {
        return None;
    }
    let committed_lines: Vec<&[u8]> = committed.split_inclusive(|&byte| byte == b'\n').collect();
    let printed_lines: Vec<&[u8]> = printed.split_inclusive(|&byte| byte == b'\n').collect();
    let first = committed_lines
        .iter()
        .zip(&printed_lines)
        .position(|(committed, printed)| committed != printed)
        .unwrap_or_else(|| committed_lines.len().min(printed_lines.len()));
    Some(format!(
        "{PROFILE} is not what {PROFILE_SCRIPT} prints for {ARCHIVES}: they differ first at line {}; \
         regenerate it with `uv run --locked python -I {PROFILE_SCRIPT} {ARCHIVES} --out {PROFILE}`",
        first + 1
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::scratch;
    use fidryn_provision::workspace::RetrievedArchive;

    const FIRST: &[u8] = b"Invented archive bytes for the tests of the sample step.\n";
    const SECOND: &[u8] = b"Other invented archive bytes for the tests of the sample step.\n";

    fn recorded(file: &str, bytes: &[u8]) -> RetrievedArchive {
        RetrievedArchive {
            file: file.to_owned(),
            url: format!("https://example.invalid/releases/42/7/{file}"),
            retrieved_at: "2026-11-16T22:14:03Z".to_owned(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            blake3: Hash32(*blake3::hash(bytes).as_bytes()),
        }
    }

    /// A directory of two invented archives and the record that matches them.
    fn sample(name: &str) -> PathBuf {
        let dir = scratch("xtask-uscode-sample-", name);
        fs::write(dir.join("demo01@42-7.zip"), FIRST).expect("write an archive");
        fs::write(dir.join("demo02@42-7.zip"), SECOND).expect("write an archive");
        let record = RetrievalRecord {
            release: "42-7".to_owned(),
            archives: vec![
                recorded("demo01@42-7.zip", FIRST),
                recorded("demo02@42-7.zip", SECOND),
            ],
        };
        fs::write(dir.join(RECORD), record.to_json()).expect("write the record");
        dir
    }

    fn problems_of(dir: &Path) -> Vec<String> {
        let problems = archive_problems(dir).expect("read the sample");
        fs::remove_dir_all(dir).expect("remove scratch dir");
        problems
    }

    #[test]
    fn archives_equal_to_their_record_pass() {
        let dir = sample("equal");
        assert_eq!(problems_of(&dir), Vec::<String>::new());
    }

    #[test]
    fn the_sample_step_names_an_archive_that_differs_from_its_record() {
        let dir = sample("differs");
        let mut changed = SECOND.to_vec();
        changed[0] ^= 1;
        fs::write(dir.join("demo02@42-7.zip"), &changed).expect("change an archive");
        let problems = problems_of(&dir);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems[0].starts_with("demo02@42-7.zip: sha256 is "),
            "{problems:?}"
        );
        assert!(
            problems[1].starts_with("demo02@42-7.zip: blake3 is "),
            "{problems:?}"
        );
    }

    #[test]
    fn the_sample_step_names_an_archive_whose_size_differs() {
        let dir = sample("size");
        fs::write(dir.join("demo01@42-7.zip"), &FIRST[1..]).expect("change an archive");
        let problems = problems_of(&dir);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert_eq!(
            problems[0],
            format!(
                "demo01@42-7.zip: {} bytes, the record gives {}",
                FIRST.len() - 1,
                FIRST.len()
            )
        );
    }

    #[test]
    fn the_sample_step_names_an_archive_without_a_record_and_a_record_without_its_archive() {
        let dir = sample("unmatched");
        fs::remove_file(dir.join("demo01@42-7.zip")).expect("remove an archive");
        fs::write(dir.join("demo03@42-7.zip"), FIRST).expect("write an archive");
        assert_eq!(
            problems_of(&dir),
            [
                "demo03@42-7.zip: an archive with no record in retrieval.json",
                "demo01@42-7.zip: recorded in retrieval.json, and not in the directory",
            ]
        );
    }

    #[test]
    fn the_sample_step_names_a_file_that_is_neither_an_archive_nor_the_record() {
        let dir = sample("other-file");
        fs::write(dir.join("notes.txt"), "Invented notes.\n").expect("write a file");
        fs::create_dir(dir.join("demo04@42-7.zip")).expect("make a directory");
        assert_eq!(
            problems_of(&dir),
            [
                "demo04@42-7.zip: neither an archive nor retrieval.json",
                "notes.txt: neither an archive nor retrieval.json",
            ]
        );
    }

    #[test]
    fn the_sample_step_names_a_record_out_of_its_layout() {
        let dir = sample("layout");
        let text = fs::read_to_string(dir.join(RECORD)).expect("read the record");
        let value: serde_json::Value = serde_json::from_str(&text).expect("a record is JSON");
        fs::write(dir.join(RECORD), value.to_string()).expect("write the record on one line");
        assert_eq!(
            problems_of(&dir),
            ["retrieval.json is not in the layout of a retrieval record"]
        );
    }

    #[test]
    fn a_directory_without_a_record_cannot_be_checked() {
        let dir = sample("no-record");
        fs::remove_file(dir.join(RECORD)).expect("remove the record");
        let error = archive_problems(&dir).expect_err("no record");
        fs::remove_dir_all(&dir).expect("remove scratch dir");
        assert!(format!("{error:#}").contains(RECORD), "{error:#}");
    }

    #[test]
    fn the_profile_is_printed_by_isolated_python_on_the_locked_project() {
        let root = Path::new("/repo");
        for (cmd, script) in [
            (profile_command(root), "conformance/usc_profile.py"),
            (text_command(root), "conformance/uslm_text.py"),
        ] {
            assert_eq!(cmd.get_program(), "uv");
            let args: Vec<&OsStr> = cmd.get_args().collect();
            let words = [
                "run",
                "--locked",
                "python",
                "-I",
                script,
                "samples/uscode/archives",
            ];
            let want: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
            assert_eq!(args, want);
            assert_eq!(cmd.get_current_dir(), Some(root));
        }
    }

    #[test]
    fn a_stale_profile_is_named_with_its_first_differing_line() {
        let committed = b"{\n  \"units\": 1,\n  \"notes\": 2\n}\n";
        assert_eq!(profile_problem(committed, committed), None);
        let printed = b"{\n  \"units\": 1,\n  \"notes\": 3\n}\n";
        let problem = profile_problem(committed, printed).expect("a stale profile");
        assert!(problem.starts_with(PROFILE), "{problem}");
        assert!(problem.contains("line 3"), "{problem}");
        let problem = profile_problem(&committed[..committed.len() - 1], committed)
            .expect("a profile without its final line end");
        assert!(problem.contains("line 4"), "{problem}");
    }

    /// Copies every entry below `from` to `to`.
    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("create a directory");
        for entry in
            fs::read_dir(from).unwrap_or_else(|error| panic!("{}: {error}", from.display()))
        {
            let entry = entry.expect("an entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("the kind of an entry").is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), &target).expect("copy a file");
            }
        }
    }

    #[test]
    fn the_first_difference_of_two_trees_is_named() {
        let dir = scratch("xtask-uscode-sample-", "trees");
        let (a, b) = (dir.join("a"), dir.join("b"));
        for (rel, text) in [
            ("workspace.json", "{}\n"),
            ("zz/demo/t1.fr", "edition 2\n"),
            ("zz/demo/t1/s1.fr", "edition 2\n"),
        ] {
            crate::test_support::write(&a, rel, text);
        }
        copy_tree(&a, &b);
        // Entries whose names begin with a dot are not a workspace's.
        crate::test_support::write(&b, ".fidryn-cache/v1/00/x.json", "{}\n");
        let difference = |a: &Path, b: &Path| first_difference(a, b).expect("read the trees");
        assert_eq!(difference(&a, &b), None);

        crate::test_support::write(&b, "zz/demo/t1/s1.fr", "edition 2\n\n");
        assert_eq!(
            difference(&a, &b).as_deref(),
            Some("different zz/demo/t1/s1.fr (1 path differs)")
        );
        fs::remove_file(b.join("zz/demo/t1/s1.fr")).expect("remove a file");
        assert_eq!(
            difference(&a, &b).as_deref(),
            Some("missing zz/demo/t1/s1.fr (1 path differs)")
        );
        // An empty directory is an entry like any other; one in place of a
        // file is different.
        fs::create_dir(b.join("zz/demo/t1/s1.fr")).expect("make a directory");
        fs::create_dir(b.join("zz/demo/t2")).expect("make a directory");
        crate::test_support::write(&b, "notes.txt", "Invented notes.\n");
        assert_eq!(
            difference(&a, &b).as_deref(),
            Some("extra notes.txt (3 paths differ)")
        );
        assert_eq!(
            difference(&b, &a).as_deref(),
            Some("missing notes.txt (3 paths differ)")
        );
        fs::remove_dir_all(&dir).expect("remove scratch dir");
    }

    #[test]
    fn the_counts_of_references_are_compared_by_layer_and_outcome() {
        let counts = |operative_exact: u64| {
            let mut references = serde_json::Map::new();
            for layer in LAYERS {
                let mut outcomes = serde_json::Map::new();
                for outcome in Outcome::NAMES {
                    let count = if layer == "operative" && outcome == "exact" {
                        operative_exact
                    } else {
                        1
                    };
                    outcomes.insert(outcome.to_owned(), Value::from(count));
                }
                references.insert(layer.to_owned(), Value::Object(outcomes));
            }
            Value::Object(references)
        };
        assert_eq!(references_problem(&counts(7), &counts(7)), None);
        assert_eq!(
            references_problem(&counts(7), &counts(8)).as_deref(),
            Some(
                "the check report of samples/uscode/workspace counts 8 references of the \
                 operative layer with the outcome `exact`, and samples/uscode/profile.json \
                 counts 7"
            )
        );
        let mut missing = counts(7);
        missing["editorial"]
            .as_object_mut()
            .expect("an object")
            .remove("malformed");
        let problem = references_problem(&missing, &counts(7)).expect("a missing count");
        assert!(
            problem.contains("editorial layer with the outcome `malformed`")
                && problem.ends_with("counts no"),
            "{problem}"
        );
    }

    /// A profile's `import` and a ledger that agree, in invented counts.
    fn agreeing() -> (Value, Value) {
        let import = serde_json::json!({
            "notes": { "editorial": 1, "enacted": 2 },
            "prefix": "/zz/demo",
            "release": "42-7",
            "units": 3,
            "units_by_kind": { "section": 3 },
        });
        let ledger = serde_json::json!({
            "schema": "fidryn.coverage/v0.1",
            "prefix": "/zz/demo",
            "release": "42-7",
            "totals": {
                "units": 3,
                "units_by_kind": { "section": 3 },
                "notes": { "enacted": 2, "editorial": 1 },
                "definitions": { "enacted": 1, "restated": 0, "unresolved": 0 },
                "definitions_by_kind": { "means": 1 },
            },
            "by_source": [],
        });
        (import, ledger)
    }

    #[test]
    fn the_ledger_is_compared_with_the_profile_key_for_key() {
        let (import, ledger) = agreeing();
        assert_eq!(ledger_problem(&import, &ledger), None);
        let problem = |change: &dyn Fn(&mut Value, &mut Value)| {
            let (mut import, mut ledger) = agreeing();
            change(&mut import, &mut ledger);
            ledger_problem(&import, &ledger).expect("a difference")
        };
        assert_eq!(
            problem(&|_, ledger| ledger["totals"]["notes"]["editorial"] = Value::from(2)),
            "samples/uscode/workspace/coverage.json has `2` at `totals.notes.editorial`, and \
             samples/uscode/profile.json has `1` at `import.notes.editorial`"
        );
        assert_eq!(
            problem(&|_, ledger| ledger["totals"]["units_by_kind"]["rule"] = Value::from(1)),
            "samples/uscode/workspace/coverage.json has `1` at `totals.units_by_kind.rule`, and \
             samples/uscode/profile.json has nothing at `import.units_by_kind.rule`"
        );
        assert_eq!(
            problem(&|import, _| import["release"] = Value::from("42-8")),
            "samples/uscode/workspace/coverage.json has `\"42-7\"` at `release`, and \
             samples/uscode/profile.json has `\"42-8\"` at `import.release`"
        );
        assert_eq!(
            problem(&|import, _| import["footnotes"] = Value::from(0)),
            "samples/uscode/profile.json has `import.footnotes`, and \
             samples/uscode/workspace/coverage.json has no `totals.footnotes`"
        );
        assert_eq!(
            problem(&|_, ledger| ledger["totals"]["footnotes"] = Value::from(0)),
            "samples/uscode/workspace/coverage.json has `totals.footnotes`, and \
             samples/uscode/profile.json has no `import.footnotes`"
        );
        // A key of the ledger alone that the profile comes to have is
        // compared like any other.
        assert!(
            problem(&|import, _| import["definitions"] = Value::from(1))
                .contains("`totals.definitions`")
        );
        // A key the step names as one side's alone must be on that side,
        // whether or not the other side has it.
        let remove = |value: &mut Value, key: &str| {
            value
                .as_object_mut()
                .expect("an object")
                .remove(key)
                .expect("the key is there");
        };
        assert_eq!(
            problem(&|_, ledger| remove(&mut ledger["totals"], "definitions_by_kind")),
            "the step names `totals.definitions_by_kind` as a key of the ledger alone, and \
             samples/uscode/workspace/coverage.json has no `totals.definitions_by_kind`"
        );
        assert_eq!(
            problem(&|import, ledger| {
                remove(&mut ledger["totals"], "definitions");
                import["definitions"] = Value::from(1);
            }),
            "the step names `totals.definitions` as a key of the ledger alone, and \
             samples/uscode/workspace/coverage.json has no `totals.definitions`"
        );
        assert_eq!(
            problem(&|import, _| remove(import, "release")),
            "the step names `import.release` as a key of the profile alone, and \
             samples/uscode/profile.json has no `import.release`"
        );
    }

    #[test]
    fn the_table_of_the_page_is_what_the_ledger_gives() {
        let root = workspace_root();
        let ledger = read_json(&root.join(WORKSPACE).join(LEDGER)).expect("the sample's ledger");
        let page = read_text(&root.join(PAGE));
        let table = coverage_table(&ledger).unwrap_or_else(|error| panic!("{error:#}"));
        assert_eq!(page_region(&page).expect("the markers"), table);
        assert!(
            table.contains("| Units imported | `totals.units` | 563 |"),
            "{table}"
        );
        assert!(
            table.contains("| Provisions imported and addressed, the top provision of each unit included | `totals.provisions` | 4,857 |"),
            "{table}"
        );

        // A figure of the ledger changes the table, and the update writes
        // the table alone.
        let mut changed = ledger.clone();
        changed["totals"]["units"] = Value::from(1);
        let other = coverage_table(&changed).expect("a table");
        assert_ne!(other, table);
        let updated = with_region(&page, &other).expect("the markers");
        assert_eq!(page_region(&updated).expect("the markers"), other);
        assert_eq!(with_region(&updated, &table).expect("the markers"), page);

        let broken = [
            "no markers\n".to_owned(),
            format!("{TABLE_END}\n{TABLE_BEGIN}\n"),
            format!("{TABLE_BEGIN}\n{TABLE_END}\n{TABLE_BEGIN}\n{TABLE_END}\n"),
        ];
        for page in broken {
            let error = page_region(&page).expect_err("broken markers");
            assert!(format!("{error:#}").contains(PAGE), "{error:#}");
        }
        let mut missing = ledger;
        missing["totals"]
            .as_object_mut()
            .expect("an object")
            .remove("footnotes");
        let error = coverage_table(&missing).expect_err("a count is missing");
        assert!(
            format!("{error:#}").contains("`totals.footnotes`"),
            "{error:#}"
        );
        assert_eq!(
            [0, 7, 999, 1000, 4857, 1_000_000].map(grouped),
            ["0", "7", "999", "1,000", "4,857", "1,000,000"]
        );
    }

    /// Every key of `value`, at any depth.
    fn keys_of(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                for (key, inner) in object {
                    out.push(key.clone());
                    keys_of(inner, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    keys_of(item, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn the_ledger_never_says_encoded() {
        let root = workspace_root();
        let ledger = read_json(&root.join(WORKSPACE).join(LEDGER)).expect("the sample's ledger");
        let mut keys = Vec::new();
        keys_of(&ledger, &mut keys);
        assert!(keys.len() > 60, "{keys:?}");
        for key in &keys {
            assert!(!key.to_ascii_lowercase().contains("encod"), "{key}");
        }
        let page = read_text(&root.join(PAGE));
        for sentence in page.split(['.', '\n']) {
            assert!(
                !sentence.to_lowercase().contains("encod"),
                "{PAGE}: {sentence}"
            );
        }
        assert!(page.contains("imported") && page.contains("addressed"));
        assert!(page.contains("classified") && page.contains("extracted"));
    }

    #[test]
    fn the_text_is_compared_line_for_line() {
        let script = b"{\"address\":\"/zz/demo/t1\",\"p\":\"A widget.\"}\n\
                       {\"address\":\"/zz/demo/t1/s1\",\"p\":\"A small widget.\"}\n";
        assert_eq!(text_problem(script, script), None);
        let text = b"{\"address\":\"/zz/demo/t1\",\"p\":\"A widget.\"}\n\
                     {\"address\":\"/zz/demo/t1/s1\",\"p\":\"A large widget.\"}\n";
        let problem = text_problem(script, text).expect("a difference");
        assert!(
            problem.contains("differ first at line 2, the record of `/zz/demo/t1/s1`, at byte 35"),
            "{problem}"
        );
        assert!(problem.contains("A small widget"), "{problem}");
        assert!(problem.contains("A large widget"), "{problem}");
        // The first line, of 42 bytes, alone.
        let problem = text_problem(script, &script[..42]).expect("a line fewer");
        assert!(
            problem.contains("the script prints 2 lines and fidryn text 1")
                && problem.contains("line 2, the record of `/zz/demo/t1/s1`"),
            "{problem}"
        );
    }

    /// A copy of the generated and of the curated sample in a scratch
    /// directory, each at its path.
    fn copy_of_the_samples(name: &str) -> PathBuf {
        let root = workspace_root();
        let copy = scratch("xtask-uscode-sample-", name);
        copy_tree(&root.join(WORKSPACE), &copy.join(WORKSPACE));
        copy_tree(&root.join(CURATED), &copy.join(CURATED));
        copy
    }

    fn read_text(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    fn manifest_of(text: &str) -> Manifest {
        Manifest::from_json(text).unwrap_or_else(|error| panic!("{error}"))
    }

    /// The citations of the committed curated module, by name, in its order.
    const CITATIONS: [&str; 8] = [
        "words_denoting_number",
        "monthly_agreement_reports",
        "reported_agreements",
        "railway_labor_definitions",
        "subsistence_allowances",
        "bankruptcy_rules_scope",
        "bankruptcy_rules_in_general",
        "respect_for_marriage_short_title",
    ];

    /// The name of each citation of the curated module under `root`, in
    /// its order, with the outcome that the check of the curated sample
    /// gives it: the outcome the unresolved citations record for it, and
    /// otherwise `exact`, every citation being counted.
    fn citation_outcomes(root: &Path) -> Vec<(String, &'static str)> {
        let curated = root.join(CURATED);
        let report = check_workspace(&curated, &curated_options());
        let modules = fidryn_driver::workspace::read_modules(&curated, 1)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
        let [fidryn_provision::Module::Curated(module)] = modules.as_slice() else {
            panic!("the curated sample holds one curated module: {modules:#?}");
        };
        let counted: u64 = report.citations.values().sum();
        assert_eq!(counted, module.citations.len() as u64, "{report:#?}");
        module
            .citations
            .iter()
            .map(|citation| {
                let outcome = report
                    .unresolved_citations
                    .iter()
                    .find(|each| each.from == module.address && each.citation == citation.name)
                    .map_or("exact", |each| each.outcome.name());
                (citation.name.clone(), outcome)
            })
            .collect()
    }

    #[test]
    fn the_curated_sample_checks_against_the_generated_one() {
        let copy = copy_of_the_samples("curated-clean");
        let committed = read_text(&copy.join(CURATED).join(MANIFEST));
        assert_eq!(manifest_of(&committed).to_json(), committed);
        let citations = check_curated(&copy).unwrap_or_else(|error| panic!("{error:#}"));
        assert_eq!(citations, CITATIONS.len() as u64);
        let expected: Vec<(String, &str)> = CITATIONS
            .iter()
            .map(|name| ((*name).to_owned(), "exact"))
            .collect();
        assert_eq!(citation_outcomes(&copy), expected);
        fs::remove_dir_all(&copy).expect("remove scratch dir");
    }

    #[test]
    fn a_stale_binding_fails_the_step_and_names_the_module() {
        let copy = copy_of_the_samples("curated-stale");
        let module = copy.join(CURATED).join("zz/sample/citations.fr");
        let original = read_text(&module);
        // The binding records another tree hash than the provision's.
        let at = original
            .find("provision_hash \"")
            .expect("the module has a binding")
            + 16;
        let mut changed = original.clone();
        let digit = if &original[at..=at] == "0" { "1" } else { "0" };
        changed.replace_range(at..=at, digit);
        fs::write(&module, changed).expect("change the module");
        let error = check_curated(&copy).expect_err("a stale binding");
        let message = format!("{error:#}");
        assert!(
            message.starts_with(
                "samples/uscode/curated/zz/sample/citations.fr: W750 StaleBinding: the binding \
                 `railway_labor_definitions_binding` of `/us/usc/t45/s151` is stale (provision \
                 text changed)"
            ) && message.contains("finds 0 errors and 1 warning, and the curated sample"),
            "{message}"
        );
        fs::remove_dir_all(&copy).expect("remove scratch dir");
    }

    #[test]
    fn a_count_in_a_message_takes_its_noun_in_the_number_it_needs() {
        assert_eq!(counted(0, "error"), "0 errors");
        assert_eq!(counted(1, "error"), "1 error");
        assert_eq!(counted(2, "warning"), "2 warnings");
    }

    #[test]
    fn a_curated_sample_that_cites_nothing_fails_the_step_and_names_it() {
        let copy = copy_of_the_samples("curated-no-citation");
        let module = copy.join(CURATED).join("zz/sample/citations.fr");
        fs::write(&module, "edition 2\nmodule \"/zz/sample/citations\" {\n}\n")
            .expect("change the module");
        let error = check_curated(&copy).expect_err("a curated sample with no citation");
        let message = format!("{error:#}");
        assert!(
            message.starts_with(
                "samples/uscode/curated: `fidryn check samples/uscode/curated --no-cache` counts \
                 no citation"
            ),
            "{message}"
        );
        fs::remove_dir_all(&copy).expect("remove scratch dir");
    }

    #[test]
    fn a_pin_that_is_not_the_lock_fails_the_step_and_names_the_curated_manifest() {
        let copy = copy_of_the_samples("curated-pin");
        let path = copy.join(CURATED).join(MANIFEST);
        let mut manifest = manifest_of(&read_text(&path));
        manifest.dependencies[0].lock_digest = Hash32([0; 32]);
        fs::write(&path, manifest.to_json()).expect("write the manifest");
        let error = check_curated(&copy).expect_err("a pin that is not the lock");
        let message = format!("{error:#}");
        assert!(
            message.contains("samples/uscode/curated/workspace.json: E740 PinMismatch: ")
                && message.contains("cargo xtask uscode-sample --update"),
            "{message}"
        );
        fs::remove_dir_all(&copy).expect("remove scratch dir");
    }

    #[test]
    fn a_citation_the_sample_does_not_hold_fails_the_step_and_names_the_module() {
        let copy = copy_of_the_samples("curated-citation");
        let module = copy.join(CURATED).join("zz/sample/citations.fr");
        let original = read_text(&module);
        let changed = original.replacen("\"/us/usc/t1/s1\"", "\"/us/usc/t1/s999\"", 1);
        assert_ne!(changed, original);
        fs::write(&module, changed).expect("change the module");
        let error = check_curated(&copy).expect_err("a citation that does not resolve");
        let message = format!("{error:#}");
        assert!(
            message.starts_with(
                "samples/uscode/curated/zz/sample/citations.fr: the citation \
                 `words_denoting_number` of `/zz/sample/citations` names `/us/usc/t1/s999`, \
                 whose outcome is `no-unit`"
            ),
            "{message}"
        );
        fs::remove_dir_all(&copy).expect("remove scratch dir");
    }

    #[test]
    fn update_rewrites_exactly_the_three_values_of_a_stale_pin() {
        let copy = copy_of_the_samples("curated-update");
        let path = copy.join(CURATED).join(MANIFEST);
        let module = copy.join(CURATED).join("zz/sample/citations.fr");
        let committed = read_text(&path);
        let module_text = read_text(&module);
        let mut stale = manifest_of(&committed);
        stale.dependencies[0].release = "119-141".to_owned();
        stale.dependencies[0].lock_digest = Hash32([0; 32]);
        stale.toolchain = "0.0.1".to_owned();
        // A value that the update leaves as it is.
        stale.name = "another-name".to_owned();
        fs::write(&path, stale.to_json()).expect("write the manifest");
        update_curated(&copy).unwrap_or_else(|error| panic!("{error:#}"));
        let expected = Manifest {
            name: "another-name".to_owned(),
            ..manifest_of(&committed)
        };
        assert_eq!(read_text(&path), expected.to_json());
        assert_eq!(read_text(&module), module_text);
        check_curated(&copy).unwrap_or_else(|error| panic!("{error:#}"));
        fs::remove_dir_all(&copy).expect("remove scratch dir");
    }

    #[test]
    fn the_sample_step_fails_on_a_hand_edit() {
        let root = workspace_root();
        let copy = scratch("xtask-uscode-sample-", "hand-edit");
        copy_tree(&root.join(ARCHIVES), &copy.join(ARCHIVES));
        copy_tree(&root.join(WORKSPACE), &copy.join(WORKSPACE));

        // One letter of the heading of one generated module.
        let module = copy.join(WORKSPACE).join("us/usc/t1/s1.fr");
        let original = fs::read_to_string(&module).expect("read a module of the sample");
        let heading = original
            .find("heading \"")
            .expect("the module has a heading")
            + 9;
        let at = heading
            + original[heading..]
                .find(|c: char| c.is_ascii_lowercase())
                .expect("a letter of the heading");
        let mut changed = original.clone();
        let letter = changed[at..=at].to_ascii_uppercase();
        changed.replace_range(at..=at, &letter);
        fs::write(&module, &changed).expect("change a module");
        let error = check(&copy, false).expect_err("a module changed by hand");
        let message = format!("{error:#}");
        assert!(
            message.starts_with("samples/uscode/workspace is not what")
                && message.contains("different us/usc/t1/s1.fr (1 path differs)"),
            "{message}"
        );
        fs::write(&module, &original).expect("restore the module");

        // One figure of the ledger.
        let ledger = copy.join(WORKSPACE).join(LEDGER);
        let original = fs::read_to_string(&ledger).expect("read the ledger of the sample");
        let changed = original.replacen("\"units\": 563", "\"units\": 564", 1);
        assert_ne!(changed, original);
        fs::write(&ledger, &changed).expect("change the ledger");
        let error = check(&copy, false).expect_err("a ledger changed by hand");
        let message = format!("{error:#}");
        assert!(
            message.contains("different coverage.json (1 path differs)"),
            "{message}"
        );
        fs::write(&ledger, &original).expect("restore the ledger");

        // One byte of one archive.
        let archive = copy.join(ARCHIVES).join("xml_usc01@119-142.zip");
        let mut bytes = fs::read(&archive).expect("read an archive");
        bytes[100] ^= 1;
        fs::write(&archive, &bytes).expect("change an archive");
        let error = check(&copy, false).expect_err("an archive changed");
        let message = format!("{error:#}");
        assert!(
            message.starts_with("samples/uscode/archives differs from its record")
                && message.contains("xml_usc01@119-142.zip: sha256 is "),
            "{message}"
        );
        fs::remove_dir_all(&copy).expect("remove scratch dir");
    }
}
