//! The cache of the check of a workspace: the result of phase one of each
//! module file ([`PhaseOne`]), kept on disk and keyed by the file's bytes.
//!
//! The key of a file is the BLAKE3 of four frames, built as the hashes of
//! `fidryn_provision::hash` are, each one tag byte, the length of its value
//! as eight bytes little-endian, and the value: `F(0x00, "fidryn.cache/v1")`,
//! `F(0x01, the toolchain's version)`, `F(0x02, the identity of the running
//! executable)` and `F(0x03, the file's bytes)`. An entry is therefore read
//! only by the build of the toolchain that wrote it, and two files with the
//! same bytes, in one workspace or in two, share one entry.
//!
//! The entry of a key is the file `v1/<first two digits>/<key>.json` below
//! the cache's directory, one line of compact JSON with the keys `schema`,
//! `key`, `digest`, `diagnostics` and `interface`. It is looked up by its
//! path, on the thread that reads its module file; the directory is never
//! listed. The check writes the entries it missed after it has read every
//! file, so that none of its reads sees an entry it wrote. An entry is
//! written to a temporary name in its directory and renamed, so that a
//! reader never sees half an entry and two checks at once leave a valid
//! cache. An entry is read back only when it is what the writer writes; any
//! other, one cut short, one that is not JSON or is JSON of another shape,
//! or one of another key or of other bytes, is a miss, and the check writes
//! it again. A failure to read an entry is a miss, and a failure to write
//! one fails nothing: the first is recorded, with the cache's directory, for
//! the command to say once, and no other entry is written.
//!
//! An entry that is what the writer writes is trusted as the result of the
//! bytes it names. Its key is no secret, since every frame of it can be
//! known, so whoever can write in the cache's directory can make a check
//! pass a module that a check without the cache refuses: the directory is
//! one that only those whose results are trusted write.

use super::check::PhaseOne;
use fidryn_core::Diagnostic;
use fidryn_provision::{Hash32, Interface};
use serde_json::{Map, Value};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// The domain of a key, and the `schema` of an entry.
const CACHE_SCHEMA: &str = "fidryn.cache/v1";

/// The directory, below the cache's, of the entries of this layout.
const LAYOUT_DIR: &str = "v1";

/// The tags of the frames of a key.
const DOMAIN_TAG: u8 = 0x00;
const TOOLCHAIN_TAG: u8 = 0x01;
const EXECUTABLE_TAG: u8 = 0x02;
const BYTES_TAG: u8 = 0x03;

/// How many temporary names a writer tries before it gives up.
const TEMPORARY_TRIES: u32 = 64;

/// A number for each temporary name this process takes.
static TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// The JSON of the entry of `key` for a file whose bytes have the BLAKE3
/// `digest`, and whose phase one gives `diagnostics`, none of which names a
/// file, and `interface`: `schema`, `key`, `digest`, `diagnostics`, each as
/// `fidryn_core::Diagnostic` writes it, and `interface`, the object that
/// the JSON of an interface holds under `interface`, or `null` when the
/// module does not lower.
fn entry(
    key: &Hash32,
    digest: &Hash32,
    diagnostics: &[Diagnostic],
    interface: Option<&Interface>,
) -> Value {
    let written = "an entry of strings, numbers, digests and addresses is written";
    let mut entry = Map::new();
    entry.insert("schema".to_owned(), Value::from(CACHE_SCHEMA));
    entry.insert("key".to_owned(), Value::from(key.to_string()));
    entry.insert("digest".to_owned(), Value::from(digest.to_string()));
    entry.insert(
        "diagnostics".to_owned(),
        serde_json::to_value(diagnostics).expect(written),
    );
    entry.insert(
        "interface".to_owned(),
        serde_json::to_value(interface).expect(written),
    );
    Value::Object(entry)
}

/// The cache in one directory, for one build of the toolchain. It is shared
/// by the worker threads of the check.
pub(super) struct Cache {
    /// The cache's directory, which a failure names.
    dir: PathBuf,
    /// The directory of the entries, [`LAYOUT_DIR`] below the cache's.
    entries: PathBuf,
    toolchain: String,
    executable: String,
    /// Why an entry could not be written, the first time one could not.
    failure: OnceLock<String>,
}

impl Cache {
    /// The cache in the directory `dir`, for the toolchain of version
    /// `toolchain` whose running executable has the identity `executable`.
    /// Nothing is read or made until an entry is.
    pub(super) fn new(dir: &Path, toolchain: &str, executable: &str) -> Cache {
        Cache {
            dir: dir.to_owned(),
            entries: dir.join(LAYOUT_DIR),
            toolchain: toolchain.to_owned(),
            executable: executable.to_owned(),
            failure: OnceLock::new(),
        }
    }

