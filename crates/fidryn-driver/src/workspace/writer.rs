//! The writer of a generated workspace, and the comparison of a directory
//! with what a run would write into it.

use super::discover::{JSON_LIMIT, Unread, Walk, is_dot_named, read_regular, walk};
use super::report::ledger_of;
use super::{COVERAGE_FILE, LOCK_FILE, MANIFEST_FILE};
use fidryn_provision::layout::module_path;
use fidryn_provision::render::render;
use fidryn_provision::workspace::{
    AddressCounts, Artifact, Generator, Lock, Manifest, NotImported, WorkspaceKind,
};
use fidryn_provision::{Address, DeclSpans, Hash32, Interface, Module, interface_of};
use fidryn_syntax::{TokenKind, decode_string_literal, lex};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// The directory, at the top of the workspace, that holds the files of a run
/// while they are written. Its name begins with a dot, so no check reads it.
const SCRATCH: &str = ".fidryn-write";

/// Writes a generated workspace: its manifest, the modules of one run, then
/// its lock, and last its coverage ledger, `coverage.json`. A run that stops
/// before its end leaves a generated workspace of the same prefix whose lock
/// is missing or outdated, which the check reports, or, stopped between the
/// two, whose ledger is missing or outdated; the next run accepts the
/// directory and puts it right.
///
/// [`GeneratedWriter::write`] may be called from several threads at once.
/// What the run writes depends only on the modules, the manifest and what
/// [`GeneratedWriter::finish`] is given, not on the order of the calls or the
/// number of threads.
///
/// A writer made by [`GeneratedWriter::compare`] writes nothing: it compares
/// each file the run would write with the file at its path, and its `finish`
/// reports every path that differs, every entry that `create` would refuse
/// among them.
pub struct GeneratedWriter {
    out: PathBuf,
    manifest: Manifest,
    /// The path of every module file the directory held when a writer that
    /// writes was made, each an earlier output of a writer of the prefix,
    /// in the byte order of the paths: the only files `finish` may remove.
    earlier: Vec<String>,
    /// The BLAKE3 of the bytes of every module the run has written, by its
    /// path.
    written: Mutex<BTreeMap<String, Hash32>>,
    /// The interface of every module the run has written, by its path, built
    /// from the module as it was given, which the ledger is counted from.
    interfaces: Mutex<BTreeMap<String, Interface>>,
    /// The number of the next temporary file.
    next: AtomicU64,
    /// What a writer made by [`GeneratedWriter::compare`] compares with;
    /// `None` for a writer that writes.
    compared: Option<Compared>,
}

/// The directory a comparing writer compares with, and what it has found to
/// differ so far.
struct Compared {
    /// Every entry below the directory when the writer was made, or none
    /// when the directory does not exist.
    found: Walk,
    /// Every path found to differ, with how it differs.
    differences: Mutex<BTreeMap<String, Difference>>,
}

/// How a path of a directory differs from what a run writes there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difference {
    /// The run writes a file at the path, and the directory holds nothing
    /// there.
    Missing,
    /// The directory holds an entry at the path that a run neither writes
    /// nor keeps: a module the run does not write, a file that is no module
    /// and not one a run keeps at the top, or an entry that is neither a
    /// file nor a directory, such as a symbolic link.
    Extra,
    /// The directory holds a file of other bytes at the path, or something
    /// that is not a file.
    Different,
}

impl Difference {
    /// The word that says how the path differs: `missing`, `extra` or
    /// `different`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Extra => "extra",
            Self::Different => "different",
        }
    }
}

/// Why a [`GeneratedWriter`] did not write.
#[derive(Debug)]
pub enum WriteError {
    /// The manifest given to `create` is not that of a generated workspace.
    Manifest(String),
    /// The directory given to `create` holds something other than a
    /// generated workspace of the manifest's prefix: `found` says what.
    Foreign { out: PathBuf, found: String },
    /// `write` was given a module that a generated workspace does not hold.
    Module(String),
    /// The run has already written a module to `path`.
    Twice { path: String },
    /// A file or a directory could not be read or written.
    Io { path: PathBuf, error: io::Error },
    /// The directory given to `compare` does not hold what the run writes:
    /// every path that differs, relative to it with `/` between its
    /// segments, in the byte order of the paths, with how it differs.
    Differs {
        out: PathBuf,
        differences: Vec<(String, Difference)>,
    },
}

