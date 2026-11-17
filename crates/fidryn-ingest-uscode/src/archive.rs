//! The archives of a release: a directory that holds the publisher's zip
//! archives and nothing else but their retrieval record, `retrieval.json`.
//!
//! [`open`] checks every archive against its record (its size, its SHA-256
//! and its BLAKE3), refuses an archive the record does not list and one it
//! lists that is absent, and checks every member before reading it: its name
//! is `usc`, two digits, an optional `a` or `A`, and `.xml`; its declared
//! size is at most 512 MiB; and no two archives of the release hold a member
//! of the same name. Reading a member stops at the limit even when its header
//! declares less. Nothing is written beside the archives.

use crate::IngestError;
use fidryn_provision::Hash32;
use fidryn_provision::workspace::{Artifact, Member, RetrievalRecord, RetrievedArchive};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The largest member the reading accepts: 512 MiB.
pub(crate) const MEMBER_LIMIT: u64 = 512 * 1024 * 1024;

/// The retrieval record, beside the archives.
const RECORD: &str = "retrieval.json";

/// The verified archives of one release and their record.
#[derive(Debug)]
pub struct ReleaseArchives {
    directory: PathBuf,
    release: String,
    artifacts: Vec<Artifact>,
    /// Every member of the release, sorted by name.
    members: Vec<Located>,
    limit: u64,
}

/// A member and the archive that holds it.
#[derive(Debug)]
struct Located {
    name: String,
    /// The index of the archive in `artifacts`.
    archive: usize,
    bytes: u64,
    blake3: Hash32,
}

/// Opens the release in `archive_dir`: reads its retrieval record, checks
/// each archive against it, and checks and digests each member.
pub fn open(archive_dir: &Path) -> Result<ReleaseArchives, IngestError> {
    open_with_limit(archive_dir, MEMBER_LIMIT)
}

/// [`open`], with `limit` as the largest member.
pub(crate) fn open_with_limit(
    directory: &Path,
    limit: u64,
) -> Result<ReleaseArchives, IngestError> {
    let shown = directory.display();
    let text = match fs::read_to_string(directory.join(RECORD)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(IngestError::new(format!(
                "{shown} has no {RECORD}, the retrieval record of its archives"
            )));
        }
        Err(error) => {
            return Err(IngestError::new(format!(
                "{RECORD} in {shown} cannot be read: {error}"
            )));
        }
    };
    let record = RetrievalRecord::from_json(&text)
        .map_err(|error| IngestError::new(format!("{RECORD} in {shown}: {error}")))?;

    let mut listed = BTreeSet::new();
    for archived in &record.archives {
        if !is_plain_file_name(&archived.file) {
            return Err(IngestError::new(format!(
                "{RECORD} in {shown} lists {:?}, which is not the name of a file in the directory",
                archived.file
            )));
        }
        if !listed.insert(archived.file.as_str()) {
            return Err(IngestError::new(format!(
                "{RECORD} in {shown} lists the archive {} twice",
                archived.file
            )));
        }
    }
    let entries = fs::read_dir(directory)
        .map_err(|error| IngestError::new(format!("{shown} cannot be listed: {error}")))?;
    let mut present = BTreeSet::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| IngestError::new(format!("{shown} cannot be listed: {error}")))?;
        let name = entry.file_name().into_string().map_err(|name| {
            IngestError::new(format!("{shown} holds {name:?}, whose name is not UTF-8"))
        })?;
        if name == RECORD {
            continue;
        }
        if !listed.contains(name.as_str()) {
            return Err(IngestError::new(format!(
                "{shown} holds {name}, which {RECORD} does not list"
            ))
            .in_archive(&name));
        }
        let is_file = entry
            .file_type()
            .map_err(|error| IngestError::new(format!("{error}")).in_archive(&name))?
            .is_file();
        if !is_file {
            return Err(
                IngestError::new(format!("{shown} holds {name}, which is not a file"))
                    .in_archive(&name),
            );
        }
        present.insert(name);
    }

    let mut artifacts = Vec::new();
    let mut members: BTreeMap<String, Located> = BTreeMap::new();
    for (index, archived) in record.archives.iter().enumerate() {
        let name = archived.file.as_str();
        if !present.contains(name) {
            return Err(IngestError::new(format!(
                "{RECORD} lists {name}, and {shown} does not hold it"
            ))
            .in_archive(name));
        }
        let artifact_members = read_archive(directory, archived, limit, |member| {
            match members.get(member) {
                Some(other) => Err(IngestError::new(format!(
                    "the member {member} is held by two archives, {} and {name}",
                    record.archives[other.archive].file
                ))),
                None => Ok(()),
            }
        })
        .map_err(|error| error.in_archive(name))?;
        for member in &artifact_members {
            members.insert(
                member.name.clone(),
                Located {
                    name: member.name.clone(),
                    archive: index,
                    bytes: member.bytes,
                    blake3: member.blake3,
                },
            );
        }
        artifacts.push(Artifact {
            file: archived.file.clone(),
            url: archived.url.clone(),
            retrieved_at: archived.retrieved_at.clone(),
            bytes: archived.bytes,
            blake3: archived.blake3,
            sha256: archived.sha256.clone(),
            members: artifact_members,
        });
    }
    Ok(ReleaseArchives {
        directory: directory.to_owned(),
        release: record.release,
        artifacts,
        members: members.into_values().collect(),
        limit,
    })
}

