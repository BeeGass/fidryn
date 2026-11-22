//! The staleness of a binding.
//!
//! A binding of a curated module records the address it binds, the release
//! it was written against, and two hashes of what it was written against:
//! the tree hash of the provision at that address (`provision_hash`) and
//! the enacted-layer hash of the unit the provision lies in (`notes_hash`).
//! An alias of a unit stands for the unit, and an address wins over an
//! alias, as for the outcome of a reference ([`AddressIndex`]). A binding is
//! stale when what it records is no longer what the index holds, for the
//! first of the reasons of [`StaleReason`] that holds. Nothing of the
//! editorial layer is in either hash, so a change to editorial matter makes
//! nothing stale; and the notes of a container enter no unit's enacted
//! layer. The release a binding records is not compared with anything.

use crate::address::Address;
use crate::hash::Hash32;
use crate::reference::AddressIndex;
use std::fmt;

/// Why a binding is stale. When more than one holds, the reason is the
/// first in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StaleReason {
    /// The address is no provision of the index, nor the alias of a unit.
    Removed,
    /// The tree hash of the provision differs from the one recorded.
    TextChanged,
    /// The enacted-layer hash of the unit the provision lies in differs
    /// from the one recorded.
    EnactedNotesChanged,
}

impl StaleReason {
    /// What the reason says to a reader: `provision removed or moved`,
    /// `provision text changed` or `enacted notes of the unit changed`.
    pub fn message(self) -> &'static str {
        match self {
            StaleReason::Removed => "provision removed or moved",
            StaleReason::TextChanged => "provision text changed",
            StaleReason::EnactedNotesChanged => "enacted notes of the unit changed",
        }
    }
}

impl fmt::Display for StaleReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// The two hashes a binding records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundHashes {
    /// The tree hash of the provision.
    pub provision_hash: Hash32,
    /// The enacted-layer hash of the unit the provision lies in.
    pub notes_hash: Hash32,
}

/// The hashes that a binding to `address` records when it is written
/// against the modules of `index`: of the provision at `address`, or of the
/// unit whose alias `address` is, and of its unit. `None` when `address` is
/// neither, as the address of a note, of a container or of nothing, and an
/// alias of a container, are not.
pub fn bound_hashes(address: &Address, index: &AddressIndex) -> Option<BoundHashes> {
    Some(BoundHashes {
        provision_hash: index.provision_hash(address)?,
        notes_hash: index.unit_enacted_hash(address)?,
    })
}

