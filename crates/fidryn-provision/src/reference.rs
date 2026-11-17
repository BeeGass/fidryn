//! The outcome of a reference, and the index of addresses that gives it.
//!
//! A reference has exactly one [`Outcome`], the first of the list that
//! applies. A beginning of a target is the target cut after any of its
//! segments, the target included; "such an address" is the address of a
//! module, a provision or a note of the index, or an alias of a module of
//! it, and an address wins over an alias. "Below" a prefix is within it and
//! not the prefix itself.

use crate::address::{self, Address};
use crate::hash::Hash32;
use crate::interface::{Interface, Shape, address_text_at};
use std::collections::{BTreeMap, HashMap};

/// What a reference reaches: exactly one of these, the first that applies.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    /// The target is no address.
    Malformed,
    /// A beginning of the target is an address beside which the address
    /// with `~2` added exists.
    Ambiguous,
    /// The target is the address of a module, a provision or a note, or an
    /// alias of a module, and neither it nor its unit has a status.
    Exact,
    /// As `Exact`, but the target or its unit has this status.
    TargetStatus(String),
    /// The longest beginning of the target that is such an address is a unit
    /// or lies in one.
    UnitOnly,
    /// The target is a range of units, or a dashed number that is no unit.
    RangeForm,
    /// The target lies below a prefix of the index and reaches no unit.
    NoUnit,
    /// The target lies below no prefix of the index; this holds its first
    /// two segments, the body of law it names.
    Outside(String),
}

impl Outcome {
    /// The name of every outcome, in the order the reports list them.
    pub const NAMES: [&'static str; 8] = [
        "exact",
        "target-status",
        "ambiguous",
        "unit-only",
        "range-form",
        "no-unit",
        "outside",
        "malformed",
    ];

    /// The outcome's name: `malformed`, `ambiguous`, `exact`,
    /// `target-status`, `unit-only`, `range-form`, `no-unit` or `outside`.
    pub fn name(&self) -> &'static str {
        match self {
            Outcome::Malformed => "malformed",
            Outcome::Ambiguous => "ambiguous",
            Outcome::Exact => "exact",
            Outcome::TargetStatus(_) => "target-status",
            Outcome::UnitOnly => "unit-only",
            Outcome::RangeForm => "range-form",
            Outcome::NoUnit => "no-unit",
            Outcome::Outside(_) => "outside",
        }
    }
}

/// What an address or an alias of the index names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Named {
    Module,
    Provision,
    Note,
    /// An alias of a module.
    Alias,
}

/// What the index holds of one address or alias.
#[derive(Clone, Debug)]
struct Entry {
    named: Named,
    /// The tree hash, of a provision and of a unit, whose address is that of
    /// its top provision.
    hash: Option<Hash32>,
    /// The status of what is named itself: of a provision, of a container,
    /// and of a unit, which is that of its top provision; an index of
    /// [`AddressIndex::statuses`].
    status: Option<u32>,
    /// The unit it lies in, a unit lying in itself; an index of
    /// [`AddressIndex::units`].
    unit: Option<u32>,
    /// Whether the address with `~2` added is an address of the index too.
    twin: bool,
}

/// What the index holds of a unit.
#[derive(Clone, Debug)]
struct UnitEntry {
    status: Option<u32>,
    enacted: Hash32,
}

/// Every address and alias of a set of interfaces, and the prefixes they lie
/// below. A lookup costs the same however many addresses the index holds,
/// so the outcome of a reference costs one lookup for each beginning of its
/// target and at most one more.
///
/// The addresses are kept in a hash map, which is only looked up and never
/// iterated, so nothing of its order reaches a result.
#[derive(Clone, Debug, Default)]
pub struct AddressIndex {
    entries: HashMap<Box<str>, Entry>,
    units: Vec<UnitEntry>,
    statuses: Vec<Box<str>>,
    prefixes: Vec<Address>,
}

