use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use serde::{Deserialize, Serialize};
use std::fs;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const MEMORY_KIB: u32 = 65_536;
const ITERATIONS: u32 = 3;
const LANES: u32 = 4;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VaultError {
    #[error("PIN must contain exactly six digits")]
    InvalidPin,
    #[error("a tracer was detected")]
    TracerDetected,
    #[error("key derivation failed")]
    KeyDerivation,
    #[error("encryption failed")]
    Encryption,
    #[error("PIN salah, silakan coba lagi")]
    Authentication,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedKeyEnvelope {
    pub version: u8,
    pub salt: [u8; 32],
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Zeroize, ZeroizeOnDrop)]
pub struct PinCode(String);

impl PinCode {
    pub fn new(pin: &str) -> Result<Self, VaultError> {
        if pin.len() != 6 || !pin.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(VaultError::InvalidPin);
        }
        Ok(Self(pin.to_owned()))
    }
}

pub fn is_being_traced() -> bool {
    #[cfg(target_os = "linux")]
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        return status.lines().any(|line| {
            line.strip_prefix("TracerPid:")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<u32>().ok())
                .is_some_and(|pid| pid != 0)
        });
    }
    false
}

fn derive_key(pin: &PinCode, salt: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let params = Params::new(MEMORY_KIB, ITERATIONS, LANES, Some(32))
        .map_err(|_| VaultError::KeyDerivation)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(pin.0.as_bytes(), salt, key.as_mut())
        .map_err(|_| VaultError::KeyDerivation)?;
    Ok(key)
}

pub fn encrypt_key(key_id: &[u8], pin: &str) -> Result<EncryptedKeyEnvelope, VaultError> {
    if is_being_traced() {
        return Err(VaultError::TracerDetected);
    }
    let pin = PinCode::new(pin)?;
    let mut salt = [0u8; 32];
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut salt).map_err(|_| VaultError::Encryption)?;
    getrandom::getrandom(&mut nonce).map_err(|_| VaultError::Encryption)?;
    let derived = derive_key(&pin, &salt)?;
    let cipher =
        ChaCha20Poly1305::new_from_slice(derived.as_ref()).map_err(|_| VaultError::Encryption)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), key_id)
        .map_err(|_| VaultError::Encryption)?;
    Ok(EncryptedKeyEnvelope {
        version: 1,
        salt,
        nonce,
        ciphertext,
    })
}

pub fn decrypt_key(
    envelope: &EncryptedKeyEnvelope,
    pin: &str,
) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    if is_being_traced() {
        return Err(VaultError::TracerDetected);
    }
    if envelope.version != 1 {
        return Err(VaultError::Authentication);
    }
    let pin = PinCode::new(pin)?;
    let derived = derive_key(&pin, &envelope.salt)?;
    let cipher = ChaCha20Poly1305::new_from_slice(derived.as_ref())
        .map_err(|_| VaultError::Authentication)?;
    cipher
        .decrypt(
            Nonce::from_slice(&envelope.nonce),
            envelope.ciphertext.as_ref(),
        )
        .map(Zeroizing::new)
        .map_err(|_| VaultError::Authentication)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_round_trip_and_wrong_pin() {
        let envelope = encrypt_key(b"passbook-key-id", "123456").unwrap();
        assert_eq!(
            &*decrypt_key(&envelope, "123456").unwrap(),
            b"passbook-key-id"
        );
        assert_eq!(
            decrypt_key(&envelope, "654321"),
            Err(VaultError::Authentication)
        );
    }
}