/// Whether a binding of `address` that records `provision_hash` and
/// `notes_hash` is stale against the modules of `index`, and why: `None`
/// when [`bound_hashes`] gives exactly the two it records.
pub fn staleness(
    address: &Address,
    provision_hash: Hash32,
    notes_hash: Hash32,
    index: &AddressIndex,
) -> Option<StaleReason> {
    let Some(found) = bound_hashes(address, index) else {
        return Some(StaleReason::Removed);
    };
    if found.provision_hash != provision_hash {
        Some(StaleReason::TextChanged)
    } else if found.notes_hash != notes_hash {
        Some(StaleReason::EnactedNotesChanged)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{hash_provision, layer_hashes_of_unit};
    use crate::interface::{DeclSpans, Interface, interface_of};
    use crate::model::build::*;
    use crate::model::{ContainerModule, Item, Layer, Module, Provision, UnitModule};

    /// A subsection `a` or `b` of `parent` that says `words`.
    fn child(parent: &str, value: &str, words_of: &str) -> Item {
        Item::Child(provision(
            &format!("{parent}/{value}"),
            "subsection",
            num(value, &format!("({value})")),
            None,
            vec![content(vec![para(vec![words(words_of)])])],
        ))
    }

    /// A unit of one section at `at`, with the subsections `a` and `b`, an
    /// enacted note and an editorial note.
    fn unit(at: &str) -> UnitModule {
        let root = Provision {
            items: vec![
                content(vec![para(vec![words("An invented section.")])]),
                child(at, "a", "An invented subsection."),
                child(at, "b", "Another invented subsection."),
            ],
            ..provision(at, "section", num("8", "§ 8."), None, Vec::new())
        };
        UnitModule {
            address: address(at),
            aliases: Vec::new(),
            within: Some(address("/zz/demo/t1/ch1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            root,
            credit: None,
            notes: vec![
                note(
                    &format!("{at}/note/demoNotes/nolaw/effective"),
                    Layer::Enacted,
                    "Effective Date",
                    "An invented note took effect on an invented day.",
                ),
                note(
                    &format!("{at}/note/demoNotes/nolaw/codification"),
                    Layer::Editorial,
                    "Codification",
                    "An invented note of an invented editor.",
                ),
            ],
            footnotes: Vec::new(),
            definitions: Vec::new(),
        }
    }

    /// The chapter `/zz/demo/t1/ch1`, with the alias `/zz/demo/t1/chI`.
    fn chapter() -> ContainerModule {
        ContainerModule {
            address: address("/zz/demo/t1/ch1"),
            aliases: vec![address("/zz/demo/t1/chI")],
            within: None,
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            kind: "chapter".to_owned(),
            num: num("1", "Chapter 1—"),
            heading: Some(plain_text("Invented Things")),
            status: None,
            members: vec![address("/zz/demo/t1/s3"), address("/zz/demo/t1/s8")],
            credit: None,
            notes: Vec::new(),
            footnotes: Vec::new(),
        }
    }

    /// The chapter, the unit `s3` with the alias `s4`, and the unit `s8`.
    fn modules() -> Vec<Module> {
        let mut s3 = unit("/zz/demo/t1/s3");
        s3.aliases.push(address("/zz/demo/t1/s4"));
        vec![
            Module::Container(chapter()),
            Module::Unit(s3),
            Module::Unit(unit("/zz/demo/t1/s8")),
        ]
    }

    fn index_of(modules: &[Module]) -> AddressIndex {
        let interfaces: Vec<Interface> = modules
            .iter()
            .map(|module| interface_of(module, &DeclSpans::default()))
            .collect();
        AddressIndex::build(&interfaces, &[address("/zz/demo")])
    }

    /// The child `value` of the top provision of `unit`.
    fn child_of<'a>(unit: &'a UnitModule, value: &str) -> &'a Provision {
        unit.root
            .items
            .iter()
            .find_map(|item| match item {
                Item::Child(child) if child.address.as_str().ends_with(&format!("/{value}")) => {
                    Some(child)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no child {value}"))
    }

    #[test]
    fn the_reasons_say_the_three_messages() {
        let said: Vec<String> = [
            StaleReason::Removed,
            StaleReason::TextChanged,
            StaleReason::EnactedNotesChanged,
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        assert_eq!(
            said,
            [
                "provision removed or moved",
                "provision text changed",
                "enacted notes of the unit changed",
            ]
        );
    }

    #[test]
    fn a_binding_is_stale_for_the_first_reason_that_holds() {
        let modules = modules();
        let index = index_of(&modules);
        let s8 = unit("/zz/demo/t1/s8");
        let a = child_of(&s8, "a");
        let tree = hash_provision(a);
        let notes = layer_hashes_of_unit(&s8).enacted;
        let other = Hash32([7; 32]);
        let at = address("/zz/demo/t1/s8/a");
        assert_eq!(
            bound_hashes(&at, &index),
            Some(BoundHashes {
                provision_hash: tree,
                notes_hash: notes
            })
        );
        assert_eq!(staleness(&at, tree, notes, &index), None);
        assert_eq!(
            staleness(&at, other, notes, &index),
            Some(StaleReason::TextChanged)
        );
        assert_eq!(
            staleness(&at, tree, other, &index),
            Some(StaleReason::EnactedNotesChanged)
        );
        // The text changed and the notes changed: the first reason.
        assert_eq!(
            staleness(&at, other, other, &index),
            Some(StaleReason::TextChanged)
        );
        // An address that is gone: the first reason, whatever is recorded.
        for at in ["/zz/demo/t1/s8/c", "/zz/demo/t1/s9"] {
            assert_eq!(
                staleness(&address(at), tree, notes, &index),
                Some(StaleReason::Removed),
                "{at}"
            );
        }
        // The unit itself, by its address, records its top provision.
        assert_eq!(
            staleness(
                &address("/zz/demo/t1/s8"),
                hash_provision(&s8.root),
                notes,
                &index
            ),
            None
        );
    }

    #[test]
    fn what_is_no_provision_is_removed_and_an_alias_of_a_unit_stands_for_it() {
        let modules = modules();
        let index = index_of(&modules);
        let any = Hash32([7; 32]);
        for at in [
            // A note of a unit, enacted and editorial.
            "/zz/demo/t1/s8/note/demoNotes/nolaw/effective",
            "/zz/demo/t1/s8/note/demoNotes/nolaw/codification",
            // A container, by its address and by its alias.
            "/zz/demo/t1/ch1",
            "/zz/demo/t1/chI",
            // The prefix, a beginning of addresses that names nothing.
            "/zz/demo",
            "/zz/demo/t1",
        ] {
            assert_eq!(bound_hashes(&address(at), &index), None, "{at}");
            assert_eq!(
                staleness(&address(at), any, any, &index),
                Some(StaleReason::Removed),
                "{at}"
            );
        }
        // The alias `s4` of the unit `s3` stands for its top provision.
        let Module::Unit(s3) = &modules[1] else {
            panic!("the second module is the unit s3");
        };
        let expected = BoundHashes {
            provision_hash: hash_provision(&s3.root),
            notes_hash: layer_hashes_of_unit(s3).enacted,
        };
        assert_eq!(
            bound_hashes(&address("/zz/demo/t1/s4"), &index),
            Some(expected)
        );
        assert_eq!(
            bound_hashes(&address("/zz/demo/t1/s3"), &index),
            Some(expected)
        );
    }

    #[test]
    fn an_address_wins_over_an_alias() {
        // `s3` takes the alias `s8/b`, which is the address of a subsection
        // of `s8`.
        let mut modules = modules();
        if let Module::Unit(s3) = &mut modules[1] {
            s3.aliases.push(address("/zz/demo/t1/s8/b"));
        }
        let index = index_of(&modules);
        let s8 = unit("/zz/demo/t1/s8");
        assert_eq!(
            bound_hashes(&address("/zz/demo/t1/s8/b"), &index),
            Some(BoundHashes {
                provision_hash: hash_provision(child_of(&s8, "b")),
                notes_hash: layer_hashes_of_unit(&s8).enacted,
            })
        );
    }
}