impl AddressIndex {
    /// The index of the addresses of `interfaces`, the modules, provisions
    /// and notes, and of the aliases of their modules, below `prefixes`.
    /// When two of them have one address, an address is kept over an alias,
    /// and otherwise the first in the order of `interfaces`.
    pub fn build<'a>(
        interfaces: impl IntoIterator<Item = &'a Interface>,
        prefixes: &[Address],
    ) -> AddressIndex {
        let interfaces: Vec<&Interface> = interfaces.into_iter().collect();
        let mut index = AddressIndex {
            entries: HashMap::with_capacity(
                interfaces
                    .iter()
                    .map(|interface| {
                        1 + interface.aliases.len()
                            + interface.provisions.len()
                            + interface.notes.len()
                    })
                    .sum(),
            ),
            units: Vec::new(),
            statuses: Vec::new(),
            prefixes: prefixes.to_vec(),
        };
        let mut statuses = BTreeMap::new();
        let mut modules = Vec::with_capacity(interfaces.len());
        for interface in &interfaces {
            let module = index.module_entry(interface, &mut statuses);
            let unit = module.unit;
            index.insert(interface.address.as_str().into(), module.clone());
            for provision in &interface.provisions {
                let status = provision
                    .status
                    .map(|status| index.status(interface.word(status), &mut statuses));
                index.insert(
                    address_text_at(&interface.address, &provision.at).into(),
                    Entry {
                        named: Named::Provision,
                        hash: Some(provision.hash),
                        status,
                        unit,
                        twin: false,
                    },
                );
            }
            for note in &interface.notes {
                index.insert(
                    address_text_at(&interface.address, &note.at).into(),
                    Entry {
                        named: Named::Note,
                        hash: None,
                        status: None,
                        unit,
                        twin: false,
                    },
                );
            }
            modules.push(module);
        }
        for (interface, module) in interfaces.iter().zip(modules) {
            for alias in &interface.aliases {
                index.insert(
                    alias.item.as_str().into(),
                    Entry {
                        named: Named::Alias,
                        hash: None,
                        ..module.clone()
                    },
                );
            }
        }
        // An address `B~2` makes `B` ambiguous when `B` is an address too.
        // Only an address whose last segment ends in `~2` is built again.
        for interface in &interfaces {
            if let Some(base) = interface.address.as_str().strip_suffix("~2") {
                index.mark_twin(base);
            }
            let places = interface
                .provisions
                .iter()
                .map(|provision| &provision.at)
                .chain(interface.notes.iter().map(|note| &note.at));
            for at in places.filter(|at| at.ends_with("~2")) {
                let address = address_text_at(&interface.address, at);
                index.mark_twin(&address[..address.len() - 2]);
            }
        }
        index
    }

    /// Marks the address `base`, when it is one, as one beside which the
    /// address `base~2` exists.
    fn mark_twin(&mut self, base: &str) {
        if let Some(entry) = self.entries.get_mut(base)
            && entry.named != Named::Alias
        {
            entry.twin = true;
        }
    }

    /// The entry of the module of `interface`, with its unit added to the
    /// index when it is a unit.
    fn module_entry(
        &mut self,
        interface: &Interface,
        statuses: &mut BTreeMap<Box<str>, u32>,
    ) -> Entry {
        match interface.shape {
            Shape::Unit => {
                let top = interface.provisions.first();
                let status = top
                    .and_then(|top| top.status)
                    .map(|status| self.status(interface.word(status), statuses));
                let unit = u32::try_from(self.units.len()).expect("fewer than 2^32 units");
                self.units.push(UnitEntry {
                    status,
                    enacted: interface
                        .layers
                        .map_or(Hash32([0; 32]), |layers| layers.enacted),
                });
                Entry {
                    named: Named::Module,
                    hash: top.map(|top| top.hash),
                    status,
                    unit: Some(unit),
                    twin: false,
                }
            }
            Shape::Container => Entry {
                named: Named::Module,
                hash: None,
                status: interface
                    .container
                    .as_ref()
                    .and_then(|container| container.status.as_deref())
                    .map(|status| self.status(status, statuses)),
                unit: None,
                twin: false,
            },
            Shape::Curated => Entry {
                named: Named::Module,
                hash: None,
                status: None,
                unit: None,
                twin: false,
            },
        }
    }

    /// The index of the status `word` in [`AddressIndex::statuses`], added
    /// when it is not there yet.
    fn status(&mut self, word: &str, statuses: &mut BTreeMap<Box<str>, u32>) -> u32 {
        if let Some(at) = statuses.get(word) {
            return *at;
        }
        let at = u32::try_from(self.statuses.len()).expect("fewer than 2^32 statuses");
        self.statuses.push(word.into());
        statuses.insert(word.into(), at);
        at
    }

    /// Adds `entry` under `key`, unless the key is there already. Every
    /// address is added before any alias, so an address is kept over an
    /// alias.
    fn insert(&mut self, key: Box<str>, entry: Entry) {
        self.entries.entry(key).or_insert(entry);
    }

    /// The status of `entry` or, when it has none, of its unit.
    fn status_of(&self, entry: &Entry) -> Option<&str> {
        entry
            .status
            .or_else(|| entry.unit.and_then(|unit| self.units[unit as usize].status))
            .map(|status| &*self.statuses[status as usize])
    }

    /// The length of the prefix of the index that `target` lies below, and
    /// is not.
    fn prefix_below(&self, target: &str) -> Option<usize> {
        self.prefixes.iter().find_map(|prefix| {
            let prefix = prefix.as_str();
            target
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
                .then_some(prefix.len())
        })
    }

    /// The outcome of a reference whose target is `target`, by the rules of
    /// the outcomes, taken in the order of [`Outcome`]: the first that
    /// applies.
    pub fn outcome(&self, target: &str) -> Outcome {
        if address::check(target).is_err() {
            return Outcome::Malformed;
        }
        // Each beginning of the target ends before a `/` after the first, or
        // at the end of the target.
        let ends = target
            .char_indices()
            .skip(1)
            .filter(|(_, character)| *character == '/')
            .map(|(at, _)| at)
            .chain(std::iter::once(target.len()));
        let mut longest = None;
        let mut ambiguous = false;
        for end in ends {
            if let Some(entry) = self.entries.get(&target[..end]) {
                ambiguous |= entry.twin;
                longest = Some((end, entry));
            }
        }
        if ambiguous {
            return Outcome::Ambiguous;
        }
        if let Some((end, entry)) = longest {
            if end == target.len() {
                return match self.status_of(entry) {
                    Some(status) => Outcome::TargetStatus(status.to_owned()),
                    None => Outcome::Exact,
                };
            }
            if entry.unit.is_some() {
                return Outcome::UnitOnly;
            }
        }
        let below = self.prefix_below(target);
        if let Some(base) = longest.map(|(end, _)| end).or(below) {
            // The segment after the base, which the target has, since the
            // base is a beginning of it and not the whole.
            let segment = target[base + 1..].split('/').next().unwrap_or_default();
            if let Some(dash) = segment.find(['\u{2013}', '-'])
                && dash > 0
                && self.entries.contains_key(&target[..base + 1 + dash])
            {
                return Outcome::RangeForm;
            }
        }
        if below.is_some() {
            return Outcome::NoUnit;
        }
        Outcome::Outside(first_two_segments(target).to_owned())
    }

    /// The tree hash of the provision at `address`, when it is a provision
    /// of the index; the address of a unit is that of its top provision.
    pub fn provision_hash(&self, address: &Address) -> Option<Hash32> {
        self.entries
            .get(address.as_str())
            .filter(|entry| entry.named != Named::Alias)
            .and_then(|entry| entry.hash)
    }

    /// The enacted-layer hash of the unit that `address` lies in: a unit, a
    /// provision or a note of a unit, or an alias of a unit.
    pub fn unit_enacted_hash(&self, address: &Address) -> Option<Hash32> {
        self.entries
            .get(address.as_str())
            .and_then(|entry| entry.unit)
            .map(|unit| self.units[unit as usize].enacted)
    }
}

