//! Containers, the outline of a document, and the addresses of units and
//! containers.
//!
//! The containers are `title`, `appendix`, `subtitle`, `chapter`,
//! `subchapter`, `part`, `subpart`, `division`, `subdivision`, `article`,
//! `courtRules`, `compiledAct`, `reorganizationPlans`, and a `level` with no
//! unit above it and neither a text part nor a `p` as a child. The top
//! container of a document is the `title` inside `main` or the `appendix`
//! under `uscDoc`. A container's members are the containers and units that
//! are its children, in document order.
//!
//! The outline of a document lists its units and containers in document
//! order with their identifiers, and the addresses of the provisions of each
//! unit relative to it. Numbering the outlines of a release, in the order of
//! the members' names, gives each unit and container its address and its
//! aliases:
//!
//! 1. An identifier that holds a space is a list: its first member is the
//!    identifier, the others are aliases. An identifier whose last segment
//!    holds `...` stands for a range: its aliases are its two ends, and every
//!    whole number between ends that differ only in a whole number at their
//!    end, at most 999 apart. The address is the identifier, followed by
//!    `~2`, `~3` for the later holders of one identifier in the release.
//! 2. An element without an identifier gets the segment `~<tag>.<n>` below
//!    the container it stands in, or below the prefix of the Code for the top
//!    element of a document, where `n` counts the elements of its tag without
//!    an identifier in that place; the top elements are counted over the
//!    release.
//! 3. An alias that is the address of a unit, a container or a provision is
//!    dropped, and so is one the element already has.
//!
//! An identifier that holds `~`, the character only the reading adds, is
//! refused, and so is a release in which two elements would end with one
//! address. Before the outline is made, a document nested deeper than
//! [`MAX_NESTING`] in a module, or in its containers, is refused.

use crate::definitions;
use crate::notes;
use crate::operative::{self, Made, UNIT_TAGS};
use crate::uslm::{self, Footnotes, Head, Labels, Scope};
use crate::{IngestError, PREFIX};
use fidryn_provision::workspace::{AddressCounts, NotImported};
use fidryn_provision::{Address, ContainerModule, Item, Module, Provision};
use roxmltree::{Document, Node, NodeId};
use std::collections::{HashMap, HashSet};

/// The tags of containers.
pub(crate) const CONTAINER_TAGS: [&str; 13] = [
    "title",
    "appendix",
    "subtitle",
    "chapter",
    "subchapter",
    "part",
    "subpart",
    "division",
    "subdivision",
    "article",
    "courtRules",
    "compiledAct",
    "reorganizationPlans",
];

/// The deepest an element may stand in its module, the element of its unit
/// or container standing at 1, and the deepest a container may stand among
/// the containers of its file, the top container standing at 1: the limit of
/// nesting of a module in the grammar of edition 2, `MAX_NESTING` of
/// `fidryn_syntax::e2`.
pub(crate) const MAX_NESTING: usize = 64;

/// Whether a child of a container is a unit (`Some(true)`) or a container
/// (`Some(false)`); `None` when it is neither.
fn class_in_container(node: Node<'_, '_>) -> Option<bool> {
    let name = uslm::uslm_name(node)?;
    if name == "level" {
        Some(operative::has_text(node))
    } else if CONTAINER_TAGS.contains(&name) {
        Some(false)
    } else if UNIT_TAGS.contains(&name) {
        Some(true)
    } else {
        None
    }
}

/// The identifier of a unit or a container: the address it names and the
/// aliases its list or its range gives, before any is dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identifier {
    pub(crate) address: String,
    pub(crate) aliases: Vec<String>,
}

/// The identifier of the unit or container `node`, or `None` when it has
/// none.
fn identifier(node: Node<'_, '_>) -> Result<Option<Identifier>, IngestError> {
    let Some(raw) = node.attribute("identifier") else {
        return Ok(None);
    };
    no_tilde(node, raw)?;
    let refused = |member: &str, error| {
        IngestError::at(
            node,
            format!("the identifier {raw:?} is not an address: {member:?}: {error}"),
        )
    };
    let mut members = raw.split(' ');
    let first = members.next().unwrap_or("");
    Address::parse(first).map_err(|error| refused(first, error))?;
    let mut aliases = Vec::new();
    for other in members {
        Address::parse(other).map_err(|error| refused(other, error))?;
        aliases.push(other.to_owned());
    }
    for end in range_aliases(first) {
        Address::parse(&end).map_err(|error| refused(&end, error))?;
        aliases.push(end);
    }
    Ok(Some(Identifier {
        address: first.to_owned(),
        aliases,
    }))
}

/// Refuses the identifier `raw` of `node` when it holds `~`, the character
/// the reading alone may add to an address: an identifier such as
/// `/us/usc/t99/s1~2` beside two holders of `/us/usc/t99/s1` would give two
/// elements one address.
pub(crate) fn no_tilde(node: Node<'_, '_>, raw: &str) -> Result<(), IngestError> {
    if raw.contains('~') {
        return Err(IngestError::at(
            node,
            format!(
                "the identifier {raw:?} holds ~, the character the reading alone may add to an \
                 address"
            ),
        ));
    }
    Ok(())
}

