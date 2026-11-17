//! The files of a workspace as data: the manifest `workspace.json`, the lock
//! `law.lock.json` of a generated workspace, and the retrieval record
//! `retrieval.json` that stands beside downloaded archives.
//!
//! Each file is read with `from_json`, which refuses an unknown field, a
//! missing field, a `schema` other than the file's and a digest of another
//! form, and names the field. It also refuses a text whose JSON value is not
//! the value that the data read from it writes back, such as an object
//! written as an array of its values, and names the place where the two
//! differ. Each is written with `to_json`: its keys in a
//! fixed order, two spaces of indentation, one member to a line, every object
//! and array that is not empty spread over lines, an empty one written `{}` or
//! `[]`, and a final line end. The `modules` of a lock are written sorted by
//! path.

use crate::address::Address;
use crate::hash::Hash32;
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

/// The `schema` of a manifest.
pub const MANIFEST_SCHEMA: &str = "fidryn.workspace/v0.1";
/// The `schema` of a lock.
pub const LOCK_SCHEMA: &str = "fidryn.law-lock/v0.1";
/// The `schema` of a retrieval record.
pub const RETRIEVAL_SCHEMA: &str = "fidryn.retrieval/v0.1";

/// The manifest of a workspace, the file `workspace.json` at its top.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Manifest {
    pub name: String,
    pub kind: WorkspaceKind,
    /// Every module of the workspace has an address below it.
    pub prefix: Address,
    /// The release of a generated workspace. When it is `None` the file has
    /// no such key. Whether a workspace of a given kind must have one is for
    /// the check of the workspace to say, not for [`Manifest::from_json`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// The version of the toolchain the workspace was last checked with.
    pub toolchain: String,
    pub dependencies: Vec<Dependency>,
}

/// What made a workspace: an ingester (`generated`) or a person (`curated`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    Generated,
    Curated,
}

/// A generated workspace that a curated workspace depends on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub name: String,
    /// The directory of the dependency, relative to the manifest's, with `/`
    /// between its segments.
    pub path: String,
    pub release: String,
    /// The BLAKE3 of the bytes of the dependency's lock ([`Lock::digest`]).
    #[serde(deserialize_with = "lock_digest")]
    pub lock_digest: Hash32,
}

/// The lock of a generated workspace, `law.lock.json`, written by an
/// ingester.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Lock {
    pub prefix: Address,
    pub release: String,
    pub generator: Generator,
    pub artifacts: Vec<Artifact>,
    pub addresses: AddressCounts,
    pub not_imported: NotImported,
    /// The BLAKE3 of the bytes of every module file of the workspace, by its
    /// path relative to the workspace's directory.
    pub modules: BTreeMap<String, Hash32>,
}

/// The program that wrote a lock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generator {
    pub name: String,
    pub version: String,
}

/// An archive a generated workspace was made from. `url`, `retrieved_at`,
/// `bytes`, `blake3` and `sha256` are those of the archive's entry in the
/// retrieval record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// The archive's file name.
    pub file: String,
    pub url: String,
    pub retrieved_at: String,
    pub bytes: u64,
    #[serde(deserialize_with = "blake3_digest")]
    pub blake3: Hash32,
    /// 64 lower-case hexadecimal digits.
    #[serde(deserialize_with = "sha256_digest")]
    pub sha256: String,
    pub members: Vec<Member>,
}

/// A member of an archive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub name: String,
    pub bytes: u64,
    #[serde(deserialize_with = "blake3_digest")]
    pub blake3: Hash32,
}

/// The addresses an ingester counts: aliases, and the elements whose address
/// it disambiguated with `~2`, `~3` and so on or whose segment it assigned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressCounts {
    pub aliases: u64,
    pub disambiguated: u64,
    pub assigned: u64,
}

/// What an ingester leaves out by rule, counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotImported {
    pub tables_of_contents: u64,
    pub footnotes_in_tables_of_contents: u64,
    pub links_in_tables_of_contents: u64,
    pub release_notes: u64,
    pub links_without_words: u64,
    pub note_group_headings: u64,
}

/// The retrieval record `retrieval.json`, which stands beside the archives of
/// one release and is written when they are downloaded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RetrievalRecord {
    pub release: String,
    pub archives: Vec<RetrievedArchive>,
}

/// A downloaded archive, as its retrieval record holds it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievedArchive {
    /// The archive's file name.
    pub file: String,
    pub url: String,
    pub retrieved_at: String,
    pub bytes: u64,
    /// 64 lower-case hexadecimal digits.
    #[serde(deserialize_with = "sha256_digest")]
    pub sha256: String,
    #[serde(deserialize_with = "blake3_digest")]
    pub blake3: Hash32,
}

