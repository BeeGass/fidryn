//! Evaluator failures. These are not legal outcomes.

use thiserror::Error;

/// Engine failure. Unknown queries, exhausted fuel, and unsupported
/// operations are never [`crate::Outcome::Inconsistent`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EngineError {
    #[error("unknown query {0}")]
    UnknownQuery(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("fuel exhausted (remaining {remaining})")]
    FuelExhausted { remaining: u32 },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("internal engine error: {0}")]
    Internal(String),
}
