//! Units and the provisions inside them.
//!
//! A unit is a `section`, a `courtRule`, a `reorganizationPlan`, or a
//! `level` with a text part or a `p` as a child and no unit above it. The
//! outermost unit holds everything inside it: any level that stands inside a
//! unit, a section included, is a provision of that unit, and its kind is its
//! tag. The children of a level in document order give its items: a text part
//! gives its parts, a `p` gives a content part, and a level gives a child.
//!
//! A provision's address is its parent's followed by the last segment of its
//! identifier (of the first member when the identifier lists several), with
//! `~2`, `~3` when a sibling before it already has that address; a provision
//! without an identifier gets the segment `~<tag>.<n>`, where `n` counts the
//! siblings of its tag without an identifier.

use crate::IngestError;
use crate::containers::{Assigned, CONTAINER_TAGS};
use crate::uslm::{self, Labels, Scope};
use fidryn_provision::{Address, Item, PartRole, Provision, UnitModule};
use roxmltree::Node;
use std::collections::HashMap;

/// The tags of units.
pub(crate) const UNIT_TAGS: [&str; 3] = ["section", "courtRule", "reorganizationPlan"];

/// The tags of the small levels, which stand only inside a unit.
const SMALL_TAGS: [&str; 8] = [
    "subsection",
    "paragraph",
    "subparagraph",
    "clause",
    "subclause",
    "item",
    "subitem",
    "subsubitem",
];

/// Whether `node` is a provision when it stands inside a unit: a container,
/// a unit, a small level or a `level`.
pub(crate) fn is_provision(node: Node<'_, '_>) -> bool {
    uslm::uslm_name(node).is_some_and(|name| {
        name == "level"
            || CONTAINER_TAGS.contains(&name)
            || UNIT_TAGS.contains(&name)
            || SMALL_TAGS.contains(&name)
    })
}

/// Whether the `level` `node` has a text part or a `p` as a child, which
/// makes it a unit when no unit stands above it.
pub(crate) fn has_text(node: Node<'_, '_>) -> bool {
    node.children()
        .any(|child| uslm::text_part_role(child).is_some() || uslm::is_p(child))
}

/// How the segment of a provision was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Made {
    /// The last segment of its identifier.
    Named,
    /// That segment followed by `~2`, `~3`: a sibling before it has it.
    Disambiguated,
    /// `~<tag>.<n>`: it has no identifier.
    Assigned,
}

/// The segment a provision adds to its parent's address.
pub(crate) struct Segment<'a, 'i> {
    pub(crate) node: Node<'a, 'i>,
    pub(crate) text: String,
    pub(crate) made: Made,
}

/// The segments of the provisions that are children of `level`, in
/// document order.
pub(crate) fn child_segments<'a, 'i>(
    level: Node<'a, 'i>,
) -> Result<Vec<Segment<'a, 'i>>, IngestError> {
    let mut named: HashMap<&str, u32> = HashMap::new();
    let mut unnamed: HashMap<&str, u32> = HashMap::new();
    let mut segments = Vec::new();
    for child in level.children().filter(|child| is_provision(*child)) {
        let segment = match child.attribute("identifier") {
            Some(identifier) => {
                let first = identifier.split(' ').next().unwrap_or("");
                let last = first.rsplit('/').next().unwrap_or("");
                if let Err(error) = Address::parse(&format!("/{last}")) {
                    return Err(IngestError::at(
                        child,
                        format!(
                            "the identifier {identifier:?} is not an address: its last segment \
                             {last:?} is no segment ({error})"
                        ),
                    ));
                }
                let count = named.entry(last).or_default();
                *count += 1;
                if *count == 1 {
                    Segment {
                        node: child,
                        text: last.to_owned(),
                        made: Made::Named,
                    }
                } else {
                    Segment {
                        node: child,
                        text: format!("{last}~{count}"),
                        made: Made::Disambiguated,
                    }
                }
            }
            None => {
                let tag = child.tag_name().name();
                let count = unnamed.entry(tag).or_default();
                *count += 1;
                Segment {
                    node: child,
                    text: format!("~{tag}.{count}"),
                    made: Made::Assigned,
                }
            }
        };
        segments.push(segment);
    }
    Ok(segments)
}

