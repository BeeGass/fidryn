//! Import of the United States Code from the official XML release point
//! published by the Office of the Law Revision Counsel into the provision
//! model of `fidryn-provision`.
//!
//! This is the one crate of the workspace that may name the Code, a title, a
//! publisher element or a status value.
//!
//! [`open`] reads a release: a directory of the publisher's archives and the
//! retrieval record beside them, with each archive checked against its record
//! and each member against the limits of the reading. [`ingest`] reads the
//! members of a release into modules, and [`ingest_xml`] reads one document.
//!
//! A document is read into its containers and units. A container lists the
//! containers and units that are its members; a unit holds its tree of
//! provisions, with their numbers, headings, statuses and text. Every one of
//! them has an address, which is the publisher's identifier except where the
//! release repeats an identifier or an element has none. Notes, source
//! credits and footnotes are found where they stand and set aside: a footnote
//! does not interrupt the paragraph it stands in, and a footnote marker gives
//! no text. What the reading leaves out by rule is counted.

mod archive;
mod containers;
mod operative;
mod uslm;

#[cfg(test)]
mod testing;

pub use archive::{ReleaseArchives, open};

use fidryn_provision::Module;
use fidryn_provision::workspace::{AddressCounts, NotImported};
use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The prefix of every address of the Code.
pub(crate) const PREFIX: &str = "/us/usc";

/// Why a release, an archive or a document is refused. It names the archive,
/// the member, and, where the refusal concerns one element, the element, the
/// line it starts on, and its identifier or that of its nearest ancestor that
/// has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngestError(Box<Refusal>);

/// What an [`IngestError`] holds.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Refusal {
    archive: Option<String>,
    member: Option<String>,
    place: Option<Place>,
    message: String,
}

/// The element a refusal concerns.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    element: String,
    line: u32,
    identifier: Option<String>,
    /// Whether `identifier` is the element's own.
    own: bool,
}

impl IngestError {
    /// A refusal that concerns no one element.
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(Box::new(Refusal {
            archive: None,
            member: None,
            place: None,
            message: message.into(),
        }))
    }

    /// A refusal of `node`.
    pub(crate) fn at(node: roxmltree::Node<'_, '_>, message: impl Into<String>) -> Self {
        let own = node.attribute("identifier");
        let identifier = node
            .ancestors()
            .find_map(|ancestor| ancestor.attribute("identifier"));
        let mut error = Self::new(message);
        error.0.place = Some(Place {
            element: uslm::tag(node),
            line: node.document().text_pos_at(node.range().start).row,
            identifier: identifier.map(str::to_owned),
            own: own.is_some(),
        });
        error
    }

    /// This refusal, naming `archive` when it names none yet.
    pub(crate) fn in_archive(mut self, archive: &str) -> Self {
        self.0.archive.get_or_insert_with(|| archive.to_owned());
        self
    }

    /// This refusal, naming `member` when it names none yet.
    pub(crate) fn in_member(mut self, member: &str) -> Self {
        self.0.member.get_or_insert_with(|| member.to_owned());
        self
    }

    /// The file name of the archive, when the refusal concerns one.
    pub fn archive(&self) -> Option<&str> {
        self.0.archive.as_deref()
    }

    /// The name of the member, when the refusal concerns one.
    pub fn member(&self) -> Option<&str> {
        self.0.member.as_deref()
    }

    /// The element, written `section` for an element of the publisher's
    /// namespace and `xhtml:td` for one of XHTML.
    pub fn element(&self) -> Option<&str> {
        self.0.place.as_ref().map(|place| place.element.as_str())
    }

    /// The identifier of the element, or of its nearest ancestor that has
    /// one.
    pub fn identifier(&self) -> Option<&str> {
        self.0
            .place
            .as_ref()
            .and_then(|place| place.identifier.as_deref())
    }

    /// Why the input is refused.
    pub fn message(&self) -> &str {
        &self.0.message
    }
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let refusal = &self.0;
        for name in [&refusal.archive, &refusal.member].into_iter().flatten() {
            write!(f, "{name}: ")?;
        }
        if let Some(place) = &refusal.place {
            write!(f, "line {}, <{}>", place.line, place.element)?;
            match (&place.identifier, place.own) {
                (Some(identifier), true) => write!(f, " {identifier}")?,
                (Some(identifier), false) => write!(f, " in {identifier}")?,
                (None, _) => {}
            }
            f.write_str(": ")?;
        }
        f.write_str(&refusal.message)
    }
}

impl std::error::Error for IngestError {}

/// What to ingest: the titles, by the publisher's designation (`1`, `5a`),
/// or every title when the list is empty; and the number of members read
/// side by side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IngestOptions {
    pub titles: Vec<String>,
    pub jobs: usize,
}

/// What an ingest read: its unit and container modules, the addresses it
/// disambiguated or assigned and the aliases it kept, and what it left out by
/// rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IngestSummary {
    pub units: u64,
    pub containers: u64,
    pub addresses: AddressCounts,
    pub not_imported: NotImported,
}