/// Checks the archive `archived` against its record, checks the name and the
/// declared size of each of its members and gives each name to `held`, which
/// refuses a member that another archive holds, and only then reads and
/// digests the members. The members are returned sorted by name.
fn read_archive(
    directory: &Path,
    archived: &RetrievedArchive,
    limit: u64,
    held: impl Fn(&str) -> Result<(), IngestError>,
) -> Result<Vec<Member>, IngestError> {
    let unreadable = |error: io::Error| IngestError::new(format!("cannot be read: {error}"));
    let mut file = File::open(directory.join(&archived.file)).map_err(unreadable)?;
    let mut sha256 = Sha256::new();
    let mut blake3 = blake3::Hasher::new();
    let bytes = read_all(&mut file, |chunk| {
        sha256.update(chunk);
        blake3.update(chunk);
    })
    .map_err(unreadable)?;
    let sha256: String = sha256
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let blake3 = Hash32(*blake3.finalize().as_bytes());
    if bytes != archived.bytes {
        return Err(IngestError::new(format!(
            "the archive has {bytes} bytes, and its record says {}",
            archived.bytes
        )));
    }
    if sha256 != archived.sha256 {
        return Err(IngestError::new(format!(
            "the SHA-256 of the archive is {sha256}, and its record says {}",
            archived.sha256
        )));
    }
    if blake3 != archived.blake3 {
        return Err(IngestError::new(format!(
            "the BLAKE3 of the archive is {blake3}, and its record says {}",
            archived.blake3
        )));
    }
    file.seek(SeekFrom::Start(0)).map_err(unreadable)?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|error| IngestError::new(format!("the archive is not a zip archive: {error}")))?;
    if zip.has_duplicate_names() {
        return Err(IngestError::new(
            "the archive holds two members of the same name",
        ));
    }
    let mut names = Vec::new();
    for position in 0..zip.len() {
        let entry = zip
            .by_index_data(position)
            .map_err(|error| IngestError::new(format!("member {position}: {error}")))?;
        let name = String::from_utf8_lossy(entry.name_raw()).into_owned();
        if !is_member_name(&name) {
            return Err(IngestError::new(format!(
                "the member {name:?} is not named usc, two digits, an optional a or A, and .xml"
            ))
            .in_member(&name));
        }
        if entry.size() > limit {
            return Err(IngestError::new(format!(
                "the member declares {} bytes, larger than {}",
                entry.size(),
                limit_text(limit)
            ))
            .in_member(&name));
        }
        held(&name).map_err(|error| error.in_member(&name))?;
        names.push((position, name));
    }
    let mut members = Vec::new();
    for (position, name) in names {
        let reader = zip
            .by_index(position)
            .map_err(|error| IngestError::new(format!("{error}")).in_member(&name))?;
        let mut hasher = blake3::Hasher::new();
        let bytes = read_limited(reader, limit, |chunk| {
            hasher.update(chunk);
        })
        .map_err(|error| error.in_member(&name))?;
        members.push(Member {
            name,
            bytes,
            blake3: Hash32(*hasher.finalize().as_bytes()),
        });
    }
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(members)
}

