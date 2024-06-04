use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Error)]
pub enum VdbError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Checksum mismatch: expected {expected}, found {found}")]
    ChecksumMismatch { expected: String, found: String },
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    #[error("Vector not found with id: {0}")]
    VectorNotFound(u64),
    #[error("Duplicate vector id: {0}")]
    DuplicateId(u64),
    #[error("Dimension mismatch: expected {expected}, found {found}")]
    DimensionMismatch { expected: usize, found: usize },
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("Transaction aborted or conflict: {0}")]
    TransactionError(String),
}
