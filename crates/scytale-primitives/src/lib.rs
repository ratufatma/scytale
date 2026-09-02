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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const ZERO: Self = Self([0u8; 32]);

    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub struct OutPoint {
    pub txid: Hash,
    pub index: u32,
}

impl OutPoint {
    pub const fn new(txid: Hash, index: u32) -> Self {
        Self { txid, index }
    }
}

/// TxOut: Represents a transaction output with a value in quanta and a locking condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    }

    #[test]
    fn test_outpoint_creation() {
        let txid = Hash::hash(b"genesis_tx");
        let outpoint = OutPoint::new(txid, 0);
        assert_eq!(outpoint.txid, txid);
        assert_eq!(outpoint.index, 0);
    }

    #[test]
    fn test_quanta_conversion() {
        let scy_coins: u64 = 5;
        let total_quanta: Quanta = scy_coins * QUANTA_PER_SCY;
        assert_eq!(total_quanta, 500_000_000);
    }
}