/// The first two segments of the address `target`, or the whole of an
/// address of one segment.
fn first_two_segments(target: &str) -> &str {
    let mut slashes = target.match_indices('/').map(|(at, _)| at).skip(2);
    match slashes.next() {
        Some(end) => &target[..end],
        None => target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{hash_provision, layer_hashes_of_unit};
    use crate::interface::{DeclSpans, interface_of};
    use crate::model::build::*;
    use crate::model::{ContainerModule, Item, Layer, Module, Provision, UnitModule};

    /// A unit of one section at `at`, with a subsection `a` when `child` is
    /// set, and the status `status`.
    fn unit(at: &str, child: bool, status: Option<&str>) -> UnitModule {
        let mut items = vec![content(vec![para(vec![words("An invented section.")])])];
        if child {
            items.push(Item::Child(provision(
                &format!("{at}/a"),
                "subsection",
                num("a", "(a)"),
                None,
                vec![content(vec![para(vec![words("An invented subsection.")])])],
            )));
        }
        let root = Provision {
            status: status.map(str::to_owned),
            ..provision(at, "section", num("1", "§ 1."), None, items)
        };
        UnitModule {
            address: address(at),
            aliases: Vec::new(),
            within: Some(address("/zz/demo/t1/ch1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            root,
            credit: None,
            notes: vec![note(
                &format!("{at}/note/statutoryNotes/nolaw/effective"),
                Layer::Enacted,
                "Effective Date",
                "An invented note took effect on an invented day.",
            )],
            footnotes: Vec::new(),
            definitions: Vec::new(),
        }
    }

    fn container(at: &str, members: &[&str]) -> ContainerModule {
        ContainerModule {
            address: address(at),
            aliases: Vec::new(),
            within: None,
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            kind: "chapter".to_owned(),
            num: num("1", "Chapter 1—"),
            heading: Some(plain_text("Invented Things")),
            status: None,
            members: members.iter().map(|member| address(member)).collect(),
            credit: None,
            notes: Vec::new(),
            footnotes: Vec::new(),
        }
    }

    /// The modules of the outcome table: the units `s1` (child `a`), `s2`
    /// (repealed), `s3` (omitted, alias `s4`), `s5` (child `a`) and `s5~2`,
    /// and the containers `t1` and `t1/ch1`.
    fn modules() -> Vec<Module> {
        let mut s3 = unit("/zz/demo/t1/s3", false, Some("omitted"));
        s3.aliases.push(address("/zz/demo/t1/s4"));
        vec![
            Module::Container(container("/zz/demo/t1", &["/zz/demo/t1/ch1"])),
            Module::Container(container(
                "/zz/demo/t1/ch1",
                &[
                    "/zz/demo/t1/s1",
                    "/zz/demo/t1/s2",
                    "/zz/demo/t1/s3",
                    "/zz/demo/t1/s5",
                    "/zz/demo/t1/s5~2",
                ],
            )),
            Module::Unit(unit("/zz/demo/t1/s1", true, None)),
            Module::Unit(unit("/zz/demo/t1/s2", false, Some("repealed"))),
            Module::Unit(s3),
            Module::Unit(unit("/zz/demo/t1/s5", true, None)),
            Module::Unit(unit("/zz/demo/t1/s5~2", false, None)),
        ]
    }

    fn index_of(modules: &[Module]) -> AddressIndex {
        let interfaces: Vec<Interface> = modules
            .iter()
            .map(|module| interface_of(module, &DeclSpans::default()))
            .collect();
        AddressIndex::build(&interfaces, &[address("/zz/demo")])
    }

    #[test]
    fn each_outcome_from_a_fixture() {
        let index = index_of(&modules());
        let status = |word: &str| Outcome::TargetStatus(word.to_owned());
        let outside = |body: &str| Outcome::Outside(body.to_owned());
        let table = [
            ("/zz/demo/t1/s1/a", Outcome::Exact),
            ("/zz/demo/t1/s1", Outcome::Exact),
            ("/zz/demo/t1/ch1", Outcome::Exact),
            ("/zz/demo/t1/s2", status("repealed")),
            ("/zz/demo/t1/s4", status("omitted")),
            ("/zz/demo/t1/s5", Outcome::Ambiguous),
            ("/zz/demo/t1/s5/a", Outcome::Ambiguous),
            ("/zz/demo/t1/s1/z", Outcome::UnitOnly),
            ("/zz/demo/t1/s2/a", Outcome::UnitOnly),
            ("/zz/demo/t1/s1\u{2013}9", Outcome::RangeForm),
            ("/zz/demo/t1/s1\u{2013}9/b", Outcome::RangeForm),
            ("/zz/demo/t1/s9", Outcome::NoUnit),
            ("/zz/demo/t9/s1", Outcome::NoUnit),
            ("/zz/demo/t1/s8\u{2013}9", Outcome::NoUnit),
            ("/zz/other/x", outside("/zz/other")),
            ("/yy/stat/116/926", outside("/yy/stat")),
            ("not an address", Outcome::Malformed),
            ("/zz/demo/t1/s1/a ", Outcome::Malformed),
        ];
        for (target, outcome) in table {
            assert_eq!(index.outcome(target), outcome, "{target:?}");
        }
    }

    #[test]
    fn the_edges_of_the_outcomes() {
        let index = index_of(&modules());
        let table = [
            // A note of a unit is exact, and a part below one reaches the
            // unit.
            (
                "/zz/demo/t1/s1/note/statutoryNotes/nolaw/effective",
                Outcome::Exact,
            ),
            (
                "/zz/demo/t1/s1/note/statutoryNotes/nolaw/effective/x",
                Outcome::UnitOnly,
            ),
            // The disambiguated address itself names one element.
            ("/zz/demo/t1/s5~2", Outcome::Exact),
            // A part below an alias reaches the unit of the alias.
            ("/zz/demo/t1/s4/a", Outcome::UnitOnly),
            // The ASCII hyphen is a dash, and the dashed segment may follow
            // the prefix.
            ("/zz/demo/t1/s1-9", Outcome::RangeForm),
            ("/zz/demo/t1\u{2013}3", Outcome::RangeForm),
            // A segment that begins with a dash names no first end.
            ("/zz/demo/t1/\u{2013}9", Outcome::NoUnit),
            // The prefix itself lies below no prefix.
            ("/zz/demo", Outcome::Outside("/zz/demo".to_owned())),
            ("/zz", Outcome::Outside("/zz".to_owned())),
            ("", Outcome::Malformed),
        ];
        for (target, outcome) in table {
            assert_eq!(index.outcome(target), outcome, "{target:?}");
        }
    }

    #[test]
    fn the_names_are_those_of_the_eight_outcomes() {
        let mut named: Vec<&str> = [
            Outcome::Malformed,
            Outcome::Ambiguous,
            Outcome::Exact,
            Outcome::TargetStatus("repealed".to_owned()),
            Outcome::UnitOnly,
            Outcome::RangeForm,
            Outcome::NoUnit,
            Outcome::Outside("/zz/other".to_owned()),
        ]
        .iter()
        .map(Outcome::name)
        .collect();
        named.sort_unstable();
        let mut listed = Outcome::NAMES.to_vec();
        listed.sort_unstable();
        assert_eq!(named, listed);
    }

    #[test]
    fn an_address_wins_over_an_alias() {
        // `s3` takes the alias `s1`, which is the address of another unit.
        let mut modules = modules();
        if let Module::Unit(s3) = &mut modules[4] {
            s3.aliases.push(address("/zz/demo/t1/s1"));
        }
        let index = index_of(&modules);
        assert_eq!(index.outcome("/zz/demo/t1/s1"), Outcome::Exact);
    }

    #[test]
    fn the_index_gives_the_hashes_of_a_provision_and_of_its_unit() {
        let modules = modules();
        let index = index_of(&modules);
        let Module::Unit(s1) = &modules[2] else {
            panic!("the third module is the unit s1");
        };
        let Item::Child(a) = &s1.root.items[1] else {
            panic!("s1 has the child a");
        };
        let enacted = layer_hashes_of_unit(s1).enacted;
        assert_eq!(
            index.provision_hash(&address("/zz/demo/t1/s1")),
            Some(hash_provision(&s1.root))
        );
        assert_eq!(
            index.provision_hash(&address("/zz/demo/t1/s1/a")),
            Some(hash_provision(a))
        );
        for at in [
            "/zz/demo/t1/s1",
            "/zz/demo/t1/s1/a",
            "/zz/demo/t1/s1/note/statutoryNotes/nolaw/effective",
        ] {
            assert_eq!(index.unit_enacted_hash(&address(at)), Some(enacted), "{at}");
        }
        for at in [
            "/zz/demo/t1/ch1",
            "/zz/demo/t1/s1/note/statutoryNotes/nolaw/effective",
            "/zz/demo/t1/s9",
        ] {
            assert_eq!(index.provision_hash(&address(at)), None, "{at}");
        }
        assert_eq!(index.unit_enacted_hash(&address("/zz/demo/t1/ch1")), None);
        assert_eq!(index.unit_enacted_hash(&address("/zz/demo/t1/s9")), None);
    }
}
