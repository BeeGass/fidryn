//! Checks the pinned sample of the Code under `samples/uscode/`: every archive
//! in `archives/` has the size, the SHA-256 and the BLAKE3 that the retrieval
//! record beside it gives, the record names every archive and no other, and
//! `profile.json` is, byte for byte, what `conformance/usc_profile.py` prints
//! for the archives today.

use crate::workspace::workspace_root;
use anyhow::{Context, Result, anyhow, bail};
use fidryn_provision::Hash32;
use fidryn_provision::workspace::RetrievalRecord;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// The directory of the archives and their retrieval record, which holds
/// nothing else.
pub const ARCHIVES: &str = "samples/uscode/archives";
/// The retrieval record, beside the archives.
pub const RECORD: &str = "retrieval.json";
/// The independent profile of the archives.
pub const PROFILE: &str = "samples/uscode/profile.json";
/// The script that prints the profile.
pub const PROFILE_SCRIPT: &str = "conformance/usc_profile.py";

pub fn run() -> Result<()> {
    check(&workspace_root())
}

/// Fails, naming each archive, when the archives under `root` differ from
/// their record, and fails when the committed profile is not the one the
/// script prints.
pub fn check(root: &Path) -> Result<()> {
    let problems = archive_problems(&root.join(ARCHIVES))?;
    if !problems.is_empty() {
        bail!(
            "{ARCHIVES} differs from its record:\n{}",
            problems.join("\n")
        );
    }
    let printed = printed_profile(root)?;
    let committed = fs::read(root.join(PROFILE)).with_context(|| format!("read {PROFILE}"))?;
    if let Some(problem) = profile_problem(&committed, &printed) {
        bail!("{problem}");
    }
    eprintln!("{ARCHIVES} matches {RECORD}, and {PROFILE} is current");
    Ok(())
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

/// `uv run --locked python -I conformance/usc_profile.py samples/uscode/archives`,
/// from `root`: the downloaded archives are read by isolated Python.
fn profile_command(root: &Path) -> Command {
    let mut cmd = Command::new("uv");
    cmd.current_dir(root);
    cmd.args(["run", "--locked", "python", "-I", PROFILE_SCRIPT, ARCHIVES]);
    cmd
}

/// The profile the script prints for the archives.
fn printed_profile(root: &Path) -> Result<Vec<u8>> {
    let mut command = profile_command(root);
    let shown: Vec<String> = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    eprintln!("+ {}", shown.join(" "));
    let output = command
        .stderr(Stdio::inherit())
        .output()
        .context("failed to spawn uv")?;
    if !output.status.success() {
        bail!("{PROFILE_SCRIPT} failed on {ARCHIVES}: {}", output.status);
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
    use std::ffi::OsStr;
    use std::path::PathBuf;

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
        let cmd = profile_command(root);
        assert_eq!(cmd.get_program(), "uv");
        let args: Vec<&OsStr> = cmd.get_args().collect();
        let want: Vec<&OsStr> = [
            "run",
            "--locked",
            "python",
            "-I",
            "conformance/usc_profile.py",
            "samples/uscode/archives",
        ]
        .iter()
        .map(OsStr::new)
        .collect();
        assert_eq!(args, want);
        assert_eq!(cmd.get_current_dir(), Some(root));
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
}