impl GeneratedWriter {
    /// A writer of the generated workspace `manifest` into the directory
    /// `out`.
    ///
    /// `out` is a directory that does not exist, which is created, one that
    /// holds nothing but entries whose name begins with a dot, or one whose
    /// `workspace.json` is the manifest of a generated workspace of the same
    /// prefix. Such a directory may hold, besides the module files of an
    /// earlier run and the manifest, the lock and the ledger `coverage.json`
    /// at its top, which the run writes again, and, anywhere, any file,
    /// directory or symbolic link whose name begins with a dot, which the
    /// run leaves as they are. A module file of an earlier run is a module of
    /// edition 2 whose address lies below the prefix and which stands at the
    /// path of its address; every `.fr` file is examined, by its head,
    /// before anything is written. Any other directory, one that holds a
    /// `.fr` file of the first edition, a module of another prefix or a
    /// module at another path among them, is refused with every such file
    /// named, and nothing in it is changed.
    /// `manifest` is of kind `generated`, has a release and has no
    /// dependencies; it is written at once, so that a run that stops before
    /// its end leaves a directory the next run accepts.
    pub fn create(out: &Path, manifest: Manifest) -> Result<GeneratedWriter, WriteError> {
        refuse_manifest(&manifest)?;
        let earlier = match fs::metadata(out) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(out).map_err(|error| io_error(out, error))?;
                Vec::new()
            }
            Err(error) => return Err(io_error(out, error)),
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io_error(out, io::ErrorKind::NotADirectory.into()));
            }
            Ok(_) => refuse_foreign(out, &manifest)?,
        };
        let scratch = out.join(SCRATCH);
        match fs::remove_dir_all(&scratch) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(io_error(&scratch, error));
            }
            _ => {}
        }
        fs::create_dir(&scratch).map_err(|error| io_error(&scratch, error))?;
        let writer = GeneratedWriter {
            out: out.to_owned(),
            manifest,
            earlier,
            written: Mutex::new(BTreeMap::new()),
            interfaces: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(0),
            compared: None,
        };
        writer.put(MANIFEST_FILE, &writer.manifest.to_json())?;
        Ok(writer)
    }

    /// A writer that writes nothing and compares what a run of the
    /// generated workspace `manifest` would write with what the directory
    /// `out` holds: the manifest, each module given to
    /// [`write`](Self::write), and the lock and the ledger
    /// [`finish`](Self::finish) writes. Every other entry of `out` that
    /// [`create`](Self::create) would refuse is extra; an entry whose name
    /// begins with a dot is not looked at.
    ///
    /// `manifest` is refused as by `create`. `out` is a directory, or does
    /// not exist, in which case every file the run writes is missing and
    /// nothing is made. A directory that `create` would refuse is not
    /// refused here: `finish` reports what in it differs.
    pub fn compare(out: &Path, manifest: Manifest) -> Result<GeneratedWriter, WriteError> {
        refuse_manifest(&manifest)?;
        let found = match fs::metadata(out) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Walk::default(),
            Err(error) => return Err(io_error(out, error)),
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io_error(out, io::ErrorKind::NotADirectory.into()));
            }
            Ok(_) => {
                let found = walk(out);
                if let Some((rel, why)) = found.unreadable.first() {
                    return Err(WriteError::Io {
                        path: out.join(rel),
                        error: io::Error::other(why.clone()),
                    });
                }
                found
            }
        };
        let writer = GeneratedWriter {
            out: out.to_owned(),
            manifest,
            earlier: Vec::new(),
            written: Mutex::new(BTreeMap::new()),
            interfaces: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(0),
            compared: Some(Compared {
                found,
                differences: Mutex::new(BTreeMap::new()),
            }),
        };
        writer.put(MANIFEST_FILE, &writer.manifest.to_json())?;
        Ok(writer)
    }

    /// Renders `module`, a unit or a container whose address is below the
    /// prefix, and writes it to its path through a temporary name, or, for
    /// a writer made by [`compare`](Self::compare), compares it with the
    /// file at its path; and keeps its interface, for the ledger. Fails when
    /// the run has already written a module to that path, so that no module
    /// replaces another.
    pub fn write(&self, module: &Module) -> Result<(), WriteError> {
        let address = match module {
            Module::Unit(unit) => &unit.address,
            Module::Container(container) => &container.address,
            Module::Curated(curated) => {
                return Err(WriteError::Module(format!(
                    "`{}` is a curated module, and a generated workspace holds units and \
                     containers",
                    curated.address
                )));
            }
        };
        let prefix = &self.manifest.prefix;
        if address == prefix || !address.is_within(prefix) {
            return Err(WriteError::Module(format!(
                "the address `{address}` is not below the prefix `{prefix}` of the workspace"
            )));
        }
        let text = render(module);
        let path = module_path(address);
        {
            let mut written = self.written.lock().unwrap_or_else(PoisonError::into_inner);
            if written.contains_key(&path) {
                return Err(WriteError::Twice { path });
            }
            written.insert(
                path.clone(),
                Hash32(*blake3::hash(text.as_bytes()).as_bytes()),
            );
        }
        self.put(&path, &text).inspect_err(|_| {
            self.written
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&path);
        })?;
        let interface = interface_of(module, &DeclSpans::default());
        self.interfaces
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path, interface);
        Ok(())
    }

    /// Ends the run: removes every module file of an earlier run, found when
    /// the writer was made, that the run did not write, and each directory
    /// that leaves empty, then writes `law.lock.json`, the lock of
    /// `generator`, `artifacts`, `addresses`, `not_imported` and every module
    /// the run wrote, which it returns, and last `coverage.json`, the
    /// coverage ledger of the workspace
    /// ([`coverage`](fidryn_provision::records::coverage)), counted from the
    /// interfaces of the modules as they were given, the manifest and the
    /// lock: what `fidryn coverage` prints of the workspace. An entry whose
    /// name begins with a dot is not touched.
    ///
    /// A writer made by [`compare`](Self::compare) removes and writes
    /// nothing. It compares the lock and the ledger, and counts as extra
    /// every module file of the directory that the run did not write and
    /// every entry that `create` would refuse. It returns the lock when
    /// nothing differs, in which case `create` accepts the directory and a
    /// run of the same modules changes none of its bytes, and
    /// [`WriteError::Differs`] otherwise.
    pub fn finish(
        self,
        generator: Generator,
        artifacts: Vec<Artifact>,
        addresses: AddressCounts,
        not_imported: NotImported,
    ) -> Result<Lock, WriteError> {
        let modules =
            std::mem::take(&mut *self.written.lock().unwrap_or_else(PoisonError::into_inner));
        if let Some(compared) = &self.compared {
            let lock = self.lock(generator, artifacts, addresses, not_imported, modules);
            self.put(LOCK_FILE, &lock.to_json())?;
            self.put(COVERAGE_FILE, &self.ledger(&lock))?;
            let mut differences = std::mem::take(
                &mut *compared
                    .differences
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner),
            );
            for rel in &compared.found.modules {
                if !lock.modules.contains_key(rel) {
                    differences.insert(rel.clone(), Difference::Extra);
                }
            }
            // An entry that is not the workspace's is extra, unless it
            // stands where the run writes a file and is already different.
            for (rel, _) in strays(&compared.found) {
                differences
                    .entry(rel.to_owned())
                    .or_insert(Difference::Extra);
            }
            if differences.is_empty() {
                return Ok(lock);
            }
            return Err(WriteError::Differs {
                out: self.out.clone(),
                differences: differences.into_iter().collect(),
            });
        }
        let found = walk(&self.out);
        if let Some((rel, why)) = found.unreadable.first() {
            return Err(WriteError::Io {
                path: self.out.join(rel),
                error: io::Error::other(why.clone()),
            });
        }
        // A `.fr` file that came after the writer was made was not examined,
        // and is left for the check to report.
        for rel in found
            .modules
            .iter()
            .filter(|rel| !modules.contains_key(*rel) && self.earlier.binary_search(*rel).is_ok())
        {
            let file = self.out.join(rel);
            fs::remove_file(&file).map_err(|error| io_error(&file, error))?;
            let mut dir = Path::new(rel).parent();
            while let Some(parent) = dir.filter(|parent| !parent.as_os_str().is_empty()) {
                if fs::remove_dir(self.out.join(parent)).is_err() {
                    break;
                }
                dir = parent.parent();
            }
        }
        let lock = self.lock(generator, artifacts, addresses, not_imported, modules);
        self.put(LOCK_FILE, &lock.to_json())?;
        self.put(COVERAGE_FILE, &self.ledger(&lock))?;
        let scratch = self.out.join(SCRATCH);
        fs::remove_dir_all(&scratch).map_err(|error| io_error(&scratch, error))?;
        Ok(lock)
    }

    /// The text of the ledger of the run, whose lock is `lock`, from the
    /// interfaces of the modules it wrote, in the order of their paths.
    fn ledger(&self, lock: &Lock) -> String {
        let interfaces: Vec<Interface> = std::mem::take(
            &mut *self
                .interfaces
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
        .into_values()
        .collect();
        ledger_of(&interfaces, &self.manifest, lock).to_json()
    }

    /// The lock of the run.
    fn lock(
        &self,
        generator: Generator,
        artifacts: Vec<Artifact>,
        addresses: AddressCounts,
        not_imported: NotImported,
        modules: BTreeMap<String, Hash32>,
    ) -> Lock {
        Lock {
            prefix: self.manifest.prefix.clone(),
            release: self
                .manifest
                .release
                .clone()
                .expect("`create` and `compare` take only a manifest with a release"),
            generator,
            artifacts,
            addresses,
            not_imported,
            modules,
        }
    }

    /// Writes `text` to the file `rel` below the workspace's directory: to a
    /// temporary file in the scratch directory first, which is then renamed.
    /// A comparing writer compares `text` with the file instead.
    fn put(&self, rel: &str, text: &str) -> Result<(), WriteError> {
        if let Some(compared) = &self.compared {
            return self.compare_file(compared, rel, text);
        }
        let target = self.out.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        let number = self.next.fetch_add(1, Ordering::Relaxed);
        let temporary = self.out.join(SCRATCH).join(format!("{number}.tmp"));
        fs::write(&temporary, text).map_err(|error| io_error(&temporary, error))?;
        fs::rename(&temporary, &target).map_err(|error| io_error(&target, error))
    }

    /// Records how the file `rel` that the walk of the directory found
    /// differs from `text`, if it does. A path the walk did not find as a
    /// file is missing when nothing stands at it, and different when
    /// something does: a symbolic link, a directory, or a file the walk does
    /// not reach.
    fn compare_file(&self, compared: &Compared, rel: &str, text: &str) -> Result<(), WriteError> {
        let listed = |files: &[String]| {
            files
                .binary_search_by(|file| file.as_str().cmp(rel))
                .is_ok()
        };
        let target = self.out.join(rel);
        let difference = if listed(&compared.found.modules) || listed(&compared.found.files) {
            // One byte more than `text` is enough to know that a larger file
            // differs.
            let mut bytes = Vec::new();
            fs::File::open(&target)
                .and_then(|file| file.take(text.len() as u64 + 1).read_to_end(&mut bytes))
                .map_err(|error| io_error(&target, error))?;
            (bytes != text.as_bytes()).then_some(Difference::Different)
        } else {
            match fs::symlink_metadata(&target) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                    ) =>
                {
                    Some(Difference::Missing)
                }
                Err(error) => return Err(io_error(&target, error)),
                Ok(_) => Some(Difference::Different),
            }
        };
        if let Some(difference) = difference {
            compared
                .differences
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(rel.to_owned(), difference);
        }
        Ok(())
    }
}

