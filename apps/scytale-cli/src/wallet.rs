//! Non-Custodial CLI Wallet and P2PKH Key Management module.
//!
//! Provides local Ed25519 asymmetric key pair generation, BLAKE3 address derivation,
//! strict POSIX file permissions (0600), and ScytaleScript P2PKH template builders.

use ed25519_dalek::SigningKey;
use scytale_account::{decrypt_key, encrypt_key, EncryptedKeyEnvelope};
use scytale_core::Address;
use scytale_primitives::{from_hex, to_hex};
use scytale_script::{builder::ScriptBuilder, opcode::OpCode};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

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
    #[error("Wallet does not contain a plaintext private key")]
    MissingPrivateKey,
    #[error("Encrypted wallet still contains plaintext secrets")]
    EncryptedWalletContainsPlaintext,
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
    #[error("PIN vault error: {0}")]
    Vault(String),
}

/// Persistent non-custodial wallet file representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletFile {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    pub public_key: String,
    #[serde(alias = "p2pkh_address")]
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passbook_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_key: Option<EncryptedKeyEnvelope>,
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
            private_key: Some(to_hex(&privkey_bytes)),
            public_key: to_hex(&pubkey_bytes),
            address: bech32_addr,
            account_number: None,
            passbook_id: None,
            encrypted_key: None,
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
            private_key: Some(to_hex(&privkey_bytes)),
            public_key: to_hex(&pubkey_bytes),
            address: bech32_addr,
            account_number: None,
            passbook_id: None,
            encrypted_key: None,
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
            private_key: Some(to_hex(&privkey_bytes)),
            public_key: to_hex(&pubkey_bytes),
            address: bech32_addr,
            account_number: None,
            passbook_id: None,
            encrypted_key: None,
        };

        wallet.save_to(path)?;
        Ok(wallet)
    }

    /// Loads a wallet file from the given path.
    pub fn load_from(path: &Path) -> Result<Self, WalletError> {
        if !path.exists() {
            return Err(WalletError::FileNotFound(path.to_path_buf()));
        }
        let content = Zeroizing::new(std::fs::read_to_string(path)?);
        let mut wallet: Self = serde_json::from_str(&content)?;
        if wallet.encrypted_key.is_some()
            && (wallet.private_key.is_some() || wallet.mnemonic.is_some())
        {
            wallet.clear_plaintext_secrets();
            wallet.save_to(path)?;
        }
        Ok(wallet)
    }

    /// Saves the wallet to disk with restrictive 0600 POSIX permissions.
    pub fn save_to(&self, path: &Path) -> Result<(), WalletError> {
        if self.encrypted_key.is_some()
            && (self.private_key.is_some() || self.mnemonic.is_some())
        {
            return Err(WalletError::EncryptedWalletContainsPlaintext);
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let json = Zeroizing::new(serde_json::to_string_pretty(self)?);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("wallet.json");
        let temp_path = path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&temp_path)?;
            file.write_all(json.as_bytes())?;
            file.flush()?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&temp_path, json.as_bytes())?;
        }

        std::fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Encrypts the private key and removes plaintext secrets from this wallet.
    pub fn encrypt(&mut self, passphrase: &str) -> Result<(), WalletError> {
        let private_key = self
            .private_key
            .as_ref()
            .ok_or(WalletError::MissingPrivateKey)?;
        let key_id = Zeroizing::new(
            from_hex(private_key).map_err(|error| WalletError::Hex(error.to_string()))?,
        );
        let envelope = encrypt_key(&key_id, passphrase)
            .map_err(|error| WalletError::Vault(error.to_string()))?;

        self.encrypted_key = Some(envelope);
        self.clear_plaintext_secrets();
        Ok(())
    }

    fn clear_plaintext_secrets(&mut self) {
        if let Some(private_key) = self.private_key.as_mut() {
            private_key.zeroize();
        }
        self.private_key = None;
        if let Some(mnemonic) = self.mnemonic.as_mut() {
            mnemonic.zeroize();
        }
        self.mnemonic = None;
    }

    /// Reconstructs the Ed25519 `SigningKey` from the hex seed.
    pub fn signing_key(&self) -> Result<SigningKey, WalletError> {
        let private_key = self
            .private_key
            .as_ref()
            .ok_or(WalletError::MissingPrivateKey)?;
        let bytes = Zeroizing::new(
            from_hex(private_key).map_err(|e| WalletError::Hex(e.to_string()))?,
        );
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

    pub fn signing_key_with_pin(&self, pin: &str) -> Result<SigningKey, WalletError> {
        let key_bytes = if let Some(envelope) = &self.encrypted_key {
            decrypt_key(envelope, pin)
                .map_err(|error| WalletError::Vault(error.to_string()))?
        } else {
            let private_key = self
                .private_key
                .as_ref()
                .ok_or(WalletError::MissingPrivateKey)?;
            Zeroizing::new(from_hex(private_key).map_err(|e| WalletError::Hex(e.to_string()))?)
        };
        if key_bytes.len() != 32 {
            return Err(WalletError::InvalidKeyLength {
                expected: 32,
                found: key_bytes.len(),
            });
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&key_bytes);
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
    use ed25519_dalek::{Signer, Verifier};
    use tempfile::tempdir;

    #[test]
    fn test_wallet_generation_and_loading() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wallet.json");

        let wallet = WalletFile::generate_new(&path, false).unwrap();
        assert_eq!(wallet.version, 1);
        assert_eq!(wallet.private_key.as_ref().unwrap().len(), 64);
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

    #[test]
    fn encrypted_wallet_serialization_excludes_plaintext_and_unlocks() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("encrypted-wallet.json");
        let (mut wallet, phrase) = WalletFile::generate_with_mnemonic(&path, false, 12).unwrap();
        let private_key = wallet.private_key.clone().unwrap();

        wallet.encrypt("123456").unwrap();
        wallet.save_to(&path).unwrap();

        let serialized = std::fs::read_to_string(&path).unwrap();
        assert!(!serialized.contains(&private_key));
        assert!(!serialized.contains(&phrase));
        assert!(wallet.private_key.is_none());
        assert!(wallet.mnemonic.is_none());

        let loaded = WalletFile::load_from(&path).unwrap();
        assert!(loaded.private_key.is_none());
        assert!(loaded.mnemonic.is_none());
        let signing_key = loaded.signing_key_with_pin("123456").unwrap();
        let message = b"wallet unlock regression";
        let signature = signing_key.sign(message);
        signing_key.verifying_key().verify(message, &signature).unwrap();
    }

    #[test]
    fn loading_legacy_encrypted_wallet_rewrites_without_plaintext() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy-encrypted-wallet.json");
        let (wallet, phrase) = WalletFile::generate_with_mnemonic(
            &dir.path().join("source-wallet.json"),
            false,
            12,
        )
        .unwrap();
        let private_key = wallet.private_key.clone().unwrap();
        let envelope = encrypt_key(&from_hex(&private_key).unwrap(), "123456").unwrap();
        let legacy = serde_json::json!({
            "version": wallet.version,
            "mnemonic": phrase,
            "private_key": private_key,
            "public_key": wallet.public_key,
            "address": wallet.address,
            "encrypted_key": envelope,
        });
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let loaded = WalletFile::load_from(&path).unwrap();
        let rewritten = std::fs::read_to_string(&path).unwrap();
        assert!(loaded.private_key.is_none());
        assert!(loaded.mnemonic.is_none());
        assert!(!rewritten.contains(&private_key));
        assert!(!rewritten.contains(&phrase));
        loaded.signing_key_with_pin("123456").unwrap();
    }
}
