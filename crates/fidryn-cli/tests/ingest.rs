//! `fidryn ingest uscode ARCHIVE_DIR --out DIR` writes a generated workspace
//! from the archives of one release. Every archive here is packed by the test,
//! in a directory of its own, from the invented documents of
//! `conformance/fixtures/uslm`, in the invented title 99, beside a retrieval
//! record the test writes.

use fidryn_provision::Hash32;
use fidryn_provision::workspace::{Lock, RetrievalRecord, RetrievedArchive};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// The release every fixture is read as.
const RELEASE: &str = "999-1";
/// The time of retrieval of every archive here, as its record gives it.
const RETRIEVED_AT: &str = "2099-03-04T05:06:07Z";
/// The version of the running toolchain.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The members of an archive: each name with its bytes.
type Members = Vec<(String, Vec<u8>)>;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/uslm")
}

/// The bytes of the fixture file `rel`.
fn fixture(rel: &str) -> Vec<u8> {
    let path = fixtures().join(rel);
    fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The names of the fixtures of one document, sorted.
fn document_fixtures() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures())
        .expect("list the fixtures")
        .map(|entry| {
            entry
                .expect("a fixture")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .filter_map(|name| name.strip_suffix(".xml").map(str::to_owned))
        .collect();
    names.sort();
    names
}

/// The fixtures that are directories of members.
const MEMBER_SETS: [&str; 2] = ["two-files", "unnamed-appendices"];

/// The members of the fixture directory `name`, sorted by name.
fn fixture_members(name: &str) -> Members {
    let mut members: Members = fs::read_dir(fixtures().join(name))
        .expect("list the members")
        .map(|entry| {
            let entry = entry.expect("a member");
            let name = entry.file_name().into_string().expect("UTF-8");
            (name, fs::read(entry.path()).expect("read a member"))
        })
        .collect();
    members.sort();
    members
}

/// The fixture document `name`, as the member `usc99.xml`.
fn as_title_99(name: &str) -> Members {
    vec![("usc99.xml".to_owned(), fixture(&format!("{name}.xml")))]
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A directory of its own under the temporary directory, removed when
/// dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fidryn-cli-ingest-{}-{n}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch directory");
        Scratch(dir)
    }

    fn path(&self) -> &Path {
        &self.0
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

/// The bytes of a zip archive of `members`, each compressed with deflate.
fn zip_of(members: &Members) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in members {
        writer
            .start_file(name.as_str(), options)
            .expect("begin a member");
        writer.write_all(bytes).expect("write a member");
    }
    writer.finish().expect("finish the archive").into_inner()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Writes into `dir` each archive of `archives`, by its name with its
/// members, and the retrieval record of `release` that lists them in that
/// order, and returns the record.
fn release(dir: &Path, release: &str, archives: &[(&str, Members)]) -> RetrievalRecord {
    fs::create_dir_all(dir).expect("create the directory of the archives");
    let mut entries = Vec::new();
    for (name, members) in archives {
        let bytes = zip_of(members);
        fs::write(dir.join(name), &bytes).expect("write an archive");
        entries.push(RetrievedArchive {
            file: (*name).to_owned(),
            url: format!("https://example.invalid/releases/{release}/{name}"),
            retrieved_at: RETRIEVED_AT.to_owned(),
            bytes: u64::try_from(bytes.len()).expect("a small archive"),
            sha256: hex(&Sha256::digest(&bytes)),
            blake3: Hash32(*blake3::hash(&bytes).as_bytes()),
        });
    }
    let record = RetrievalRecord {
        release: release.to_owned(),
        archives: entries,
    };
    fs::write(dir.join("retrieval.json"), record.to_json()).expect("write the record");
    record
}

/// A release of one archive that holds `members`.
fn one_archive(dir: &Path, members: Members) -> RetrievalRecord {
    release(dir, RELEASE, &[("xml_usc99@999-1.zip", members)])
}

/// A release of many members: every fixture of one document, the `n`th as
/// the member `usc<n>.xml`, in three archives, so that a run reads more
/// members than it has workers and numbers addresses repeated across them.
fn many_members(dir: &Path) -> RetrievalRecord {
    let mut parts: [Members; 3] = Default::default();
    for (index, name) in document_fixtures().iter().enumerate() {
        parts[index % 3].push((
            format!("usc{:02}.xml", index + 1),
            fixture(&format!("{name}.xml")),
        ));
    }
    let [first, second, third] = parts;
    release(
        dir,
        RELEASE,
        &[
            ("part1.zip", first),
            ("part2.zip", second),
            ("part3.zip", third),
        ],
    )
}

fn fidryn() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fidryn"))
}

/// The command `fidryn ingest uscode ARCHIVES --out OUT FLAGS...`.
fn ingest_command(archives: &Path, out: &Path, flags: &[&str]) -> Command {
    let mut command = fidryn();
    command
        .args([
            OsStr::new("ingest"),
            OsStr::new("uscode"),
            archives.as_os_str(),
            OsStr::new("--out"),
            out.as_os_str(),
        ])
        .args(flags);
    command
}

fn ingest(archives: &Path, out: &Path, flags: &[&str]) -> Output {
    ingest_command(archives, out, flags)
        .output()
        .expect("run fidryn")
}

/// `fidryn check PATH`, with `--no-cache` for a directory, so that no check
/// writes a cache.
fn check(path: &Path) -> Output {
    let mut command = fidryn();
    command.args([OsStr::new("check"), path.as_os_str()]);
    if path.is_dir() {
        command.arg("--no-cache");
    }
    command.output().expect("run fidryn")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Asserts that `output` is that of a command that succeeded.
fn succeeded(output: &Output, what: &str) {
    assert!(
        output.status.success(),
        "{what}: exit {:?}\n{}{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

/// What `root` holds: every entry below it, a directory with `None` and a
/// file with its bytes, entries whose name begins with a dot included.
fn tree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(dir: &Path, prefix: &str, entries: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        {
            let entry = entry.expect("an entry");
            let name = entry.file_name().into_string().expect("UTF-8");
            let rel = format!("{prefix}{name}");
            let kind = entry.file_type().expect("the kind of an entry");
            if kind.is_dir() {
                entries.insert(rel.clone(), None);
                walk(&entry.path(), &format!("{rel}/"), entries);
            } else if kind.is_symlink() {
                let target = fs::read_link(entry.path()).expect("a link");
                let mut recorded = b"link to ".to_vec();
                recorded.extend_from_slice(target.to_string_lossy().as_bytes());
                entries.insert(rel, Some(recorded));
            } else {
                entries.insert(rel, Some(fs::read(entry.path()).expect("read a file")));
            }
        }
    }
    let mut entries = BTreeMap::new();
    walk(root, "", &mut entries);
    entries
}

/// The module files below `root`, by path, with their text.
fn modules(root: &Path) -> BTreeMap<String, String> {
    tree(root)
        .into_iter()
        .filter(|(rel, _)| rel.ends_with(".fr"))
        .map(|(rel, bytes)| {
            let text = String::from_utf8(bytes.expect("a module is a file")).expect("UTF-8");
            (rel, text)
        })
        .collect()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The keys of the object `value`, in order.
fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("an object: {value}"))
        .keys()
        .map(String::as_str)
        .collect()
}

/// Every text of the form `YYYY-MM-DDTHH:MM:SS` in `text`.
fn times(text: &str) -> Vec<String> {
    const SHAPE: &[u8] = b"0000-00-00T00:00:00";
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(SHAPE.len() - 1))
        .filter(|&at| {
            SHAPE.iter().zip(&bytes[at..]).all(|(shape, byte)| {
                if *shape == b'0' {
                    byte.is_ascii_digit()
                } else {
                    shape == byte
                }
            })
        })
        .map(|at| text[at..at + SHAPE.len()].to_owned())
        .collect()
}

/// Every fixture, each as the members of one release: a fixture of one
/// document as the member `usc99.xml`, and a fixture of several members as
/// those members.
fn fixture_releases() -> Vec<(String, Members)> {
    let mut releases: Vec<(String, Members)> = document_fixtures()
        .into_iter()
        .map(|name| {
            let members = as_title_99(&name);
            (name, members)
        })
        .collect();
    releases.extend(
        MEMBER_SETS
            .iter()
            .map(|name| ((*name).to_owned(), fixture_members(name))),
    );
    assert!(releases.len() >= 34, "{} fixtures", releases.len());
    releases
}

#[test]
fn ingest_writes_a_workspace_that_checks() {
    for (name, members) in fixture_releases() {
        let scratch = Scratch::new(&format!("checks-{name}"));
        let archives = scratch.join("archives");
        one_archive(&archives, members);
        let out = scratch.join("out");
        let ingested = ingest(&archives, &out, &[]);
        succeeded(&ingested, &name);
        assert!(
            stdout(&ingested).starts_with("ingested: "),
            "{name}: {}",
            stdout(&ingested)
        );
        assert_eq!(stderr(&ingested), "", "{name}");

        let checked = check(&out);
        succeeded(&checked, &format!("check of the workspace of {name}"));
        let summary = stdout(&checked);
        assert!(
            summary.contains("errors: 0; warnings: 0") && summary.ends_with("\nok\n"),
            "{name}: {summary}"
        );
        let files = modules(&out);
        assert!(!files.is_empty(), "{name}");
        for rel in files.keys() {
            let checked = check(&out.join(rel));
            succeeded(&checked, &format!("check of {rel} of {name}"));
            assert_eq!(stdout(&checked), "ok\n", "{name}: {rel}");
        }
    }
}

/// The text that `fidryn text --json` reads back from the workspace written
/// of each fixture is, line for line, the records written by hand for the
/// fixture, which the independent reading of the publisher's XML prints: the
/// modules lose nothing of the text, and the records of all of them are in
/// the order of their addresses.
#[test]
fn the_text_of_the_workspace_of_a_fixture_is_its_expected_records() {
    for (name, members) in fixture_releases() {
        let scratch = Scratch::new(&format!("text-{name}"));
        let archives = scratch.join("archives");
        one_archive(&archives, members);
        let out = scratch.join("out");
        succeeded(&ingest(&archives, &out, &[]), &name);
        let printed = fidryn()
            .args([OsStr::new("text"), out.as_os_str(), OsStr::new("--json")])
            .output()
            .expect("run fidryn");
        succeeded(&printed, &format!("text of {name}"));
        assert_eq!(stderr(&printed), "", "{name}");
        let expected = read(&fixtures().join(format!("{name}.records.jsonl")));
        assert_eq!(stdout(&printed), expected, "{name}");
    }
}

/// The keys of the ledger's `totals` that the profile of a fixture does not
/// have: the independent reading extracts no definitions.
const LEDGER_ONLY: [&str; 2] = ["definitions", "definitions_by_kind"];
/// The keys of the profile that stand at the top of the ledger, not in its
/// `totals`.
const AT_THE_TOP: [&str; 2] = ["prefix", "release"];

/// `fidryn coverage DIR` prints, byte for byte, the ledger that the ingest
/// of each fixture wrote as `coverage.json`, and the ledger counts what the
/// profile written by hand for the fixture counts, key for key: the units,
/// containers and provisions, the notes, the footnotes and images, the
/// references by layer and outcome and the lock's counts. A curated
/// workspace and a directory with no workspace have no ledger.
#[test]
fn the_ledger_of_a_fixture_is_what_coverage_prints_and_counts_what_its_profile_counts() {
    let coverage = |dir: &Path| {
        fidryn()
            .args([OsStr::new("coverage"), dir.as_os_str()])
            .output()
            .expect("run fidryn")
    };
    for (name, members) in fixture_releases() {
        let scratch = Scratch::new(&format!("coverage-{name}"));
        let archives = scratch.join("archives");
        one_archive(&archives, members);
        let out = scratch.join("out");
        succeeded(&ingest(&archives, &out, &[]), &name);
        let written = read(&out.join("coverage.json"));
        let printed = coverage(&out);
        succeeded(&printed, &format!("coverage of {name}"));
        assert_eq!(stderr(&printed), "", "{name}");
        assert_eq!(stdout(&printed), written, "{name}");

        let ledger: Value = serde_json::from_str(&written).expect("the ledger is JSON");
        assert_eq!(ledger["schema"], "fidryn.coverage/v0.1", "{name}");
        let profile: Value =
            serde_json::from_slice(&fixture(&format!("{name}.profile.json"))).expect("JSON");
        let totals = ledger["totals"].as_object().expect("an object");
        for (key, counted) in profile.as_object().expect("an object") {
            let ours = if AT_THE_TOP.contains(&key.as_str()) {
                &ledger[key]
            } else {
                totals
                    .get(key)
                    .unwrap_or_else(|| panic!("{name}: the ledger has no `{key}`"))
            };
            assert_eq!(ours, counted, "{name}: `{key}`");
        }
        for key in totals.keys() {
            assert!(
                profile.get(key).is_some() || LEDGER_ONLY.contains(&key.as_str()),
                "{name}: the profile has no `{key}`"
            );
        }
    }

    // A curated workspace has no ledger, and neither has a directory with
    // no workspace; nothing is printed to stdout.
    let scratch = Scratch::new("coverage-refused");
    let curated = scratch.join("curated");
    fs::create_dir_all(&curated).expect("create a directory");
    let manifest = fidryn_provision::workspace::Manifest {
        name: "notes".to_owned(),
        kind: fidryn_provision::workspace::WorkspaceKind::Curated,
        prefix: fidryn_provision::Address::parse("/zz/notes").expect("an address"),
        release: None,
        toolchain: VERSION.to_owned(),
        dependencies: Vec::new(),
    };
    fs::write(curated.join("workspace.json"), manifest.to_json()).expect("write");
    for (dir, words) in [
        (
            &curated,
            "holds a curated workspace, and the coverage ledger belongs to a generated workspace",
        ),
        (&scratch.join("archives"), "E730"),
    ] {
        let refused = coverage(dir);
        assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
        assert_eq!(stdout(&refused), "");
        assert!(stderr(&refused).contains(words), "{}", stderr(&refused));
    }
}

/// Every module the importer reads from a fixture, written in its canonical
/// form, passes the one-module check with the definitions extracted from it
/// and reads back to the same model; so does the fixture of a file that is
/// not positive law with its flag turned on. The fixtures of definitions
/// give the number of definitions they hold.
#[test]
fn every_extracted_definition_passes_the_checker() {
    let mut documents: Vec<(String, String)> = document_fixtures()
        .into_iter()
        .map(|name| {
            let text = String::from_utf8(fixture(&format!("{name}.xml"))).expect("UTF-8");
            (name, text)
        })
        .collect();
    let restated = String::from_utf8(fixture("definitions-restated.xml")).expect("UTF-8");
    let flag = "<property role=\"is-positive-law\">no</property>";
    assert!(restated.contains(flag));
    documents.push((
        "definitions-restated, flag on".to_owned(),
        restated.replace(flag, "<property role=\"is-positive-law\">yes</property>"),
    ));
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (name, text) in &documents {
        let (modules, _) = fidryn_ingest_uscode::ingest_xml("usc99.xml", text, RELEASE)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        for module in modules {
            if let fidryn_provision::Module::Unit(unit) = &module {
                *counts.entry(name.clone()).or_default() += unit.definitions.len();
            }
            let source = fidryn_provision::render::render(&module);
            match fidryn_check::e2::check_source(&source) {
                Ok(lowered) => assert_eq!(lowered.module, module, "{name}"),
                Err(diagnostics) => panic!("{name}: {diagnostics:#?}\n{source}"),
            }
        }
    }
    for (name, expected) in [
        ("definitions-chapeau", 14),
        ("definitions-restated", 5),
        ("definitions-restated, flag on", 5),
        ("definitions-sentence", 10),
    ] {
        assert_eq!(counts.get(name), Some(&expected), "{name}");
    }
}

/// A document without the property `is-positive-law` has no flag: none of
/// its modules declares `positive_law`, each checks by itself, and the
/// record of its unit has `null`. The same document with the property
/// `yes` gives the same modules, each with the declaration `positive_law
/// true`.
#[test]
fn a_document_without_the_flag_has_none() {
    let document = String::from_utf8(fixture("meta-no-flag.xml")).expect("UTF-8");
    assert!(!document.contains("is-positive-law"));
    let name = "<docPublicationName>Online@999-1</docPublicationName>";
    let with_flag = document.replace(
        name,
        &format!("{name}\n<property role=\"is-positive-law\">yes</property>"),
    );
    assert_ne!(with_flag, document);
    let scratch = Scratch::new("no-flag");
    let mut written = Vec::new();
    for (tag, text) in [("none", &document), ("yes", &with_flag)] {
        let archives = scratch.join(&format!("archives-{tag}"));
        one_archive(
            &archives,
            vec![("usc99.xml".to_owned(), text.as_bytes().to_vec())],
        );
        let out = scratch.join(&format!("out-{tag}"));
        succeeded(&ingest(&archives, &out, &[]), tag);
        written.push((out.clone(), modules(&out)));
    }
    let [(out, unflagged), (_, flagged)] = written.try_into().expect("two workspaces");

    // A unit and two containers, none of which declares the flag.
    assert_eq!(
        unflagged.keys().map(String::as_str).collect::<Vec<_>>(),
        ["us/usc/t99.fr", "us/usc/t99/ch1.fr", "us/usc/t99/s1.fr"]
    );
    for (rel, text) in &unflagged {
        assert!(!text.contains("positive_law"), "{rel}:\n{text}");
        let checked = check(&out.join(rel));
        succeeded(&checked, &format!("check of {rel}"));
        assert_eq!(stdout(&checked), "ok\n", "{rel}");
        let declared = &flagged[rel];
        assert_eq!(
            declared.matches("\n  positive_law true\n").count(),
            1,
            "{rel}"
        );
        assert_eq!(
            &declared.replace("\n  positive_law true\n", "\n"),
            text,
            "{rel}"
        );
    }
    let checked = check(&out);
    succeeded(&checked, "check of the workspace");
    assert!(stdout(&checked).contains("errors: 0; warnings: 0"));

    let printed = fidryn()
        .args([OsStr::new("text"), out.as_os_str(), OsStr::new("--json")])
        .output()
        .expect("run fidryn");
    succeeded(&printed, "text");
    let records = stdout(&printed);
    assert_eq!(
        records,
        read(&fixtures().join("meta-no-flag.records.jsonl"))
    );
    let unit: Value =
        serde_json::from_str(records.lines().nth(2).expect("the unit's record")).expect("a record");
    assert_eq!(unit["address"], "/us/usc/t99/s1");
    assert_eq!(unit.get("positive_law"), Some(&Value::Null));
    assert_eq!(records.matches("\"positive_law\":null").count(), 1);
}

#[test]
fn two_runs_give_the_same_bytes() {
    let scratch = Scratch::new("same-bytes");
    let archives = scratch.join("archives");
    many_members(&archives);
    let one = scratch.join("one");
    let eight = scratch.join("eight");
    let first = ingest(&archives, &one, &["--jobs", "1"]);
    succeeded(&first, "one worker");
    let second = ingest(&archives, &eight, &["--jobs", "8"]);
    succeeded(&second, "eight workers");
    assert_eq!(stdout(&first), stdout(&second));
    let written = tree(&one);
    assert!(
        modules(&one).len() > 2 * document_fixtures().len(),
        "{} modules",
        modules(&one).len()
    );
    assert_eq!(tree(&eight), written);

    // A run into its own earlier output writes the same bytes again.
    let again = ingest(&archives, &one, &["--jobs", "8"]);
    succeeded(&again, "a second run into the same directory");
    assert_eq!(stdout(&again), stdout(&first));
    assert_eq!(tree(&one), written);
}

/// The values of every attribute `name` in `text`, in document order.
fn attribute_values(text: &str, name: &str) -> Vec<String> {
    let opening = format!(" {name}=\"");
    text.match_indices(&opening)
        .map(|(at, _)| {
            let start = at + opening.len();
            let end = start + text[start..].find('"').expect("a closed attribute");
            text[start..end].to_owned()
        })
        .collect()
}

/// The text of the one element `tag` of `text`.
fn element_text<'a>(text: &'a str, tag: &str) -> &'a str {
    let opening = format!("<{tag}>");
    let start = text.find(&opening).expect("the element") + opening.len();
    let end = start + text[start..].find("</").expect("a closed element");
    &text[start..end]
}

