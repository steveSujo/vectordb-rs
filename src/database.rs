use thiserror::Error;
use serde::{Serialize, Deserialize};

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Vector not found")]
    VectorNotFound,
    #[error("Duplicate vector ID")]
    DuplicateId,
    #[error("Serialization error")]
    SerializationError,
}

pub trait Database {
    type Vector: Serialize + for<'de> Deserialize<'de>;
    
    /// Insert a vector into the database.
    fn insert(&mut self, id: u64, vector: Self::Vector) -> Result<(), DatabaseError>;
    
    /// Retrieve a vector by its ID.
    fn get(&self, id: u64) -> Result<Self::Vector, DatabaseError>;
