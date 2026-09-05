//! Non-Custodial CLI Wallet and P2PKH Key Management module.
//!
//! Provides local Ed25519 asymmetric key pair generation, BLAKE3 address derivation,
//! strict POSIX file permissions (0600), and ScytaleScript P2PKH template builders.

use ed25519_dalek::SigningKey;
use scytale_core::Address;
use scytale_primitives::{from_hex, to_hex};
use scytale_script::{builder::ScriptBuilder, opcode::OpCode};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WalletError {
    #[error("Wallet file IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Wallet serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Hex decoding error: {0}")]
    Hex(String),
    #[error("Invalid key length: expected {expected} bytes, found {found}")]
    InvalidKeyLength { expected: usize, found: usize },
    #[error("Invalid address format: {0}")]
    InvalidAddress(String),
    #[error("Wallet file already exists at '{0}'. Use a different path or back it up.")]
    FileAlreadyExists(PathBuf),
    #[error("Wallet file not found at '{0}'. Run 'scytale-cli wallet new' to generate one.")]
    FileNotFound(PathBuf),
    #[error("Insufficient funds: required {required} quanta, available {available} quanta")]
    InsufficientFunds { required: u64, available: u64 },
    #[error("Data payload exceeds maximum limit of {max} bytes (got {size} bytes)")]
    DataPayloadTooLarge { size: usize, max: usize },
    #[error("Mnemonic error: {0}")]
    Mnemonic(String),
}

/// Persistent non-custodial wallet file representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletFile {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
    pub private_key: String,
    pub public_key: String,
    #[serde(alias = "p2pkh_address")]
    pub address: String,
}

impl WalletFile {
    /// Returns the default wallet file path: `~/.scytale/wallet.json`.
    pub fn default_path() -> PathBuf {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".scytale").join("wallet.json")
    }

    /// Generates a new cryptographic Ed25519 keypair and writes a POSIX 0600 wallet file.
    pub fn generate_new(path: &Path, overwrite: bool) -> Result<Self, WalletError> {
        if path.exists() && !overwrite {
            return Err(WalletError::FileAlreadyExists(path.to_path_buf()));
        }

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_bytes = verifying_key.to_bytes();
        let privkey_bytes = signing_key.to_bytes();
        let address_bytes = *blake3::hash(&pubkey_bytes).as_bytes();
        let bech32_addr = Address::new(address_bytes)
            .to_bech32()
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;

        let wallet = Self {
            version: 1,
            mnemonic: None,
            private_key: to_hex(&privkey_bytes),
            public_key: to_hex(&pubkey_bytes),
            address: bech32_addr,
        };

        wallet.save_to(path)?;
        Ok(wallet)
    }

    /// Generates a new cryptographic Ed25519 keypair derived from a BIP-39 mnemonic phrase.
    pub fn generate_with_mnemonic(
        path: &Path,
        overwrite: bool,
        word_count: usize,
    ) -> Result<(Self, String), WalletError> {
        if path.exists() && !overwrite {
            return Err(WalletError::FileAlreadyExists(path.to_path_buf()));
        }

        let mut entropy = match word_count {
            12 => vec![0u8; 16],
            24 => vec![0u8; 32],
            other => {
                return Err(WalletError::Mnemonic(format!(
                    "Unsupported word count: {other}. Expected 12 or 24."
                )))
            }
        };
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut entropy);
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy)
            .map_err(|e| WalletError::Mnemonic(e.to_string()))?;

        let phrase = mnemonic.to_string();
        let seed = mnemonic.to_seed("");
        let mut privkey_bytes = [0u8; 32];
        privkey_bytes.copy_from_slice(&seed[0..32]);

        let signing_key = SigningKey::from_bytes(&privkey_bytes);
        let verifying_key = signing_key.verifying_key();
        let pubkey_bytes = verifying_key.to_bytes();
        let address_bytes = *blake3::hash(&pubkey_bytes).as_bytes();
        let bech32_addr = Address::new(address_bytes)
            .to_bech32()
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;

        let wallet = Self {
            version: 2,
            mnemonic: Some(phrase.clone()),
            private_key: to_hex(&privkey_bytes),
            public_key: to_hex(&pubkey_bytes),
            address: bech32_addr,
        };

        wallet.save_to(path)?;
        Ok((wallet, phrase))
    }

    /// Restores a wallet from an existing BIP-39 mnemonic phrase.
    pub fn restore_from_mnemonic(
        path: &Path,
        phrase: &str,
        overwrite: bool,
    ) -> Result<Self, WalletError> {
        if path.exists() && !overwrite {
            return Err(WalletError::FileAlreadyExists(path.to_path_buf()));
        }

        let clean_phrase = phrase.split_whitespace().collect::<Vec<_>>().join(" ");
        let mnemonic =
            bip39::Mnemonic::parse_in_normalized(bip39::Language::English, &clean_phrase)
                .map_err(|e| WalletError::Mnemonic(e.to_string()))?;

        let seed = mnemonic.to_seed("");
        let mut privkey_bytes = [0u8; 32];
        privkey_bytes.copy_from_slice(&seed[0..32]);

        let signing_key = SigningKey::from_bytes(&privkey_bytes);
        let verifying_key = signing_key.verifying_key();
        let pubkey_bytes = verifying_key.to_bytes();
        let address_bytes = *blake3::hash(&pubkey_bytes).as_bytes();
        let bech32_addr = Address::new(address_bytes)
            .to_bech32()
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;

        let wallet = Self {
            version: 2,
            mnemonic: Some(clean_phrase),
            private_key: to_hex(&privkey_bytes),
            public_key: to_hex(&pubkey_bytes),
            address: bech32_addr,
        };

        wallet.save_to(path)?;
        Ok(wallet)
    }

    /// Loads a wallet file from the given path.
    pub fn load_from(path: &Path) -> Result<Self, WalletError> {
        if !path.exists() {
            return Err(WalletError::FileNotFound(path.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        let wallet: Self = serde_json::from_str(&content)?;
        Ok(wallet)
    }

    /// Saves the wallet to disk with restrictive 0600 POSIX permissions.
    pub fn save_to(&self, path: &Path) -> Result<(), WalletError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let json = serde_json::to_string_pretty(self)?;

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)?;
            file.write_all(json.as_bytes())?;
            file.flush()?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(path, json.as_bytes())?;
        }

        Ok(())
    }

    /// Reconstructs the Ed25519 `SigningKey` from the hex seed.
    pub fn signing_key(&self) -> Result<SigningKey, WalletError> {
        let bytes = from_hex(&self.private_key).map_err(|e| WalletError::Hex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(WalletError::InvalidKeyLength {
                expected: 32,
                found: bytes.len(),
            });
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        Ok(SigningKey::from_bytes(&seed))
    }

    /// Extracts the 32-byte public key bytes.
    pub fn verifying_key_bytes(&self) -> Result<[u8; 32], WalletError> {
        let bytes = from_hex(&self.public_key).map_err(|e| WalletError::Hex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(WalletError::InvalidKeyLength {
                expected: 32,
                found: bytes.len(),
            });
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(key)
    }

    /// Extracts the 32-byte BLAKE3 address hash bytes.
    /// Tolerantly parses either a Bech32 (`scy1...`) or legacy raw hex string.
    pub fn address_bytes(&self) -> Result<[u8; 32], WalletError> {
        let addr = Address::parse(&self.address)
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
        Ok(*addr.hash())
    }

    /// Returns the address formatted as a canonical Bech32 string (`scy1...`).
    pub fn bech32_address(&self) -> Result<String, WalletError> {
        let addr = Address::parse(&self.address)
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
        addr.to_bech32()
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))
    }

    /// Generates the standard P2PKH locking script for this wallet's address.
    pub fn p2pkh_locking_script(&self) -> Result<Vec<u8>, WalletError> {
        let addr = self.address_bytes()?;
        Ok(build_p2pkh_locking_script(&addr))
    }
}