impl Drop for GeneratedWriter {
    /// Removes the scratch directory, which `finish` removes when it ends
    /// the run, so that a run that stops with an error leaves no temporary
    /// file behind. A run that is killed leaves it, and the next `create`
    /// clears it. A comparing writer has none, and removes nothing.
    fn drop(&mut self) {
        if self.compared.is_none() {
            let _ = fs::remove_dir_all(self.out.join(SCRATCH));
        }
    }
}

/// `Err` when `manifest` is not that of a generated workspace: of kind
/// `generated`, with a release and with no dependencies.
fn refuse_manifest(manifest: &Manifest) -> Result<(), WriteError> {
    if manifest.kind != WorkspaceKind::Generated {
        return Err(WriteError::Manifest(
            "the manifest of a generated workspace has the kind `generated`".to_owned(),
        ));
    }
    if manifest.release.is_none() {
        return Err(WriteError::Manifest(
            "the manifest of a generated workspace has a `release`".to_owned(),
        ));
    }
    if !manifest.dependencies.is_empty() {
        return Err(WriteError::Manifest(
            "the manifest of a generated workspace has no dependencies".to_owned(),
        ));
    }
    Ok(())
}

/// `Err` when the directory `out` holds anything but a generated workspace of
/// the prefix of `manifest`, and what a run keeps beside it. Entries whose
/// name begins with a dot are not the workspace's and are not looked at, so
/// a directory that holds nothing else counts as empty. Every `.fr` file is
/// examined before anything is written: one that is not an earlier output
/// of a writer of the prefix ([`not_written_here`]) is refused, each such
/// file named. `Ok` with the path of every module file found, the earlier
/// outputs, which are all that [`GeneratedWriter::finish`] may remove.
fn refuse_foreign(out: &Path, manifest: &Manifest) -> Result<Vec<String>, WriteError> {
    let mut names = Vec::new();
    for entry in fs::read_dir(out).map_err(|error| io_error(out, error))? {
        let entry = entry.map_err(|error| io_error(out, error))?;
        let name = entry.file_name();
        if !is_dot_named(&name) {
            names.push(name.to_string_lossy().into_owned());
        }
    }
    if names.is_empty() {
        return Ok(Vec::new());
    }
    names.sort();
    // The manifest is read as the check reads it, only from a regular file
    // of a bounded size.
    let path = out.join(MANIFEST_FILE);
    let bytes = match read_regular(&path, JSON_LIMIT) {
        Ok(bytes) => bytes,
        Err(Unread::Missing) => {
            return Err(foreign(
                out,
                format!("`{}` and no `{MANIFEST_FILE}`", names[0]),
            ));
        }
        Err(Unread::Io(error)) => return Err(io_error(&path, error)),
        Err(unread) => {
            return Err(foreign(
                out,
                format!("a `{MANIFEST_FILE}` that {}", unread.clause()),
            ));
        }
    };
    let existing = String::from_utf8(bytes)
        .map_err(|error| error.to_string())
        .and_then(|text| Manifest::from_json(&text))
        .map_err(|error| {
            foreign(
                out,
                format!("a `{MANIFEST_FILE}` that is no manifest: {error}"),
            )
        })?;
    if existing.kind != WorkspaceKind::Generated {
        return Err(foreign(out, "a curated workspace".to_owned()));
    }
    if existing.prefix != manifest.prefix {
        return Err(foreign(
            out,
            format!("a generated workspace of the prefix `{}`", existing.prefix),
        ));
    }
    let found = walk(out);
    if let Some((rel, why)) = found.unreadable.first() {
        return Err(WriteError::Io {
            path: out.join(rel),
            error: io::Error::other(why.clone()),
        });
    }
    match strays(&found).next() {
        Some((rel, Stray::Special)) => {
            return Err(foreign(
                out,
                format!("`{rel}`, which is neither a file nor a directory"),
            ));
        }
        Some((rel, Stray::File)) => return Err(foreign(out, format!("`{rel}`"))),
        None => {}
    }
    let mut refused = Vec::new();
    for rel in &found.modules {
        let path = out.join(rel);
        let head = head_of(&path).map_err(|error| io_error(&path, error))?;
        if let Some(why) = not_written_here(rel, &head, &manifest.prefix) {
            refused.push(format!("`{rel}`, {why}"));
        }
    }
    if refused.is_empty() {
        return Ok(found.modules);
    }
    Err(foreign(out, listed(&refused)))
}