/// The aliases of `identifier` when its last segment holds three full stops
/// in a row: its first end, the whole numbers between the ends where they
/// differ only in a whole number at their end, and its second end.
fn range_aliases(identifier: &str) -> Vec<String> {
    let start = identifier.rfind('/').map_or(0, |slash| slash + 1);
    let segment = &identifier[start..];
    let Some(dots) = segment.find("...") else {
        return Vec::new();
    };
    let first = identifier[..start + dots].to_owned();
    let letters: String = segment
        .chars()
        .take_while(char::is_ascii_lowercase)
        .collect();
    let second = format!("{}{letters}{}", &identifier[..start], &segment[dots + 3..]);
    let mut aliases = vec![first.clone()];
    let (head, low) = split_number(&first);
    let (tail, high) = split_number(&second);
    if let (Some(low), Some(high)) = (low, high)
        && head == tail
        && low <= high
        && high - low <= 999
    {
        aliases.extend((low + 1..high).map(|number| format!("{head}{number}")));
    }
    aliases.push(second);
    aliases
}

/// `text` without the ASCII digits at its end, and the whole number they
/// write, when there are any.
fn split_number(text: &str) -> (&str, Option<u64>) {
    let head = text.trim_end_matches(|c: char| c.is_ascii_digit());
    (head, text[head.len()..].parse().ok())
}

/// A unit or a container of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) node: NodeId,
    pub(crate) tag: String,
    pub(crate) unit: bool,
    /// The index of the container it stands in; `None` for the top
    /// container.
    pub(crate) parent: Option<usize>,
    pub(crate) identifier: Option<Identifier>,
    /// Of a unit: the addresses of its provisions relative to it, each
    /// beginning with `/`, sorted.
    pub(crate) provisions: Vec<String>,
    /// Of a unit: its provisions whose segment was disambiguated, and those
    /// whose segment was assigned.
    pub(crate) disambiguated: u64,
    pub(crate) assigned: u64,
}

/// The units and containers of a document, in document order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Outline {
    pub(crate) entries: Vec<Entry>,
}

/// The outline of the document whose top container is `top`, once its
/// nesting is checked ([`nesting`]).
pub(crate) fn outline(top: Node<'_, '_>) -> Result<Outline, IngestError> {
    nesting(top)?;
    let mut entries = Vec::new();
    visit(top, None, false, &mut entries)?;
    Ok(Outline { entries })
}

/// Refuses, before anything below the top container `top` is read, the
/// first element in document order that stands more than [`MAX_NESTING`]
/// deep in its module, and the first container that stands more than
/// [`MAX_NESTING`] deep among the containers of its file. The element of a
/// unit or a container stands at depth 1 of its module, and each element
/// inside it one deeper than its parent, except the units and containers
/// that are members of a container, which begin modules of their own; the
/// top container stands at depth 1 among the containers, and a container
/// that is a member of another one deeper. A module holds no block deeper
/// than the elements it is read from, so a module the reading writes is
/// never deeper than the grammar of edition 2 reads; and no reading of a
/// document goes deeper than these limits.
fn nesting(top: Node<'_, '_>) -> Result<(), IngestError> {
    // Each element still to be looked at, with its depth in its module, the
    // depth among the containers of the container nearest it, and whether
    // it is the element of a container.
    let mut waiting = vec![(top, 1, 1, true)];
    while let Some((node, depth, containers, container)) = waiting.pop() {
        if container && containers > MAX_NESTING {
            return Err(IngestError::at(
                node,
                format!(
                    "<{}> stands {containers} containers deep in its file, deeper than the \
                     {MAX_NESTING} the reading reads",
                    uslm::tag(node)
                ),
            ));
        }
        if depth > MAX_NESTING {
            return Err(IngestError::at(
                node,
                format!(
                    "<{}> stands {depth} deep in its unit or container, deeper than the \
                     {MAX_NESTING} the reading reads",
                    uslm::tag(node)
                ),
            ));
        }
        let children: Vec<Node<'_, '_>> = node.children().filter(Node::is_element).collect();
        // Pushed in reverse, so that they are looked at in document order.
        for child in children.into_iter().rev() {
            waiting.push(match class_in_container(child) {
                Some(false) if container => (child, 1, containers + 1, true),
                Some(true) if container => (child, 1, containers, false),
                _ => (child, depth + 1, containers, false),
            });
        }
    }
    Ok(())
}

/// Adds the unit or container `node`, and for a container the units and
/// containers below it, to `entries`.
fn visit(
    node: Node<'_, '_>,
    parent: Option<usize>,
    unit: bool,
    entries: &mut Vec<Entry>,
) -> Result<(), IngestError> {
    let index = entries.len();
    let mut entry = Entry {
        node: node.id(),
        tag: node.tag_name().name().to_owned(),
        unit,
        parent,
        identifier: identifier(node)?,
        provisions: Vec::new(),
        disambiguated: 0,
        assigned: 0,
    };
    if unit {
        provision_paths(node, "", &mut entry)?;
        entry.provisions.sort();
        entries.push(entry);
        return Ok(());
    }
    entries.push(entry);
    for child in node.children() {
        if let Some(unit) = class_in_container(child) {
            visit(child, Some(index), unit, entries)?;
        }
    }
    Ok(())
}

/// Adds the relative addresses of the provisions below `level`, whose own
/// relative address is `prefix`, to `entry`.
fn provision_paths(
    level: Node<'_, '_>,
    prefix: &str,
    entry: &mut Entry,
) -> Result<(), IngestError> {
    for segment in operative::child_segments(level)? {
        let path = format!("{prefix}/{}", segment.text);
        match segment.made {
            Made::Named => {}
            Made::Disambiguated => entry.disambiguated += 1,
            Made::Assigned => entry.assigned += 1,
        }
        provision_paths(segment.node, &path, entry)?;
        entry.provisions.push(path);
    }
    Ok(())
}

/// The address and the aliases of a unit or a container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Assigned {
    pub(crate) address: Address,
    pub(crate) aliases: Vec<Address>,
}

/// The addresses of the units and containers of each member of a release,
/// indexed like the entries of its outline, and the counts of each member.
#[derive(Debug)]
pub(crate) struct Numbering {
    pub(crate) members: Vec<Vec<Assigned>>,
    pub(crate) counts: Vec<AddressCounts>,
}

