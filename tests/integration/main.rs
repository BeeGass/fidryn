//! Cross-crate Fidryn integration tests.
//!
//! Programs compile through `fidryn_cli::compile_source` (in-memory, empty
//! manifest). That is the mill paste path: no `check_path`, no `packages/`.

mod adversarial_regressions;
mod boundary_review_20260917;
mod comments;
mod declaration_text;
mod deep_text;
mod integration_suite;
mod known_limits;
mod repaired_examples;
mod review_regressions;
mod rule_basis;
mod semantic_regressions;
mod unreadable_text;
mod verify_blocks;
mod whole_words;
