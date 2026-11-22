//! The canonical text of a module.
//!
//! [`render`] writes a module of edition 2 in its canonical form, the form
//! `docs/law-modules.md` gives under "Syntax": `\n` line ends and a final
//! one, two spaces of indentation for each open brace, one field to a line,
//! the declarations in a fixed order, and every hash computed from what is
//! written above it.

use crate::address::Address;
use crate::hash::{
    Hash32, LayerHashes, hash_container, hash_note, layer_hashes_of_container,
    layer_hashes_of_unit, tree_hashes,
};
use crate::model::{
    Basis, BindTarget, Block, Cell, ContainerModule, CuratedModule, Definition, DefinitionKind,
    Footnote, Item, Layer, Module, Note, Num, PartRole, Provision, Quoted, QuotedItem, RowGroup,
    Run, Text, UnitModule,
};
use std::fmt::Write as _;

/// The canonical text of `module`.
///
/// A unit and a container are written with every `hash` and the three values
/// of `layers` computed from the model. A curated module is written with its
/// imports, its citations and its bindings, each in the order of its list.
/// The footnotes of a module are written by ordinal and its definitions in
/// the document order of the provisions they name, then by their first term;
/// every other list is written in its own order. Kinds, status words, groups,
/// names and aliases are written as the identifiers they are.
pub fn render(module: &Module) -> String {
    let mut out = Writer::default();
    out.line("edition 2");
    match module {
        Module::Unit(unit) => out.unit(unit),
        Module::Container(container) => out.container(container),
        Module::Curated(curated) => out.curated(curated),
    }
    out.text
}

#[derive(Default)]
struct Writer {
    text: String,
    depth: usize,
}

impl Writer {
    fn line(&mut self, line: &str) {
        for _ in 0..self.depth {
            self.text.push_str("  ");
        }
        self.text.push_str(line);
        self.text.push('\n');
    }

    /// The line `head {`, and the lines after it one level deeper.
    fn open(&mut self, head: &str) {
        self.line(&format!("{head} {{"));
        self.depth += 1;
    }

    fn close(&mut self) {
        self.depth -= 1;
        self.line("}");
    }

    fn unit(&mut self, unit: &UnitModule) {
        self.open(&format!("module {}", string(unit.address.as_str())));
        self.places(&Places {
            aliases: &unit.aliases,
            within: unit.within.as_ref(),
            source: &unit.source,
            positive_law: unit.positive_law,
        });
        let hashes = tree_hashes(&unit.root);
        self.provision(&unit.root, &mut hashes.iter());
        self.matter(unit.credit.as_ref(), &unit.notes, &unit.footnotes);
        for definition in definitions_in_order(unit) {
            self.definition(definition);
        }
        self.layers(&layer_hashes_of_unit(unit));
        self.close();
    }

    fn container(&mut self, container: &ContainerModule) {
        self.open(&format!("module {}", string(container.address.as_str())));
        self.places(&Places {
            aliases: &container.aliases,
            within: container.within.as_ref(),
            source: &container.source,
            positive_law: container.positive_law,
        });
        self.open(&format!(
            "container {} {}",
            string(container.address.as_str()),
            container.kind
        ));
        self.heading_fields(
            container.num.as_ref(),
            container.heading.as_ref(),
            container.status.as_deref(),
        );
        for member in &container.members {
            self.line(&format!("member {}", string(member.as_str())));
        }
        self.hash("hash", hash_container(container));
        self.close();
        self.matter(
            container.credit.as_ref(),
            &container.notes,
            &container.footnotes,
        );
        self.layers(&layer_hashes_of_container(container));
        self.close();
    }

    fn curated(&mut self, curated: &CuratedModule) {
        self.open(&format!("module {}", string(curated.address.as_str())));
        for import in &curated.imports {
            let module = string(import.module.as_str());
            match &import.alias {
                Some(alias) => self.line(&format!("import {module} as {alias}")),
                None => self.line(&format!(
                    "import {module} {{ {} }}",
                    import.names.join(", ")
                )),
            }
        }
        for citation in &curated.citations {
            self.line(&format!(
                "citation {} -> {}",
                citation.name,
                string(citation.target.as_str())
            ));
        }
        for binding in &curated.bindings {
            self.open(&format!("binding {}", binding.name));
            let binds = match &binding.binds {
                BindTarget::Address(address) => string(address.as_str()),
                BindTarget::Name {
                    module: Some(module),
                    name,
                } => format!("{module}.{name}"),
                BindTarget::Name { module: None, name } => name.clone(),
            };
            self.line(&format!("binds {binds}"));
            self.line(&format!("release {}", string(&binding.release)));
            self.hash("provision_hash", binding.provision_hash);
            self.hash("notes_hash", binding.notes_hash);
            self.line("pending");
            self.close();
        }
        self.close();
    }

