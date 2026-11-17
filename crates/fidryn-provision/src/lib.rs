//! The provision model: addresses, the normalization of text and the file
//! path of a module.
//!
//! This crate depends on no other crate of the workspace and names no corpus;
//! `xtask/tests/corpus_neutrality.rs` checks the second rule.

mod address;
pub mod layout;
pub mod normalize;

pub use address::{Address, AddressError};
pub use layout::LayoutError;

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
