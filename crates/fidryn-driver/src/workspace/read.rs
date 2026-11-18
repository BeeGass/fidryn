//! Reading the modules of a workspace into the model, for what prints their
//! text, and into their interfaces, for the coverage ledger of a generated
//! workspace.

use super::check::{lower_file, of_file, read_lock, read_manifest, read_module};
use super::discover::walk;
use super::index::order;
use super::report::ledger_of;
use fidryn_check::e2::Lowered;
use fidryn_core::{Diagnostic, DiagnosticCode};
use fidryn_provision::workspace::WorkspaceKind;
use fidryn_provision::{Coverage, Module, interface_of};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The modules of the workspace whose directory is `root`, lowered into the
/// model, in the byte order of their paths, read on `jobs` threads; the
/// result is the same for any number of them. A module is every `.fr` file
/// below `root`, but for an entry whose name begins with a dot and what lies
/// below such a directory.
///
/// Nothing is checked beyond what lowering needs: neither the lock, nor
/// where a module stands, nor the hashes. `Err` holds every diagnostic,
/// each naming its file by its path relative to `root`, sorted as the check
/// of a workspace sorts them, when the manifest cannot be read (`E730`, and
/// nothing else is read), when an entry cannot be read or is neither a file
/// nor a directory (`E730`), or when a module file is not a module of
/// edition 2 that lowers.
pub fn read_modules(root: &Path, jobs: usize) -> Result<Vec<Module>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    if read_manifest(root, &mut diagnostics).is_none() {
        return Err(diagnostics);
    }
    read_each(root, jobs, |lowered| lowered.module)
}

/// Why [`read_coverage`] gives no ledger.
#[derive(Debug)]
pub enum CoverageError {
    /// The workspace is curated: a coverage ledger is that of a generated
    /// workspace, which an importer writes with its lock.
    Curated,
    /// The workspace cannot be read: every diagnostic, each naming its file
    /// by its path relative to the workspace's directory.
    Unread(Vec<Diagnostic>),
}

/// The coverage ledger of the generated workspace whose directory is
/// `root`, counted from the interfaces of its modules as their files stand,
/// its manifest and its lock: what the writer of the workspace wrote as
/// `coverage.json`, when no file was changed since. The module files are
/// read on `jobs` threads, and the ledger is the same for any number of
/// them.
///
/// The modules are read as [`read_modules`] reads them, and nothing else is
/// checked, the module files against the lock among them, which the check
/// of the workspace does. A curated workspace has no ledger
/// ([`CoverageError::Curated`]). The diagnostics of [`read_modules`], those
/// of a generated workspace without a lock, with a lock that cannot be read
/// or one whose prefix or release is not its manifest's (`E730`), are
/// [`CoverageError::Unread`]; then no module is read.
pub fn read_coverage(root: &Path, jobs: usize) -> Result<Coverage, CoverageError> {
    let mut diagnostics = Vec::new();
    let Some(manifest) = read_manifest(root, &mut diagnostics) else {
        return Err(CoverageError::Unread(diagnostics));
    };
    if manifest.kind == WorkspaceKind::Curated {
        return Err(CoverageError::Curated);
    }
    let lock = read_lock(root, &manifest, &mut diagnostics);
    let lock = match lock {
        Some((lock, _)) if diagnostics.is_empty() => lock,
        _ => return Err(CoverageError::Unread(diagnostics)),
    };
    let interfaces = read_each(root, jobs, |lowered| {
        interface_of(&lowered.module, &lowered.decl_spans())
    })
    .map_err(CoverageError::Unread)?;
    Ok(ledger_of(&interfaces, &manifest, &lock))
}

