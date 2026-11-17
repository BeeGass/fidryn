//! The writer of a generated workspace.

use super::discover::walk;
use super::{LOCK_FILE, MANIFEST_FILE};
use fidryn_provision::layout::module_path;
use fidryn_provision::render::render;
use fidryn_provision::workspace::{
    AddressCounts, Artifact, Generator, Lock, Manifest, NotImported, WorkspaceKind,
};
use fidryn_provision::{Hash32, Module};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// The directory, at the top of the workspace, that holds the files of a run
/// while they are written. Its name begins with a dot, so no check reads it.
const SCRATCH: &str = ".fidryn-write";

/// The file at the top of a generated workspace that the writer keeps
/// besides its own two.
const COVERAGE_FILE: &str = "coverage.json";

/// Writes a generated workspace: the modules of one run, then its manifest,
/// then its lock.
///
/// [`GeneratedWriter::write`] may be called from several threads at once.
/// What the run writes depends only on the modules, the manifest and what
/// [`GeneratedWriter::finish`] is given, not on the order of the calls or the
/// number of threads.
pub struct GeneratedWriter {
    out: PathBuf,
    manifest: Manifest,
    /// The BLAKE3 of the bytes of every module the run has written, by its
    /// path.
    written: Mutex<BTreeMap<String, Hash32>>,
    /// The number of the next temporary file.
    next: AtomicU64,
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
}

impl GeneratedWriter {
    /// A writer of the generated workspace `manifest` into the directory
    /// `out`.
    ///
    /// `out` is a directory that does not exist, which is created, an empty
    /// one, or one whose `workspace.json` is the manifest of a generated
    /// workspace of the same prefix. Such a directory may hold, besides
    /// `.fr` files and the manifest and the lock, a file `coverage.json` at
    /// its top and any directory whose name begins with a dot, which the run
    /// leaves as they are. Any other directory is refused, and nothing in it
    /// is changed. `manifest` is of kind `generated`, has a release and has no
    /// dependencies.
    pub fn create(out: &Path, manifest: Manifest) -> Result<GeneratedWriter, WriteError> {
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
        match fs::metadata(out) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(out).map_err(|error| io_error(out, error))?;
            }
            Err(error) => return Err(io_error(out, error)),
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io_error(out, io::ErrorKind::NotADirectory.into()));
            }
            Ok(_) => refuse_foreign(out, &manifest)?,
        }
        let scratch = out.join(SCRATCH);
        match fs::remove_dir_all(&scratch) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(io_error(&scratch, error));
            }
            _ => {}
        }
        fs::create_dir(&scratch).map_err(|error| io_error(&scratch, error))?;
        Ok(GeneratedWriter {
            out: out.to_owned(),
            manifest,
            written: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(0),
        })
    }

    /// Renders `module`, a unit or a container whose address is below the
    /// prefix, and writes it to its path through a temporary name. Fails when
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
        })
    }

    /// Ends the run: removes every `.fr` file the run did not write, and each
    /// directory that leaves empty, then writes `workspace.json`, then
    /// `law.lock.json`, the lock of `generator`, `artifacts`, `addresses`,
    /// `not_imported` and every module the run wrote, which it returns.
    pub fn finish(
        self,
        generator: Generator,
        artifacts: Vec<Artifact>,
        addresses: AddressCounts,
        not_imported: NotImported,
    ) -> Result<Lock, WriteError> {
        let modules =
            std::mem::take(&mut *self.written.lock().unwrap_or_else(PoisonError::into_inner));
        let found = walk(&self.out);
        if let Some((rel, why)) = found.unreadable.first() {
            return Err(WriteError::Io {
                path: self.out.join(rel),
                error: io::Error::other(why.clone()),
            });
        }
        for rel in found
            .modules
            .iter()
            .filter(|rel| !modules.contains_key(*rel))
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
        let lock = Lock {
            prefix: self.manifest.prefix.clone(),
            release: self
                .manifest
                .release
                .clone()
                .expect("`create` takes only a manifest with a release"),
            generator,
            artifacts,
            addresses,
            not_imported,
            modules,
        };
        self.put(MANIFEST_FILE, &self.manifest.to_json())?;
        self.put(LOCK_FILE, &lock.to_json())?;
        let scratch = self.out.join(SCRATCH);
        fs::remove_dir_all(&scratch).map_err(|error| io_error(&scratch, error))?;
        Ok(lock)
    }

    /// Writes `text` to the file `rel` below the workspace's directory: to a
    /// temporary file in the scratch directory first, which is then renamed.
    fn put(&self, rel: &str, text: &str) -> Result<(), WriteError> {
        let target = self.out.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        let number = self.next.fetch_add(1, Ordering::Relaxed);
        let temporary = self.out.join(SCRATCH).join(format!("{number}.tmp"));
        fs::write(&temporary, text).map_err(|error| io_error(&temporary, error))?;
        fs::rename(&temporary, &target).map_err(|error| io_error(&target, error))
    }
}

impl Drop for GeneratedWriter {
    /// Removes the scratch directory, which `finish` removes when it ends
    /// the run, so that a run that stops before then leaves no temporary
    /// file behind.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.out.join(SCRATCH));
    }
}

/// `Err` when the directory `out` holds anything but a generated workspace of
/// the prefix of `manifest`, and what a run keeps beside it.
fn refuse_foreign(out: &Path, manifest: &Manifest) -> Result<(), WriteError> {
    let mut names = Vec::new();
    for entry in fs::read_dir(out).map_err(|error| io_error(out, error))? {
        let entry = entry.map_err(|error| io_error(out, error))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    if names.is_empty() {
        return Ok(());
    }
    names.sort();
    let path = out.join(MANIFEST_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(foreign(
                out,
                format!("`{}` and no `{MANIFEST_FILE}`", names[0]),
            ));
        }
        Err(error) => return Err(io_error(&path, error)),
    };
    let existing = Manifest::from_json(&text).map_err(|error| {
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
    if let Some(rel) = found.special.first() {
        return Err(foreign(
            out,
            format!("`{rel}`, which is neither a file nor a directory"),
        ));
    }
    let kept = [MANIFEST_FILE, LOCK_FILE, COVERAGE_FILE];
    match found.files.iter().find(|rel| !kept.contains(&rel.as_str())) {
        Some(rel) => Err(foreign(out, format!("`{rel}`"))),
        None => Ok(()),
    }
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
