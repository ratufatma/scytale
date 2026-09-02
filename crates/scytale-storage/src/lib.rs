//! Scytale Storage: ACID-compliant embedded storage engine using Redb.

use std::path::Path;
use redb::{
    CommitError, Database, DatabaseError, TableDefinition, TableError, TransactionError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
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

pub const META_TABLE: TableDefinition<&str, &str> = TableDefinition::new("meta");

pub struct StorageEngine {
    db: Database,
}

impl StorageEngine {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = Database::create(path)?;
        Ok(Self { db })
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(META_TABLE)?;
            table.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(META_TABLE)?;
        let val = table.get(key)?;
        Ok(val.map(|v| v.value().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redb::TableHandle;

    #[test]
    fn test_meta_table_definition() {
        assert_eq!(META_TABLE.name(), "meta");
    }
}