#[test]
fn two_files_that_differ_only_in_ids_and_release_name_give_identical_modules() {
    let old = String::from_utf8(fixture("reissue/old.xml")).expect("UTF-8");
    let new = String::from_utf8(fixture("reissue/new.xml")).expect("UTF-8");

    // The two documents differ in every element id, in the publication name,
    // in the creation time of the metadata and in the release note, and in
    // nothing else.
    let normalized = |text: &str, release_name: &str| {
        let mut normalized = text.to_owned();
        for id in attribute_values(text, "id") {
            normalized = normalized.replace(&id, "ID");
        }
        normalized
            .replace(element_text(text, "dcterms:created"), "CREATED")
            .replace(release_name, "RELEASE")
    };
    assert_eq!(normalized(&old, "999-1"), normalized(&new, "999-2"));
    let (old_ids, new_ids) = (attribute_values(&old, "id"), attribute_values(&new, "id"));
    assert!(old_ids.len() >= 15, "{old_ids:?}");
    assert_eq!(old_ids.len(), new_ids.len());
    assert!(old_ids.iter().all(|id| !new.contains(id.as_str())));
    assert!(!attribute_values(&old, "idref").is_empty());
    assert_ne!(
        element_text(&old, "dcterms:created"),
        element_text(&new, "dcterms:created")
    );
    assert!(old.contains("<p>Current through 999-1</p>"));
    assert!(new.contains("<p>Current through 999-2</p>"));

    let scratch = Scratch::new("reissue");
    let mut outs = Vec::new();
    for (release_name, text) in [("999-1", &old), ("999-2", &new)] {
        let archives = scratch.join(&format!("archives-{release_name}"));
        let archive = format!("xml_usc99@{release_name}.zip");
        release(
            &archives,
            release_name,
            &[(
                archive.as_str(),
                vec![("usc99.xml".to_owned(), text.as_bytes().to_vec())],
            )],
        );
        let out = scratch.join(&format!("out-{release_name}"));
        succeeded(&ingest(&archives, &out, &[]), release_name);
        outs.push(out);
    }
    let (first, second) = (modules(&outs[0]), modules(&outs[1]));
    assert!(first.len() >= 4, "{first:?}");
    assert_eq!(first, second);

    // The manifests differ in their release alone, and the locks in their
    // release and their artifacts.
    let without = |path: &Path, fields: &[&str]| {
        let mut value = json(path);
        let object = value.as_object_mut().expect("an object");
        for field in fields {
            assert!(object.remove(*field).is_some(), "{field}");
        }
        value
    };
    for (file, fields) in [
        ("workspace.json", &["release"][..]),
        ("law.lock.json", &["release", "artifacts"][..]),
    ] {
        let (a, b) = (outs[0].join(file), outs[1].join(file));
        assert_ne!(read(&a), read(&b), "{file}");
        assert_eq!(without(&a, fields), without(&b, fields), "{file}");
        for field in fields {
            assert_ne!(json(&a)[field], json(&b)[field], "{file}: {field}");
        }
    }
    assert_eq!(json(&outs[0].join("law.lock.json"))["release"], "999-1");
    assert_eq!(json(&outs[1].join("law.lock.json"))["release"], "999-2");
}

