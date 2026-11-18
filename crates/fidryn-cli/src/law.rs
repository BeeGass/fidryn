//! The commands of law modules: `fidryn ingest`, a generated workspace
//! written from the archives of one release of a corpus, `fidryn text`, the
//! text of the provisions and notes a workspace holds, `fidryn coverage`, the
//! coverage ledger of a generated workspace, and `fidryn changes`, what
//! changed between two releases.
//!
//! `ingest` is the one place of the command line that names the corpus it
//! imports, the United States Code, read from the official XML release point
//! published by the Office of the Law Revision Counsel by
//! `fidryn-ingest-uscode`. What it writes, `fidryn-driver` writes, the
//! coverage ledger last: the command joins the two. `text` names no corpus:
//! it prints the text records of `fidryn_provision::records` of the modules
//! `fidryn-driver` reads, or the same text laid out for a person, or the two
//! hashes a binding to one provision records. Nor does `coverage`, which
//! prints the ledger `fidryn-driver` counts from a workspace's files, nor
//! `changes`, which prints the change set that `fidryn-driver` finds between
//! two generated workspaces.

use clap::Subcommand;
use fidryn_core::Diagnostic;
use fidryn_driver::workspace::changes::compare;
use fidryn_driver::workspace::{
    CheckOptions, CoverageError, GeneratedWriter, WriteError, read_coverage, read_modules,
};
use fidryn_ingest_uscode::{IngestError, IngestOptions, generator, ingest, open};
use fidryn_provision::records::text_records;
use fidryn_provision::workspace::{Manifest, WorkspaceKind};
use fidryn_provision::{
    Address, AddressIndex, Block, DeclSpans, Footnote, Interface, Item, Layer, Module, Note, Num,
    Provision, QuotedItem, Run, Text, bound_hashes, interface_of,
};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The corpora `fidryn ingest` imports.
#[derive(Subcommand, Debug)]
pub enum Corpus {
    /// Write a generated workspace from the archives of one release of the
    /// official XML release point published by the Office of the Law Revision
    /// Counsel.
    Uscode {
        /// The directory of the archives of the release and their retrieval
        /// record, `retrieval.json`.
        archive_dir: PathBuf,
        /// The directory of the workspace to write.
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        /// A title to write, by the publisher's designation (`1`, `5a`); may
        /// be given more than once. By default every title of the release.
        #[arg(long = "title", value_name = "T")]
        titles: Vec<String>,
        /// Worker threads (default: the number available).
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
        jobs: Option<u32>,
        /// Write nothing; print each path under DIR that differs from what the
        /// run would write, every entry a run would refuse among them, and exit
        /// 1 when there is one.
        #[arg(long)]
        verify: bool,
    },
}

/// The name of the workspace of the Code.
const NAME: &str = "uscode";

/// Runs `fidryn ingest`.
pub(crate) fn run(corpus: Corpus) -> ExitCode {
    match corpus {
        Corpus::Uscode {
            archive_dir,
            out,
            titles,
            jobs,
            verify,
        } => uscode(&archive_dir, &out, titles, jobs, verify),
    }
}