/// Numbers the outlines of a release, given in the order of the members'
/// names `names`. `locate` names the member at an index, and its archive, in
/// an error.
///
/// No two elements of the release end with one address: a unit or a
/// container whose address is that of another, or that of a provision of a
/// unit, is refused.
pub(crate) fn number(
    outlines: &[Outline],
    names: &[String],
    locate: &dyn Fn(usize, IngestError) -> IngestError,
) -> Result<Numbering, IngestError> {
    let prefix = Address::parse(PREFIX).map_err(|error| IngestError::new(error.to_string()))?;
    let mut held: HashMap<&str, u32> = HashMap::new();
    let mut top_unnamed: HashMap<&str, u32> = HashMap::new();
    let mut addresses: Vec<Vec<Address>> = Vec::with_capacity(outlines.len());
    let mut counts = vec![AddressCounts::default(); outlines.len()];
    for (member, outline) in outlines.iter().enumerate() {
        let mut unnamed: HashMap<(usize, &str), u32> = HashMap::new();
        let mut own: Vec<Address> = Vec::with_capacity(outline.entries.len());
        for entry in &outline.entries {
            let refused = |error: fidryn_provision::AddressError| {
                locate(
                    member,
                    IngestError::new(format!("an element of the outline has no address: {error}")),
                )
            };
            let address = match &entry.identifier {
                Some(identifier) => {
                    let holders = held.entry(identifier.address.as_str()).or_default();
                    *holders += 1;
                    if *holders == 1 {
                        Address::parse(&identifier.address).map_err(refused)?
                    } else {
                        counts[member].disambiguated += 1;
                        Address::parse(&format!("{}~{holders}", identifier.address))
                            .map_err(refused)?
                    }
                }
                None => {
                    counts[member].assigned += 1;
                    let (place, count) = match entry.parent {
                        Some(parent) => (
                            &own[parent],
                            unnamed.entry((parent, entry.tag.as_str())).or_default(),
                        ),
                        None => (&prefix, top_unnamed.entry(entry.tag.as_str()).or_default()),
                    };
                    *count += 1;
                    place
                        .child(&format!("~{}.{count}", entry.tag))
                        .map_err(refused)?
                }
            };
            counts[member].disambiguated += entry.disambiguated;
            counts[member].assigned += entry.assigned;
            own.push(address);
        }
        addresses.push(own);
    }

    // Every address of a unit or a container, and the element that has it.
    let mut taken: HashMap<&str, (usize, usize)> = HashMap::new();
    let element = |member: usize, index: usize| {
        let entry = &outlines[member].entries[index];
        format!("a {} of {}", entry.tag, names[member])
    };
    for (member, own) in addresses.iter().enumerate() {
        for (index, address) in own.iter().enumerate() {
            let address = address.as_str();
            if let Some(&(first, at)) = taken.get(address) {
                return Err(locate(
                    member,
                    IngestError::new(format!(
                        "{address} is the address of two elements: {} and {}",
                        element(first, at),
                        element(member, index)
                    )),
                ));
            }
            taken.insert(address, (member, index));
        }
    }
    // The unit whose provision is at `address`, when one is.
    let provision_of = |address: &str| {
        address.match_indices('/').find_map(|(at, _)| {
            let &(member, index) = taken.get(&address[..at])?;
            let entry = &outlines[member].entries[index];
            (entry.unit
                && entry
                    .provisions
                    .binary_search_by(|path| path.as_str().cmp(&address[at..]))
                    .is_ok())
            .then_some((member, index))
        })
    };
    for (member, own) in addresses.iter().enumerate() {
        for (index, address) in own.iter().enumerate() {
            let address = address.as_str();
            if let Some((unit, at)) = provision_of(address) {
                return Err(locate(
                    member,
                    IngestError::new(format!(
                        "{address} is the address of two elements: a provision of {} and {}",
                        element(unit, at),
                        element(member, index)
                    )),
                ));
            }
        }
    }
    let is_address = |alias: &str| taken.contains_key(alias) || provision_of(alias).is_some();

    let mut members = Vec::with_capacity(outlines.len());
    for (member, outline) in outlines.iter().enumerate() {
        let mut assigned = Vec::with_capacity(outline.entries.len());
        for (index, entry) in outline.entries.iter().enumerate() {
            let address = addresses[member][index].clone();
            let mut aliases: Vec<Address> = Vec::new();
            for alias in entry
                .identifier
                .iter()
                .flat_map(|identifier| &identifier.aliases)
            {
                if alias == address.as_str()
                    || aliases.iter().any(|kept| kept.as_str() == alias)
                    || is_address(alias)
                {
                    continue;
                }
                aliases.push(Address::parse(alias).map_err(|error| {
                    locate(
                        member,
                        IngestError::new(format!("the alias {alias:?} is not an address: {error}")),
                    )
                })?);
            }
            counts[member].aliases += aliases.len() as u64;
            assigned.push(Assigned { address, aliases });
        }
        members.push(assigned);
    }
    Ok(Numbering { members, counts })
}

/// What the reading of one member built and counted.
#[derive(Debug, Default)]
pub(crate) struct Built {
    pub(crate) units: u64,
    pub(crate) containers: u64,
    pub(crate) not_imported: NotImported,
}