    /// The declarations a unit and a container both have before their
    /// provision or container.
    fn places(&mut self, places: &Places<'_>) {
        for alias in places.aliases {
            self.line(&format!("alias {}", string(alias.as_str())));
        }
        if let Some(within) = places.within {
            self.line(&format!("within {}", string(within.as_str())));
        }
        self.line(&format!("source {}", string(places.source)));
        if let Some(positive_law) = places.positive_law {
            self.line(&format!("positive_law {positive_law}"));
        }
    }

    /// The credit, the notes and the footnotes of a unit or a container.
    fn matter(&mut self, credit: Option<&Text>, notes: &[Note], footnotes: &[Footnote]) {
        if let Some(credit) = credit {
            self.line(&format!("credit {}", text(credit)));
        }
        for note in notes {
            self.note(note);
        }
        let mut footnotes: Vec<&Footnote> = footnotes.iter().collect();
        footnotes.sort_by_key(|footnote| footnote.ordinal);
        for footnote in footnotes {
            self.open(&format!(
                "footnote {} {}",
                footnote.ordinal,
                string(&footnote.shown)
            ));
            self.blocks(&footnote.blocks);
            self.close();
        }
    }

    fn heading_fields(&mut self, num: Option<&Num>, heading: Option<&Text>, status: Option<&str>) {
        if let Some(num) = num {
            self.num(num);
        }
        if let Some(heading) = heading {
            self.line(&format!("heading {}", text(heading)));
        }
        if let Some(status) = status {
            self.line(&format!("status {status}"));
        }
    }

    fn num(&mut self, num: &Num) {
        self.line(&format!("num {} {}", string(&num.value), text(&num.shown)));
    }

    fn hash(&mut self, field: &str, hash: Hash32) {
        self.line(&format!("{field} {}", string(&hash.to_string())));
    }

    /// `provision` and the tree below it. `hashes` yields the tree hash of
    /// each provision of the tree in document order.
    fn provision<'a>(
        &mut self,
        provision: &Provision,
        hashes: &mut impl Iterator<Item = &'a Hash32>,
    ) {
        let hash = *hashes
            .next()
            .expect("one tree hash for each provision of the tree");
        self.open(&format!(
            "provision {} {}",
            string(provision.address.as_str()),
            provision.kind
        ));
        self.heading_fields(
            provision.num.as_ref(),
            provision.heading.as_ref(),
            provision.status.as_deref(),
        );
        for item in &provision.items {
            match item {
                Item::Part { role, blocks } => self.part(*role, blocks),
                Item::Child(child) => self.provision(child, hashes),
            }
        }
        self.hash("hash", hash);
        self.close();
    }

    /// A text part: in the short form when it is exactly one paragraph.
    fn part(&mut self, role: PartRole, blocks: &[Block]) {
        let role = match role {
            PartRole::Chapeau => "chapeau",
            PartRole::Content => "content",
            PartRole::Continuation => "continuation",
            PartRole::Proviso => "proviso",
        };
        if let [Block::Para(paragraph)] = blocks {
            self.line(&format!("{role} {}", text(paragraph)));
        } else {
            self.open(role);
            self.blocks(blocks);
            self.close();
        }
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.block(block);
        }
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Para(paragraph) => self.line(&format!("p {}", text(paragraph))),
            Block::Subheading(subheading) => {
                self.line(&format!("subheading {}", text(subheading)));
            }
            Block::Table(table) => {
                self.open("table");
                for row in &table.rows {
                    self.open(match row.group {
                        RowGroup::Head => "head",
                        RowGroup::Body => "body",
                        RowGroup::Foot => "foot",
                    });
                    for cell in &row.cells {
                        self.cell(cell);
                    }
                    self.close();
                }
                self.close();
            }
            Block::Quoted(quoted) => self.quoted(quoted),
            Block::Image { source } => self.line(&format!("image {}", string(source))),
        }
    }

    /// A cell: `td ""` when it holds no block, in the short form when it is
    /// exactly one paragraph.
    fn cell(&mut self, cell: &Cell) {
        let mut head = String::from(if cell.header { "th" } else { "td" });
        if cell.colspan != 1 || cell.rowspan != 1 {
            let _ = write!(head, " span {} {}", cell.colspan, cell.rowspan);
        }
        match cell.blocks.as_slice() {
            [] => self.line(&format!("{head} \"\"")),
            [Block::Para(paragraph)] => self.line(&format!("{head} {}", text(paragraph))),
            blocks => {
                self.open(&head);
                self.blocks(blocks);
                self.close();
            }
        }
    }

    fn quoted(&mut self, quoted: &Quoted) {
        match &quoted.origin {
            Some(origin) => self.open(&format!("quoted {}", string(origin))),
            None => self.open("quoted"),
        }
        self.quoted_items(&quoted.items);
        self.close();
    }

    fn quoted_items(&mut self, items: &[QuotedItem]) {
        for item in items {
            match item {
                QuotedItem::Block(block) => self.block(block),
                QuotedItem::Part { role, blocks } => self.part(*role, blocks),
                QuotedItem::Level(level) => {
                    self.open(&format!("level {}", level.kind));
                    self.heading_fields(level.num.as_ref(), level.heading.as_ref(), None);
                    self.quoted_items(&level.items);
                    self.close();
                }
            }
        }
    }

    fn note(&mut self, note: &Note) {
        let layer = match note.layer {
            Layer::Enacted => "enacted",
            Layer::Editorial => "editorial",
            Layer::Operative => "operative",
        };
        self.open(&format!("note {} {layer}", string(note.address.as_str())));
        self.line(&format!("group {}", note.group));
        for topic in &note.topics {
            self.line(&format!("topic {}", string(topic)));
        }
        if let Some(heading) = &note.heading {
            self.line(&format!("heading {}", text(heading)));
        }
        self.blocks(&note.blocks);
        self.hash("hash", hash_note(note));
        self.close();
    }

    fn definition(&mut self, definition: &Definition) {
        self.open(&format!("definition {}", string(definition.at.as_str())));
        for term in &definition.terms {
            self.line(&format!("term {}", string(term)));
        }
        self.line(&format!(
            "kind {}",
            match definition.kind {
                DefinitionKind::Means => "means",
                DefinitionKind::Includes => "includes",
                DefinitionKind::DoesNotInclude => "does_not_include",
                DefinitionKind::Borrowed => "borrowed",
            }
        ));
        match &definition.words {
            Some(words) => self.line(&format!("words {}", string(words))),
            None => self.line("words unstated"),
        }
        if let Some(region) = &definition.region {
            self.line(&format!("region {}", string(region.as_str())));
        }
        self.line(&format!(
            "basis {}",
            match definition.basis {
                Basis::Enacted => "enacted",
                Basis::Restated => "restated",
                Basis::Unresolved => "unresolved",
            }
        ));
        if let Some(borrowed_from) = &definition.borrowed_from {
            self.line(&format!("borrowed_from {}", string(borrowed_from)));
        }
        if let Some(see_note) = &definition.see_note {
            self.line(&format!("see_note {}", string(see_note.as_str())));
        }
        self.close();
    }

    fn layers(&mut self, layers: &LayerHashes) {
        self.open("layers");
        self.hash("operative", layers.operative);
        self.hash("enacted", layers.enacted);
        self.hash("editorial", layers.editorial);
        self.close();
    }
}