/// `fidryn ingest uscode ARCHIVE_DIR --out DIR`: opens the release in
/// `archive_dir`, checking each archive against its retrieval record, and
/// writes the generated workspace of `titles`, or of every title, into
/// `out` on `jobs` threads; then prints one line of what it wrote. With
/// `verify` it writes nothing, prints each path under `out` that differs
/// from what it would write with the word that says how, and exits 1 when
/// there is one; when there is none it prints `ok`.
fn uscode(
    archive_dir: &Path,
    out: &Path,
    titles: Vec<String>,
    jobs: Option<u32>,
    verify: bool,
) -> ExitCode {
    let archives = match open(archive_dir) {
        Ok(archives) => archives,
        Err(error) => return super::fail(error.to_string()),
    };
    // A title that no member holds is refused before anything is written.
    let artifacts = match archives.artifacts_of(&titles) {
        Ok(artifacts) => artifacts,
        Err(error) => return super::fail(error.to_string()),
    };
    let prefix = match Address::parse(fidryn_ingest_uscode::PREFIX) {
        Ok(prefix) => prefix,
        Err(error) => return super::fail(error.to_string()),
    };
    let manifest = Manifest {
        name: NAME.to_owned(),
        kind: WorkspaceKind::Generated,
        prefix,
        release: Some(archives.release().to_owned()),
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        dependencies: Vec::new(),
    };
    let writer = if verify {
        GeneratedWriter::compare(out, manifest)
    } else {
        GeneratedWriter::create(out, manifest)
    };
    let writer = match writer {
        Ok(writer) => writer,
        Err(error) => return super::fail(error.to_string()),
    };
    let jobs = match jobs {
        Some(jobs) => usize::try_from(jobs).unwrap_or(usize::MAX),
        None => std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
    };
    let options = IngestOptions { titles, jobs };
    let summary = match ingest(&archives, &options, &|module| {
        writer
            .write(&module)
            .map_err(|error| IngestError::new(error.to_string()))
    }) {
        Ok(summary) => summary,
        Err(error) => return super::fail(error.to_string()),
    };
    let read = artifacts.len();
    match writer.finish(
        generator(),
        artifacts,
        summary.addresses,
        summary.not_imported,
    ) {
        Ok(_) if verify => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Ok(_) => {
            println!(
                "ingested: {} units, {} containers from {read} archives (release {})",
                summary.units,
                summary.containers,
                archives.release()
            );
            ExitCode::SUCCESS
        }
        Err(WriteError::Differs { differences, .. }) => {
            for (path, difference) in differences {
                println!("{} {path}", difference.word());
            }
            ExitCode::from(1)
        }
        Err(error) => super::fail(error.to_string()),
    }
}

/// `fidryn text DIR [ADDRESS] [--json]`: prints to stdout what
/// [`write_text`] writes. Exit 1, with the reason on stderr and nothing on
/// stdout, when the workspace cannot be read or `address` is not in it.
pub(crate) fn text(dir: &Path, address: Option<&str>, json: bool) -> ExitCode {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    match write_text(dir, address, json, &mut out) {
        Ok(()) => match out.flush() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => super::fail(format!("cannot write the text: {error}")),
        },
        Err(message) => super::fail(message),
    }
}

/// Writes to `out` the text of the provisions and notes of the workspace in
/// `dir`, or, with `address`, of the provision, container or note at it and
/// of what lies below it: every record whose address is `address` or
/// continues it at a segment boundary.
///
/// With `json`, each record that [`text_records`] gives of a module, one
/// compact line of JSON, `fidryn.text-record/v0.1`, to a line, sorted by the
/// bytes of their addresses across the whole workspace; for one address in
/// the order of its module's records. Without it, the same text laid out for
/// a person, module by module in the order of their addresses, with a blank
/// line between two modules. Everything is read and chosen before anything
/// is written, so an error writes nothing: a workspace that cannot be read,
/// a text that is no address, and an address at which the workspace holds
/// no provision, container or note.
pub fn write_text(
    dir: &Path,
    address: Option<&str>,
    json: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let mut modules =
        read_modules(dir, jobs).map_err(|diagnostics| diagnostics_under(dir, &diagnostics))?;
    let at = match address {
        None => None,
        Some(text) => {
            let at =
                Address::parse(text).map_err(|error| format!("`{text}` is no address: {error}"))?;
            if !modules.iter().any(|module| holds(module, &at)) {
                return Err(format!(
                    "the workspace holds no provision, container or note at `{at}`"
                ));
            }
            Some(at)
        }
    };
    let written = if json {
        // Each module is dropped once its records are written as lines.
        let mut lines: Vec<(Address, String)> = Vec::new();
        for module in modules {
            for record in text_records(&module) {
                if below(record.address(), at.as_ref()) {
                    lines.push((record.address().clone(), record.to_json()));
                }
            }
        }
        // A stable sort: the records of one address keep their module's order.
        lines.sort_by(|a, b| a.0.cmp(&b.0));
        lines
            .iter()
            .try_for_each(|(_, line)| writeln!(out, "{line}"))
    } else {
        modules.sort_by(|a, b| module_address(a).cmp(module_address(b)));
        let mut first = true;
        modules.iter().try_for_each(|module| {
            let page = for_a_person(module, at.as_ref());
            if page.is_empty() {
                return Ok(());
            }
            if !std::mem::replace(&mut first, false) {
                writeln!(out)?;
            }
            out.write_all(page.as_bytes())
        })
    };
    written.map_err(|error| format!("cannot write the text: {error}"))
}