impl Manifest {
    /// The manifest written as `text`.
    pub fn from_json(text: &str) -> Result<Manifest, String> {
        let file: ManifestFile = read(text)?;
        closed(
            text,
            MANIFEST_SCHEMA,
            Manifest {
                name: file.name,
                kind: file.kind,
                prefix: file.prefix,
                release: file.release,
                toolchain: file.toolchain,
                dependencies: file.dependencies,
            },
        )
    }

    /// The text of the manifest's file.
    pub fn to_json(&self) -> String {
        write(MANIFEST_SCHEMA, self)
    }
}

impl Lock {
    /// The lock written as `text`.
    pub fn from_json(text: &str) -> Result<Lock, String> {
        let file: LockFile = read(text)?;
        closed(
            text,
            LOCK_SCHEMA,
            Lock {
                prefix: file.prefix,
                release: file.release,
                generator: file.generator,
                artifacts: file.artifacts,
                addresses: file.addresses,
                not_imported: file.not_imported,
                modules: file.modules,
            },
        )
    }

    /// The text of the lock's file.
    pub fn to_json(&self) -> String {
        write(LOCK_SCHEMA, self)
    }

    /// The BLAKE3 of the bytes of the lock file `text`, which a dependency
    /// records as its `lock_digest`.
    pub fn digest(text: &str) -> Hash32 {
        Hash32(*blake3::hash(text.as_bytes()).as_bytes())
    }
}

impl RetrievalRecord {
    /// The retrieval record written as `text`.
    pub fn from_json(text: &str) -> Result<RetrievalRecord, String> {
        let file: RetrievalFile = read(text)?;
        closed(
            text,
            RETRIEVAL_SCHEMA,
            RetrievalRecord {
                release: file.release,
                archives: file.archives,
            },
        )
    }

    /// The text of the retrieval record's file.
    pub fn to_json(&self) -> String {
        write(RETRIEVAL_SCHEMA, self)
    }
}

impl AddressCounts {
    /// Adds each count of `other` to the same count of these.
    pub fn add(&mut self, other: &Self) {
        self.aliases += other.aliases;
        self.disambiguated += other.disambiguated;
        self.assigned += other.assigned;
    }
}

impl NotImported {
    /// Adds each count of `other` to the same count of these.
    pub fn add(&mut self, other: &Self) {
        self.tables_of_contents += other.tables_of_contents;
        self.footnotes_in_tables_of_contents += other.footnotes_in_tables_of_contents;
        self.links_in_tables_of_contents += other.links_in_tables_of_contents;
        self.release_notes += other.release_notes;
        self.links_without_words += other.links_without_words;
        self.note_group_headings += other.note_group_headings;
    }
}

/// The file written as `text`; the error of `serde_json` names the field and
/// where it stands.
fn read<'a, T: Deserialize<'a>>(text: &'a str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

/// `data`, read from `text`, when the JSON value of `text` is the value that
/// `data` writes back with `schema` before it. The readers `serde` derives
/// also take an object written as an array of its values; no file of a
/// workspace is written so, and this refuses such a text. The values are
/// compared, not the bytes, so the layout and the order of the keys do not
/// matter.
fn closed<T: Serialize>(text: &str, schema: &'static str, data: T) -> Result<T, String> {
    let given: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let written = serde_json::to_value(Tagged {
        schema,
        body: &data,
    })
    .expect("a file of strings, numbers, digests and addresses is written");
    let mut pointer = String::new();
    match difference(&given, &written, &mut pointer) {
        None => Ok(data),
        Some(found) => {
            let place = if pointer.is_empty() {
                "the top of the file".to_owned()
            } else {
                format!("`{pointer}`")
            };
            Err(format!("{place} is {found}"))
        }
    }
}

/// The first place, in the order of `written`, where `given` differs from
/// it: `pointer` is left as the JSON pointer of that place, and the result
/// says what stands there.
fn difference(given: &Value, written: &Value, pointer: &mut String) -> Option<String> {
    match (given, written) {
        (Value::Object(given), Value::Object(written)) => {
            for (key, value) in written {
                let at = pointer.len();
                push_key(pointer, key);
                match given.get(key) {
                    Some(found) => {
                        if let Some(found) = difference(found, value, pointer) {
                            return Some(found);
                        }
                    }
                    None => return Some("missing where the file is written with it".to_owned()),
                }
                pointer.truncate(at);
            }
            given
                .keys()
                .find(|key| !written.contains_key(*key))
                .map(|key| {
                    push_key(pointer, key);
                    "a key the file is not written with".to_owned()
                })
        }
        (Value::Array(given), Value::Array(written)) if given.len() == written.len() => {
            for (index, (found, value)) in given.iter().zip(written).enumerate() {
                let at = pointer.len();
                pointer.push_str(&format!("/{index}"));
                if let Some(found) = difference(found, value, pointer) {
                    return Some(found);
                }
                pointer.truncate(at);
            }
            None
        }
        (Value::Array(given), Value::Array(written)) => Some(format!(
            "an array of {} values where the file is written with {}",
            given.len(),
            written.len()
        )),
        _ if given == written => None,
        _ if kind_of(given) == kind_of(written) => {
            Some("a value other than the one the file is written with".to_owned())
        }
        _ => Some(format!(
            "{} where the file is written with {}",
            kind_of(given),
            kind_of(written)
        )),
    }
}

/// Appends `key` to the JSON pointer `pointer`, with `~` written `~0` and
/// `/` written `~1`.
fn push_key(pointer: &mut String, key: &str) {
    pointer.push('/');
    pointer.push_str(&key.replace('~', "~0").replace('/', "~1"));
}

/// What a JSON value is, as a diagnostic names it.
fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "true or false",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The text of a file: `schema`, then the fields of `body` in the order of
/// its type, as `serde_json` writes them with two spaces of indentation, and
/// a final line end.
fn write<T: Serialize>(schema: &'static str, body: &T) -> String {
    let mut text = serde_json::to_string_pretty(&Tagged { schema, body })
        .expect("a file of strings, numbers, digests and addresses is written");
    text.push('\n');
    text
}

impl<'de> Deserialize<'de> for WorkspaceKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(KindVisitor)
    }
}

