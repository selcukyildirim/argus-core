//! Centralized error types for the Argus workspace.

use thiserror::Error;

/// Top-level error enum. Variants map to subsystems.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ArgusError {
    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Simulation error: {0}")]
    Simulation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type ArgusResult<T> = Result<T, ArgusError>;