/// The unit `node`, at its address and with its aliases, without its notes,
/// credit and footnotes, which `scope` sets aside.
pub(crate) fn unit(
    node: Node<'_, '_>,
    assigned: &Assigned,
    within: Option<Address>,
    source: &str,
    positive_law: bool,
    scope: &mut Scope,
) -> Result<UnitModule, IngestError> {
    let root = provision(node, assigned.address.clone(), true, scope)?;
    Ok(UnitModule {
        address: assigned.address.clone(),
        aliases: assigned.aliases.clone(),
        within,
        source: source.to_owned(),
        positive_law: Some(positive_law),
        root,
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
        definitions: Vec::new(),
    })
}

/// The provision `node` at `address`, and the tree below it. `top` is true
/// for the top provision of a unit, the only one that may hold a source
/// credit.
fn provision(
    node: Node<'_, '_>,
    address: Address,
    top: bool,
    scope: &mut Scope,
) -> Result<Provision, IngestError> {
    let mut segments = child_segments(node)?.into_iter();
    let mut labels = Labels::default();
    let mut items = Vec::new();
    for child in node.children() {
        uslm::only_elements(child)?;
        if !child.is_element() || labels.read(child, scope)? {
            continue;
        }
        if is_provision(child) {
            let segment = segments
                .next()
                .filter(|segment| segment.node == child)
                .ok_or_else(|| IngestError::at(child, "the provision has no segment"))?;
            let at = address.child(&segment.text).map_err(|error| {
                IngestError::at(child, format!("the provision has no address: {error}"))
            })?;
            items.push(Item::Child(provision(child, at, false, scope)?));
            continue;
        }
        if let Some(role) = uslm::text_part_role(child) {
            for (role, blocks) in uslm::text_part(child, role, scope)? {
                items.push(Item::Part { role, blocks });
            }
            continue;
        }
        if uslm::is_p(child) {
            let blocks = uslm::flow(child, scope)?;
            if !blocks.is_empty() {
                items.push(Item::Part {
                    role: PartRole::Content,
                    blocks,
                });
            }
            continue;
        }
        match uslm::uslm_name(child) {
            Some("note" | "notes") => scope.set_aside_note(child),
            Some("sourceCredit") if top => scope.set_aside_credit(child)?,
            Some("sourceCredit") => {
                return Err(IngestError::at(
                    child,
                    "a source credit that is not the direct child of its unit or container",
                ));
            }
            Some("toc") => uslm::table_of_contents(child, &mut scope.counts),
            _ => return Err(uslm::misplaced(child, node)),
        }
    }
    Ok(Provision {
        address,
        kind: node.tag_name().name().to_owned(),
        num: labels.num,
        heading: labels.heading,
        status: uslm::status(node)?,
        items,
    })
}

#[cfg(test)]
mod tests {
    use crate::testing::{Document, assert_fixture, read_fixture, refusal};
    use fidryn_provision::{Item, Module, PartRole};

    #[test]
    fn a_section_with_subsections_and_paragraphs_is_read() {
        assert_fixture("section");
    }

    #[test]
    fn text_between_two_children_keeps_its_place() {
        assert_fixture("continuation");
        let modules = read_fixture("continuation");
        let Module::Unit(unit) = &modules[1] else {
            panic!("{:?}", modules[1]);
        };
        let Item::Child(a) = &unit.root.items[0] else {
            panic!("{:?}", unit.root.items);
        };
        let shape: Vec<String> = a
            .items
            .iter()
            .map(|item| match item {
                Item::Part { role, .. } => format!("{role:?}"),
                Item::Child(child) => child.address.to_string(),
            })
            .collect();
        assert_eq!(
            shape,
            [
                "Chapeau",
                "/us/usc/t99/s1/a/1",
                "Continuation",
                "/us/usc/t99/s1/a/2",
                "Continuation"
            ]
        );
    }

    #[test]
    fn a_proviso_is_a_part_of_its_own() {
        assert_fixture("proviso");
        let modules = read_fixture("proviso");
        let Module::Unit(unit) = &modules[1] else {
            panic!("{:?}", modules[1]);
        };
        for item in &unit.root.items {
            let Item::Child(subsection) = item else {
                panic!("{item:?}");
            };
            let roles: Vec<PartRole> = subsection
                .items
                .iter()
                .map(|item| match item {
                    Item::Part { role, .. } => *role,
                    Item::Child(_) => panic!("{item:?}"),
                })
                .collect();
            assert_eq!(roles, [PartRole::Content, PartRole::Proviso]);
        }
    }