/// Builds the canonical P2PKH locking script (ScriptPubKey):
/// `OP_DUP OP_BLAKE3 <address_hash: 32B> OP_EQUALVERIFY OP_CHECKSIG`
pub fn build_p2pkh_locking_script(address_hash: &[u8; 32]) -> Vec<u8> {
    ScriptBuilder::new()
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpBlake3)
        .push_data(address_hash)
        .push_opcode(OpCode::OpEqualVerify)
        .push_opcode(OpCode::OpCheckSig)
        .build()
}

/// Builds the canonical P2PKH unlocking script (ScriptSig / Authorization):
/// `<sig: 64B> <pubkey: 32B>`
pub fn build_p2pkh_unlocking_script(sig: &[u8; 64], pubkey: &[u8; 32]) -> Vec<u8> {
    ScriptBuilder::new()
        .push_data(sig)
        .push_data(pubkey)
        .build()
}

/// Builds an OP_RETURN data carrier script with the given payload:
/// `OP_RETURN <payload>`
pub fn build_op_return_script(data: &[u8]) -> Vec<u8> {
    ScriptBuilder::new()
        .push_opcode(OpCode::OpReturn)
        .push_data(data)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wallet_generation_and_loading() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wallet.json");

        let wallet = WalletFile::generate_new(&path, false).unwrap();
        assert_eq!(wallet.version, 1);
        assert_eq!(wallet.private_key.len(), 64);
        assert_eq!(wallet.public_key.len(), 64);
        assert!(wallet.address.starts_with("scy1"));

        // Permissions check on unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&path).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "Wallet file must be 0600");
        }

        // Duplicate create without overwrite should error
        assert!(matches!(
            WalletFile::generate_new(&path, false),
            Err(WalletError::FileAlreadyExists(_))
        ));

        // Load existing wallet
        let loaded = WalletFile::load_from(&path).unwrap();
        assert_eq!(wallet, loaded);

        // Keys and address consistency
        let signing_key = wallet.signing_key().unwrap();
        let verifying_key = signing_key.verifying_key();
        assert_eq!(to_hex(verifying_key.as_bytes()), wallet.public_key);
        let derived_addr = blake3::hash(verifying_key.as_bytes());
        assert_eq!(*derived_addr.as_bytes(), wallet.address_bytes().unwrap());
        let expected_bech32 = Address::new(*derived_addr.as_bytes()).to_bech32().unwrap();
        assert_eq!(expected_bech32, wallet.address);
    }

    #[test]
    fn test_p2pkh_script_builders() {
        let addr = [0x55u8; 32];
        let lock_script = build_p2pkh_locking_script(&addr);
        assert_eq!(lock_script[0], OpCode::OpDup as u8);
        assert_eq!(lock_script[1], OpCode::OpBlake3 as u8);

        let sig = [0x77u8; 64];
        let pubkey = [0x88u8; 32];
        let unlock_script = build_p2pkh_unlocking_script(&sig, &pubkey);
        assert!(!unlock_script.is_empty());
    }
}