/// Builds the modules of the document whose head is `head` and outline
/// `outline`, at the addresses `assigned`, and gives each to `give` in
/// document order. `source` is the member's file name.
pub(crate) fn build(
    document: &Document<'_>,
    head: &Head<'_, '_>,
    outline: &Outline,
    assigned: &[Assigned],
    source: &str,
    give: &mut dyn FnMut(Module) -> Result<(), IngestError>,
) -> Result<Built, IngestError> {
    let mut members: Vec<Vec<Address>> = vec![Vec::new(); outline.entries.len()];
    for (index, entry) in outline.entries.iter().enumerate() {
        if let Some(parent) = entry.parent {
            members[parent].push(assigned[index].address.clone());
        }
    }
    let mut built = Built::default();
    for (index, (entry, members)) in outline.entries.iter().zip(members).enumerate() {
        let node = document
            .get_node(entry.node)
            .ok_or_else(|| IngestError::new("an element of the outline is not in the document"))?;
        let within = entry.parent.map(|parent| assigned[parent].address.clone());
        // A unit holds everything inside it; a container holds what is not
        // one of its members.
        let is_member = |child: Node<'_, '_>| !entry.unit && class_in_container(child).is_some();
        let mut scope = Scope {
            footnotes: Footnotes::of_module(node, &is_member)?,
            ..Scope::default()
        };
        let module = if entry.unit {
            built.units += 1;
            let mut unit = operative::unit(
                node,
                &assigned[index],
                within,
                source,
                head.positive_law,
                &mut scope,
            )?;
            let mut provisions = HashSet::new();
            provisions_of(&unit.root, &mut provisions);
            let matter = notes::matter(document, &mut scope, &|address| {
                provisions.contains(address)
            })?;
            unit.credit = matter.credit;
            unit.notes = matter.notes;
            unit.footnotes = matter.footnotes;
            unit.definitions =
                definitions::extract(&unit, &containers_of(outline, assigned, index));
            Module::Unit(unit)
        } else {
            built.containers += 1;
            let mut container = container(
                node,
                &assigned[index],
                within,
                members,
                source,
                head.positive_law,
                &mut scope,
            )?;
            let matter = notes::matter(document, &mut scope, &|_| false)?;
            container.credit = matter.credit;
            container.notes = matter.notes;
            container.footnotes = matter.footnotes;
            Module::Container(container)
        };
        built.not_imported.add(&scope.counts);
        give(module)?;
    }
    Ok(built)
}

/// The containers of the entry `index` of `outline`, from the nearest
/// outward, each at its address and with its kind.
fn containers_of(outline: &Outline, assigned: &[Assigned], index: usize) -> Vec<(Address, String)> {
    let mut containers = Vec::new();
    let mut parent = outline.entries[index].parent;
    while let Some(at) = parent {
        let entry = &outline.entries[at];
        containers.push((assigned[at].address.clone(), entry.tag.clone()));
        parent = entry.parent;
    }
    containers
}

/// Adds the address of `provision` and of every provision below it to
/// `out`.
fn provisions_of<'p>(provision: &'p Provision, out: &mut HashSet<&'p str>) {
    out.insert(provision.address.as_str());
    for item in &provision.items {
        if let Item::Child(child) = item {
            provisions_of(child, out);
        }
    }
}