/// `items` as one list: `a`, `a and b`, or `a, b and c`.
fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    }
}

/// Why the module file `rel`, whose head is `head` ([`head_of`]), is not an
/// earlier output of a writer of the prefix `prefix`, as the end of a
/// sentence about it; `None` when it is one: a module of edition 2 whose
/// address lies below the prefix and whose path is that of its address, as
/// [`GeneratedWriter::write`] writes it. Only the head of the file is read:
/// a file that one run wrote and a person then changed below its head is an
/// earlier output, which a run writes again or removes.
fn not_written_here(rel: &str, head: &Head, prefix: &Address) -> Option<String> {
    let declared = match head {
        Head::Address(declared) => declared,
        Head::NoModule => return Some("which is no module of edition 2".to_owned()),
        Head::TooFar => {
            return Some(format!(
                "in whose first {HEAD_LIMIT} bytes no module of edition 2 begins"
            ));
        }
    };
    let address = match Address::parse(declared) {
        Ok(address) => address,
        Err(_) => return Some(format!("whose module address `{declared}` is no address")),
    };
    if address == *prefix || !address.is_within(prefix) {
        return Some(format!(
            "whose module `{address}` is not below the prefix `{prefix}`"
        ));
    }
    let path = module_path(&address);
    (path != rel).then(|| format!("whose module `{address}` has the path `{path}`"))
}

