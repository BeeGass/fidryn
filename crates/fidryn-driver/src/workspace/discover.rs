//! Discovery: what a workspace's directory holds.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

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