/// Reads all of `reader`, giving each chunk to `each`, and returns the
/// number of bytes read.
fn read_all(reader: &mut impl Read, mut each: impl FnMut(&[u8])) -> io::Result<u64> {
    let mut buffer = vec![0; 1 << 16];
    let mut total = 0;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(read) => {
                each(&buffer[..read]);
                total += read as u64;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

/// Reads `reader` to its end, giving each chunk to `each`, and refuses it
/// when it holds more than `limit` bytes, whatever its header declares:
/// reading stops one byte past the limit.
fn read_limited(
    reader: impl Read,
    limit: u64,
    each: impl FnMut(&[u8]),
) -> Result<u64, IngestError> {
    let mut taken = reader.take(limit.saturating_add(1));
    let bytes = read_all(&mut taken, each)
        .map_err(|error| IngestError::new(format!("the member cannot be read: {error}")))?;
    if bytes > limit {
        return Err(IngestError::new(format!(
            "the member is larger than {}",
            limit_text(limit)
        )));
    }
    Ok(bytes)
}

/// The limit as a message writes it.
fn limit_text(limit: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if limit.is_multiple_of(MIB) {
        format!("{} MiB", limit / MIB)
    } else {
        format!("{limit} bytes")
    }
}

/// Whether `name` is the name of a file directly in a directory.
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name != RECORD
        && !name.contains(['/', '\\', '\0'])
}

/// Whether `name` is `usc`, two digits, an optional `a` or `A`, and `.xml`.
pub(crate) fn is_member_name(name: &str) -> bool {
    let Some(core) = name
        .strip_prefix("usc")
        .and_then(|rest| rest.strip_suffix(".xml"))
    else {
        return false;
    };
    matches!(
        core.as_bytes(),
        [a, b] | [a, b, b'a' | b'A'] if a.is_ascii_digit() && b.is_ascii_digit()
    )
}

/// The publisher's designation of the title a member holds: `usc05A.xml`
/// holds title `5a`.
fn title_of(member: &str) -> String {
    designation(&member[3..member.len() - 4])
}

/// A title designation written without leading zeros and in lower case, so
/// that `05A` and `5a` are one designation.
fn designation(title: &str) -> String {
    let digits = title.len() - title.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let number = title[..digits].trim_start_matches('0');
    let number = if number.is_empty() && digits > 0 {
        "0"
    } else {
        number
    };
    format!("{number}{}", title[digits..].to_ascii_lowercase())
}

impl ReleaseArchives {
    /// The release, as the retrieval record names it.
    pub fn release(&self) -> &str {
        &self.release
    }

    /// The archives with their members, in the order of the record, each as
    /// a lock lists it.
    pub fn artifacts(&self) -> Vec<Artifact> {
        self.artifacts.clone()
    }

    /// The archives as the lock of an ingest of `titles` lists them: in the
    /// order of the record, each archive that holds a member of those
    /// titles, with those of its members alone. When `titles` is empty,
    /// every archive that holds a member, with all of them. A title that no
    /// member holds is refused, as by [`crate::ingest`].
    pub fn artifacts_of(&self, titles: &[String]) -> Result<Vec<Artifact>, IngestError> {
        let read: BTreeSet<&str> = self
            .select(titles)?
            .into_iter()
            .map(|index| self.members[index].name.as_str())
            .collect();
        Ok(self
            .artifacts
            .iter()
            .filter_map(|artifact| {
                let members: Vec<Member> = artifact
                    .members
                    .iter()
                    .filter(|member| read.contains(member.name.as_str()))
                    .cloned()
                    .collect();
                (!members.is_empty()).then(|| Artifact {
                    members,
                    ..artifact.clone()
                })
            })
            .collect())
    }

    /// The file names of every member of the release, sorted.
    pub fn members(&self) -> Vec<String> {
        self.members
            .iter()
            .map(|member| member.name.clone())
            .collect()
    }

    /// The indexes, in [`Self::members`], of the members that hold `titles`,
    /// or of every member when `titles` is empty.
    pub(crate) fn select(&self, titles: &[String]) -> Result<Vec<usize>, IngestError> {
        if titles.is_empty() {
            return Ok((0..self.members.len()).collect());
        }
        let wanted: Vec<String> = titles.iter().map(|title| designation(title)).collect();
        for (title, designation) in titles.iter().zip(&wanted) {
            if !self
                .members
                .iter()
                .any(|member| &title_of(&member.name) == designation)
            {
                return Err(IngestError::new(format!(
                    "the release {} holds no title {title}",
                    self.release
                )));
            }
        }
        Ok(self
            .members
            .iter()
            .enumerate()
            .filter(|(_, member)| wanted.contains(&title_of(&member.name)))
            .map(|(index, _)| index)
            .collect())
    }

    /// The text of the member at `index` of [`Self::members`], read again
    /// from its archive and checked against what [`open`] found.
    pub(crate) fn read_member(&self, index: usize) -> Result<String, IngestError> {
        let member = &self.members[index];
        let archive = &self.artifacts[member.archive].file;
        let read = || -> Result<String, IngestError> {
            let unreadable =
                |error: io::Error| IngestError::new(format!("cannot be read: {error}"));
            let file = File::open(self.directory.join(archive)).map_err(unreadable)?;
            let mut zip = zip::ZipArchive::new(BufReader::new(file)).map_err(|error| {
                IngestError::new(format!("the archive is not a zip archive: {error}"))
            })?;
            let reader = zip
                .by_name(&member.name)
                .map_err(|error| IngestError::new(format!("{error}")))?;
            let mut bytes = Vec::with_capacity(usize::try_from(member.bytes).unwrap_or(0));
            let mut hasher = blake3::Hasher::new();
            read_limited(reader, self.limit, |chunk| {
                hasher.update(chunk);
                bytes.extend_from_slice(chunk);
            })?;
            if Hash32(*hasher.finalize().as_bytes()) != member.blake3 {
                return Err(IngestError::new(
                    "the member differs from the one the archive held when it was opened",
                ));
            }
            String::from_utf8(bytes)
                .map_err(|error| IngestError::new(format!("the member is not UTF-8: {error}")))
        };
        read().map_err(|error| self.locate(index, error))
    }

    /// `error`, naming the member at `index` and its archive.
    pub(crate) fn locate(&self, index: usize, error: IngestError) -> IngestError {
        let member = &self.members[index];
        error
            .in_archive(&self.artifacts[member.archive].file)
            .in_member(&member.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{RELEASE, Scratch, entry_for, pack, record_all, write_record, zip_of};

    const DOCUMENT: &[u8] = b"<uscDoc/>";

    /// The bytes of `zip` with the size of its one member declared as
    /// `size` in the local header and in the central directory.
    fn declaring(mut zip: Vec<u8>, size: u32) -> Vec<u8> {
        let mut patched = 0;
        for at in 0..zip.len().saturating_sub(4) {
            let offset = match &zip[at..at + 4] {
                [0x50, 0x4b, 0x03, 0x04] => 22,
                [0x50, 0x4b, 0x01, 0x02] => 24,
                _ => continue,
            };
            zip[at + offset..at + offset + 4].copy_from_slice(&size.to_le_bytes());
            patched += 1;
        }
        assert_eq!(
            patched, 2,
            "one local header and one central directory entry"
        );
        zip
    }

    #[test]
    fn open_reads_the_record_and_digests_every_member() {
        let scratch = Scratch::new("open-reads");
        pack(
            scratch.path(),
            "b.zip",
            &[("usc02.xml", b"two"), ("usc05A.xml", b"five")],
        );
        pack(scratch.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        record_all(scratch.path(), RELEASE);
        let archives = open(scratch.path()).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(archives.release(), RELEASE);
        assert_eq!(archives.members(), ["usc01.xml", "usc02.xml", "usc05A.xml"]);
        let artifacts = archives.artifacts();
        assert_eq!(
            artifacts
                .iter()
                .map(|artifact| artifact.file.as_str())
                .collect::<Vec<_>>(),
            ["a.zip", "b.zip"]
        );
        assert_eq!(artifacts[0], {
            let entry = entry_for(scratch.path(), "a.zip");
            Artifact {
                file: entry.file,
                url: entry.url,
                retrieved_at: entry.retrieved_at,
                bytes: entry.bytes,
                blake3: entry.blake3,
                sha256: entry.sha256,
                members: vec![Member {
                    name: "usc01.xml".to_owned(),
                    bytes: 9,
                    blake3: Hash32(*blake3::hash(DOCUMENT).as_bytes()),
                }],
            }
        });
        assert_eq!(
            artifacts[1]
                .members
                .iter()
                .map(|member| (member.name.as_str(), member.bytes))
                .collect::<Vec<_>>(),
            [("usc02.xml", 3), ("usc05A.xml", 4)]
        );
        assert_eq!(archives.read_member(2).as_deref(), Ok("five"));
        assert_eq!(archives.select(&[]), Ok(vec![0, 1, 2]));
        assert_eq!(
            archives.select(&["5A".to_owned(), "01".to_owned()]),
            Ok(vec![0, 2])
        );
        let error = archives.select(&["3".to_owned()]).expect_err("no title 3");
        assert!(error.message().contains("holds no title 3"), "{error}");
        // Nothing is written beside the archives.
        let mut names: Vec<String> = fs::read_dir(scratch.path())
            .expect("listed")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("UTF-8")
            })
            .collect();
        names.sort();
        assert_eq!(names, ["a.zip", "b.zip", "retrieval.json"]);
    }

    #[test]
    fn the_artifacts_of_titles_are_the_archives_and_members_that_hold_them() {
        let scratch = Scratch::new("artifacts-of");
        pack(
            scratch.path(),
            "b.zip",
            &[("usc02.xml", b"two"), ("usc05A.xml", b"five")],
        );
        pack(scratch.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        pack(scratch.path(), "c.zip", &[]);
        record_all(scratch.path(), RELEASE);
        let archives = open(scratch.path()).unwrap_or_else(|error| panic!("{error}"));
        let all = archives.artifacts();
        assert_eq!(all.len(), 3);
        // Every title: every archive of the record that holds a member, with
        // all of its members.
        assert_eq!(archives.artifacts_of(&[]), Ok(all[..2].to_vec()));
        // One title: its archive, with that member alone.
        assert_eq!(
            archives.artifacts_of(&["5a".to_owned()]),
            Ok(vec![Artifact {
                members: vec![all[1].members[1].clone()],
                ..all[1].clone()
            }])
        );
        // Two titles of two archives, in the order of the record.
        let named = |artifacts: Vec<Artifact>| -> Vec<(String, Vec<String>)> {
            artifacts
                .into_iter()
                .map(|artifact| {
                    let members = artifact.members.into_iter().map(|m| m.name).collect();
                    (artifact.file, members)
                })
                .collect()
        };
        let two = archives
            .artifacts_of(&["2".to_owned(), "01".to_owned()])
            .expect("titles 2 and 1");
        assert_eq!(
            named(two),
            [
                ("a.zip".to_owned(), vec!["usc01.xml".to_owned()]),
                ("b.zip".to_owned(), vec!["usc02.xml".to_owned()]),
            ]
        );
        let error = archives
            .artifacts_of(&["3".to_owned()])
            .expect_err("no title 3");
        assert!(error.message().contains("holds no title 3"), "{error}");
    }

    #[test]
    fn open_refuses_a_directory_without_a_retrieval_record() {
        let scratch = Scratch::new("no-record");
        pack(scratch.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        let error = open(scratch.path()).expect_err("no record");
        assert!(error.message().contains("has no retrieval.json"), "{error}");
        assert!(
            error
                .message()
                .contains(&scratch.path().display().to_string()),
            "{error}"
        );
    }

    #[test]
    fn open_refuses_an_archive_whose_size_or_digest_differs_from_its_record() {
        let scratch = Scratch::new("differs");
        pack(scratch.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        let right = entry_for(scratch.path(), "a.zip");
        let mut wrong_size = right.clone();
        wrong_size.bytes += 1;
        let mut wrong_sha = right.clone();
        wrong_sha.sha256 = "0".repeat(64);
        let mut wrong_blake3 = right.clone();
        wrong_blake3.blake3 = Hash32([0; 32]);
        for (entry, phrase) in [
            (wrong_size, "bytes, and its record says"),
            (wrong_sha, "SHA-256 of the archive"),
            (wrong_blake3, "BLAKE3 of the archive"),
        ] {
            write_record(scratch.path(), RELEASE, vec![entry]);
            let error = open(scratch.path()).expect_err(phrase);
            assert_eq!(error.archive(), Some("a.zip"), "{error}");
            assert!(error.message().contains(phrase), "{error}");
        }
        write_record(scratch.path(), RELEASE, vec![right]);
        assert!(open(scratch.path()).is_ok());
    }

    #[test]
    fn open_refuses_an_archive_the_record_does_not_list() {
        let scratch = Scratch::new("unlisted");
        pack(scratch.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        record_all(scratch.path(), RELEASE);
        pack(scratch.path(), "b.zip", &[("usc02.xml", DOCUMENT)]);
        let error = open(scratch.path()).expect_err("an unlisted archive");
        assert_eq!(error.archive(), Some("b.zip"), "{error}");
        assert!(error.message().contains("does not list"), "{error}");

        // An archive the record lists and the directory does not hold.
        fs::remove_file(scratch.path().join("a.zip")).expect("removed");
        let mut entry = entry_for(scratch.path(), "b.zip");
        let listed = entry.clone();
        entry.file = "a.zip".to_owned();
        write_record(scratch.path(), RELEASE, vec![entry, listed]);
        let error = open(scratch.path()).expect_err("an absent archive");
        assert_eq!(error.archive(), Some("a.zip"), "{error}");
        assert!(error.message().contains("does not hold it"), "{error}");
    }

    #[test]
    fn open_refuses_a_member_whose_name_is_not_a_title_file() {
        for name in [
            "../x.xml",
            "dir/x.xml",
            "x.txt",
            "usc999.xml",
            "usc01b.xml",
            "dir/",
        ] {
            let scratch = Scratch::new("member-name");
            pack(scratch.path(), "a.zip", &[(name, DOCUMENT)]);
            record_all(scratch.path(), RELEASE);
            let error = open(scratch.path()).expect_err(name);
            assert_eq!(error.archive(), Some("a.zip"), "{name}: {error}");
            assert_eq!(error.member(), Some(name), "{name}: {error}");
            assert!(
                error.message().contains("is not named usc"),
                "{name}: {error}"
            );
        }
        for name in ["usc01.xml", "usc11a.xml", "usc50A.xml"] {
            assert!(is_member_name(name), "{name}");
        }
    }

    #[test]
    fn open_refuses_a_member_larger_than_the_limit() {
        // A header that declares more than 512 MiB is refused before the
        // member is read.
        let scratch = Scratch::new("too-large");
        let zip = declaring(zip_of(&[("usc01.xml", DOCUMENT)]), 600 * 1024 * 1024);
        fs::write(scratch.path().join("a.zip"), zip).expect("written");
        record_all(scratch.path(), RELEASE);
        let error = open(scratch.path()).expect_err("too large");
        assert_eq!(error.member(), Some("usc01.xml"), "{error}");
        assert!(error.message().contains("larger than 512 MiB"), "{error}");

        // A header that declares less than the member holds does not let the
        // reading run past what it declares.
        let scratch = Scratch::new("lying-header");
        let text = vec![b'x'; 5000];
        let zip = declaring(zip_of(&[("usc01.xml", &text)]), 10);
        fs::write(scratch.path().join("a.zip"), zip).expect("written");
        record_all(scratch.path(), RELEASE);
        for limit in [1000, MEMBER_LIMIT] {
            let error = open_with_limit(scratch.path(), limit).expect_err("past the header");
            assert_eq!(error.member(), Some("usc01.xml"), "{error}");
            assert!(error.message().contains("larger than"), "{error}");
        }

        // Nor past the limit, whatever the reader below it does.
        let error = read_limited(&text[..], 1000, |_| {}).expect_err("past the limit");
        assert!(
            error.message().contains("larger than 1000 bytes"),
            "{error}"
        );
        let mut read = 0;
        assert_eq!(
            read_limited(&text[..], 5000, |chunk| read += chunk.len()),
            Ok(5000)
        );
        assert_eq!(read, 5000);
    }

    #[test]
    fn open_refuses_two_archives_that_hold_a_member_of_one_name() {
        let scratch = Scratch::new("twice");
        pack(scratch.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        pack(scratch.path(), "b.zip", &[("usc01.xml", DOCUMENT)]);
        record_all(scratch.path(), RELEASE);
        let error = open(scratch.path()).expect_err("held twice");
        assert_eq!(error.member(), Some("usc01.xml"), "{error}");
        assert!(
            error
                .message()
                .contains("held by two archives, a.zip and b.zip"),
            "{error}"
        );
    }

    #[test]
    fn two_scratch_directories_of_one_name_are_two_directories() {
        // Tests that run side by side never remove or overwrite each other's
        // archives.
        let first = Scratch::new("same-name");
        let second = Scratch::new("same-name");
        assert_ne!(first.path(), second.path());
        pack(first.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        drop(second);
        assert!(first.path().join("a.zip").is_file());
    }

    #[test]
    fn a_member_that_changed_after_open_is_refused() {
        let scratch = Scratch::new("changed");
        pack(scratch.path(), "a.zip", &[("usc01.xml", DOCUMENT)]);
        record_all(scratch.path(), RELEASE);
        let archives = open(scratch.path()).expect("opened");
        pack(
            scratch.path(),
            "a.zip",
            &[("usc01.xml", b"<uscDoc></uscDoc>")],
        );
        let error = archives.read_member(0).expect_err("changed");
        assert!(error.message().contains("differs from the one"), "{error}");
        assert_eq!(error.archive(), Some("a.zip"));
    }
}