/// Reads the `kind` of a manifest, from a string and nothing else.
struct KindVisitor;

impl Visitor<'_> for KindVisitor {
    type Value = WorkspaceKind;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("`generated` or `curated` in `kind`")
    }

    fn visit_str<E: de::Error>(self, word: &str) -> Result<WorkspaceKind, E> {
        match word {
            "generated" => Ok(WorkspaceKind::Generated),
            "curated" => Ok(WorkspaceKind::Curated),
            _ => Err(E::custom(format_args!(
                "`kind` is {word:?}, and a workspace is `generated` or `curated`"
            ))),
        }
    }
}

/// Reads a digest of 64 lower-case hexadecimal digits; an error names
/// `field`, the place of the digest in its file.
struct Digest<'a> {
    field: &'a str,
}

impl<'de> DeserializeSeed<'de> for Digest<'_> {
    type Value = Hash32;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Hash32, D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl Visitor<'_> for Digest<'_> {
    type Value = Hash32;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "64 lower-case hexadecimal digits in {}", self.field)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Hash32, E> {
        text.parse()
            .map_err(|error| E::custom(format_args!("{}: {error}", self.field)))
    }
}

fn blake3_digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Hash32, D::Error> {
    Digest { field: "`blake3`" }.deserialize(deserializer)
}

fn lock_digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Hash32, D::Error> {
    Digest {
        field: "`lock_digest`",
    }
    .deserialize(deserializer)
}

/// A SHA-256 digest is written in the form of a BLAKE3 digest, 64 lower-case
/// hexadecimal digits, so [`Hash32`] checks it, and writes back the same text.
fn sha256_digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Digest { field: "`sha256`" }
        .deserialize(deserializer)
        .map(|digest| digest.to_string())
}

/// Reads the `modules` of a lock; an error names the path whose digest has
/// another form, or that is listed twice.
fn modules<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Hash32>, D::Error> {
    deserializer.deserialize_map(ModulesVisitor)
}

struct ModulesVisitor;

impl<'de> Visitor<'de> for ModulesVisitor {
    type Value = BTreeMap<String, Hash32>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object of module paths and their digests in `modules`")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut modules = BTreeMap::new();
        while let Some(path) = map.next_key::<String>()? {
            let field = format!("the digest of {path:?} in `modules`");
            let digest = map.next_value_seed(Digest { field: &field })?;
            if modules.contains_key(&path) {
                return Err(de::Error::custom(format_args!(
                    "`modules` lists {path:?} twice"
                )));
            }
            modules.insert(path, digest);
        }
        Ok(modules)
    }
}

/// Reads a key that may be left out but, when it is present, holds a string.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

/// Reads the `schema` of a file, and refuses every string but the schema of
/// the file's kind, which it holds.
struct Schema(&'static str);

impl Visitor<'_> for Schema {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} in `schema`", self.0)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<(), E> {
        if text == self.0 {
            Ok(())
        } else {
            Err(E::custom(format_args!(
                "`schema` is {text:?}, not {:?}",
                self.0
            )))
        }
    }
}

fn manifest_schema<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    deserializer.deserialize_str(Schema(MANIFEST_SCHEMA))
}

fn lock_schema<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    deserializer.deserialize_str(Schema(LOCK_SCHEMA))
}

