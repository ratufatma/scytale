use crate::candidate::AccountNumber;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sidecar I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sidecar serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("account number is already bound")]
    AccountConflict,
    #[error("passbook ID is already bound")]
    PassbookConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SidecarData {
    by_account: HashMap<AccountNumber, String>,
    by_passbook: HashMap<String, AccountNumber>,
}

/// Optional alias store. It is independent from consensus and UTXO storage.
#[derive(Debug, Clone)]
pub struct AliasStore {
    path: Option<PathBuf>,
    data: SidecarData,
}

impl AliasStore {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            data: SidecarData::default(),
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_owned();
        let data = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            SidecarData::default()
        };
        Ok(Self {
            path: Some(path),
            data,
        })
    }

    pub fn bind(
        &mut self,
        account: AccountNumber,
        passbook_id: impl Into<String>,
    ) -> Result<(), StoreError> {
        let passbook_id = passbook_id.into();
        if let Some(existing) = self.data.by_account.get(&account) {
            if existing == &passbook_id {
                return Ok(());
            }
            return Err(StoreError::AccountConflict);
        }
        if self.data.by_passbook.contains_key(&passbook_id) {
            return Err(StoreError::PassbookConflict);
        }
        self.data
            .by_account
            .insert(account.clone(), passbook_id.clone());
        self.data.by_passbook.insert(passbook_id, account);
        self.flush()
    }

    pub fn by_account(&self, account: &AccountNumber) -> Option<&str> {
        self.data.by_account.get(account).map(String::as_str)
    }

    pub fn by_passbook(&self, passbook_id: &str) -> Option<&AccountNumber> {
        self.data.by_passbook.get(passbook_id)
    }

    fn flush(&self) -> Result<(), StoreError> {
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, serde_json::to_vec_pretty(&self.data)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_is_first_come_first_served_and_bidirectional() {
        let mut store = AliasStore::in_memory();
        let account: AccountNumber = "SCY-000001".parse().unwrap();
        store.bind(account.clone(), "passbook-1").unwrap();
        assert_eq!(store.by_account(&account), Some("passbook-1"));
        assert_eq!(store.by_passbook("passbook-1"), Some(&account));
        assert!(matches!(
            store.bind(account, "passbook-2"),
            Err(StoreError::AccountConflict)
        ));
    }
}
