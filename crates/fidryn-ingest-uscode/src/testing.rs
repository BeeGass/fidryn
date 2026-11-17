//! Helpers for the tests of this crate: the fixtures of the publisher's XML
//! and their expected records, archives packed by hand with their retrieval
//! record, and the counts a profile of the release holds.

use crate::{IngestError, IngestOptions, IngestSummary, ReleaseArchives, ingest};
use fidryn_provision::records::text_records;
use fidryn_provision::workspace::{RetrievalRecord, RetrievedArchive};
use fidryn_provision::{Hash32, Item, Module, Provision};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The release every fixture is read as.
pub(crate) const RELEASE: &str = "999-1";
/// The member every fixture of one document is read as.
pub(crate) const MEMBER: &str = "usc99.xml";

/// The root of the workspace.
pub(crate) fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// The directory of the invented documents and their expected records.
pub(crate) fn fixtures() -> PathBuf {
    workspace().join("conformance/fixtures/uslm")
}

/// The names of the fixtures of one document, sorted.
pub(crate) fn document_fixtures() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures())
        .expect("the fixtures are listed")
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

/// The fixtures that are directories of members, each read through an
/// archive.
pub(crate) const MEMBER_SETS: [&str; 2] = ["two-files", "unnamed-appendices"];

/// The text of the fixture `name`.
pub(crate) fn fixture(name: &str) -> String {
    let path = fixtures().join(format!("{name}.xml"));
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The members of the fixture directory `name`, by name.
pub(crate) fn fixture_members(name: &str) -> Vec<(String, Vec<u8>)> {
    let directory = fixtures().join(name);
    let mut members: Vec<(String, Vec<u8>)> = fs::read_dir(&directory)
        .expect("the fixture directory is listed")
        .map(|entry| {
            let entry = entry.expect("a member");
            let name = entry.file_name().into_string().expect("UTF-8");
            (name, fs::read(entry.path()).expect("the member is read"))
        })
        .collect();
    members.sort();
    members
}

/// The modules of the fixture `name`, read as `usc99.xml` of release `999-1`.
pub(crate) fn read_fixture(name: &str) -> Vec<Module> {
    crate::ingest_xml(MEMBER, &fixture(name), RELEASE)
        .unwrap_or_else(|error| panic!("{name}: {error}"))
        .0
}

/// A record line with every `"marks"` list left out, and the shown form of a
/// number written as the plain string when, without its markers, it holds
/// words only: the form the record of the same text without markers has.
pub(crate) fn without_marks(line: &str) -> String {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.remove("marks");
                for inner in object.values_mut() {
                    strip(inner);
                }
                if let Some(Value::Array(num)) = object.get_mut("num")
                    && let [_, shown @ Value::Object(_)] = num.as_mut_slice()
                {
                    let words_only = ["refs", "sup", "sub"]
                        .iter()
                        .all(|key| shown[*key].as_array().is_some_and(Vec::is_empty));
                    if words_only {
                        *shown = shown["text"].clone();
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value: Value = serde_json::from_str(line).expect("a record is JSON");
    strip(&mut value);
    serde_json::to_string(&value).expect("a record is written")
}

/// Whether a record line is that of a provision or a container.
fn is_operative(line: &str) -> bool {
    let value: Value = serde_json::from_str(line).expect("a record is JSON");
    value.get("layer") == Some(&Value::from("operative"))
}

/// The records of provisions and containers that `<name>.records.jsonl`
/// expects, without their markers.
pub(crate) fn expected_operative(name: &str) -> Vec<String> {
    let path = fixtures().join(format!("{name}.records.jsonl"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .lines()
        .filter(|line| is_operative(line))
        .map(without_marks)
        .collect()
}

/// The records of provisions and containers of `modules`, in the order of
/// their addresses, without their markers.
pub(crate) fn operative_records(modules: &[Module]) -> Vec<String> {
    let mut records: Vec<(String, String)> = modules
        .iter()
        .flat_map(text_records)
        .map(|record| (record.address().as_str().to_owned(), record.to_json()))
        .filter(|(_, line)| is_operative(line))
        .collect();
    records.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    records
        .into_iter()
        .map(|(_, line)| without_marks(&line))
        .collect()
}

/// Asserts that the fixture `name` reads to its expected records.
pub(crate) fn assert_fixture(name: &str) {
    let expected = expected_operative(name);
    assert!(!expected.is_empty(), "{name} expects no operative record");
    assert_eq!(operative_records(&read_fixture(name)), expected, "{name}");
}

/// An empty directory under the system's directory for temporary files,
/// removed when the value is dropped. Each one is named by the process, by a
/// number no other directory of the process has, and by `name`, so that tests
/// that run side by side never share one.
pub(crate) struct Scratch(PathBuf);

/// The number of the next scratch directory of the process.
static NEXT_SCRATCH: AtomicUsize = AtomicUsize::new(0);

impl Scratch {
    pub(crate) fn new(name: &str) -> Self {
        let number = NEXT_SCRATCH.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "fidryn-ingest-uscode-{}-{number}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory is made");
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The bytes of a zip archive of `members`, each compressed with deflate.
pub(crate) fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in members {
        writer
            .start_file(*name, options)
            .expect("a member is begun");
        writer.write_all(bytes).expect("a member is written");
    }
    writer
        .finish()
        .expect("the archive is finished")
        .into_inner()
}

/// Writes the archive `name` of `members` into `directory`.
pub(crate) fn pack(directory: &Path, name: &str, members: &[(&str, &[u8])]) {
    fs::write(directory.join(name), zip_of(members)).expect("the archive is written");
}

/// The entry of a retrieval record for the archive `name` in `directory`,
/// with its size and digests as they are.
pub(crate) fn entry_for(directory: &Path, name: &str) -> RetrievedArchive {
    let bytes = fs::read(directory.join(name)).expect("the archive is read");
    let sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    RetrievedArchive {
        file: name.to_owned(),
        url: format!("https://example.invalid/{name}"),
        retrieved_at: "2099-01-01T00:00:00Z".to_owned(),
        bytes: u64::try_from(bytes.len()).expect("a small archive"),
        sha256,
        blake3: Hash32(*blake3::hash(&bytes).as_bytes()),
    }
}

/// Writes `retrieval.json` into `directory` for `archives` of `release`.
pub(crate) fn write_record(directory: &Path, release: &str, archives: Vec<RetrievedArchive>) {
    let record = RetrievalRecord {
        release: release.to_owned(),
        archives,
    };
    fs::write(directory.join("retrieval.json"), record.to_json()).expect("the record is written");
}

/// Writes the record of every archive in `directory` as it is, for
/// `release`.
pub(crate) fn record_all(directory: &Path, release: &str) {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("the directory is listed")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .filter(|name| name.ends_with(".zip"))
        .collect();
    names.sort();
    let archives = names
        .iter()
        .map(|name| entry_for(directory, name))
        .collect();
    write_record(directory, release, archives);
}

/// A release of one archive of `members`, opened.
pub(crate) fn release_of(scratch: &Scratch, members: &[(&str, &[u8])]) -> ReleaseArchives {
    pack(scratch.path(), "xml_usc99@999-1.zip", members);
    record_all(scratch.path(), RELEASE);
    crate::open(scratch.path()).unwrap_or_else(|error| panic!("{error}"))
}

/// The modules an ingest of `archives` gives, in the order of their
/// addresses, and its summary.
pub(crate) fn ingest_all(
    archives: &ReleaseArchives,
    titles: &[&str],
    jobs: usize,
) -> Result<(Vec<Module>, IngestSummary), IngestError> {
    let modules = Mutex::new(Vec::new());
    let options = IngestOptions {
        titles: titles.iter().map(|title| (*title).to_owned()).collect(),
        jobs,
    };
    let summary = ingest(archives, &options, &|module| {
        modules.lock().expect("the modules").push(module);
        Ok(())
    })?;
    let mut modules = modules.into_inner().expect("the modules");
    modules.sort_by(|a, b| address_of(a).cmp(address_of(b)));
    Ok((modules, summary))
}

/// The address of a module.
pub(crate) fn address_of(module: &Module) -> &str {
    match module {
        Module::Unit(unit) => unit.address.as_str(),
        Module::Container(container) => container.address.as_str(),
        Module::Curated(curated) => curated.address.as_str(),
    }
}

/// The counts of a profile's `import` that the modules give: units by kind
/// and by status, containers by kind, and provisions.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) units: u64,
    pub(crate) units_by_kind: BTreeMap<String, u64>,
    pub(crate) units_by_status: BTreeMap<String, u64>,
    pub(crate) containers: u64,
    pub(crate) containers_by_kind: BTreeMap<String, u64>,
    pub(crate) provisions: u64,
}

impl Counts {
    /// The counts of `modules`.
    pub(crate) fn of(modules: &[Module]) -> Self {
        fn provisions(provision: &Provision) -> u64 {
            1 + provision
                .items
                .iter()
                .map(|item| match item {
                    Item::Child(child) => provisions(child),
                    Item::Part { .. } => 0,
                })
                .sum::<u64>()
        }
        let mut counts = Self::default();
        for module in modules {
            match module {
                Module::Unit(unit) => {
                    counts.units += 1;
                    *counts
                        .units_by_kind
                        .entry(unit.root.kind.clone())
                        .or_default() += 1;
                    let status = unit.root.status.clone().unwrap_or("in-force".to_owned());
                    *counts.units_by_status.entry(status).or_default() += 1;
                    counts.provisions += provisions(&unit.root);
                }
                Module::Container(container) => {
                    counts.containers += 1;
                    *counts
                        .containers_by_kind
                        .entry(container.kind.clone())
                        .or_default() += 1;
                }
                Module::Curated(_) => {}
            }
        }
        counts
    }

    /// The counts a profile's `import` object holds.
    pub(crate) fn of_profile(profile: &Value) -> Self {
        let number = |key: &str| profile[key].as_u64().unwrap_or_else(|| panic!("{key}"));
        let map = |key: &str| -> BTreeMap<String, u64> {
            profile[key]
                .as_object()
                .unwrap_or_else(|| panic!("{key}"))
                .iter()
                .map(|(name, count)| (name.clone(), count.as_u64().expect("a count")))
                .collect()
        };
        Self {
            units: number("units"),
            units_by_kind: map("units_by_kind"),
            units_by_status: map("units_by_status"),
            containers: number("containers"),
            containers_by_kind: map("containers_by_kind"),
            provisions: number("provisions"),
        }
    }
}

/// The JSON of the file at `path`.
pub(crate) fn json_file(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A document of release `999-1` with one section `/us/usc/t99/s1`, the
/// title and the section holding what is given, in the form of the
/// refusal documents of the independent reading.
pub(crate) struct Document {
    pub(crate) doctype: &'static str,
    pub(crate) root: &'static str,
    pub(crate) namespace: &'static str,
    pub(crate) schema: &'static str,
    pub(crate) release: &'static str,
    pub(crate) flag: &'static str,
    pub(crate) body: String,
}

impl Document {
    /// The document whose section holds `inner`.
    pub(crate) fn with_section(inner: &str) -> Self {
        Self::with_body(format!(
            "<section identifier=\"/us/usc/t99/s1\"><num value=\"1\">§ 1.</num>\
             <heading>Invented</heading>{inner}</section>"
        ))
    }

    /// The document whose title holds `body`.
    pub(crate) fn with_body(body: String) -> Self {
        Self {
            doctype: "",
            root: "uscDoc",
            namespace: "http://xml.house.gov/schemas/uslm/1.0",
            schema: "USLM-1.0.15.xsd",
            release: "Online@999-1",
            flag: "yes",
            body,
        }
    }

    pub(crate) fn text(&self) -> String {
        format!(
            "<!-- Invented for tests: this document is not the text of any law. -->\n\
             {doctype}<{root} xmlns=\"{namespace}\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
             xsi:schemaLocation=\"http://xml.house.gov/schemas/uslm/1.0 {schema}\">\n\
             <meta><docPublicationName>{release}</docPublicationName>\
             <property role=\"is-positive-law\">{flag}</property></meta>\n\
             <main><title identifier=\"/us/usc/t99\"><num value=\"99\">Title 99</num>\
             <heading>Invented</heading>\n{body}\n</title></main>\n</{root}>\n",
            doctype = self.doctype,
            root = self.root,
            namespace = self.namespace,
            schema = self.schema,
            release = self.release,
            flag = self.flag,
            body = self.body,
        )
    }

    /// The modules of the document, or why it is refused.
    pub(crate) fn read(&self) -> Result<Vec<Module>, IngestError> {
        crate::ingest_xml(MEMBER, &self.text(), RELEASE).map(|(modules, _)| modules)
    }
}

/// The refusal of `document`, which must be refused.
pub(crate) fn refusal(document: &Document) -> IngestError {
    match document.read() {
        Ok(modules) => panic!(
            "{} was read into {} modules",
            document.text(),
            modules.len()
        ),
        Err(error) => error,
    }
}