/// What the head of a `.fr` file declares ([`head_of`]).
#[derive(Debug)]
enum Head {
    /// The head of a module of edition 2, with the value of the string after
    /// `module`.
    Address(String),
    /// A head that is not that of a module of edition 2, the head of every
    /// file of the first edition among them.
    NoModule,
    /// A file whose first [`HEAD_LIMIT`] bytes hold no whole head, as no
    /// writer writes one.
    TooFar,
}

/// The first bytes of a module file that [`head_of`] reads: the head of
/// every module a writer writes lies well inside them.
const HEAD_BYTES: usize = 512;

/// The most bytes of a module file that [`head_of`] reads to find its head.
/// A writer writes no white space or comment before the head, and a file
/// whose head lies further is no earlier output of one.
const HEAD_LIMIT: usize = 64 * 1024;

/// The first significant tokens of a module of edition 2: the marker, the
/// word `module`, the module's address and the brace that opens its body.
const HEAD: [(TokenKind, Option<&str>); 5] = [
    (TokenKind::Ident, Some("edition")),
    (TokenKind::Int, Some("2")),
    (TokenKind::Ident, Some("module")),
    (TokenKind::String, None),
    (TokenKind::LBrace, None),
];

/// The head of the module file at `path`: the value of the string after
/// `module` when its first significant tokens are [`HEAD`], as the grammar
/// of edition 2 begins, and what it is otherwise. White space, line ends
/// and comments are skipped, as the parser skips them.
///
/// Only the head is read: the first [`HEAD_BYTES`] bytes, and twice as many
/// each time those do not hold the head, up to [`HEAD_LIMIT`]. A token of a
/// window is trusted when the window's text goes on after it, so that the
/// lexer saw where it ends; when it is the brace, which is one byte and so
/// whole wherever the window ends; and when the window cannot grow because
/// the file ends in it or holds bytes there that are not UTF-8. The end of
/// the text, which the lexer gives as a token, is none. A head that lies in
/// the first [`HEAD_LIMIT`] bytes is therefore found whatever the white
/// space and the comments before it, as the edition check of the command
/// line finds the marker.
fn head_of(path: &Path) -> io::Result<Head> {
    let mut file = fs::File::open(path)?;
    let mut head = Vec::new();
    let mut window = HEAD_BYTES;
    loop {
        let wanted = (window - head.len()) as u64;
        (&mut file).take(wanted).read_to_end(&mut head)?;
        let (text, more) = match std::str::from_utf8(&head) {
            Ok(text) => (text, head.len() == window),
            Err(error) => (
                std::str::from_utf8(&head[..error.valid_up_to()]).unwrap_or_default(),
                head.len() == window && error.error_len().is_none(),
            ),
        };
        let grows = more && window < HEAD_LIMIT;
        let tokens: Vec<_> = lex(text)
            .into_iter()
            .filter(|token| {
                !matches!(
                    token.kind,
                    TokenKind::Whitespace
                        | TokenKind::Newline
                        | TokenKind::Comment
                        | TokenKind::DocComment
                        | TokenKind::Eof
                )
            })
            .take(HEAD.len())
            .collect();
        // Only the last of them can reach the end of the text, where the
        // window may have cut it.
        let cut = more
            && tokens.last().is_some_and(|last| {
                last.end as usize == text.len() && last.kind != TokenKind::LBrace
            });
        let trusted = tokens.len() - usize::from(cut);
        let token_text = |at: usize| {
            text.get(tokens[at].start as usize..tokens[at].end as usize)
                .unwrap_or_default()
        };
        let differs = (0..trusted.min(HEAD.len())).any(|at| {
            let (kind, word) = HEAD[at];
            tokens[at].kind != kind || word.is_some_and(|word| token_text(at) != word)
        });
        if differs || (trusted < HEAD.len() && !more) {
            return Ok(Head::NoModule);
        }
        if trusted >= HEAD.len() {
            return Ok(match decode_string_literal(token_text(3)) {
                Ok(address) => Head::Address(address),
                Err(_) => Head::NoModule,
            });
        }
        if !grows {
            return Ok(Head::TooFar);
        }
        window *= 2;
    }
}