#[test]
fn the_lock_records_the_archives_and_every_module() {
    let scratch = Scratch::new("lock");
    let archives = scratch.join("archives");
    let members = fixture_members("two-files");
    let record = release(
        &archives,
        RELEASE,
        &[
            ("xml_usc99@999-1.zip", vec![members[0].clone()]),
            ("xml_usc99a@999-1.zip", vec![members[1].clone()]),
        ],
    );
    let out = scratch.join("out");
    let ingested = ingest(&archives, &out, &[]);
    succeeded(&ingested, "two archives");
    assert_eq!(
        stdout(&ingested),
        "ingested: 2 units, 2 containers from 2 archives (release 999-1)\n"
    );

    let text = read(&out.join("law.lock.json"));
    let lock = Lock::from_json(&text).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(lock.to_json(), text, "the lock is written as it is read");
    let value: Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(
        keys(&value),
        [
            "schema",
            "prefix",
            "release",
            "generator",
            "artifacts",
            "addresses",
            "not_imported",
            "modules"
        ]
    );
    assert_eq!(value["schema"], "fidryn.law-lock/v0.1");
    assert_eq!(value["prefix"], "/us/usc");
    assert_eq!(value["release"], RELEASE);
    assert_eq!(
        value["generator"],
        serde_json::json!({ "name": "fidryn-ingest-uscode", "version": VERSION })
    );

    // Each archive as its record gives it, in the order of the record, with
    // the size and the BLAKE3 of each member.
    let artifacts = value["artifacts"].as_array().expect("an array");
    assert_eq!(artifacts.len(), 2);
    for ((artifact, entry), member) in artifacts.iter().zip(&record.archives).zip(&members) {
        assert_eq!(
            keys(artifact),
            [
                "file",
                "url",
                "retrieved_at",
                "bytes",
                "blake3",
                "sha256",
                "members"
            ]
        );
        assert_eq!(artifact["file"], entry.file.as_str());
        assert_eq!(artifact["url"], entry.url.as_str());
        assert_eq!(artifact["retrieved_at"], RETRIEVED_AT);
        assert_eq!(artifact["bytes"], entry.bytes);
        assert_eq!(artifact["blake3"], entry.blake3.to_string());
        assert_eq!(artifact["sha256"], entry.sha256.as_str());
        assert_eq!(
            artifact["members"],
            serde_json::json!([{
                "name": member.0,
                "bytes": member.1.len(),
                "blake3": hex(blake3::hash(&member.1).as_bytes()),
            }])
        );
    }

    // What the profile of the two files counts.
    let profile: Value =
        serde_json::from_slice(&fixture("two-files.profile.json")).expect("the profile");
    assert_eq!(value["addresses"], profile["addresses"]);
    assert_eq!(value["not_imported"], profile["not_imported"]);

    // Every module file, by its path, with the BLAKE3 of its bytes, and no
    // other.
    let listed: BTreeMap<String, String> = value["modules"]
        .as_object()
        .expect("an object")
        .iter()
        .map(|(rel, digest)| (rel.clone(), digest.as_str().expect("a digest").to_owned()))
        .collect();
    let present: BTreeMap<String, String> = modules(&out)
        .into_iter()
        .map(|(rel, text)| (rel, hex(blake3::hash(text.as_bytes()).as_bytes())))
        .collect();
    assert_eq!(listed, present);
    assert_eq!(
        listed.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "us/usc/t99.fr",
            "us/usc/t99/s1.fr",
            "us/usc/t99/s1~2.fr",
            "us/usc/t99a.fr"
        ]
    );

    // The manifest.
    assert_eq!(
        read(&out.join("workspace.json")),
        format!(
            "{{\n  \"schema\": \"fidryn.workspace/v0.1\",\n  \"name\": \"uscode\",\n  \
             \"kind\": \"generated\",\n  \"prefix\": \"/us/usc\",\n  \"release\": \"999-1\",\n  \
             \"toolchain\": \"{VERSION}\",\n  \"dependencies\": []\n}}\n"
        )
    );

    // No time but the time of retrieval of the record, and that only in the
    // lock.
    for (rel, bytes) in tree(&out) {
        let Some(bytes) = bytes else { continue };
        let found = times(&String::from_utf8(bytes).expect("UTF-8"));
        let expected = if rel == "law.lock.json" {
            vec![RETRIEVED_AT[..19].to_owned(); 2]
        } else {
            Vec::new()
        };
        assert_eq!(found, expected, "{rel}");
    }

    // For every fixture, the lock counts what its profile counts.
    for name in document_fixtures() {
        let scratch = Scratch::new(&format!("counts-{name}"));
        let archives = scratch.join("archives");
        one_archive(&archives, as_title_99(&name));
        let out = scratch.join("out");
        succeeded(&ingest(&archives, &out, &[]), &name);
        let lock = json(&out.join("law.lock.json"));
        let profile: Value =
            serde_json::from_slice(&fixture(&format!("{name}.profile.json"))).expect("JSON");
        assert_eq!(lock["addresses"], profile["addresses"], "{name}");
        assert_eq!(lock["not_imported"], profile["not_imported"], "{name}");
    }
}