/// `fidryn coverage DIR`: prints to stdout what [`write_coverage`] writes.
/// Exit 1, with the reason on stderr and nothing on stdout, when the
/// workspace is curated or cannot be read.
pub(crate) fn coverage(dir: &Path) -> ExitCode {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    match write_coverage(dir, &mut out) {
        Ok(()) => match out.flush() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => super::fail(format!("cannot write the ledger: {error}")),
        },
        Err(message) => super::fail(message),
    }
}

/// Writes to `out` the coverage ledger of the generated workspace in `dir`,
/// `fidryn.coverage/v0.1`, counted from its modules as their files stand,
/// its manifest and its lock: what its writer wrote as `coverage.json`, when
/// no file was changed since. The modules are read and nothing else of the
/// workspace is checked; the module files are not held against the lock,
/// which `fidryn check DIR` does. Nothing is written when the workspace is
/// curated, since a ledger is that of a generated workspace, or when it
/// cannot be read: no manifest, no lock or one that cannot be read or that
/// disagrees with the manifest, or a module file that does not lower.
pub fn write_coverage(dir: &Path, out: &mut dyn Write) -> Result<(), String> {
    let jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let ledger = read_coverage(dir, jobs).map_err(|error| match error {
        CoverageError::Curated => format!(
            "{} holds a curated workspace, and the coverage ledger belongs to a generated \
             workspace, whose lock records what its importer counted",
            dir.display()
        ),
        CoverageError::Unread(diagnostics) => diagnostics_under(dir, &diagnostics),
    })?;
    out.write_all(ledger.to_json().as_bytes())
        .map_err(|error| format!("cannot write the ledger: {error}"))
}

/// `fidryn text DIR ADDRESS --hashes`: prints to stdout what
/// [`write_hashes`] writes. Exit 1, with the reason on stderr and nothing on
/// stdout, when the workspace cannot be read or `address` is not what a
/// binding binds.
pub(crate) fn hashes(dir: &Path, address: &str) -> ExitCode {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    match write_hashes(dir, address, &mut out) {
        Ok(()) => match out.flush() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => super::fail(format!("cannot write the hashes: {error}")),
        },
        Err(message) => super::fail(message),
    }
}

/// Writes to `out` the two lines that a binding to `address` records when it
/// is written against the workspace in `dir`, as a person writes them into
/// the binding:
///
/// ```text
/// provision_hash "<64 hexadecimal digits>"
/// notes_hash "<64 hexadecimal digits>"
/// ```
///
/// They are what [`bound_hashes`] gives from the interfaces of the modules,
/// the place the check of a workspace takes them from, so a binding that
/// records them is not stale against this workspace: the tree hash of the
/// provision at `address`, or of the top provision of the unit `address` is
/// an alias of, and the enacted-layer hash of its unit. Any other address,
/// that of a note, of a container or of nothing, is refused, saying what it
/// is, and nothing is written; so are a workspace that cannot be read and a
/// text that is no address.
pub fn write_hashes(dir: &Path, address: &str, out: &mut dyn Write) -> Result<(), String> {
    let jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let modules =
        read_modules(dir, jobs).map_err(|diagnostics| diagnostics_under(dir, &diagnostics))?;
    let at =
        Address::parse(address).map_err(|error| format!("`{address}` is no address: {error}"))?;
    let interfaces: Vec<Interface> = modules
        .iter()
        .map(|module| interface_of(module, &DeclSpans::default()))
        .collect();
    let index = AddressIndex::build(&interfaces, &[]);
    let Some(hashes) = bound_hashes(&at, &index) else {
        return Err(unbound(&modules, &at));
    };
    write!(
        out,
        "provision_hash \"{}\"\nnotes_hash \"{}\"\n",
        hashes.provision_hash, hashes.notes_hash
    )
    .map_err(|error| format!("cannot write the hashes: {error}"))
}

