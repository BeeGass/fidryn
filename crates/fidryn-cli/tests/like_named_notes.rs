//! `fidryn changes` between two imports of the Code pairs like-named notes
//! by their text before it compares them by address. Each pair of
//! `conformance/fixtures/uslm/pairs/` used here is two invented documents of
//! the invented title 99, `old.xml` of the release `999-1` and `new.xml` of
//! `999-2`, which the test packs into archives beside a retrieval record,
//! imports with `fidryn ingest uscode`, and compares; the whole change set is
//! written out below by hand from the rules of change sets.

use fidryn_provision::Address;
use fidryn_provision::Hash32;
use fidryn_provision::changes::{ChangeSet, Moved, Release};
use fidryn_provision::workspace::{RetrievalRecord, RetrievedArchive};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// The first of the like-named notes of the section of each pair.
const EFFECTIVE: &str = "/us/usc/t99/s1/note/statutoryNotes/nolaw/effective-date";

fn pairs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/uslm/pairs")
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A directory of its own under the temporary directory, removed when
/// dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fidryn-cli-like-named-{}-{n}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch directory");
        Scratch(dir)
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Writes into `dir` one archive of the release `release` that holds
/// `document` as the member `usc99.xml`, and its retrieval record.
fn release(dir: &Path, release: &str, document: &Path) {
    fs::create_dir_all(dir).expect("create the directory of the archives");
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("usc99.xml", options)
        .expect("begin the member");
    writer
        .write_all(&fs::read(document).expect("read the document"))
        .expect("write the member");
    let bytes = writer.finish().expect("finish the archive").into_inner();
    let name = format!("xml_usc99@{release}.zip");
    fs::write(dir.join(&name), &bytes).expect("write the archive");
    let record = RetrievalRecord {
        release: release.to_owned(),
        archives: vec![RetrievedArchive {
            file: name.clone(),
            url: format!("https://example.invalid/releases/{release}/{name}"),
            retrieved_at: "2099-03-04T05:06:07Z".to_owned(),
            bytes: u64::try_from(bytes.len()).expect("a small archive"),
            sha256: hex(&Sha256::digest(&bytes)),
            blake3: Hash32(*blake3::hash(&bytes).as_bytes()),
        }],
    };
    fs::write(dir.join("retrieval.json"), record.to_json()).expect("write the record");
}

fn fidryn(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fidryn"))
        .args(args)
        .env_remove("FIDRYN_CACHE_DIR")
        .output()
        .expect("run fidryn")
}

fn succeeded(output: &Output, what: &str) -> String {
    assert!(
        output.status.success(),
        "{what}: exit {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("the output is UTF-8")
}

/// The pair `name` imported into `old/` and `new/` of `scratch`, and the
/// change set that holds nothing but the releases and lock digests of the
/// two, as each list of it is filled in.
fn imported(scratch: &Scratch, name: &str) -> ChangeSet {
    let mut sides = Vec::new();
    for (document, version, side) in [("old.xml", "999-1", "old"), ("new.xml", "999-2", "new")] {
        let archives = scratch.join(&format!("{side}-archives"));
        release(&archives, version, &pairs().join(name).join(document));
        let out = scratch.join(side);
        succeeded(
            &fidryn(&[
                OsStr::new("ingest"),
                OsStr::new("uscode"),
                archives.as_os_str(),
                OsStr::new("--out"),
                out.as_os_str(),
            ]),
            &format!("ingest of {name}/{document}"),
        );
        let lock = fs::read(out.join("law.lock.json")).expect("read the lock");
        sides.push(Release {
            release: version.to_owned(),
            lock_digest: Hash32(*blake3::hash(&lock).as_bytes()),
        });
    }
    let new = sides.pop().expect("the new side");
    let old = sides.pop().expect("the old side");
    ChangeSet::empty(address("/us/usc"), old, new)
}

/// Asserts that `fidryn changes` between the two sides in `scratch` prints
/// `expected`, as JSON and as text.
fn prints(scratch: &Scratch, expected: &ChangeSet) {
    let (old, new) = (scratch.join("old"), scratch.join("new"));
    let json = fidryn(&[
        OsStr::new("changes"),
        old.as_os_str(),
        new.as_os_str(),
        OsStr::new("--json"),
        OsStr::new("--no-cache"),
    ]);
    assert_eq!(succeeded(&json, "changes --json"), expected.to_json());
    let text = fidryn(&[
        OsStr::new("changes"),
        old.as_os_str(),
        new.as_os_str(),
        OsStr::new("--no-cache"),
    ]);
    assert_eq!(succeeded(&text, "changes"), expected.to_text());
}

fn address(text: &str) -> Address {
    Address::parse(text).expect("an address")
}

/// The like-named note `ordinal` of the section: none for the first, `/2`
/// for the second.
fn effective(ordinal: u32) -> Address {
    match ordinal {
        1 => address(EFFECTIVE),
        ordinal => address(&format!("{EFFECTIVE}/{ordinal}")),
    }
}

fn moved(from: u32, to: u32) -> Moved {
    Moved {
        from: effective(from),
        to: effective(to),
    }
}

#[test]
fn a_note_inserted_before_two_like_named_notes_is_added_and_they_have_moved() {
    let scratch = Scratch::new("inserted");
    let mut expected = imported(&scratch, "inserted-note");
    expected.enacted_notes.added = vec![effective(1)];
    expected.enacted_notes.moved = vec![moved(1, 2), moved(2, 3)];
    prints(&scratch, &expected);
}

#[test]
fn a_note_amended_while_another_is_inserted_before_it_is_compared_by_address() {
    let scratch = Scratch::new("amended-inserted");
    let mut expected = imported(&scratch, "amended-and-inserted-note");
    expected.enacted_notes.amended = vec![effective(1)];
    expected.enacted_notes.added = vec![effective(2)];
    expected.enacted_notes.moved = vec![moved(2, 3)];
    prints(&scratch, &expected);
}
