//! Cross-crate Fidryn integration tests.
//!
//! Programs compile through `fidryn_cli::compile_source` (in-memory, empty
//! manifest). That is the mill paste path: no `check_path`, no `packages/`.

mod adversarial_regressions;
mod boundary_review_20260917;
mod integration_suite;
mod review_regressions;
mod semantic_regressions;