    #[test]
    fn a_court_rule_and_each_other_unit_kind_are_units() {
        assert_fixture("unit-kinds");
        let kinds: Vec<(String, String)> = read_fixture("unit-kinds")
            .iter()
            .filter_map(|module| match module {
                Module::Unit(unit) => Some((unit.root.kind.clone(), unit.address.to_string())),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            [
                (
                    "courtRule".to_owned(),
                    "/us/usc/t99a/courtRules/Demo/rule1".to_owned()
                ),
                (
                    "level".to_owned(),
                    "/us/usc/t99a/courtRules/Demo/Form1".to_owned()
                ),
                (
                    "reorganizationPlan".to_owned(),
                    "/us/usc/t99a/reorgPlan/2099/1".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_section_inside_a_section_is_a_provision_of_the_outer_unit() {
        assert_fixture("section-in-section");
        let modules = read_fixture("section-in-section");
        assert_eq!(modules.len(), 3, "a title and two units");
        let Module::Unit(outer) = &modules[2] else {
            panic!("{:?}", modules[2]);
        };
        assert_eq!(outer.address.as_str(), "/us/usc/t99/s5");
        let Item::Child(inner) = &outer.root.items[1] else {
            panic!("{:?}", outer.root.items);
        };
        assert_eq!(
            (inner.kind.as_str(), inner.address.as_str()),
            ("section", "/us/usc/t99/s5/s1")
        );
    }

    #[test]
    fn a_repeated_identifier_keeps_both_elements_at_addresses_of_their_own() {
        assert_fixture("repeated-identifiers");
        let modules = read_fixture("repeated-identifiers");
        let addresses: Vec<String> = modules
            .iter()
            .filter_map(|module| match module {
                Module::Unit(unit) => Some(unit.address.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            addresses,
            ["/us/usc/t99/s1", "/us/usc/t99/s1~2", "/us/usc/t99/s2"]
        );
        let Module::Unit(second) = &modules[2] else {
            panic!("{:?}", modules[2]);
        };
        let Item::Child(inside) = &second.root.items[0] else {
            panic!("{:?}", second.root.items);
        };
        assert_eq!(inside.address.as_str(), "/us/usc/t99/s1~2/a");
        let Module::Unit(paragraphs) = &modules[3] else {
            panic!("{:?}", modules[3]);
        };
        let children: Vec<&str> = paragraphs
            .root
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Child(child) => Some(child.address.as_str()),
                Item::Part { .. } => None,
            })
            .collect();
        assert_eq!(children, ["/us/usc/t99/s2/3", "/us/usc/t99/s2/3~2"]);
    }

    #[test]
    fn a_provision_reads_only_the_last_segment_of_the_first_member_of_its_identifier() {
        let modules = Document::with_section(
            "<subsection identifier=\"/us/usc/t99/s9/a /us/usc/t99/s1/b\"><content>A.</content>\
             </subsection><subsection identifier=\"/elsewhere/c\"><content>C.</content>\
             </subsection><level><content>D.</content></level><level><content>E.</content>\
             </level>",
        )
        .read()
        .expect("read");
        let Module::Unit(unit) = &modules[1] else {
            panic!("{:?}", modules[1]);
        };
        let children: Vec<&str> = unit
            .root
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Child(child) => Some(child.address.as_str()),
                Item::Part { .. } => None,
            })
            .collect();
        assert_eq!(
            children,
            [
                "/us/usc/t99/s1/a",
                "/us/usc/t99/s1/c",
                "/us/usc/t99/s1/~level.1",
                "/us/usc/t99/s1/~level.2"
            ]
        );
        for identifier in ["/us/usc/t99/s1/(a)", "/us/usc/t99/s1/", " /us/usc/t99/s1/a"] {
            let error = refusal(&Document::with_section(&format!(
                "<subsection identifier=\"{identifier}\"><content>A.</content></subsection>"
            )));
            assert!(
                error.message().contains("is not an address"),
                "{identifier}: {error}"
            );
        }
    }

    #[test]
    fn a_source_credit_stands_only_directly_in_its_unit() {
        let error = refusal(&Document::with_section(
            "<sourceCredit>(One.)</sourceCredit><sourceCredit>(Two.)</sourceCredit>",
        ));
        assert!(
            error.message().contains("a second source credit"),
            "{error}"
        );
        let error = refusal(&Document::with_section(
            "<subsection identifier=\"/us/usc/t99/s1/a\"><content>Text.</content>\
             <sourceCredit>(One.)</sourceCredit></subsection>",
        ));
        assert!(error.message().contains("not the direct child"), "{error}");
        assert_eq!(error.identifier(), Some("/us/usc/t99/s1/a"));
    }
}
