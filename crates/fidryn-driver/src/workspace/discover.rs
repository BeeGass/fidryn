//! Discovery: what a workspace's directory holds, and the reading of the
//! JSON files a command reads from a workspace and from its cache.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The most bytes that a command reads of a manifest, a lock or an entry of
/// the cache: 64 MiB, the bound on the source of a module, many times the
/// largest of these files a workspace of a whole corpus has needed. A larger
/// file is refused unread, so that no such file can exhaust memory.
pub(crate) const JSON_LIMIT: u64 = 64 * 1024 * 1024;

/// The refusal of an entry that is neither a file nor a directory, a
/// symbolic link among them, which the check of a workspace gives both for
/// what its walk finds and for a manifest or a lock in such an entry.
pub(crate) const NOT_FILE_OR_DIRECTORY: &str =
    "this entry is neither a file nor a directory; a workspace check follows no symbolic link";

/// Why [`read_regular`] read nothing.
#[derive(Debug)]
pub(crate) enum Unread {
    /// Nothing stands at the path.
    Missing,
    /// A directory stands there.
    Directory,
    /// An entry that is neither a file nor a directory stands there: a
    /// symbolic link, a named pipe, a device or a socket.
    Special,
    /// A file larger than [`JSON_LIMIT`] stands there.
    TooLarge,
    /// It could not be read.
    Io(io::Error),
}

impl Unread {
    /// Why nothing was read, as a sentence about the entry, `this ...`.
    pub(crate) fn sentence(&self) -> String {
        match self {
            Unread::Missing => "nothing stands at this path".to_owned(),
            Unread::Special => NOT_FILE_OR_DIRECTORY.to_owned(),
            Unread::Directory => {
                "this entry is a directory, and a workspace check reads it only from a regular \
                 file"
                    .to_owned()
            }
            Unread::TooLarge => format!(
                "this file is larger than {JSON_LIMIT} bytes (64 MiB), the most a workspace check \
                 reads of a manifest, a lock or an entry of the cache"
            ),
            Unread::Io(error) => format!("this file cannot be read: {error}"),
        }
    }

    /// Why nothing was read, as the end of a sentence about the entry, from
    /// its verb on.
    pub(crate) fn clause(&self) -> String {
        match self {
            Unread::Missing => "is missing".to_owned(),
            Unread::Special => "is neither a file nor a directory, and a workspace check follows \
                                no symbolic link"
                .to_owned(),
            Unread::Directory => "is a directory".to_owned(),
            Unread::TooLarge => format!(
                "is larger than {JSON_LIMIT} bytes (64 MiB), the most a workspace check reads of \
                 it"
            ),
            Unread::Io(error) => format!("cannot be read: {error}"),
        }
    }
}

/// The bytes of the regular file at `path`, of at most `limit` bytes. The
/// entry is looked at, without a link being followed, before it is opened:
/// a symbolic link, a named pipe, a device or a directory is not opened, and
/// a file larger than `limit` is not read. What is opened is read only when
/// it is a regular file, and never past `limit`, should the entry have been
/// replaced or grown in the meantime.
pub(crate) fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>, Unread> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(Unread::Missing),
        Err(error) => return Err(Unread::Io(error)),
    };
    let kind = metadata.file_type();
    if kind.is_dir() {
        return Err(Unread::Directory);
    }
    if !kind.is_file() {
        return Err(Unread::Special);
    }
    if metadata.len() > limit {
        return Err(Unread::TooLarge);
    }
    let file = fs::File::open(path).map_err(Unread::Io)?;
    if !file.metadata().map_err(Unread::Io)?.is_file() {
        return Err(Unread::Special);
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(Unread::Io)?;
    if bytes.len() as u64 > limit {
        return Err(Unread::TooLarge);
    }
    Ok(bytes)
}

/// What [`walk`] found below a directory, every path relative to it with `/`
/// between its segments, each list in the byte order of its paths.
#[derive(Debug, Default)]
pub(crate) struct Walk {
    /// Every regular file whose name ends in `.fr`.
    pub(crate) modules: Vec<String>,
    /// Every other regular file.
    pub(crate) files: Vec<String>,
    /// Every entry that is neither a regular file nor a directory, a
    /// symbolic link among them. The walk follows none of them.
    pub(crate) special: Vec<String>,
    /// Every entry that could not be read, with the reason: a directory that
    /// could not be listed, or a name that is not UTF-8.
    pub(crate) unreadable: Vec<(String, String)>,
}

/// Whether the entry named `name` is outside every workspace: a file, a
/// directory or a symbolic link whose name begins with a dot.
pub(crate) fn is_dot_named(name: &OsStr) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

/// Every entry below `root`, except those whose name begins with a dot,
/// which are not read and, when they are directories, not entered.
pub(crate) fn walk(root: &Path) -> Walk {
    let mut found = Walk::default();
    let mut pending: Vec<(PathBuf, String)> = vec![(root.to_owned(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let place = if prefix.is_empty() {
            ".".to_owned()
        } else {
            prefix.clone()
        };
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                found.unreadable.push((place, error.to_string()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    found.unreadable.push((place.clone(), error.to_string()));
                    continue;
                }
            };
            let name = entry.file_name();
            if is_dot_named(&name) {
                continue;
            }
            let Some(name) = name.to_str() else {
                found.unreadable.push((
                    format!("{prefix}{}", name.to_string_lossy()),
                    "the name is not UTF-8".to_owned(),
                ));
                continue;
            };
            let rel = format!("{prefix}{name}");
            match entry.file_type() {
                Err(error) => found.unreadable.push((rel, error.to_string())),
                Ok(kind) if kind.is_dir() => pending.push((entry.path(), format!("{rel}/"))),
                Ok(kind) if kind.is_file() => {
                    if name.ends_with(".fr") {
                        found.modules.push(rel);
                    } else {
                        found.files.push(rel);
                    }
                }
                Ok(_) => found.special.push(rel),
            }
        }
    }
    found.modules.sort();
    found.files.sort();
    found.special.sort();
    found.unreadable.sort();
    found
}