#[test]
fn verify_reports_a_changed_a_missing_and_an_extra_file_and_writes_nothing() {
    let scratch = Scratch::new("verify");
    let archives = scratch.join("archives");
    one_archive(&archives, fixture_members("two-files"));
    let out = scratch.join("out");
    succeeded(&ingest(&archives, &out, &[]), "the run");
    // Entries whose name begins with a dot are not looked at.
    fs::write(out.join(".notes"), "not the workspace's\n").expect("write");
    let verified = ingest(&archives, &out, &["--verify"]);
    succeeded(&verified, "a directory that holds what the run writes");
    assert_eq!(stdout(&verified), "ok\n");

    // The ledger a run writes after the lock is compared like any other
    // file: missing when a run stopped before it, different when it is
    // that of another run. The check of the workspace does not read it.
    let ledger = read(&out.join("coverage.json"));
    for (bytes, word) in [(None, "missing"), (Some("{}\n"), "different")] {
        match bytes {
            None => fs::remove_file(out.join("coverage.json")).expect("remove the ledger"),
            Some(bytes) => fs::write(out.join("coverage.json"), bytes).expect("write"),
        }
        succeeded(
            &check(&out),
            "the check of a workspace whose ledger is not its own",
        );
        let verified = ingest(&archives, &out, &["--verify"]);
        assert_eq!(verified.status.code(), Some(1), "{}", stderr(&verified));
        assert_eq!(stdout(&verified), format!("{word} coverage.json\n"));
    }
    succeeded(
        &ingest(&archives, &out, &[]),
        "a run that writes the ledger again",
    );
    assert_eq!(read(&out.join("coverage.json")), ledger);
    assert_eq!(stdout(&ingest(&archives, &out, &["--verify"])), "ok\n");

    let s1 = out.join("us/usc/t99/s1.fr");
    let changed = read(&s1).replace("The first holder", "The first keeper");
    fs::write(&s1, changed).expect("change a module");
    fs::remove_file(out.join("us/usc/t99a.fr")).expect("remove a module");
    fs::copy(&s1, out.join("us/usc/t99/s7.fr")).expect("add a module");
    // A stray file at the top and a symbolic link are entries a run refuses.
    fs::write(out.join("notes.txt"), "a note\n").expect("write");
    let mut expected = vec![
        "extra notes.txt",
        "different us/usc/t99/s1.fr",
        "extra us/usc/t99/s7.fr",
        "missing us/usc/t99a.fr",
    ];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("t99", out.join("us/usc/link")).expect("link");
        expected.insert(1, "extra us/usc/link");
    }
    let before = tree(&out);

    let verified = ingest(&archives, &out, &["--verify"]);
    assert_eq!(verified.status.code(), Some(1), "{}", stderr(&verified));
    assert_eq!(
        stdout(&verified),
        expected
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>()
    );
    assert_eq!(stderr(&verified), "");
    assert_eq!(tree(&out), before, "nothing is written");

    // A run refuses the directory, which holds entries that are not the
    // workspace's, and changes nothing.
    let refused = ingest(&archives, &out, &[]);
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert_eq!(tree(&out), before);

    // A directory that does not exist lacks every file, and is not made.
    let absent = scratch.join("absent");
    let verified = ingest(&archives, &absent, &["--verify"]);
    assert_eq!(verified.status.code(), Some(1), "{}", stderr(&verified));
    assert_eq!(
        stdout(&verified),
        [
            "coverage.json",
            "law.lock.json",
            "us/usc/t99.fr",
            "us/usc/t99/s1.fr",
            "us/usc/t99/s1~2.fr",
            "us/usc/t99a.fr",
            "workspace.json"
        ]
        .map(|rel| format!("missing {rel}\n"))
        .concat()
    );
    assert!(!absent.exists());
}