/// What a unit and a container record of where they come from.
struct Places<'a> {
    aliases: &'a [Address],
    within: Option<&'a Address>,
    source: &'a str,
    positive_law: Option<bool>,
}

/// The definitions of `unit` in the document order of the provisions they
/// name, then by their first term. A definition that names no provision of
/// the unit comes after the others.
fn definitions_in_order(unit: &UnitModule) -> Vec<&Definition> {
    fn walk<'a>(provision: &'a Provision, out: &mut Vec<&'a Address>) {
        out.push(&provision.address);
        for item in &provision.items {
            if let Item::Child(child) = item {
                walk(child, out);
            }
        }
    }
    let mut order = Vec::new();
    walk(&unit.root, &mut order);
    let position = |definition: &Definition| {
        order
            .iter()
            .position(|address| **address == definition.at)
            .unwrap_or(order.len())
    };
    let mut definitions: Vec<&Definition> = unit.definitions.iter().collect();
    definitions.sort_by(|a, b| {
        position(a)
            .cmp(&position(b))
            .then_with(|| a.terms.first().cmp(&b.terms.first()))
    });
    definitions
}

/// `value` as a string literal: `\\` for a backslash, `\"` for a quotation
/// mark, `\u00XX` for a character below U+0020 or U+007F, and every other
/// character as itself.
fn string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\u{0}'..='\u{1F}' | '\u{7F}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(character));
            }
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

/// `text` on one line, its runs separated by one space. A text with no run is
/// the empty string.
fn text(text: &Text) -> String {
    if text.runs.is_empty() {
        return String::from("\"\"");
    }
    let runs: Vec<String> = text
        .runs
        .iter()
        .map(|run| match run {
            Run::Words(words) => string(words),
            Run::Ref { words, target } => format!("[{} -> {}]", string(words), string(target)),
            Run::Sup(words) => format!("sup {}", string(words)),
            Run::Sub(words) => format!("sub {}", string(words)),
            Run::Mark { shown, footnote } => format!("mark {} {footnote}", string(shown)),
        })
        .collect();
    runs.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_string_escapes_what_json_requires_and_the_delete_character() {
        assert_eq!(string("plain “quoted” — §"), "\"plain “quoted” — §\"");
        assert_eq!(string("a\\b\"c"), "\"a\\\\b\\\"c\"");
        assert_eq!(
            string("\u{0}\u{1F}\u{7F}\u{80}"),
            "\"\\u0000\\u001f\\u007f\u{80}\""
        );
        for value in [
            "",
            "a\\b\"c",
            "\u{0}\u{1F}\u{7F}\u{80}\u{2028}",
            "\u{10348}",
        ] {
            let decoded: String = serde_json::from_str(&string(value)).expect("a JSON string");
            assert_eq!(decoded, value);
        }
    }
}