/// The files a run writes at the top of a generated workspace besides its
/// modules: its manifest, its lock, and its ledger, `coverage.json`.
const KEPT: [&str; 3] = [MANIFEST_FILE, LOCK_FILE, COVERAGE_FILE];

/// What kind of entry a stray entry is.
enum Stray {
    /// Neither a file nor a directory: a symbolic link, for one.
    Special,
    /// A file that is neither a module nor one of [`KEPT`] at the top.
    File,
}

/// The entries of `found` that are not a generated workspace's: first every
/// entry that is neither a file nor a directory, then every file that is
/// neither a module nor one of [`KEPT`] at the top, each list in the byte
/// order of its paths. Entries whose name begins with a dot are in no walk.
/// `create` refuses a directory that holds one, and a comparing writer
/// reports each as extra, so that the two cannot disagree.
fn strays(found: &Walk) -> impl Iterator<Item = (&str, Stray)> + '_ {
    let special = found
        .special
        .iter()
        .map(|rel| (rel.as_str(), Stray::Special));
    let files = found
        .files
        .iter()
        .filter(|rel| !KEPT.contains(&rel.as_str()))
        .map(|rel| (rel.as_str(), Stray::File));
    special.chain(files)
}

fn foreign(out: &Path, found: String) -> WriteError {
    WriteError::Foreign {
        out: out.to_owned(),
        found,
    }
}