#[test]
fn title_selects_titles() {
    let scratch = Scratch::new("titles");
    let archives = scratch.join("archives");
    let unnamed = fixture_members("unnamed-appendices");
    let record = release(
        &archives,
        RELEASE,
        &[
            ("xml_usc99@999-1.zip", fixture_members("two-files")),
            ("xml_usc98a@999-1.zip", vec![unnamed[0].clone()]),
        ],
    );
    let all = scratch.join("all");
    succeeded(&ingest(&archives, &all, &[]), "every title");
    let every = modules(&all);
    let of = |members: &[&str]| -> BTreeMap<String, String> {
        every
            .iter()
            .filter(|(_, text)| {
                members
                    .iter()
                    .any(|member| text.contains(&format!("\n  source \"{member}\"\n")))
            })
            .map(|(rel, text)| (rel.clone(), text.clone()))
            .collect()
    };
    assert_eq!(
        of(&["usc98a.xml", "usc99.xml", "usc99a.xml"]),
        every,
        "every module has its source"
    );
    let members_of = |lock: &Value| -> Vec<(String, Vec<String>)> {
        lock["artifacts"]
            .as_array()
            .expect("an array")
            .iter()
            .map(|artifact| {
                let names = artifact["members"]
                    .as_array()
                    .expect("an array")
                    .iter()
                    .map(|member| member["name"].as_str().expect("a name").to_owned())
                    .collect();
                (artifact["file"].as_str().expect("a file").to_owned(), names)
            })
            .collect()
    };

    let one = scratch.join("one");
    let ingested = ingest(&archives, &one, &["--title", "99a"]);
    succeeded(&ingested, "title 99a");
    assert_eq!(
        stdout(&ingested),
        "ingested: 1 unit, 1 container from 1 archive (release 999-1)\n"
    );
    let selected = modules(&one);
    assert_eq!(selected, of(&["usc99a.xml"]));
    assert_eq!(
        selected.keys().map(String::as_str).collect::<Vec<_>>(),
        ["us/usc/t99/s1~2.fr", "us/usc/t99a.fr"]
    );
    let lock = json(&one.join("law.lock.json"));
    assert_eq!(
        members_of(&lock),
        [(
            "xml_usc99@999-1.zip".to_owned(),
            vec!["usc99a.xml".to_owned()]
        )]
    );
    let artifact = &lock["artifacts"][0];
    assert_eq!(artifact["sha256"], record.archives[0].sha256.as_str());

    // The letter of a title in either case.
    let upper = scratch.join("upper");
    succeeded(&ingest(&archives, &upper, &["--title", "99A"]), "title 99A");
    assert_eq!(tree(&upper), tree(&one));

    // Two titles, of two archives.
    let two = scratch.join("two");
    succeeded(
        &ingest(&archives, &two, &["--title", "98a", "--title", "99"]),
        "titles 98a and 99",
    );
    assert_eq!(modules(&two), of(&["usc98a.xml", "usc99.xml"]));
    assert_eq!(
        members_of(&json(&two.join("law.lock.json"))),
        [
            (
                "xml_usc99@999-1.zip".to_owned(),
                vec!["usc99.xml".to_owned()]
            ),
            (
                "xml_usc98a@999-1.zip".to_owned(),
                vec!["usc98a.xml".to_owned()]
            ),
        ]
    );

    // A title no member holds is refused, by its name, and nothing is
    // written.
    let none = scratch.join("none");
    let refused = ingest(&archives, &none, &["--title", "98"]);
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert!(
        stderr(&refused).contains("no title 98"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(stdout(&refused), "");
    assert!(!none.exists());
}

/// Copies every file and directory below `from` to `to`.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create a directory");
    for entry in fs::read_dir(from).unwrap_or_else(|error| panic!("{}: {error}", from.display())) {
        let entry = entry.expect("read an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("the kind of an entry").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

#[test]
fn ingest_refuses_a_directory_that_is_not_its_own() {
    let scratch = Scratch::new("refuses");
    let archives = scratch.join("archives");
    one_archive(&archives, fixture_members("two-files"));
    let demo =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/workspace/demo");

    let stray = scratch.join("stray");
    fs::create_dir_all(&stray).expect("create");
    fs::write(stray.join("notes.txt"), "a note\n").expect("write");

    let curated = scratch.join("curated");
    fs::create_dir_all(&curated).expect("create");
    fs::write(
        curated.join("workspace.json"),
        format!(
            "{{\n  \"schema\": \"fidryn.workspace/v0.1\",\n  \"name\": \"demo\",\n  \
             \"kind\": \"curated\",\n  \"prefix\": \"/zz/demo\",\n  \
             \"toolchain\": \"{VERSION}\",\n  \"dependencies\": []\n}}\n"
        ),
    )
    .expect("write");

    let other_prefix = scratch.join("other-prefix");
    copy_tree(&demo, &other_prefix);

    let own_with_stray = scratch.join("own-with-stray");
    succeeded(
        &ingest(&archives, &own_with_stray, &[]),
        "the first run into the directory",
    );
    fs::write(own_with_stray.join("us/usc/stray.txt"), "a note\n").expect("write");

    for (dir, phrase) in [
        (&stray, "holds `notes.txt` and no `workspace.json`"),
        (&curated, "holds a curated workspace"),
        (
            &other_prefix,
            "holds a generated workspace of the prefix `/zz/demo`",
        ),
        (&own_with_stray, "holds `us/usc/stray.txt`"),
    ] {
        let before = tree(dir);
        let refused = ingest(&archives, dir, &[]);
        assert_eq!(
            refused.status.code(),
            Some(1),
            "{}: {}",
            dir.display(),
            stderr(&refused)
        );
        assert!(
            stderr(&refused).contains(phrase),
            "{}: {}",
            dir.display(),
            stderr(&refused)
        );
        assert_eq!(stdout(&refused), "", "{}", dir.display());
        assert_eq!(tree(dir), before, "{} is not changed", dir.display());
    }

    // A directory that holds nothing but entries whose name begins with a
    // dot is accepted, and they are left alone.
    let repository = scratch.join("repository");
    fs::create_dir_all(repository.join(".git")).expect("create");
    fs::write(repository.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write");
    succeeded(&ingest(&archives, &repository, &[]), "a new repository");
    assert_eq!(
        read(&repository.join(".git/HEAD")),
        "ref: refs/heads/main\n"
    );
    assert!(repository.join("law.lock.json").is_file());
}

#[test]
fn ingest_refuses_a_workspace_that_holds_a_module_file_it_did_not_write() {
    // A generated workspace of the importer's prefix, and two `.fr` files
    // that no run wrote: one of the first edition, and a module of edition 2
    // of another prefix in a directory of the workspace. Both are invented.
    let scratch = Scratch::new("foreign-modules");
    let archives = scratch.join("archives");
    one_archive(&archives, fixture_members("two-files"));
    let out = scratch.join("out");
    succeeded(&ingest(&archives, &out, &[]), "the first run");
    let mine = "// An invented file of the first edition.\nmodule Mine version \"0.1.0\" {\n}\n";
    let other = "edition 2\n// An invented module of another prefix.\nmodule \"/zz/mine/x\" {\n}\n";
    fs::write(out.join("mine.fr"), mine).expect("write");
    fs::create_dir_all(out.join("us/usc/notes")).expect("create");
    fs::write(out.join("us/usc/notes/x.fr"), other).expect("write");
    let before = tree(&out);

    // The check of the workspace refuses the module of another prefix.
    let checked = check(&out);
    assert_eq!(checked.status.code(), Some(1), "{}", stderr(&checked));

    // `--verify` reports both as extra.
    let verified = ingest(&archives, &out, &["--verify"]);
    assert_eq!(verified.status.code(), Some(1), "{}", stderr(&verified));
    assert_eq!(
        stdout(&verified),
        "extra mine.fr\nextra us/usc/notes/x.fr\n"
    );

    // A run refuses the directory, names both, and changes nothing: both
    // files are there with their bytes.
    let refused = ingest(&archives, &out, &[]);
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert_eq!(stdout(&refused), "");
    let message = stderr(&refused);
    assert!(
        message.contains("`mine.fr`, which is no module of edition 2"),
        "{message}"
    );
    assert!(
        message.contains(
            "`us/usc/notes/x.fr`, whose module `/zz/mine/x` is not below the prefix `/us/usc`"
        ),
        "{message}"
    );
    assert_eq!(message.lines().count(), 1, "{message}");
    assert_eq!(tree(&out), before);
    assert_eq!(read(&out.join("mine.fr")), mine);
    assert_eq!(read(&out.join("us/usc/notes/x.fr")), other);

    // Without them the run is accepted again.
    fs::remove_file(out.join("mine.fr")).expect("remove");
    fs::remove_dir_all(out.join("us/usc/notes")).expect("remove");
    succeeded(&ingest(&archives, &out, &[]), "a run into its own output");
    assert_eq!(stdout(&ingest(&archives, &out, &["--verify"])), "ok\n");
}

#[test]
fn nothing_of_the_host_reaches_the_output() {
    let scratch = Scratch::new("host");
    let first_archives = scratch.join("first/archives");
    many_members(&first_archives);
    let second_archives = scratch.join("second/release/archives");
    copy_tree(&first_archives, &second_archives);
    let first_home = scratch.join("first/home");
    let second_home = scratch.join("second/home");
    for dir in [&first_home, &second_home] {
        fs::create_dir_all(dir).expect("create a home");
    }

    // One run with absolute paths, from one working directory, and the
    // other with paths relative to another, with another time zone, home,
    // locale and user.
    let first_out = scratch.join("first/out");
    let first = ingest_command(&first_archives, &first_out, &["--jobs", "3"])
        .current_dir(scratch.path())
        .env("TZ", "UTC")
        .env("HOME", &first_home)
        .env("LC_ALL", "C")
        .env("USER", "ingest-user-alpha")
        .output()
        .expect("run fidryn");
    succeeded(&first, "the first run");
    let second = ingest_command(
        Path::new("archives"),
        Path::new("../output/workspace"),
        &["--jobs", "5"],
    )
    .current_dir(scratch.join("second/release"))
    .env("TZ", "Pacific/Chatham")
    .env("HOME", &second_home)
    .env("LC_ALL", "tr_TR.UTF-8")
    .env("USER", "ingest-user-omega")
    .output()
    .expect("run fidryn");
    succeeded(&second, "the second run");
    assert_eq!(stdout(&first), stdout(&second));

    let written = tree(&first_out);
    assert_eq!(tree(&scratch.join("second/output/workspace")), written);
    let root = scratch.path().to_string_lossy().into_owned();
    for (rel, bytes) in &written {
        let Some(bytes) = bytes else { continue };
        let text = String::from_utf8_lossy(bytes);
        for host in [root.as_str(), "../output", "ingest-user-", "Chatham"] {
            assert!(!text.contains(host), "{rel} holds {host:?}");
        }
    }
    // Neither home was written into.
    for dir in [&first_home, &second_home] {
        assert!(tree(dir).is_empty(), "{}", dir.display());
    }
}