/// What `each` gives of every module of the workspace whose directory is
/// `root`, whose manifest was read, lowered on `jobs` threads, in the byte
/// order of the paths of their files; or every diagnostic, as
/// [`read_modules`] gives them.
fn read_each<T: Send>(
    root: &Path,
    jobs: usize,
    each: impl Fn(Lowered) -> T + Sync,
) -> Result<Vec<T>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let found = walk(root);
    for (rel, why) in &found.unreadable {
        diagnostics.push(of_file(
            DiagnosticCode::E730,
            rel,
            format!("this entry cannot be read: {why}"),
        ));
    }
    for rel in &found.special {
        diagnostics.push(of_file(
            DiagnosticCode::E730,
            rel,
            "this entry is neither a file nor a directory; a workspace is read through no \
             symbolic link",
        ));
    }
    let next = AtomicUsize::new(0);
    let work = || {
        let mut done = Vec::new();
        loop {
            let at = next.fetch_add(1, Ordering::Relaxed);
            let Some(rel) = found.modules.get(at) else {
                break;
            };
            let lowered = match read_module(&root.join(rel)) {
                Ok((_, head)) => lower_file(head).map(&each),
                Err(error) => Err(vec![Diagnostic::new(
                    DiagnosticCode::E730,
                    format!("this file cannot be read: {error}"),
                )]),
            };
            done.push((at, lowered));
        }
        done
    };
    let workers = jobs.clamp(1, found.modules.len().max(1));
    let mut done = if workers == 1 {
        work()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers).map(|_| scope.spawn(work)).collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("a worker of the reading panicked"))
                .collect()
        })
    };
    done.sort_by_key(|(at, _)| *at);
    let mut modules = Vec::with_capacity(done.len());
    for ((_, lowered), rel) in done.into_iter().zip(&found.modules) {
        match lowered {
            Ok(read) => modules.push(read),
            Err(found) => diagnostics.extend(
                found
                    .into_iter()
                    .map(|diagnostic| diagnostic.with_source(rel.as_str())),
            ),
        }
    }
    if diagnostics.is_empty() {
        Ok(modules)
    } else {
        diagnostics.sort_by(|a, b| order(a).cmp(&order(b)));
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::read_modules;
    use fidryn_core::DiagnosticCode;
    use fidryn_provision::Module;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// A copy of the demo workspace in a directory of its own under the
    /// temporary directory, named by the process and by `tag`.
    fn demo_copy(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fidryn-workspace-read-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let demo =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/workspace/demo");
        copy(&demo, &dir);
        dir
    }

    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("create a directory");
        for entry in fs::read_dir(from).expect("list a directory") {
            let entry = entry.expect("an entry");
            if entry.file_name().as_encoded_bytes().first() == Some(&b'.') {
                continue;
            }
            let target = to.join(entry.file_name());
            if entry.file_type().expect("the kind of an entry").is_dir() {
                copy(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), &target).expect("copy a file");
            }
        }
    }

    fn addresses(modules: &[Module]) -> Vec<&str> {
        modules
            .iter()
            .map(|module| match module {
                Module::Unit(unit) => unit.address.as_str(),
                Module::Container(container) => container.address.as_str(),
                Module::Curated(curated) => curated.address.as_str(),
            })
            .collect()
    }

    #[test]
    fn the_modules_of_a_workspace_are_read_in_the_order_of_their_paths() {
        let dir = demo_copy("order");
        // An entry whose name begins with a dot is not the workspace's.
        fs::create_dir_all(dir.join(".cache/zz")).expect("create a directory");
        fs::write(dir.join(".cache/zz/x.fr"), "not a module\n").expect("write a file");
        let one = read_modules(&dir, 1).expect("the demo workspace is read");
        // `zz/demo/t1.fr` before `zz/demo/t1/ch1.fr`: a full stop is a byte
        // below a slash.
        assert_eq!(
            addresses(&one),
            [
                "/zz/demo/t1",
                "/zz/demo/t1/ch1",
                "/zz/demo/t1/s1",
                "/zz/demo/t1/s2",
                "/zz/demo/t1/s3",
            ]
        );
        assert_eq!(read_modules(&dir, 8).expect("read on eight workers"), one);
        fs::remove_dir_all(&dir).expect("remove the copy");
    }

    #[test]
    fn a_module_that_does_not_lower_is_named_with_its_diagnostics() {
        let dir = demo_copy("refused");
        fs::write(
            dir.join("zz/demo/t1/s9.fr"),
            "edition 2\nmodule \"/zz/demo/t1/s9\" {\n",
        )
        .expect("write a file");
        fs::write(
            dir.join("zz/demo/t1/s0.fr"),
            "module Old version \"1\" {\n}\n",
        )
        .expect("write a file");
        let found = read_modules(&dir, 4).expect_err("two files are no modules that lower");
        let files: Vec<Option<&str>> = found
            .iter()
            .map(|diagnostic| diagnostic.source_file.as_deref())
            .collect();
        assert!(files.contains(&Some("zz/demo/t1/s0.fr")), "{found:?}");
        assert!(files.contains(&Some("zz/demo/t1/s9.fr")), "{found:?}");
        assert!(
            files
                .iter()
                .all(|file| matches!(file, Some("zz/demo/t1/s0.fr" | "zz/demo/t1/s9.fr"))),
            "{found:?}"
        );
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
        fs::remove_file(dir.join("workspace.json")).expect("remove the manifest");
        let found = read_modules(&dir, 4).expect_err("no manifest");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, DiagnosticCode::E730);
        assert_eq!(found[0].source_file.as_deref(), Some("workspace.json"));
        fs::remove_dir_all(&dir).expect("remove the copy");
    }
}
