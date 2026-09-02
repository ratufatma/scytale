//! Scytale Primitives: Atomic units, 32-byte cryptographic hashes, OutPoints, and base traits.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Smallest atomic unit of Scytale Coin: 1 SCY = 100,000,000 quanta.
pub type Quanta = u64;

/// Number of quanta per 1 SCY.
pub const QUANTA_PER_SCY: Quanta = 100_000_000;

#[derive(Debug, Error)]
pub enum PrimitiveError {
    #[error("Invalid hash length: expected 32 bytes, got {0}")]
    InvalidHashLength(usize),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// 32-byte cryptographic hash wrapper using BLAKE3.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct Hash([u8; 32]);

/// Alias for 32-byte Hash primitive.
pub type Hash256 = Hash;

impl Hash {
    pub const ZERO: Self = Self([0u8; 32]);

    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub const fn is_zero(&self) -> bool {
        let mut i = 0;
        while i < 32 {
            if self.0[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, PrimitiveError> {
        if slice.len() != 32 {
            return Err(PrimitiveError::InvalidHashLength(slice.len()));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(slice);
        Ok(Self(bytes))
    }

    #[allow(clippy::self_named_constructors)]
    pub fn hash(data: &[u8]) -> Self {
        let digest = blake3::hash(data);
        Self(*digest.as_bytes())
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// OutPoint: Uniquely identifies a specific transaction output on the ledger.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct OutPoint {
    pub txid: Hash256,
    pub index: u32,
}

impl OutPoint {
    pub const fn new(txid: Hash256, index: u32) -> Self {
        Self { txid, index }
    }

    pub const fn null() -> Self {
        Self {
            txid: Hash::ZERO,
            index: u32::MAX,
        }
    }

    pub const fn is_null(&self) -> bool {
        self.txid.is_zero() && self.index == u32::MAX
    }

    pub fn to_canonical_bytes(&self) -> [u8; 36] {
        let mut bytes = [0u8; 36];
        bytes[0..32].copy_from_slice(self.txid.as_bytes());
        bytes[32..36].copy_from_slice(&self.index.to_le_bytes());
        bytes
    }
}

/// TxOut: Represents a transaction output with a value in quanta and a locking condition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxOut {
    pub value: Quanta,
    pub locking_condition: Vec<u8>,
}

impl TxOut {
    pub fn new(value: Quanta, locking_condition: Vec<u8>) -> Self {
        Self {
            value,
            locking_condition,
        }
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12 + self.locking_condition.len());
        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes.extend_from_slice(&(self.locking_condition.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&self.locking_condition);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_hashing() {
        let data = b"scytale blockchain primitives";
        let h1 = Hash::hash(data);
        let h2 = Hash::hash(data);
        assert_eq!(h1, h2);
        assert_ne!(h1, Hash::ZERO);
        assert!(!h1.is_zero());
        assert!(Hash::ZERO.is_zero());
    }

    #[test]
    fn test_outpoint_creation() {
        let txid = Hash::hash(b"genesis_tx");
        let outpoint = OutPoint::new(txid, 0);
        assert_eq!(outpoint.txid, txid);
        assert_eq!(outpoint.index, 0);
        assert!(!outpoint.is_null());

        let null_op = OutPoint::null();
        assert!(null_op.is_null());
        assert_eq!(null_op.index, u32::MAX);
    }

    #[test]
    fn test_quanta_conversion() {
        let scy_coins: u64 = 5;
        let total_quanta: Quanta = scy_coins * QUANTA_PER_SCY;
        assert_eq!(total_quanta, 500_000_000);
    }
}