/// The container `node`, at its address and with its aliases and members,
/// without its notes and credit, which `scope` sets aside, and its
/// footnotes, which `scope` holds.
fn container(
    node: Node<'_, '_>,
    assigned: &Assigned,
    within: Option<Address>,
    members: Vec<Address>,
    source: &str,
    positive_law: Option<bool>,
    scope: &mut Scope,
) -> Result<ContainerModule, IngestError> {
    let mut labels = Labels::default();
    for child in node.children() {
        uslm::only_elements(child)?;
        if !child.is_element()
            || class_in_container(child).is_some()
            || labels.read(child, scope)?
        {
            continue;
        }
        match uslm::uslm_name(child) {
            Some("note" | "notes") => scope.set_aside_note(child, &assigned.address),
            Some("sourceCredit") => scope.set_aside_credit(child)?,
            Some("toc") => uslm::table_of_contents(child, &mut scope.counts),
            _ => {
                return Err(uslm::misplaced_in(
                    child,
                    &format!("a container, <{}>", uslm::tag(node)),
                ));
            }
        }
    }
    Ok(ContainerModule {
        address: assigned.address.clone(),
        aliases: assigned.aliases.clone(),
        within,
        source: source.to_owned(),
        positive_law,
        kind: node.tag_name().name().to_owned(),
        num: labels.num,
        heading: labels.heading,
        status: uslm::status(node)?,
        members,
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{
        Counts, Document, MEMBER_SETS, RELEASE, Scratch, assert_fixture, document_fixtures,
        expected_records, fixture, fixture_members, ingest_all, json_file, not_imported_counts,
        not_imported_of_profile, read_fixture, records, refusal, release_of, workspace,
    };
    use fidryn_provision::Item;

    fn unit_addresses(modules: &[Module]) -> Vec<(String, Vec<String>)> {
        modules
            .iter()
            .filter_map(|module| match module {
                Module::Unit(unit) => Some((
                    unit.address.to_string(),
                    unit.aliases.iter().map(ToString::to_string).collect(),
                )),
                _ => None,
            })
            .collect()
    }

    /// The release of the fixture directory `name`, packed into one archive.
    fn member_set(scratch: &Scratch, name: &str) -> crate::ReleaseArchives {
        let members = fixture_members(name);
        let members: Vec<(&str, &[u8])> = members
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        release_of(scratch, &members)
    }

    #[test]
    fn every_fixture_reads_to_its_expected_records() {
        let names = document_fixtures();
        // Every file of expected records is compared: one for each document
        // and one for each directory of members.
        let mut expected: Vec<String> = std::fs::read_dir(crate::testing::fixtures())
            .expect("the fixtures are listed")
            .map(|entry| {
                entry
                    .expect("a fixture")
                    .file_name()
                    .into_string()
                    .expect("UTF-8")
            })
            .filter_map(|name| name.strip_suffix(".records.jsonl").map(str::to_owned))
            .collect();
        expected.sort();
        let mut compared: Vec<String> = names
            .iter()
            .cloned()
            .chain(MEMBER_SETS.iter().map(|name| (*name).to_owned()))
            .collect();
        compared.sort();
        assert_eq!(compared, expected);
        assert!(names.len() >= 32, "{names:?}");
        for name in &names {
            assert_fixture(name);
        }
        for name in MEMBER_SETS {
            let scratch = Scratch::new(name);
            let archives = member_set(&scratch, name);
            let (modules, _) =
                ingest_all(&archives, &[], 2).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(records(&modules), expected_records(name), "{name}");
        }
    }

    #[test]
    fn every_fixture_has_the_counts_of_its_profile() {
        let names: Vec<String> = document_fixtures()
            .into_iter()
            .chain(MEMBER_SETS.iter().map(|name| (*name).to_owned()))
            .collect();
        for name in names {
            let scratch = Scratch::new(&format!("profile-{name}"));
            let archives = if MEMBER_SETS.contains(&name.as_str()) {
                member_set(&scratch, &name)
            } else {
                release_of(
                    &scratch,
                    &[(crate::testing::MEMBER, fixture(&name).as_bytes())],
                )
            };
            let (modules, summary) =
                ingest_all(&archives, &[], 1).unwrap_or_else(|error| panic!("{name}: {error}"));
            let profile =
                json_file(&crate::testing::fixtures().join(format!("{name}.profile.json")));
            assert_eq!(Counts::of(&modules), Counts::of_profile(&profile), "{name}");
            let addresses: AddressCounts =
                serde_json::from_value(profile["addresses"].clone()).expect("addresses");
            assert_eq!(summary.addresses, addresses, "{name}");
            assert_eq!(
                not_imported_counts(&summary.not_imported),
                not_imported_of_profile(&profile),
                "{name}"
            );
            assert_eq!(
                (summary.units, summary.containers),
                (Counts::of(&modules).units, Counts::of(&modules).containers),
                "{name}"
            );
        }
    }

    #[test]
    fn an_element_nested_deeper_than_a_module_holds_is_refused() {
        // The section stands at depth 1 of its module, so the content inside
        // `levels` levels stands at `levels + 2`.
        let nested = |levels: usize, inner: &str| {
            format!(
                "{}{inner}{}",
                "<level>".repeat(levels),
                "</level>".repeat(levels)
            )
        };
        let content = "<content>A widget is red.</content>";
        let modules = Document::with_section(&nested(MAX_NESTING - 2, content))
            .read()
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(crate::testing::Counts::of(&modules).provisions, 63);
        for (levels, inner, element) in [
            (MAX_NESTING - 1, content, "content"),
            (5000, content, "level"),
            (
                0,
                &format!(
                    "<content>{}A widget{}</content>",
                    "<b>".repeat(70),
                    "</b>".repeat(70)
                ),
                "b",
            ),
        ] {
            let error = refusal(&Document::with_section(&nested(levels, inner)));
            assert!(
                error.message().ends_with(
                    "deep in its unit or container, deeper than the 64 the reading reads"
                ),
                "{levels}: {error}"
            );
            assert_eq!(error.element(), Some(element), "{levels}: {error}");
            assert_eq!(error.identifier(), Some("/us/usc/t99/s1"), "{levels}");
        }
        // The notes of a container are counted from the container: the
        // chapter stands at 1, its `notes` at 2, the note at 3 and its `p` at
        // 4, so the 61st `b` in it stands at 65.
        let error = refusal(&Document::with_body(format!(
            "<chapter identifier=\"/us/usc/t99/ch1\"><notes type=\"uscNote\"><note \
             topic=\"miscellaneous\"><heading>Deep</heading><p>{}A widget{}</p></note></notes>\
             </chapter>",
            "<b>".repeat(61),
            "</b>".repeat(61)
        )));
        assert!(error.message().contains("stands 65 deep"), "{error}");
        assert_eq!(error.identifier(), Some("/us/usc/t99/ch1"));
    }

    #[test]
    fn a_container_nested_deeper_than_the_reading_reads_is_refused() {
        // The title stands at depth 1 among the containers of its file, and
        // each container in it one deeper; each container is a module of its
        // own, whose depth is counted from 1 again.
        let chapters = |count: usize| {
            format!(
                "{}<section identifier=\"/us/usc/t99/s1\"><content>Text.</content></section>{}",
                "<chapter>".repeat(count),
                "</chapter>".repeat(count)
            )
        };
        let modules = Document::with_body(chapters(MAX_NESTING - 1))
            .read()
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(crate::testing::Counts::of(&modules).containers, 64);
        for count in [MAX_NESTING, 5000] {
            let error = refusal(&Document::with_body(chapters(count)));
            assert!(
                error.message().ends_with(
                    "<chapter> stands 65 containers deep in its file, deeper than the 64 the \
                     reading reads"
                ),
                "{count}: {error}"
            );
            assert_eq!(error.element(), Some("chapter"), "{count}: {error}");
        }
    }

    #[test]
    fn every_container_kind_is_read() {
        assert_fixture("containers");
        assert_fixture("appendix-containers");
        let kinds: Vec<String> = ["containers", "appendix-containers", "unit-kinds"]
            .iter()
            .flat_map(|name| read_fixture(name))
            .filter_map(|module| match module {
                Module::Container(container) => Some(container.kind),
                _ => None,
            })
            .collect();
        for kind in CONTAINER_TAGS.iter().copied().chain(["level"]) {
            assert!(kinds.iter().any(|read| read == kind), "{kind}: {kinds:?}");
        }
    }

    #[test]
    fn containers_list_their_members_in_document_order_and_units_name_their_container() {
        let modules = read_fixture("containers");
        let by_address: HashMap<&str, &Module> = modules
            .iter()
            .map(|module| (crate::testing::address_of(module), module))
            .collect();
        let Module::Container(title) = by_address["/us/usc/t99"] else {
            panic!("the title");
        };
        assert_eq!(title.within, None);
        assert_eq!(
            title
                .members
                .iter()
                .map(Address::as_str)
                .collect::<Vec<_>>(),
            ["/us/usc/t99/stA", "/us/usc/t99/stB", "/us/usc/t99/ch3"]
        );
        // Every member names its container, and every module that names a
        // container is one of its members, in document order.
        for module in &modules {
            let (address, within) = match module {
                Module::Unit(unit) => (&unit.address, &unit.within),
                Module::Container(container) => (&container.address, &container.within),
                Module::Curated(_) => unreachable!(),
            };
            if let Some(within) = within {
                let Module::Container(parent) = by_address[within.as_str()] else {
                    panic!("{within} is not a container");
                };
                assert!(parent.members.contains(address), "{address} in {within}");
            }
        }
        let Module::Unit(s1) = by_address["/us/usc/t99/s1"] else {
            panic!("s1");
        };
        assert_eq!(
            s1.within.as_ref().map(Address::as_str),
            Some("/us/usc/t99/stA/ptI/sptA/ch1/schI")
        );
        let Module::Unit(s3) = by_address["/us/usc/t99/s3"] else {
            panic!("s3");
        };
        assert_eq!(
            s3.within.as_ref().map(Address::as_str),
            Some("/us/usc/t99/ch3/~level.1")
        );
        let members: usize = modules
            .iter()
            .map(|module| match module {
                Module::Container(container) => container.members.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(
            members + 1,
            modules.len(),
            "every module but the title is a member once"
        );
        // Units of a container that are not adjacent keep their order.
        let modules = Document::with_body(
            "<chapter identifier=\"/us/usc/t99/ch1\"><num value=\"1\">CHAPTER 1</num>\
             <section identifier=\"/us/usc/t99/s9\"><content>A.</content></section>\
             <subchapter identifier=\"/us/usc/t99/ch1/schI\"><section identifier=\"/us/usc/t99/s2\">\
             <content>B.</content></section></subchapter>\
             <section identifier=\"/us/usc/t99/s1\"><content>C.</content></section></chapter>"
                .to_owned(),
        )
        .read()
        .expect("read");
        let Module::Container(chapter) = &modules[1] else {
            panic!("{:?}", modules[1]);
        };
        assert_eq!(
            chapter
                .members
                .iter()
                .map(Address::as_str)
                .collect::<Vec<_>>(),
            ["/us/usc/t99/s9", "/us/usc/t99/ch1/schI", "/us/usc/t99/s1"]
        );
    }

    #[test]
    fn a_small_level_does_not_stand_in_a_container() {
        let error = refusal(&Document::with_body(
            "<subsection identifier=\"/us/usc/t99/s1/a\"><content>Text.</content></subsection>"
                .to_owned(),
        ));
        assert!(
            error.message().contains("does not stand in a container"),
            "{error}"
        );
        assert_eq!(error.element(), Some("subsection"));
        let error = refusal(&Document::with_body("<content>Text.</content>".to_owned()));
        assert!(
            error.message().contains("does not stand in a container"),
            "{error}"
        );
        let text = Document::with_body(String::new())
            .text()
            .replace("<main><title", "<main><chapter")
            .replace("</title></main>", "</chapter></main>");
        let error = crate::ingest_xml("usc99.xml", &text, RELEASE).expect_err("a chapter on top");
        assert!(error.message().contains("not a title"), "{error}");
    }

    #[test]
    fn an_identifier_that_lists_several_gives_one_address_and_aliases() {
        assert_fixture("identifier-lists");
        let addresses = unit_addresses(&read_fixture("identifier-lists"));
        assert_eq!(
            addresses[0],
            (
                "/us/usc/t99/s3".to_owned(),
                vec!["/us/usc/t99/s4".to_owned()]
            )
        );
        let modules = Document::with_body(
            "<section identifier=\"/us/usc/t99/s5 /us/usc/t99/s6 /us/usc/t99/s7\" status=\"repealed\">\
             </section>"
                .to_owned(),
        )
        .read()
        .expect("read");
        assert_eq!(
            unit_addresses(&modules),
            [(
                "/us/usc/t99/s5".to_owned(),
                vec!["/us/usc/t99/s6".to_owned(), "/us/usc/t99/s7".to_owned()]
            )]
        );
        let error = refusal(&Document::with_body(
            "<section identifier=\"/us/usc/t99/s5  /us/usc/t99/s6\"></section>".to_owned(),
        ));
        assert!(error.message().contains("is not an address"), "{error}");
    }

    #[test]
    fn a_range_identifier_gives_its_ends_and_the_whole_numbers_between_them_as_aliases() {
        let addresses = unit_addresses(&read_fixture("identifier-lists"));
        assert_eq!(
            addresses[1],
            (
                "/us/usc/t99/s10...12".to_owned(),
                vec![
                    "/us/usc/t99/s10".to_owned(),
                    "/us/usc/t99/s11".to_owned(),
                    "/us/usc/t99/s12".to_owned()
                ]
            )
        );
        // Ends that carry letters give the two ends only.
        assert_eq!(
            addresses[2],
            (
                "/us/usc/t99/s20a...20c".to_owned(),
                vec!["/us/usc/t99/s20a".to_owned(), "/us/usc/t99/s20c".to_owned()]
            )
        );
        assert_eq!(
            range_aliases("/us/usc/t99/s60e\u{2013}3...60e\u{2013}14").len(),
            12
        );
        assert_eq!(
            range_aliases("/us/usc/t99/s44...46"),
            ["/us/usc/t99/s44", "/us/usc/t99/s45", "/us/usc/t99/s46"]
        );
        // Ends more than 999 apart, or in the wrong order, give the ends only.
        assert_eq!(range_aliases("/zz/s1...1001").len(), 2);
        assert_eq!(range_aliases("/zz/s1...1000").len(), 1000);
        assert_eq!(range_aliases("/zz/s9...3"), ["/zz/s9", "/zz/s3"]);
        assert!(range_aliases("/zz/s9").is_empty());
    }

    #[test]
    fn an_alias_that_is_another_elements_address_is_dropped() {
        let addresses = unit_addresses(&read_fixture("identifier-lists"));
        // The list of s30 names s31, which is a section's address.
        assert_eq!(addresses[3], ("/us/usc/t99/s30".to_owned(), Vec::new()));
        // An alias that is the address of a provision, of a container, or the
        // element's own, or one it already has, is dropped.
        let modules = Document::with_body(
            "<section identifier=\"/us/usc/t99/s5 /us/usc/t99/s1/a /us/usc/t99 /us/usc/t99/s5 \
             /us/usc/t99/s6 /us/usc/t99/s6\"></section>\
             <section identifier=\"/us/usc/t99/s1\"><subsection identifier=\"/us/usc/t99/s1/a\">\
             <content>A.</content></subsection></section>"
                .to_owned(),
        )
        .read()
        .expect("read");
        assert_eq!(
            unit_addresses(&modules)[0],
            (
                "/us/usc/t99/s5".to_owned(),
                vec!["/us/usc/t99/s6".to_owned()]
            )
        );
    }

    #[test]
    fn an_element_without_an_identifier_gets_an_assigned_segment() {
        assert_fixture("no-identifier");
        let modules = read_fixture("no-identifier");
        let Module::Container(chapter) = &modules[1] else {
            panic!("{:?}", modules[1]);
        };
        assert_eq!(chapter.address.as_str(), "/us/usc/t99/~chapter.1");
        assert_eq!(
            unit_addresses(&modules)
                .into_iter()
                .map(|(address, _)| address)
                .collect::<Vec<_>>(),
            [
                "/us/usc/t99/~chapter.1/~section.1",
                "/us/usc/t99/~chapter.1/~section.2"
            ]
        );
        let Module::Unit(second) = &modules[3] else {
            panic!("{:?}", modules[3]);
        };
        let Item::Child(paragraph) = &second.root.items[0] else {
            panic!("{:?}", second.root.items);
        };
        assert_eq!(
            paragraph.address.as_str(),
            "/us/usc/t99/~chapter.1/~section.2/~paragraph.1"
        );
        assert_fixture("containers");
    }

    #[test]
    fn a_file_with_no_units_is_its_top_container() {
        assert_fixture("no-units");
        let modules = read_fixture("no-units");
        let [Module::Container(appendix)] = modules.as_slice() else {
            panic!("{modules:?}");
        };
        assert_eq!(appendix.address.as_str(), "/us/usc/~appendix.1");
        assert!(appendix.members.is_empty());
        assert_eq!(appendix.within, None);
        assert_eq!(appendix.positive_law, Some(false));
    }

    #[test]
    fn top_elements_without_an_identifier_are_numbered_over_the_release() {
        let scratch = Scratch::new("unnamed-appendices");
        let archives = member_set(&scratch, "unnamed-appendices");
        for jobs in [1, 8] {
            let (modules, summary) = ingest_all(&archives, &[], jobs).expect("ingested");
            let addresses: Vec<&str> = modules.iter().map(crate::testing::address_of).collect();
            assert_eq!(addresses, ["/us/usc/~appendix.1", "/us/usc/~appendix.2"]);
            assert_eq!(summary.addresses.assigned, 2);
        }
        // Ingesting one title still numbers over the release.
        let (modules, _) = ingest_all(&archives, &["99a"], 1).expect("ingested");
        let addresses: Vec<&str> = modules.iter().map(crate::testing::address_of).collect();
        assert_eq!(addresses, ["/us/usc/~appendix.2"]);
    }

    #[test]
    fn a_repeated_identifier_in_two_members_is_numbered_in_the_order_of_their_names() {
        let scratch = Scratch::new("two-files");
        let archives = member_set(&scratch, "two-files");
        for jobs in [1, 8] {
            let (modules, summary) = ingest_all(&archives, &[], jobs).expect("ingested");
            assert_eq!(records(&modules), expected_records("two-files"));
            assert_eq!(
                unit_addresses(&modules)
                    .into_iter()
                    .map(|(address, _)| address)
                    .collect::<Vec<_>>(),
                ["/us/usc/t99/s1", "/us/usc/t99/s1~2"]
            );
            assert_eq!(summary.addresses.disambiguated, 1, "{jobs} jobs");
            assert_eq!((summary.units, summary.containers), (2, 2));
        }
        // The second holder is numbered by the release when its title alone
        // is ingested.
        let (modules, summary) = ingest_all(&archives, &["99a"], 8).expect("ingested");
        assert_eq!(
            unit_addresses(&modules),
            [("/us/usc/t99/s1~2".to_owned(), Vec::new())]
        );
        assert_eq!((summary.units, summary.containers), (1, 1));
    }

    #[test]
    fn an_identifier_that_holds_a_tilde_is_refused() {
        // `~` is the character the reading adds: `/us/usc/t99/s1~2` beside two
        // holders of `/us/usc/t99/s1` would give two elements one address.
        let two = "<section identifier=\"/us/usc/t99/s1\"><content>A.</content></section>\
                   <section identifier=\"/us/usc/t99/s1\"><content>B.</content></section>";
        for body in [
            format!(
                "{two}<section identifier=\"/us/usc/t99/s1~2\"><content>C.</content></section>"
            ),
            "<chapter identifier=\"/us/usc/t99/~chapter.1\"></chapter>".to_owned(),
            "<section identifier=\"/us/usc/t99/s5 /us/usc/t99/s6~2\"></section>".to_owned(),
            "<section identifier=\"/us/usc/t99/s1\"><subsection identifier=\"/us/usc/t99/s1/a~2\">\
             <content>A.</content></subsection></section>"
                .to_owned(),
        ] {
            let error = refusal(&Document::with_body(body.clone()));
            assert!(error.message().contains("holds ~"), "{body}: {error}");
            assert!(
                error
                    .identifier()
                    .is_some_and(|identifier| identifier.contains('~'))
            );
        }
    }

    #[test]
    fn two_elements_of_a_release_never_share_an_address() {
        // A section whose identifier is the address of a provision of
        // another section, in the same member and in a later one.
        let holder = "<section identifier=\"/us/usc/t99/s1\"><subsection \
                      identifier=\"/us/usc/t99/s1/a\"><content>A.</content></subsection></section>";
        let error = refusal(&Document::with_body(format!(
            "{holder}<section identifier=\"/us/usc/t99/s1/a\"><content>B.</content></section>"
        )));
        assert!(
            error
                .message()
                .contains("/us/usc/t99/s1/a is the address of two elements"),
            "{error}"
        );
        assert_eq!(error.member(), Some(crate::testing::MEMBER), "{error}");

        let scratch = Scratch::new("shared-address");
        let first = Document::with_body(holder.to_owned()).text();
        let second = Document::with_body(
            "<section identifier=\"/us/usc/t99/s1/a\"><content>B.</content></section>".to_owned(),
        )
        .text()
        .replace("/us/usc/t99\"", "/us/usc/t99a\"");
        let archives = release_of(
            &scratch,
            &[
                ("usc99.xml", first.as_bytes()),
                ("usc99a.xml", second.as_bytes()),
            ],
        );
        let error = ingest_all(&archives, &[], 2).expect_err("one address twice");
        assert!(
            error
                .message()
                .contains("/us/usc/t99/s1/a is the address of two elements"),
            "{error}"
        );
        assert!(error.message().contains("usc99.xml"), "{error}");
        // An error of the numbering names the member and its archive.
        assert_eq!(error.member(), Some("usc99a.xml"), "{error}");
        assert_eq!(error.archive(), Some("xml_usc99@999-1.zip"), "{error}");
    }

    #[test]
    fn identifiers_without_a_title_segment_are_kept_as_they_are() {
        assert_fixture("no-title-segment");
        let addresses: Vec<String> = read_fixture("no-title-segment")
            .iter()
            .map(|module| crate::testing::address_of(module).to_owned())
            .collect();
        assert_eq!(
            addresses,
            [
                "/us/usc/t99a",
                "/us/usc/ptI",
                "/us/usc/ptI/rule1",
                "/us/usc/rule2"
            ]
        );
    }

    #[test]
    fn the_modules_are_the_same_for_one_and_eight_workers() {
        let archives =
            crate::open(&workspace().join("samples/uscode/archives")).expect("the sample opens");
        let one = ingest_all(&archives, &[], 1).expect("one worker");
        let eight = ingest_all(&archives, &[], 8).expect("eight workers");
        assert_eq!(one, eight);
        assert!(one.0.len() > 600, "{}", one.0.len());
    }

    #[test]
    fn the_sample_archives_ingest_with_their_notes() {
        let root = workspace().join("samples/uscode");
        let archives = crate::open(&root.join("archives")).expect("the sample opens");
        assert_eq!(archives.release(), "119-142");
        assert_eq!(archives.members(), ["usc01.xml", "usc11a.xml", "usc45.xml"]);
        let (modules, summary) = ingest_all(&archives, &[], 4).expect("the sample ingests");
        let profile = json_file(&root.join("profile.json"));
        let import = &profile["import"];
        let counts = Counts::of(&modules);
        let expected = Counts::of_profile(import);
        // The notes by layer and by group, the footnotes and the images.
        assert_eq!(
            (
                &counts.notes,
                &counts.notes_by_group,
                counts.footnotes,
                counts.with_images
            ),
            (
                &expected.notes,
                &expected.notes_by_group,
                expected.footnotes,
                expected.with_images
            )
        );
        assert!(counts.footnotes > 100 && counts.notes["enacted"] > 1000);
        assert_eq!(counts, expected);
        assert_eq!(
            (summary.units, summary.containers),
            (counts.units, counts.containers)
        );
        let addresses: AddressCounts =
            serde_json::from_value(import["addresses"].clone()).expect("addresses");
        assert_eq!(summary.addresses, addresses);
        assert_eq!(
            not_imported_counts(&summary.not_imported),
            not_imported_of_profile(import)
        );
        // Each title alone gives its own modules, numbered as in the release.
        let (title_one, _) = ingest_all(&archives, &["1"], 2).expect("title 1");
        let in_title_one: Vec<&Module> = modules
            .iter()
            .filter(|module| match module {
                Module::Unit(unit) => unit.source == "usc01.xml",
                Module::Container(container) => container.source == "usc01.xml",
                Module::Curated(_) => false,
            })
            .collect();
        assert_eq!(title_one.iter().collect::<Vec<_>>(), in_title_one);
    }
}
