use thiserror::Error;

// === Payment errors

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("invalid amount: {0}")]
    InvalidAmount(String),

    #[error("unsupported asset")]
    UnsupportedAsset,

    #[error("reference already exists: {0}")]
    DuplicateReference(String),

    #[error("payment not found: {0}")]
    NotFound(String),

    #[error("invalid status transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: crate::payment::PaymentStatus,
        to: crate::payment::PaymentStatus,
    },

    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),
}

// === Repository errors

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(String),

    #[error("not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(String),
}