fn io_error(path: &Path, error: io::Error) -> WriteError {
    WriteError::Io {
        path: path.to_owned(),
        error,
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(why) | Self::Module(why) => f.write_str(why),
            Self::Foreign { out, found } => write!(
                f,
                "{} holds {found}; a generated workspace is written only into a directory that \
                 does not exist, an empty one, or one that holds a generated workspace of the \
                 same prefix",
                out.display()
            ),
            Self::Twice { path } => write!(
                f,
                "the run has already written a module to `{path}`, and no module replaces another"
            ),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Differs { out, differences } => {
                write!(f, "{} does not hold what the run writes:", out.display())?;
                for (index, (path, difference)) in differences.iter().enumerate() {
                    let separator = if index == 0 { "" } else { "," };
                    write!(f, "{separator} {} {path}", difference.word())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    //! The writer that compares. Every model here is invented, in the
    //! invented corpus `/zz/demo`.

    use super::*;
    use fidryn_provision::{
        Address, Block, ContainerModule, Item, PartRole, Provision, Run, Text, UnitModule,
    };
    use fidryn_syntax::edition::{Edition, edition_of};

    static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

    /// A directory of its own under the temporary directory, removed when
    /// dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let n = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("fidryn-writer-{}-{n}-{tag}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create the scratch directory");
            Scratch(path)
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

    fn address(text: &str) -> Address {
        Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    fn manifest(toolchain: &str) -> Manifest {
        Manifest {
            name: "demo".to_owned(),
            kind: WorkspaceKind::Generated,
            prefix: address("/zz/demo"),
            release: Some("42-7".to_owned()),
            toolchain: toolchain.to_owned(),
            dependencies: Vec::new(),
        }
    }

    /// A section of the title `/zz/demo/t1` that holds one paragraph.
    fn section(number: &str, words: &str) -> Module {
        let at = address(&format!("/zz/demo/t1/s{number}"));
        Module::Unit(UnitModule {
            address: at.clone(),
            aliases: Vec::new(),
            within: Some(address("/zz/demo/t1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            root: Provision {
                address: at,
                kind: "section".to_owned(),
                num: None,
                heading: None,
                status: None,
                items: vec![Item::Part {
                    role: PartRole::Content,
                    blocks: vec![Block::Para(Text::from_runs(vec![Run::Words(
                        words.to_owned(),
                    )]))],
                }],
            },
            credit: None,
            notes: Vec::new(),
            footnotes: Vec::new(),
            definitions: Vec::new(),
        })
    }

    /// The title `/zz/demo/t1` and its three sections.
    fn models() -> Vec<Module> {
        vec![
            Module::Container(ContainerModule {
                address: address("/zz/demo/t1"),
                aliases: Vec::new(),
                within: None,
                source: "demo01.xml".to_owned(),
                positive_law: Some(true),
                kind: "title".to_owned(),
                num: None,
                heading: None,
                status: None,
                members: ["s1", "s2", "s3"]
                    .iter()
                    .map(|segment| address(&format!("/zz/demo/t1/{segment}")))
                    .collect(),
                credit: None,
                notes: Vec::new(),
                footnotes: Vec::new(),
            }),
            section("1", "An invented widget is a small device."),
            section("2", "An invented widget is small or large."),
            section("3", "An invented widget may be of any color."),
        ]
    }

    /// Gives every model to `writer`, in the order given, and finishes the
    /// run.
    fn run(writer: GeneratedWriter, models: &[Module]) -> Result<Lock, WriteError> {
        for model in models {
            writer.write(model)?;
        }
        writer.finish(
            Generator {
                name: "demo-writer".to_owned(),
                version: "0.0.0".to_owned(),
            },
            Vec::new(),
            AddressCounts::default(),
            NotImported::default(),
        )
    }

    /// What `out` holds: every entry below it, a directory with `None`, a
    /// file with its bytes, and a symbolic link with its target, entries
    /// whose name begins with a dot included.
    fn tree(out: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
        fn walk_into(dir: &Path, prefix: &str, entries: &mut BTreeMap<String, Option<Vec<u8>>>) {
            for entry in fs::read_dir(dir).expect("list a directory") {
                let entry = entry.expect("an entry");
                let name = entry.file_name().into_string().expect("UTF-8");
                let rel = format!("{prefix}{name}");
                let kind = entry.file_type().expect("the kind of an entry");
                if kind.is_dir() {
                    entries.insert(rel.clone(), None);
                    walk_into(&entry.path(), &format!("{rel}/"), entries);
                } else if kind.is_symlink() {
                    let target = fs::read_link(entry.path()).expect("a link");
                    entries.insert(rel, Some(target.to_string_lossy().as_bytes().to_vec()));
                } else {
                    entries.insert(rel, Some(fs::read(entry.path()).expect("read a file")));
                }
            }
        }
        let mut entries = BTreeMap::new();
        walk_into(out, "", &mut entries);
        entries
    }

    fn differences(result: Result<Lock, WriteError>) -> Vec<(String, Difference)> {
        match result {
            Err(WriteError::Differs { differences, .. }) => differences,
            Err(error) => panic!("an error that is no difference: {error}"),
            Ok(_) => panic!("no difference was found"),
        }
    }

    fn compare(out: &Path, manifest: Manifest) -> GeneratedWriter {
        GeneratedWriter::compare(out, manifest)
            .unwrap_or_else(|error| panic!("compare {}: {error}", out.display()))
    }

    fn create(out: &Path) -> GeneratedWriter {
        GeneratedWriter::create(out, manifest("0.0.0"))
            .unwrap_or_else(|error| panic!("create {}: {error}", out.display()))
    }

    #[test]
    fn a_comparing_writer_finds_no_difference_in_what_a_run_wrote() {
        let scratch = Scratch::new("same");
        let out = scratch.join("out");
        let written = run(create(&out), &models()).expect("the run is written");
        let before = tree(&out);

        let mut reversed = models();
        reversed.reverse();
        let compared = run(compare(&out, manifest("0.0.0")), &reversed)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(compared, written);
        assert_eq!(tree(&out), before);

        // A module given twice is refused as by a writer that writes.
        let writer = compare(&out, manifest("0.0.0"));
        writer.write(&models()[1]).expect("the first time");
        assert!(matches!(
            writer.write(&models()[1]),
            Err(WriteError::Twice { path }) if path == "zz/demo/t1/s1.fr"
        ));
        drop(writer);
        assert_eq!(tree(&out), before);
    }

    #[test]
    fn a_comparing_writer_reports_what_is_missing_extra_or_different_and_writes_nothing() {
        let scratch = Scratch::new("differs");
        let out = scratch.join("out");
        run(create(&out), &models()).expect("the run is written");
        let s1 = out.join("zz/demo/t1/s1.fr");
        let changed = fs::read_to_string(&s1)
            .expect("read")
            .replace("small device", "small devise");
        fs::write(&s1, changed).expect("change a module");
        fs::remove_file(out.join("zz/demo/t1/s2.fr")).expect("remove a module");
        fs::copy(out.join("zz/demo/t1/s3.fr"), out.join("zz/demo/t1/s9.fr")).expect("add a module");
        fs::remove_file(out.join(LOCK_FILE)).expect("remove the lock");
        // Every entry a run would refuse is extra: a file that is neither a
        // module nor one a run keeps at the top, and a symbolic link.
        fs::write(out.join("notes.txt"), "a note\n").expect("write");
        fs::write(out.join("zz/demo/coverage.json"), "{}\n").expect("write");
        let mut expected = vec![
            (LOCK_FILE, Difference::Missing),
            ("notes.txt", Difference::Extra),
            (MANIFEST_FILE, Difference::Different),
            ("zz/demo/coverage.json", Difference::Extra),
            ("zz/demo/t1/s1.fr", Difference::Different),
            ("zz/demo/t1/s2.fr", Difference::Missing),
            ("zz/demo/t1/s9.fr", Difference::Extra),
        ];
        #[cfg(unix)]
        {
            // A link where the run writes a module is different, and one
            // where it writes nothing is extra.
            fs::remove_file(out.join("zz/demo/t1/s3.fr")).expect("remove a module");
            std::os::unix::fs::symlink("s1.fr", out.join("zz/demo/t1/s3.fr")).expect("link");
            std::os::unix::fs::symlink("t1", out.join("zz/demo/link")).expect("link");
            expected.push(("zz/demo/link", Difference::Extra));
            expected.push(("zz/demo/t1/s3.fr", Difference::Different));
            expected.sort_by(|a, b| a.0.cmp(b.0));
        }
        // The ledger a run writes is compared like any other file. Entries
        // whose name begins with a dot are not looked at, and a comparing
        // writer does not touch the directory of a writer's temporary files.
        fs::write(out.join(COVERAGE_FILE), "{}\n").expect("write");
        expected.insert(0, (COVERAGE_FILE, Difference::Different));
        fs::write(out.join(".notes.fr"), "not a module\n").expect("write");
        fs::create_dir_all(out.join(SCRATCH)).expect("create");
        fs::write(out.join(SCRATCH).join("0.tmp"), "pending\n").expect("write");
        let before = tree(&out);

        let found = differences(run(compare(&out, manifest("0.0.1")), &models()));
        let expected: Vec<(String, Difference)> = expected
            .into_iter()
            .map(|(path, difference)| (path.to_owned(), difference))
            .collect();
        assert_eq!(found, expected);
        assert_eq!(tree(&out), before);
        // A writer that writes refuses the directory, and changes nothing.
        assert!(matches!(
            GeneratedWriter::create(&out, manifest("0.0.1")),
            Err(WriteError::Foreign { .. })
        ));
        assert_eq!(tree(&out), before);

        // The lock and the ledger a run writes are compared like any other
        // file: a run of fewer modules writes a lock and a ledger of other
        // bytes.
        let other = scratch.join("other");
        run(create(&other), &models()).expect("the run is written");
        let error = run(compare(&other, manifest("0.0.0")), &models()[..3])
            .expect_err("the third section is not given");
        assert_eq!(
            error.to_string(),
            format!(
                "{} does not hold what the run writes: different coverage.json, different \
                 law.lock.json, extra zz/demo/t1/s3.fr",
                other.display()
            )
        );
        // A ledger that is missing, as after a run stopped between the lock
        // and the ledger, is missing.
        fs::remove_file(other.join(COVERAGE_FILE)).expect("remove the ledger");
        assert_eq!(
            differences(run(compare(&other, manifest("0.0.0")), &models())),
            [(COVERAGE_FILE.to_owned(), Difference::Missing)]
        );
    }

    /// A comment of `length` bytes, its line ends included: lines of
    /// invented words, one of them not ASCII, after a first line as long as
    /// it needs to be.
    fn comment(length: usize) -> String {
        const LINE: &str = "// An invented \u{2014} note.\n";
        let lines = (length - 3) / LINE.len();
        let first = length - lines * LINE.len();
        format!("//{}\n{}", "-".repeat(first - 3), LINE.repeat(lines))
    }

    #[test]
    fn a_module_that_begins_with_a_comment_of_any_length_is_recognized() {
        // The reader's windows end at `HEAD_BYTES` bytes and at every double
        // of that up to `HEAD_LIMIT`. For each, a comment puts the head of a
        // module a little before the window's end, across it at each of its
        // bytes, and a little after it; the windows before it end at each
        // byte of a line of the comment, the two slashes and the bytes of a
        // character that is not ASCII among them.
        let scratch = Scratch::new("comments");
        let rel = "zz/demo/t1/s3.fr";
        let path = scratch.join("s3.fr");
        let module = render(&section("3", "An invented widget may be of any color."));
        let head = module.find('{').expect("the brace of the head") + 1;
        let prefix = address("/zz/demo");
        let mut window = HEAD_BYTES;
        while window <= HEAD_LIMIT {
            for length in window - head - 2..=window + 2 {
                let text = format!("{}{module}", comment(length));
                assert_eq!(text.find("edition"), Some(length));
                fs::write(&path, &text).expect("write a module");
                // The edition check of the command line reads these bytes as
                // a module of edition 2.
                assert_eq!(edition_of(&text), Ok(Edition::Two), "{length}");
                let found = head_of(&path).expect("read the head");
                if length + head <= HEAD_LIMIT {
                    assert!(
                        matches!(&found, Head::Address(at) if at == "/zz/demo/t1/s3"),
                        "a comment of {length} bytes: {found:?}"
                    );
                    assert_eq!(not_written_here(rel, &found, &prefix), None);
                } else {
                    // The head lies past the first `HEAD_LIMIT` bytes.
                    assert!(
                        matches!(found, Head::TooFar),
                        "a comment of {length} bytes: {found:?}"
                    );
                }
            }
            window *= 2;
        }
    }

    #[test]
    fn a_comparing_writer_of_a_directory_that_does_not_exist_finds_every_file_missing() {
        let scratch = Scratch::new("absent");
        let out = scratch.join("absent");
        let found = differences(run(compare(&out, manifest("0.0.0")), &models()));
        let mut expected = vec![
            COVERAGE_FILE.to_owned(),
            LOCK_FILE.to_owned(),
            MANIFEST_FILE.to_owned(),
        ];
        expected.extend(["t1", "t1/s1", "t1/s2", "t1/s3"].map(|rel| format!("zz/demo/{rel}.fr")));
        expected.sort();
        assert_eq!(
            found,
            expected
                .into_iter()
                .map(|path| (path, Difference::Missing))
                .collect::<Vec<_>>()
        );
        assert!(!out.exists(), "nothing is made");

        // A file is no directory, and the manifest of a curated workspace is
        // refused as by `create`.
        let file = scratch.join("file");
        fs::write(&file, "a file\n").expect("write");
        assert!(matches!(
            GeneratedWriter::compare(&file, manifest("0.0.0")),
            Err(WriteError::Io { .. })
        ));
        let mut curated = manifest("0.0.0");
        curated.kind = WorkspaceKind::Curated;
        curated.release = None;
        assert!(matches!(
            GeneratedWriter::compare(&out, curated),
            Err(WriteError::Manifest(_))
        ));
    }
}