/// Reads the members of `archives` that hold the titles of `options` and
/// gives each module to `sink`, from as many threads as `options.jobs` says.
/// The modules of one member are given in document order; the order across
/// members is not fixed.
///
/// Addresses are numbered over the whole release: the units and containers
/// of every member are listed first, in the order of the members' names, and
/// only then are modules built. The modules are therefore the same
/// for any number of workers. Each worker holds one document at a time.
pub fn ingest(
    archives: &ReleaseArchives,
    options: &IngestOptions,
    sink: &(dyn Fn(Module) -> Result<(), IngestError> + Sync),
) -> Result<IngestSummary, IngestError> {
    let names = archives.members();
    let selected = archives.select(&options.titles)?;
    let jobs = options.jobs.max(1);
    let release = archives.release();

    let outlines = in_parallel(names.len(), jobs, |index| {
        let xml = archives.read_member(index)?;
        let document = uslm::parse(&xml).map_err(|error| archives.locate(index, error))?;
        let head = uslm::head(&document, release).map_err(|error| archives.locate(index, error))?;
        containers::outline(head.top).map_err(|error| archives.locate(index, error))
    })?;
    let numbering = containers::number(&outlines, &names, &|index, error| {
        archives.locate(index, error)
    })?;

    let read = in_parallel(selected.len(), jobs, |position| {
        let index = selected[position];
        let xml = archives.read_member(index)?;
        let locate = |error| archives.locate(index, error);
        let document = uslm::parse(&xml).map_err(locate)?;
        let head = uslm::head(&document, release).map_err(locate)?;
        let outline = containers::outline(head.top).map_err(locate)?;
        if outline != outlines[index] {
            return Err(locate(IngestError::new(
                "the member reads differently from the first time it was read",
            )));
        }
        let mut give = |module: Module| sink(module);
        containers::build(
            &document,
            &head,
            &outline,
            &numbering.members[index],
            &names[index],
            &mut give,
        )
        .map_err(locate)
    })?;

    let mut summary = IngestSummary::default();
    for (position, built) in read.iter().enumerate() {
        summary.units += built.units;
        summary.containers += built.containers;
        summary.not_imported.add(&built.not_imported);
        summary.addresses.add(&numbering.counts[selected[position]]);
    }
    Ok(summary)
}

/// Reads one document, the member `name` of the release `release`, into its
/// modules in document order, and counts what it leaves out by rule.
/// Addresses are numbered within the document alone.
pub fn ingest_xml(
    name: &str,
    xml: &str,
    release: &str,
) -> Result<(Vec<Module>, NotImported), IngestError> {
    let read = || {
        let document = uslm::parse(xml)?;
        let head = uslm::head(&document, release)?;
        let outline = containers::outline(head.top)?;
        let numbering = containers::number(
            std::slice::from_ref(&outline),
            &[name.to_owned()],
            &|_, error| error,
        )?;
        let mut modules = Vec::new();
        let mut keep = |module: Module| {
            modules.push(module);
            Ok(())
        };
        let built = containers::build(
            &document,
            &head,
            &outline,
            &numbering.members[0],
            name,
            &mut keep,
        )?;
        Ok((modules, built.not_imported))
    };
    read().map_err(|error: IngestError| error.in_member(name))
}

/// The results of `work` for `0..count`, computed on `jobs` threads. When
/// some fail, the error of the first in order is returned, whatever the
/// number of threads: a worker stops taking items past the first failure it
/// knows of, and every item before it is still done.
fn in_parallel<T: Send>(
    count: usize,
    jobs: usize,
    work: impl Fn(usize) -> Result<T, IngestError> + Sync,
) -> Result<Vec<T>, IngestError> {
    let next = AtomicUsize::new(0);
    let first_failure = AtomicUsize::new(usize::MAX);
    let results: Mutex<Vec<Option<Result<T, IngestError>>>> =
        Mutex::new((0..count).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(count.max(1)) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= count || index > first_failure.load(Ordering::SeqCst) {
                        break;
                    }
                    let result = work(index);
                    if result.is_err() {
                        first_failure.fetch_min(index, Ordering::SeqCst);
                    }
                    results
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())[index] = Some(result);
                }
            });
        }
    });
    let results = results
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut done = Vec::with_capacity(count);
    for (index, result) in results.into_iter().enumerate() {
        match result {
            Some(Ok(value)) => done.push(value),
            Some(Err(error)) => return Err(error),
            None => return Err(IngestError::new(format!("item {index} was not read"))),
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_failure_in_order_is_the_one_returned() {
        for jobs in [1, 2, 8] {
            let result = in_parallel(20, jobs, |index| {
                if index % 7 == 3 {
                    Err(IngestError::new(format!("item {index}")))
                } else {
                    Ok(index)
                }
            });
            assert_eq!(result, Err(IngestError::new("item 3")), "{jobs} jobs");
            let result = in_parallel(20, jobs, Ok);
            assert_eq!(result, Ok((0..20).collect::<Vec<_>>()), "{jobs} jobs");
        }
        assert_eq!(in_parallel(0, 4, Ok), Ok(Vec::new()));
    }

    #[test]
    fn an_error_names_its_archive_member_element_and_identifier() {
        let xml = "<a identifier=\"/zz/x\">\n<b>\n<c/></b></a>";
        let document = roxmltree::Document::parse(xml).expect("well-formed");
        let c = document
            .descendants()
            .find(|node| node.has_tag_name("c"))
            .expect("c");
        let error = IngestError::at(c, "refused")
            .in_member("usc99.xml")
            .in_archive("demo.zip")
            .in_member("other.xml");
        assert_eq!(error.archive(), Some("demo.zip"));
        assert_eq!(error.member(), Some("usc99.xml"));
        assert_eq!(error.element(), Some("c"));
        assert_eq!(error.identifier(), Some("/zz/x"));
        assert_eq!(
            error.to_string(),
            "demo.zip: usc99.xml: line 3, <c> in /zz/x: refused"
        );
        let a = document.root_element();
        assert_eq!(
            IngestError::at(a, "refused").to_string(),
            "line 1, <a> /zz/x: refused"
        );
        assert_eq!(IngestError::new("refused").to_string(), "refused");
    }
}
