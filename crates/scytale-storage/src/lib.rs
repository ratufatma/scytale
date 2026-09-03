//! Scytale Storage: ACID-compliant embedded storage engine using redb.
//!
//! # Architecture
//! `scytale-storage` is a one-way dependency: it imports `scytale-core` types
//! but `scytale-core` MUST NOT import `redb` or `scytale-storage`.
//!
//! All block commits are executed inside a single `redb::WriteTransaction`,
//! guaranteeing all-or-nothing atomicity across all five canonical tables.

pub mod engine;
pub mod error;
pub mod tables;

pub use engine::{outpoint_to_key, StorageEngine, UtxoSnapshotDto};
pub use error::StorageError;
pub use tables::{BlockMeta, BLOCKS, BLOCK_INDEX, CHAIN_STATE, TRANSACTIONS, UTXOS};

// ── Legacy compatibility re-exports ──────────────────────────────────────────
pub use tables::{BLOCKS_TABLE, META_TABLE, UTXO_TABLE};

use redb::{CommitError, DatabaseError, TableError, TransactionError};

/// Legacy `StorageError` alias for backward compatibility with the existing unit test.
#[allow(clippy::result_large_err)]
#[derive(Debug, thiserror::Error)]
#[doc(hidden)]
pub enum LegacyStorageError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),
    #[error("Table error: {0}")]
    Table(#[from] TableError),
    #[error("Redb storage error: {0}")]
    Storage(#[from] redb::StorageError),
    #[error("Commit error: {0}")]
    Commit(#[from] CommitError),
    #[error("Key not found")]
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use redb::TableHandle;

    #[test]
    fn test_meta_table_definition() {
        assert_eq!(META_TABLE.name(), "meta");
        assert_eq!(BLOCKS_TABLE.name(), "blocks");
        assert_eq!(UTXO_TABLE.name(), "utxos");
    }
}
