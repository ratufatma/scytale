//! Local Identity and Wallet Manager: handles local accounts, secret keys, and locking scripts.

use scytale_primitives::{from_hex, to_hex, Hash256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("Account with alias '{0}' already exists")]
    AccountAlreadyExists(String),
    #[error("Account with alias '{0}' not found")]
    AccountNotFound(String),
    #[error("No active account is set")]
    NoActiveAccount,
    #[error("Invalid alias '{0}': alias cannot be empty")]
    InvalidAlias(String),
    #[error("Identity store IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Identity serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Metadata and cryptographic credentials for a single local account identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountRecord {
    pub alias: String,
    pub secret_key_hex: String,
    pub locking_script_hex: String,
    pub created_at_epoch: u64,
}

/// Local identity registry storing account profiles and tracking the active identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityStore {
    pub active_account: String,
    pub accounts: BTreeMap<String, AccountRecord>,
}

impl Default for IdentityStore {
    fn default() -> Self {
        let now = current_timestamp();
        let default_account = AccountRecord {
            alias: "default".to_string(),
            secret_key_hex: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
                .to_string(),
            locking_script_hex: "010203".to_string(),
            created_at_epoch: now,
        };

        let mut accounts = BTreeMap::new();
        accounts.insert("default".to_string(), default_account);

        Self {
            active_account: "default".to_string(),
            accounts,
        }
    }
}

impl IdentityStore {
    /// Returns the default filesystem path for identity registry (`~/.scytale/identities.json`).
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".scytale").join("identities.json")
    }

    /// Loads an existing identity store from `path`, or creates and saves a new default store.
    pub fn load_or_create(path: &Path) -> Result<Self, IdentityError> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            let store: Self = serde_json::from_str(&data)?;
            Ok(store)
        } else {
            let store = Self::default();
            store.save(path)?;
            Ok(store)
        }
    }

    /// Saves the identity store to the specified path.
    pub fn save(&self, path: &Path) -> Result<(), IdentityError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Creates a new account with the given alias and generates a cryptographic keypair and locking script.
    pub fn create_account(&mut self, alias: &str) -> Result<AccountRecord, IdentityError> {
        let trimmed = alias.trim();
        if trimmed.is_empty() {
            return Err(IdentityError::InvalidAlias(alias.to_string()));
        }
        if self.accounts.contains_key(trimmed) {
            return Err(IdentityError::AccountAlreadyExists(trimmed.to_string()));
        }

        let now = current_timestamp();
        // Generate entropy from timestamp, account count, and alias
        let seed_str = format!("{}:{}:{}", now, self.accounts.len(), trimmed);
        let secret_hash = Hash256::hash(seed_str.as_bytes());
        let script_hash = Hash256::hash(secret_hash.as_bytes());

        let record = AccountRecord {
            alias: trimmed.to_string(),
            secret_key_hex: to_hex(secret_hash.as_bytes()),
            locking_script_hex: to_hex(script_hash.as_bytes()),
            created_at_epoch: now,
        };

        self.accounts.insert(trimmed.to_string(), record.clone());
        Ok(record)
    }

    /// Switches the active account to the specified alias.
    pub fn switch_account(&mut self, alias: &str) -> Result<(), IdentityError> {
        let trimmed = alias.trim();
        if !self.accounts.contains_key(trimmed) {
            return Err(IdentityError::AccountNotFound(trimmed.to_string()));
        }
        self.active_account = trimmed.to_string();
        Ok(())
    }

    /// Returns a reference to the active account record.
    pub fn get_active(&self) -> Option<&AccountRecord> {
        self.accounts.get(&self.active_account)
    }

    /// Resolves an input parameter to a locking script hex string.
    ///
    /// Rules:
    /// - If `target` is `None`: returns the active account's locking script.
    /// - If `target` matches a known alias in the identity store: returns that account's locking script.
    /// - If `target` is a valid hex string: returns `target` directly.
    /// - Otherwise: returns an error indicating account was not found.
    pub fn resolve_locking_script(&self, target: Option<&str>) -> Result<String, IdentityError> {
        match target {
            None => {
                let active = self.get_active().ok_or(IdentityError::NoActiveAccount)?;
                Ok(active.locking_script_hex.clone())
            }
            Some(t) => {
                let trimmed = t.trim();
                if let Some(record) = self.accounts.get(trimmed) {
                    return Ok(record.locking_script_hex.clone());
                }
                // Check if it is a valid hex string
                if from_hex(trimmed).is_ok() {
                    return Ok(trimmed.to_string());
                }
                Err(IdentityError::AccountNotFound(trimmed.to_string()))
            }
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_identity_store() {
        let store = IdentityStore::default();
        assert_eq!(store.active_account, "default");
        assert!(store.accounts.contains_key("default"));
        let active = store.get_active().unwrap();
        assert_eq!(active.locking_script_hex, "010203");
    }

    #[test]
    fn test_create_and_switch_accounts() {
        let mut store = IdentityStore::default();
        let bob = store.create_account("bob").unwrap();
        assert_eq!(bob.alias, "bob");
        assert!(!bob.locking_script_hex.is_empty());

        assert!(store.switch_account("bob").is_ok());
        assert_eq!(store.active_account, "bob");

        assert!(matches!(
            store.create_account("bob"),
            Err(IdentityError::AccountAlreadyExists(_))
        ));
        assert!(matches!(
            store.switch_account("nonexistent"),
            Err(IdentityError::AccountNotFound(_))
        ));
    }

    #[test]
    fn test_resolve_locking_script() {
        let mut store = IdentityStore::default();
        store.create_account("alice").unwrap();

        // Resolves active (default) when None
        assert_eq!(store.resolve_locking_script(None).unwrap(), "010203");

        // Resolves alias "alice"
        let alice_script = store.accounts["alice"].locking_script_hex.clone();
        assert_eq!(
            store.resolve_locking_script(Some("alice")).unwrap(),
            alice_script
        );

        // Resolves raw hex
        assert_eq!(
            store.resolve_locking_script(Some("aabbcc")).unwrap(),
            "aabbcc"
        );

        // Errors on unknown non-hex string
        assert!(matches!(
            store.resolve_locking_script(Some("unknown_not_hex!")),
            Err(IdentityError::AccountNotFound(_))
        ));
    }
}