    /// The key of a module file whose bytes are `bytes`.
    pub(super) fn key(&self, bytes: &[u8]) -> Hash32 {
        let mut hasher = blake3::Hasher::new();
        let frames = [
            (DOMAIN_TAG, CACHE_SCHEMA.as_bytes()),
            (TOOLCHAIN_TAG, self.toolchain.as_bytes()),
            (EXECUTABLE_TAG, self.executable.as_bytes()),
            (BYTES_TAG, bytes),
        ];
        for (tag, value) in frames {
            let length = u64::try_from(value.len()).expect("a length fits in 64 bits");
            hasher.update(&[tag]);
            hasher.update(&length.to_le_bytes());
            hasher.update(value);
        }
        Hash32(*hasher.finalize().as_bytes())
    }

    /// The path of the entry of `key`.
    pub(super) fn entry_path(&self, key: &Hash32) -> PathBuf {
        let key = key.to_string();
        self.entries.join(&key[..2]).join(format!("{key}.json"))
    }

    /// The result of phase one kept under `key` for a file whose bytes have
    /// the BLAKE3 `digest`, when the entry is one the writer wrote for them;
    /// `None` when there is none, when it cannot be read, and when it is
    /// damaged.
    pub(super) fn read(&self, key: &Hash32, digest: &Hash32) -> Option<PhaseOne> {
        let text = fs::read_to_string(self.entry_path(key)).ok()?;
        let given: Value = serde_json::from_str(&text).ok()?;
        let fields = given.as_object()?;
        let digest_of = |name: &str| fields.get(name)?.as_str()?.parse::<Hash32>().ok();
        let named = fields.get("schema").and_then(Value::as_str) == Some(CACHE_SCHEMA)
            && digest_of("key") == Some(*key)
            && digest_of("digest") == Some(*digest);
        if !named {
            return None;
        }
        let diagnostics: Vec<Diagnostic> =
            serde_json::from_value(fields.get("diagnostics")?.clone()).ok()?;
        let interface: Option<Interface> =
            serde_json::from_value(fields.get("interface")?.clone()).ok()?;
        // The readers `serde` derives take more than the writer writes: a
        // struct written as an array of its values, a missing optional
        // field, a file named on a diagnostic. The value read is compared
        // with the value that what was read from it writes.
        if entry(key, digest, &diagnostics, interface.as_ref()) != given
            || diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_file.is_some())
        {
            return None;
        }
        let interface = match interface {
            Some(interface) => Some(interface.validated().ok()?),
            None => None,
        };
        Some(PhaseOne {
            digest: Some(*digest),
            diagnostics,
            interface,
        })
    }

    /// Keeps `result`, phase one of a file whose key is `key`, replacing
    /// any entry of the key. A result without a digest, of a file that could
    /// not be read, is not kept. A failure is recorded, the first one only,
    /// as the cache's directory and the error, and is not an error; once
    /// one is recorded, nothing more is written.
    pub(super) fn write(&self, key: &Hash32, result: &PhaseOne) {
        let Some(digest) = result.digest else {
            return;
        };
        if self.failure.get().is_some() {
            return;
        }
        let entry = entry(key, &digest, &result.diagnostics, result.interface.as_ref());
        let mut text = entry.to_string();
        text.push('\n');
        if let Err(error) = write_replacing(&self.entry_path(key), text.as_bytes()) {
            let _ = self.failure.set(format!("{}: {error}", self.dir.display()));
        }
    }

    /// Why an entry could not be written, the first time one could not: the
    /// cache's directory and the error.
    pub(super) fn failure(self) -> Option<String> {
        self.failure.into_inner()
    }
}

/// Writes `bytes` to the file `path`, replacing it: to a temporary name in
/// its directory, which is made when it is missing, and then renamed, so
/// that a reader finds the whole of the new file or the whole of the old
/// one.
fn write_replacing(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().expect("an entry lies in a directory");
    let name = path
        .file_name()
        .expect("an entry has a name")
        .to_string_lossy();
    fs::create_dir_all(dir)?;
    let (temporary, mut file) = temporary_file(dir, &name)?;
    let mut written = file.write_all(bytes);
    drop(file);
    if written.is_ok() {
        written = fs::rename(&temporary, path);
    }
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

/// A new file in `dir` whose name no other writer takes: a dot, `name`, the
/// number of this process and a number of its own, which is never the name
/// of an entry.
fn temporary_file(dir: &Path, name: &str) -> io::Result<(PathBuf, fs::File)> {
    let process = std::process::id();
    let mut taken = None;
    for _ in 0..TEMPORARY_TRIES {
        let n = TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(".{name}.{process}.{n}.tmp"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => taken = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(taken.expect("a name was tried"))
}