/// `fidryn changes OLD_DIR NEW_DIR [--bindings DIR]... [--json]`: prints the
/// change set from the generated workspace in `old` to the one in `new`,
/// with the bindings of the curated workspaces in `bindings` that are stale
/// against `new`, as JSON with `json` and otherwise as the text of a commit
/// message. Every workspace is read as `fidryn check` reads it, on `--jobs`
/// threads and through the cache of `--cache-dir`, else of
/// `$FIDRYN_CACHE_DIR`, else of `.fidryn-cache` inside each workspace;
/// `--no-cache` keeps none. When the cache could not be used or written, it
/// says so once on stderr, as `fidryn check` does, and the change set is the
/// same. Exit 1, with the diagnostics on stderr and nothing on stdout, when
/// a directory is not what the command compares or a workspace has an
/// error.
pub(crate) fn changes(
    old: &Path,
    new: &Path,
    bindings: &[PathBuf],
    json: bool,
    flags: &super::WorkspaceFlags,
) -> ExitCode {
    let jobs = match flags.jobs {
        Some(jobs) => usize::try_from(jobs).unwrap_or(usize::MAX),
        None => std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
    };
    let mut cache = changes_cache(flags, std::env::var_os(super::CACHE_DIR_VARIABLE));
    let mut executable = String::new();
    let mut unused = None;
    if cache.is_some() {
        match super::executable_digest() {
            Ok(digest) => executable = digest,
            Err(error) => {
                cache = None;
                unused = Some(error);
            }
        }
    }
    let options = CheckOptions {
        jobs,
        cache_dir: cache,
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        executable,
    };
    let compared = match compare(old, new, bindings, &options) {
        Ok(compared) => compared,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            return ExitCode::from(1);
        }
    };
    // What `fidryn check` says of its cache, once.
    if let Some(error) = unused.as_ref().or(compared.cache_unused.as_ref()) {
        eprintln!("the cache could not be used, and the check went on without it: {error}");
    }
    if let Some(failure) = &compared.cache_failure {
        eprintln!("the cache could not be written, and the check went on without it: {failure}");
    }
    let set = compared.set;
    let text = if json { set.to_json() } else { set.to_text() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => super::fail(format!("cannot write the change set: {error}")),
    }
}

/// The cache of every workspace that `fidryn changes` reads: none with
/// `--no-cache`; otherwise `--cache-dir`, else `variable`, the value of
/// `FIDRYN_CACHE_DIR` when it is set and not empty, made absolute, which
/// every workspace shares; else `.fidryn-cache`, a relative path, which
/// [`compare`] takes inside the directory of each workspace.
fn changes_cache(flags: &super::WorkspaceFlags, variable: Option<OsString>) -> Option<PathBuf> {
    if flags.no_cache {
        return None;
    }
    let named = flags.cache_dir.clone().or_else(|| {
        variable
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    });
    Some(match named {
        Some(dir) => std::path::absolute(&dir).unwrap_or(dir),
        None => PathBuf::from(super::DEFAULT_CACHE_DIR),
    })
}

/// What `at` is in `modules`, where it is no provision and no alias of a
/// unit, as the reason `--hashes` refuses it. An address is looked for
/// before an alias, as the index of the check looks.
fn unbound(modules: &[Module], at: &Address) -> String {
    let notes = |module: &Module| match module {
        Module::Unit(unit) => unit.notes.iter().any(|note| note.address == *at),
        Module::Container(container) => container.notes.iter().any(|note| note.address == *at),
        Module::Curated(_) => false,
    };
    let what = if modules.iter().any(notes) {
        format!("`{at}` is the address of a note")
    } else if let Some(module) = modules.iter().find(|module| module_address(module) == at) {
        let shape = match module {
            Module::Unit(_) => "a unit",
            Module::Container(_) => "a container",
            Module::Curated(_) => "a curated module",
        };
        format!("`{at}` is the address of {shape}")
    } else if let Some(container) = modules.iter().find_map(|module| match module {
        Module::Container(container) if container.aliases.contains(at) => Some(container),
        _ => None,
    }) {
        format!(
            "`{at}` is an alias of the container `{}`",
            container.address
        )
    } else {
        return format!("the workspace holds no provision, container or note at `{at}`");
    };
    format!(
        "{what}, and `--hashes` prints the hashes of a provision, or of a unit named by an alias"
    )
}