fn retrieval_schema<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    deserializer.deserialize_str(Schema(RETRIEVAL_SCHEMA))
}

/// A manifest as its file holds it: its `schema`, then the fields of
/// [`Manifest`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    #[serde(rename = "schema", deserialize_with = "manifest_schema")]
    _schema: (),
    name: String,
    kind: WorkspaceKind,
    prefix: Address,
    #[serde(default, deserialize_with = "present")]
    release: Option<String>,
    toolchain: String,
    dependencies: Vec<Dependency>,
}

/// A lock as its file holds it: its `schema`, then the fields of [`Lock`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LockFile {
    #[serde(rename = "schema", deserialize_with = "lock_schema")]
    _schema: (),
    prefix: Address,
    release: String,
    generator: Generator,
    artifacts: Vec<Artifact>,
    addresses: AddressCounts,
    not_imported: NotImported,
    #[serde(deserialize_with = "modules")]
    modules: BTreeMap<String, Hash32>,
}

/// A retrieval record as its file holds it: its `schema`, then the fields of
/// [`RetrievalRecord`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrievalFile {
    #[serde(rename = "schema", deserialize_with = "retrieval_schema")]
    _schema: (),
    release: String,
    archives: Vec<RetrievedArchive>,
}

/// A file's data with its `schema` before it, for writing.
#[derive(Serialize)]
struct Tagged<'a, T> {
    schema: &'static str,
    #[serde(flatten)]
    body: &'a T,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, Value, json};

    const ARCHIVE_BLAKE3: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    const ARCHIVE_SHA256: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";
    const MEMBER_BLAKE3: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    const TITLE_BLAKE3: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const SECTION_BLAKE3: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const LOCK_DIGEST: &str = "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf";

    // The examples of the three files, with invented names, digests and
    // counts, in the layout `to_json` writes.

    const GENERATED_MANIFEST: &str = r#"{
  "schema": "fidryn.workspace/v0.1",
  "name": "demo",
  "kind": "generated",
  "prefix": "/zz/demo",
  "release": "42-7",
  "toolchain": "0.3.0",
  "dependencies": []
}
"#;

    const CURATED_MANIFEST: &str = r#"{
  "schema": "fidryn.workspace/v0.1",
  "name": "demo-curated",
  "kind": "curated",
  "prefix": "/zz/demo-curated",
  "toolchain": "0.3.0",
  "dependencies": [
    {
      "name": "demo",
      "path": "../demo",
      "release": "42-7",
      "lock_digest": "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf"
    }
  ]
}
"#;

    const LOCK: &str = r#"{
  "schema": "fidryn.law-lock/v0.1",
  "prefix": "/zz/demo",
  "release": "42-7",
  "generator": {
    "name": "demo-ingest",
    "version": "0.3.0"
  },
  "artifacts": [
    {
      "file": "demo01@42-7.zip",
      "url": "https://example.invalid/releases/42/7/demo01@42-7.zip",
      "retrieved_at": "2026-11-16T22:14:03Z",
      "bytes": 42241,
      "blake3": "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90",
      "sha256": "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0",
      "members": [
        {
          "name": "demo01.xml",
          "bytes": 284795,
          "blake3": "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
        }
      ]
    }
  ],
  "addresses": {
    "aliases": 3,
    "disambiguated": 1,
    "assigned": 2
  },
  "not_imported": {
    "tables_of_contents": 1,
    "footnotes_in_tables_of_contents": 2,
    "links_in_tables_of_contents": 3,
    "release_notes": 4,
    "links_without_words": 5,
    "note_group_headings": 6
  },
  "modules": {
    "zz/demo/t1.fr": "1111111111111111111111111111111111111111111111111111111111111111",
    "zz/demo/t1/s1.fr": "2222222222222222222222222222222222222222222222222222222222222222"
  }
}
"#;

    const RETRIEVAL: &str = r#"{
  "schema": "fidryn.retrieval/v0.1",
  "release": "42-7",
  "archives": [
    {
      "file": "demo01@42-7.zip",
      "url": "https://example.invalid/releases/42/7/demo01@42-7.zip",
      "retrieved_at": "2026-11-16T22:14:03Z",
      "bytes": 42241,
      "sha256": "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0",
      "blake3": "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90"
    }
  ]
}
"#;

    fn hash(text: &str) -> Hash32 {
        text.parse()
            .unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    fn address(text: &str) -> Address {
        Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    fn generated_manifest() -> Manifest {
        Manifest {
            name: "demo".to_owned(),
            kind: WorkspaceKind::Generated,
            prefix: address("/zz/demo"),
            release: Some("42-7".to_owned()),
            toolchain: "0.3.0".to_owned(),
            dependencies: Vec::new(),
        }
    }

    fn curated_manifest() -> Manifest {
        Manifest {
            name: "demo-curated".to_owned(),
            kind: WorkspaceKind::Curated,
            prefix: address("/zz/demo-curated"),
            release: None,
            toolchain: "0.3.0".to_owned(),
            dependencies: vec![Dependency {
                name: "demo".to_owned(),
                path: "../demo".to_owned(),
                release: "42-7".to_owned(),
                lock_digest: hash(LOCK_DIGEST),
            }],
        }
    }

    const URL: &str = "https://example.invalid/releases/42/7/demo01@42-7.zip";

    fn lock() -> Lock {
        Lock {
            prefix: address("/zz/demo"),
            release: "42-7".to_owned(),
            generator: Generator {
                name: "demo-ingest".to_owned(),
                version: "0.3.0".to_owned(),
            },
            artifacts: vec![Artifact {
                file: "demo01@42-7.zip".to_owned(),
                url: URL.to_owned(),
                retrieved_at: "2026-11-16T22:14:03Z".to_owned(),
                bytes: 42241,
                blake3: hash(ARCHIVE_BLAKE3),
                sha256: ARCHIVE_SHA256.to_owned(),
                members: vec![Member {
                    name: "demo01.xml".to_owned(),
                    bytes: 284_795,
                    blake3: hash(MEMBER_BLAKE3),
                }],
            }],
            addresses: AddressCounts {
                aliases: 3,
                disambiguated: 1,
                assigned: 2,
            },
            not_imported: NotImported {
                tables_of_contents: 1,
                footnotes_in_tables_of_contents: 2,
                links_in_tables_of_contents: 3,
                release_notes: 4,
                links_without_words: 5,
                note_group_headings: 6,
            },
            modules: BTreeMap::from([
                ("zz/demo/t1/s1.fr".to_owned(), hash(SECTION_BLAKE3)),
                ("zz/demo/t1.fr".to_owned(), hash(TITLE_BLAKE3)),
            ]),
        }
    }

    fn retrieval() -> RetrievalRecord {
        RetrievalRecord {
            release: "42-7".to_owned(),
            archives: vec![RetrievedArchive {
                file: "demo01@42-7.zip".to_owned(),
                url: URL.to_owned(),
                retrieved_at: "2026-11-16T22:14:03Z".to_owned(),
                bytes: 42241,
                sha256: ARCHIVE_SHA256.to_owned(),
                blake3: hash(ARCHIVE_BLAKE3),
            }],
        }
    }

    #[test]
    fn each_file_round_trips_byte_for_byte() {
        for (text, manifest) in [
            (GENERATED_MANIFEST, generated_manifest()),
            (CURATED_MANIFEST, curated_manifest()),
        ] {
            let read = Manifest::from_json(text).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(read, manifest);
            assert_eq!(read.to_json(), text);
        }
        let read = Lock::from_json(LOCK).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(read, lock());
        assert_eq!(read.to_json(), LOCK);
        let read = RetrievalRecord::from_json(RETRIEVAL).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(read, retrieval());
        assert_eq!(read.to_json(), RETRIEVAL);
    }

    type Read = fn(&str) -> Result<(), String>;

    fn read_manifest(text: &str) -> Result<(), String> {
        Manifest::from_json(text).map(drop)
    }

    fn read_lock(text: &str) -> Result<(), String> {
        Lock::from_json(text).map(drop)
    }

    fn read_retrieval(text: &str) -> Result<(), String> {
        RetrievalRecord::from_json(text).map(drop)
    }

    /// `text` with `change` made to the object at `pointer`, written out
    /// again.
    fn changed(text: &str, pointer: &str, change: impl FnOnce(&mut Map<String, Value>)) -> String {
        let mut value: Value = serde_json::from_str(text).expect("an example is JSON");
        let object = value
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .unwrap_or_else(|| panic!("no object at {pointer:?}"));
        change(object);
        serde_json::to_string_pretty(&value).expect("a JSON value is written")
    }

    fn keys(text: &str, pointer: &str) -> Vec<String> {
        let value: Value = serde_json::from_str(text).expect("an example is JSON");
        value
            .pointer(pointer)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("no object at {pointer:?}"))
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn an_unknown_field_a_missing_field_and_a_wrong_schema_are_refused_and_named() {
        let files: [(&str, &str, Read, &str, &[&str]); 4] = [
            (
                "a generated manifest",
                GENERATED_MANIFEST,
                read_manifest,
                MANIFEST_SCHEMA,
                &[""],
            ),
            (
                "a curated manifest",
                CURATED_MANIFEST,
                read_manifest,
                MANIFEST_SCHEMA,
                &["", "/dependencies/0"],
            ),
            (
                "a lock",
                LOCK,
                read_lock,
                LOCK_SCHEMA,
                &[
                    "",
                    "/generator",
                    "/artifacts/0",
                    "/artifacts/0/members/0",
                    "/addresses",
                    "/not_imported",
                ],
            ),
            (
                "a retrieval record",
                RETRIEVAL,
                read_retrieval,
                RETRIEVAL_SCHEMA,
                &["", "/archives/0"],
            ),
        ];
        let mut missing = 0;
        for (what, text, read, schema, objects) in files {
            assert_eq!(read(text), Ok(()), "{what}");
            for &pointer in objects {
                let at = format!("{what}, at {pointer:?}");
                let text_with_extra = changed(text, pointer, |object| {
                    object.insert("extra".to_owned(), json!(1));
                });
                let error = read(&text_with_extra).expect_err(&at);
                assert!(error.contains("unknown field `extra`"), "{at}: {error}");

                for key in keys(text, pointer) {
                    let without = changed(text, pointer, |object| {
                        object.remove(&key);
                    });
                    if pointer.is_empty() && key == "release" && schema == MANIFEST_SCHEMA {
                        // A manifest may leave out its release.
                        assert_eq!(read(&without), Ok(()), "{at}");
                        continue;
                    }
                    let error = read(&without).expect_err(&format!("{at}: without {key}"));
                    assert!(
                        error.contains(&format!("missing field `{key}`")),
                        "{at}: without {key}: {error}"
                    );
                    missing += 1;
                }
            }
            for wrong in [
                MANIFEST_SCHEMA,
                LOCK_SCHEMA,
                RETRIEVAL_SCHEMA,
                "fidryn.workspace/v0.2",
                "",
            ] {
                if wrong == schema {
                    continue;
                }
                let text = changed(text, "", |object| {
                    object.insert("schema".to_owned(), json!(wrong));
                });
                let error = read(&text).expect_err(&format!("{what}, schema {wrong:?}"));
                assert!(
                    error.contains(&format!("`schema` is {wrong:?}, not {schema:?}")),
                    "{what}, schema {wrong:?}: {error}"
                );
            }
            let text = changed(text, "", |object| {
                object.insert("schema".to_owned(), json!(1));
            });
            let error = read(&text).expect_err(&format!("{what}, schema 1"));
            assert!(error.contains("in `schema`"), "{what}, schema 1: {error}");
        }
        // Every field of every object but the release of a manifest.
        assert_eq!(missing, 54);
    }

    #[test]
    fn a_digest_of_another_form_is_refused_and_named() {
        let places: [(&str, Read, &str, &str, &str); 7] = [
            (LOCK, read_lock, "/artifacts/0", "blake3", "`blake3`"),
            (LOCK, read_lock, "/artifacts/0", "sha256", "`sha256`"),
            (
                LOCK,
                read_lock,
                "/artifacts/0/members/0",
                "blake3",
                "`blake3`",
            ),
            (
                LOCK,
                read_lock,
                "/modules",
                "zz/demo/t1.fr",
                "the digest of \"zz/demo/t1.fr\" in `modules`",
            ),
            (
                RETRIEVAL,
                read_retrieval,
                "/archives/0",
                "sha256",
                "`sha256`",
            ),
            (
                RETRIEVAL,
                read_retrieval,
                "/archives/0",
                "blake3",
                "`blake3`",
            ),
            (
                CURATED_MANIFEST,
                read_manifest,
                "/dependencies/0",
                "lock_digest",
                "`lock_digest`",
            ),
        ];
        let others = [
            json!(ARCHIVE_BLAKE3.to_uppercase()),
            json!(&ARCHIVE_BLAKE3[..63]),
            json!(format!("{ARCHIVE_BLAKE3}0")),
            json!(format!(" {}", &ARCHIVE_BLAKE3[1..])),
            json!(""),
            json!(7),
            json!(null),
        ];
        for (text, read, pointer, key, named) in places {
            for other in &others {
                let text = changed(text, pointer, |object| {
                    object.insert(key.to_owned(), other.clone());
                });
                let error = read(&text).expect_err(&format!("{key} = {other}"));
                assert!(error.contains(named), "{key} = {other}: {error}");
            }
        }
    }

    /// `text` with the object at `pointer` written as the array of its
    /// values, in their order.
    fn as_array(text: &str, pointer: &str) -> String {
        let mut value: Value = serde_json::from_str(text).expect("an example is JSON");
        let place = value
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("nothing at {pointer:?}"));
        let values: Vec<Value> = place
            .as_object()
            .unwrap_or_else(|| panic!("no object at {pointer:?}"))
            .values()
            .cloned()
            .collect();
        *place = Value::Array(values);
        serde_json::to_string_pretty(&value).expect("a JSON value is written")
    }

    #[test]
    fn an_object_written_as_an_array_is_refused() {
        let places: [(&str, &str, Read, &str); 9] = [
            ("a manifest", GENERATED_MANIFEST, read_manifest, ""),
            (
                "a manifest",
                CURATED_MANIFEST,
                read_manifest,
                "/dependencies/0",
            ),
            ("a lock", LOCK, read_lock, ""),
            ("a lock", LOCK, read_lock, "/generator"),
            ("a lock", LOCK, read_lock, "/artifacts/0"),
            ("a lock", LOCK, read_lock, "/artifacts/0/members/0"),
            ("a lock", LOCK, read_lock, "/not_imported"),
            ("a retrieval record", RETRIEVAL, read_retrieval, ""),
            (
                "a retrieval record",
                RETRIEVAL,
                read_retrieval,
                "/archives/0",
            ),
        ];
        for (what, text, read, pointer) in places {
            let at = format!("{what}, at {pointer:?}");
            let text = as_array(text, pointer);
            let error = read(&text).expect_err(&at);
            let place = if pointer.is_empty() {
                "the top of the file".to_owned()
            } else {
                format!("`{pointer}`")
            };
            assert!(
                error.contains(&format!(
                    "{place} is an array where the file is written with an object"
                )),
                "{at}: {error}"
            );
        }
        // The value is compared, not the bytes: another layout and another
        // order of the keys read as before.
        let compact: Value = serde_json::from_str(LOCK).expect("the lock is JSON");
        let compact = serde_json::to_string(&compact).expect("a JSON value is written");
        assert_eq!(Lock::from_json(&compact), Ok(lock()));
        let reordered = changed(GENERATED_MANIFEST, "", |object| {
            let name = object.remove("name").expect("a name");
            object.insert("name".to_owned(), name);
        });
        assert_ne!(keys(&reordered, "")[1], "name");
        assert_eq!(Manifest::from_json(&reordered), Ok(generated_manifest()));
    }

    #[test]
    fn a_path_listed_twice_in_modules_is_refused() {
        let text = LOCK.replace("\"zz/demo/t1/s1.fr\":", "\"zz/demo/t1.fr\":");
        assert_ne!(text, LOCK);
        let error = Lock::from_json(&text).expect_err("a path listed twice");
        assert!(
            error.contains("`modules` lists \"zz/demo/t1.fr\" twice"),
            "{error}"
        );
    }

    #[test]
    fn modules_are_written_sorted_by_path() {
        let paths = [
            "zz/demo/t2.fr",
            "zz/demo/t1/s1.fr",
            "zz/demo/t10.fr",
            "zz/demo/t1.fr",
        ];
        let with_modules = |order: [usize; 4]| {
            let mut lock = lock();
            lock.modules.clear();
            for index in order {
                let digest = Hash32([u8::try_from(index).expect("a small index"); 32]);
                lock.modules.insert(paths[index].to_owned(), digest);
            }
            lock
        };
        let text = with_modules([0, 1, 2, 3]).to_json();
        for order in [[3, 2, 1, 0], [1, 3, 0, 2], [2, 0, 3, 1]] {
            assert_eq!(with_modules(order).to_json(), text, "{order:?}");
        }
        // Sorted by the bytes of the path: `.` comes before `/`, and `/`
        // before `0`.
        let at: Vec<usize> = [
            "zz/demo/t1.fr",
            "zz/demo/t1/s1.fr",
            "zz/demo/t10.fr",
            "zz/demo/t2.fr",
        ]
        .iter()
        .map(|path| {
            text.find(&format!("\"{path}\": "))
                .unwrap_or_else(|| panic!("{path} is not written"))
        })
        .collect();
        assert!(at.is_sorted(), "{text}");

        // A lock read from a file that lists its modules in another order is
        // written sorted.
        let unsorted = LOCK.replace(
            "    \"zz/demo/t1.fr\": \"1111111111111111111111111111111111111111111111111111111111111111\",\n    \"zz/demo/t1/s1.fr\": \"2222222222222222222222222222222222222222222222222222222222222222\"\n",
            "    \"zz/demo/t1/s1.fr\": \"2222222222222222222222222222222222222222222222222222222222222222\",\n    \"zz/demo/t1.fr\": \"1111111111111111111111111111111111111111111111111111111111111111\"\n",
        );
        assert_ne!(unsorted, LOCK);
        let read = Lock::from_json(&unsorted).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(read.to_json(), LOCK);
    }

    #[test]
    fn kind_is_generated_or_curated() {
        let with_kind = |kind: Value| {
            changed(GENERATED_MANIFEST, "", |object| {
                object.insert("kind".to_owned(), kind);
            })
        };
        for (word, kind) in [
            ("generated", WorkspaceKind::Generated),
            ("curated", WorkspaceKind::Curated),
        ] {
            let read = Manifest::from_json(&with_kind(json!(word)))
                .unwrap_or_else(|error| panic!("{word}: {error}"));
            assert_eq!(read.kind, kind);
            // That a curated workspace has no release is checked later.
            assert_eq!(read.release.as_deref(), Some("42-7"));
            assert!(read.to_json().contains(&format!("\"kind\": \"{word}\"")));
        }
        for word in [
            "Generated",
            "CURATED",
            "library",
            "",
            "curated ",
            "generated-workspace",
        ] {
            let error = Manifest::from_json(&with_kind(json!(word))).expect_err(word);
            assert!(
                error.contains(&format!(
                    "`kind` is {word:?}, and a workspace is `generated` or `curated`"
                )),
                "{word:?}: {error}"
            );
        }
        for other in [
            json!(null),
            json!(1),
            json!(["curated"]),
            json!({ "curated": null }),
        ] {
            let error = Manifest::from_json(&with_kind(other.clone())).expect_err("no word");
            assert!(
                error.contains("expected `generated` or `curated` in `kind`"),
                "{other}: {error}"
            );
        }
    }

    #[test]
    fn a_manifest_without_a_release_has_no_such_key() {
        let top_keys = |text: &str| keys(text, "");
        assert!(!top_keys(&curated_manifest().to_json()).contains(&"release".to_owned()));
        assert_eq!(
            top_keys(&generated_manifest().to_json()),
            [
                "schema",
                "name",
                "kind",
                "prefix",
                "release",
                "toolchain",
                "dependencies"
            ]
        );
        let with_release = |release: Value| {
            changed(CURATED_MANIFEST, "", |object| {
                object.insert("release".to_owned(), release);
            })
        };
        let error = Manifest::from_json(&with_release(json!(null))).expect_err("a null release");
        assert!(error.contains("invalid type: null"), "{error}");
        let read = Manifest::from_json(&with_release(json!("42-7")))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(read.release.as_deref(), Some("42-7"));
    }

    #[test]
    fn the_lock_digest_is_the_blake3_of_its_bytes() {
        // Known answers of BLAKE3.
        assert_eq!(
            Lock::digest(""),
            hash("af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262")
        );
        assert_eq!(
            Lock::digest("abc"),
            hash("6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85")
        );
        assert_eq!(
            Lock::digest(LOCK),
            Hash32(*blake3::hash(LOCK.as_bytes()).as_bytes())
        );
        // The digest is of the bytes, not of the data they hold.
        let without_line_end = LOCK.trim_end();
        assert_eq!(Lock::from_json(without_line_end), Ok(lock()));
        assert_ne!(Lock::digest(without_line_end), Lock::digest(LOCK));
        // What a dependency records.
        let digest = Lock::digest(&lock().to_json());
        assert_eq!(digest, Lock::digest(LOCK));
        let text = CURATED_MANIFEST.replace(LOCK_DIGEST, &digest.to_string());
        let manifest = Manifest::from_json(&text).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(manifest.dependencies[0].lock_digest, digest);
    }

    #[test]
    fn counts_add_field_by_field() {
        let mut addresses = AddressCounts {
            aliases: 1,
            disambiguated: 2,
            assigned: 3,
        };
        addresses.add(&AddressCounts {
            aliases: 10,
            disambiguated: 20,
            assigned: 30,
        });
        assert_eq!(
            addresses,
            AddressCounts {
                aliases: 11,
                disambiguated: 22,
                assigned: 33,
            }
        );
        assert_eq!(
            AddressCounts::default(),
            AddressCounts {
                aliases: 0,
                disambiguated: 0,
                assigned: 0,
            }
        );

        let mut not_imported = NotImported {
            tables_of_contents: 1,
            footnotes_in_tables_of_contents: 2,
            links_in_tables_of_contents: 3,
            release_notes: 4,
            links_without_words: 5,
            note_group_headings: 6,
        };
        not_imported.add(&NotImported {
            tables_of_contents: 10,
            footnotes_in_tables_of_contents: 20,
            links_in_tables_of_contents: 30,
            release_notes: 40,
            links_without_words: 50,
            note_group_headings: 60,
        });
        assert_eq!(
            not_imported,
            NotImported {
                tables_of_contents: 11,
                footnotes_in_tables_of_contents: 22,
                links_in_tables_of_contents: 33,
                release_notes: 44,
                links_without_words: 55,
                note_group_headings: 66,
            }
        );
        assert_eq!(
            NotImported::default(),
            NotImported {
                tables_of_contents: 0,
                footnotes_in_tables_of_contents: 0,
                links_in_tables_of_contents: 0,
                release_notes: 0,
                links_without_words: 0,
                note_group_headings: 0,
            }
        );
    }
}
