use crate::candidate::AccountNumber;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
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
    #[error("passbook ID must not be empty")]
    EmptyPassbookId,
    #[error("sidecar indexes are inconsistent")]
    InconsistentIndexes,
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
            let data: SidecarData = serde_json::from_slice(&fs::read(&path)?)?;
            validate_indexes(&data)?;
            data
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
        if passbook_id.trim().is_empty() {
            return Err(StoreError::EmptyPassbookId);
        }
        if let Some(existing) = self.data.by_account.get(&account) {
            if existing == &passbook_id {
                return Ok(());
            }
            return Err(StoreError::AccountConflict);
        }
        if self.data.by_passbook.contains_key(&passbook_id) {
            return Err(StoreError::PassbookConflict);
        }
        let mut next = self.data.clone();
        next.by_account.insert(account.clone(), passbook_id.clone());
        next.by_passbook.insert(passbook_id, account);
        self.flush(&next)?;
        self.data = next;
        Ok(())
    }

    pub fn by_account(&self, account: &AccountNumber) -> Option<&str> {
        self.data.by_account.get(account).map(String::as_str)
    }

    pub fn by_passbook(&self, passbook_id: &str) -> Option<&AccountNumber> {
        self.data.by_passbook.get(passbook_id)
    }

    fn flush(&self, data: &SidecarData) -> Result<(), StoreError> {
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            let temporary_path = parent.join(format!(".{}.tmp", temporary_name(path)));
            let payload = serde_json::to_vec_pretty(data)?;
            let result = (|| {
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                options.mode(0o600);
                let mut file = options.open(&temporary_path)?;
                file.write_all(&payload)?;
                file.sync_all()?;
                drop(file);
                fs::rename(&temporary_path, path)?;
                #[cfg(unix)]
                std::fs::File::open(parent)?.sync_all()?;
                Ok::<(), std::io::Error>(())
            })();
            if result.is_err() {
                let _ = fs::remove_file(&temporary_path);
            }
            result?;
            #[cfg(unix)]
            {
                let mut permissions = fs::metadata(path)?.permissions();
                permissions.set_mode(0o600);
                fs::set_permissions(path, permissions)?;
            }
        }
        Ok(())
    }
}

fn temporary_name(path: &Path) -> String {
    format!(
        "scytale-account-{}-{}",
        std::process::id(),
        path.file_name().unwrap_or_default().to_string_lossy()
    )
}

fn validate_indexes(data: &SidecarData) -> Result<(), StoreError> {
    if data
        .by_account
        .iter()
        .any(|(account, passbook)| data.by_passbook.get(passbook) != Some(account))
        || data
            .by_passbook
            .iter()
            .any(|(passbook, account)| data.by_account.get(account) != Some(passbook))
    {
        return Err(StoreError::InconsistentIndexes);
    }
    Ok(())
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

    #[test]
    fn persisted_bind_is_private_and_reloads_atomically() {
        let path = std::env::temp_dir().join(format!(
            "scytale-account-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let account: AccountNumber = "SCY-000002".parse().unwrap();
        {
            let mut store = AliasStore::open(&path).unwrap();
            store.bind(account.clone(), "passbook-2").unwrap();
        }
        #[cfg(unix)]
        {
            let permissions = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(permissions, 0o600);
        }
        let restored = AliasStore::open(&path).unwrap();
        assert_eq!(restored.by_account(&account), Some("passbook-2"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn empty_passbook_id_is_rejected_without_mutating_state() {
        let mut store = AliasStore::in_memory();
        let account: AccountNumber = "SCY-000003".parse().unwrap();
        assert!(matches!(
            store.bind(account.clone(), "  "),
            Err(StoreError::EmptyPassbookId)
        ));
        assert_eq!(store.by_account(&account), None);
    }

    #[test]
    fn inconsistent_sidecar_indexes_are_rejected() {
        let path = std::env::temp_dir().join(format!(
            "scytale-account-corrupt-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            r#"{"by_account":{"SCY-000004":"passbook-a"},"by_passbook":{}}"#,
        )
        .unwrap();
        assert!(matches!(
            AliasStore::open(&path),
            Err(StoreError::InconsistentIndexes)
        ));
        let _ = fs::remove_file(path);
    }
}
