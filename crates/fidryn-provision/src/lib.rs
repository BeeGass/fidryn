//! The model of law modules and the rules over it: addresses, the
//! normalization of text, the file path of a module, the model of provisions,
//! notes and modules ([`model`]), their hashes ([`hash`]), their text
//! records ([`records`]) and their canonical text ([`render`]).
//!
//! This crate depends on no other crate of the workspace and names no corpus;
//! `xtask/tests/corpus_neutrality.rs` checks the second rule.

mod address;
pub mod hash;
pub mod layout;
pub mod model;
pub mod normalize;
pub mod records;
pub mod render;
pub mod workspace;

pub use address::{Address, AddressError};
pub use hash::{Hash32, HashError, LayerHashes};
pub use layout::LayoutError;
pub use model::{
    Basis, BindTarget, Binding, Block, Cell, Citation, ContainerModule, CuratedModule, Definition,
    DefinitionKind, Footnote, Import, Item, Layer, Module, Note, Num, PartRole, Provision, Quoted,
    QuotedItem, QuotedLevel, Row, RowGroup, Run, Table, Text, UnitModule,
};
pub use records::TextRecord;

/// A seeded generator for the tests: the same sequence on every run and every
/// host.
#[cfg(test)]
pub(crate) struct Xorshift(u64);

#[cfg(test)]
impl Xorshift {
    pub(crate) fn new(seed: u64) -> Self {
        assert_ne!(seed, 0, "xorshift needs a seed other than zero");
        Self(seed)
    }

    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A number in `0..bound`.
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).expect("bound fits in u64");
        usize::try_from(self.next() % bound).expect("value below a usize bound")
    }

    /// One element of `items`, which is not empty.
    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}