/// The diagnostics of a workspace, one to a line, each naming its file under
/// `dir`.
fn diagnostics_under(dir: &Path, diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let mut shown = diagnostic.clone();
            shown.source_file = diagnostic
                .source_file
                .as_ref()
                .map(|file| dir.join(file).display().to_string());
            shown.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `address` is `at`, or lies below it at a segment boundary; any
/// address when there is no `at`.
fn below(address: &Address, at: Option<&Address>) -> bool {
    at.is_none_or(|at| address.is_within(at))
}

fn module_address(module: &Module) -> &Address {
    match module {
        Module::Unit(unit) => &unit.address,
        Module::Container(container) => &container.address,
        Module::Curated(curated) => &curated.address,
    }
}

/// Whether `module` holds a provision, a container or a note at `at`.
fn holds(module: &Module, at: &Address) -> bool {
    fn in_tree(provision: &Provision, at: &Address) -> bool {
        provision.address == *at
            || provision.items.iter().any(|item| match item {
                Item::Child(child) => in_tree(child, at),
                Item::Part { .. } => false,
            })
    }
    let noted = |notes: &[Note]| notes.iter().any(|note| note.address == *at);
    match module {
        Module::Unit(unit) => in_tree(&unit.root, at) || noted(&unit.notes),
        Module::Container(container) => container.address == *at || noted(&container.notes),
        Module::Curated(_) => false,
    }
}

/// The text of `module` for a person, or of what of it lies below `at`; empty
/// when nothing does. Each provision is its address on a line of its own,
/// then its number and heading on one line, the blocks of its text parts and
/// its children, each indented by two spaces for each level of depth; the
/// provision printed first is at depth 0. After the provisions come the
/// source credit and the footnotes, at depth 1, when the module itself is
/// printed, and then the notes, each its address with its layer, its heading
/// and its blocks. A container is its address, its number and heading,
/// credit, footnotes and notes. A status follows its address in
/// parentheses, a footnote marker is its shown text in brackets, a table is
/// one line for each row with its cells separated by ` | `, the items of
/// quoted content are indented one level further, and an image is
/// `[image SOURCE]`.
fn for_a_person(module: &Module, at: Option<&Address>) -> String {
    let mut page = Page::default();
    let (whole, credit, footnotes, notes) = match module {
        Module::Unit(unit) => {
            page.chosen(&unit.root, at);
            (
                below(&unit.address, at),
                unit.credit.as_ref(),
                &unit.footnotes[..],
                &unit.notes[..],
            )
        }
        Module::Container(container) => {
            let whole = below(&container.address, at);
            if whole {
                page.line(
                    0,
                    &with_status(&container.address, container.status.as_deref()),
                );
                page.label(1, container.num.as_ref(), container.heading.as_ref());
            }
            (
                whole,
                container.credit.as_ref(),
                &container.footnotes[..],
                &container.notes[..],
            )
        }
        Module::Curated(_) => return String::new(),
    };
    if whole {
        if let Some(credit) = credit {
            page.line(1, &shown(credit));
        }
        for footnote in footnotes {
            page.footnote(footnote, 1);
        }
    }
    for note in notes.iter().filter(|note| below(&note.address, at)) {
        let layer = match note.layer {
            Layer::Operative => "operative",
            Layer::Enacted => "enacted",
            Layer::Editorial => "editorial",
        };
        page.line(0, &format!("{} ({layer})", note.address));
        if let Some(heading) = &note.heading {
            page.line(1, &shown(heading));
        }
        page.blocks(&note.blocks, 1);
    }
    page.text()
}

/// The lines of a text for a person, each with its depth.
#[derive(Default)]
struct Page {
    lines: Vec<(usize, String)>,
}

impl Page {
    /// Adds `text` at `depth`; a text with nothing but white space adds no
    /// line.
    fn line(&mut self, depth: usize, text: &str) {
        let text = text.trim_end();
        if !text.trim_start().is_empty() {
            self.lines.push((depth, text.to_owned()));
        }
    }

    /// Every line, indented, each with its line end.
    fn text(&self) -> String {
        let mut text = String::new();
        for (depth, line) in &self.lines {
            for _ in 0..*depth {
                text.push_str("  ");
            }
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    /// The provisions of the tree of `provision` that lie below `at`, each
    /// from depth 0 with what lies below it.
    fn chosen(&mut self, provision: &Provision, at: Option<&Address>) {
        if below(&provision.address, at) {
            self.provision(provision, 0);
            return;
        }
        for item in &provision.items {
            if let Item::Child(child) = item {
                self.chosen(child, at);
            }
        }
    }

    fn provision(&mut self, provision: &Provision, depth: usize) {
        self.line(
            depth,
            &with_status(&provision.address, provision.status.as_deref()),
        );
        self.label(
            depth + 1,
            provision.num.as_ref(),
            provision.heading.as_ref(),
        );
        for item in &provision.items {
            match item {
                Item::Part { blocks, .. } => self.blocks(blocks, depth + 1),
                Item::Child(child) => self.provision(child, depth + 1),
            }
        }
    }

    /// The number and the heading, on one line.
    fn label(&mut self, depth: usize, num: Option<&Num>, heading: Option<&Text>) {
        let parts: Vec<String> = num
            .map(|num| shown(&num.shown))
            .into_iter()
            .chain(heading.map(shown))
            .filter(|part| !part.is_empty())
            .collect();
        self.line(depth, &parts.join(" "));
    }

    fn blocks(&mut self, blocks: &[Block], depth: usize) {
        for block in blocks {
            self.block(block, depth);
        }
    }

    fn block(&mut self, block: &Block, depth: usize) {
        match block {
            Block::Para(text) | Block::Subheading(text) => self.line(depth, &shown(text)),
            Block::Table(table) => {
                for row in &table.rows {
                    let cells: Vec<String> =
                        row.cells.iter().map(|cell| flat(&cell.blocks)).collect();
                    self.line(depth, &cells.join(" | "));
                }
            }
            Block::Quoted(quoted) => self.quoted(&quoted.items, depth + 1),
            Block::Image { source } => self.line(depth, &format!("[image {source}]")),
        }
    }

    fn quoted(&mut self, items: &[QuotedItem], depth: usize) {
        for item in items {
            match item {
                QuotedItem::Block(block) => self.block(block, depth),
                QuotedItem::Part { blocks, .. } => self.blocks(blocks, depth),
                QuotedItem::Level(level) => {
                    self.label(depth, level.num.as_ref(), level.heading.as_ref());
                    self.quoted(&level.items, depth + 1);
                }
            }
        }
    }

    /// A footnote: its blocks, the first preceded by its marker in brackets.
    fn footnote(&mut self, footnote: &Footnote, depth: usize) {
        let marker = format!("[{}]", footnote.shown);
        let first = self.lines.len();
        self.blocks(&footnote.blocks, depth);
        match self.lines.get_mut(first) {
            Some((_, line)) => *line = format!("{marker} {line}"),
            None => self.line(depth, &marker),
        }
    }
}

/// The blocks of a table cell on one line.
fn flat(blocks: &[Block]) -> String {
    let mut page = Page::default();
    page.blocks(blocks, 0);
    page.lines
        .into_iter()
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join(" ")
}

/// An address, followed by its status in parentheses when it has one.
fn with_status(address: &Address, status: Option<&str>) -> String {
    match status {
        Some(status) => format!("{address} ({status})"),
        None => address.to_string(),
    }
}

/// A text for a person: its words, each footnote marker as its shown text in
/// brackets.
fn shown(text: &Text) -> String {
    let mut shown = String::new();
    for run in &text.runs {
        match run {
            Run::Words(words) | Run::Sup(words) | Run::Sub(words) | Run::Ref { words, .. } => {
                shown.push_str(words);
            }
            Run::Mark { shown: marker, .. } => {
                shown.push('[');
                shown.push_str(marker);
                shown.push(']');
            }
        }
    }
    shown
}
