use thiserror::Error;

/// Strongly-typed errors returned by the Scytale storage engine.
#[allow(clippy::result_large_err)]
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("redb underlying error: {0}")]
    Redb(#[from] redb::Error),
    #[error("redb database error: {0}")]
    Database(#[from] redb::DatabaseError),
    #[error("redb transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),
    #[error("redb table error: {0}")]
    Table(#[from] redb::TableError),
    #[error("redb commit error: {0}")]
    Commit(#[from] redb::CommitError),
    #[error("redb storage error: {0}")]
    Storage(#[from] redb::StorageError),
    #[error("canonical codec serialization failure: {0}")]
    Serialization(String),
    #[error("requested key not found: {0}")]
    NotFound(String),
    #[error("inconsistent chain state detected: {0}")]
    InconsistentState(String),
}

impl StorageError {
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }
    pub fn not_found(key: impl Into<String>) -> Self {
        Self::NotFound(key.into())
    }
}
