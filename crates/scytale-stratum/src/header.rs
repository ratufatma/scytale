use crate::error::StratumError;
use byteorder::{ByteOrder, LittleEndian};
use scytale_core::{BlockHeader, Hash256};
use serde::{Deserialize, Serialize};

/// Exact canonical Scytale block header size without padding (120 bytes).
pub const HEADER_SIZE: usize = 120;

/// Fixed 120-byte block header representation for Stratum mining workers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawBlockHeader120 {
    pub version: u32,
    pub prev_hash: Hash256,
    pub merkle_root: Hash256,
    pub utxo_root: Hash256,
    pub timestamp: u64,
    pub bits: u32,
    pub nonce: u64,
}

impl RawBlockHeader120 {
    /// Constructs a new 120-byte block header.
    pub fn new(
        version: u32,
        prev_hash: Hash256,
        merkle_root: Hash256,
        utxo_root: Hash256,
        timestamp: u64,
        bits: u32,
        nonce: u64,
    ) -> Self {
        Self {
            version,
            prev_hash,
            merkle_root,
            utxo_root,
            timestamp,
            bits,
            nonce,
        }
    }

    /// Serializes the header into exactly 120 little-endian binary bytes.
    pub fn serialize(&self) -> [u8; HEADER_SIZE] {
        let mut bytes = [0u8; HEADER_SIZE];
        LittleEndian::write_u32(&mut bytes[0..4], self.version);
        bytes[4..36].copy_from_slice(self.prev_hash.as_bytes());
        bytes[36..68].copy_from_slice(self.merkle_root.as_bytes());
        bytes[68..100].copy_from_slice(self.utxo_root.as_bytes());
        LittleEndian::write_u64(&mut bytes[100..108], self.timestamp);
        LittleEndian::write_u32(&mut bytes[108..112], self.bits);
        LittleEndian::write_u64(&mut bytes[112..120], self.nonce);
        bytes
    }

    /// Deserializes a 120-byte slice into a `RawBlockHeader120`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StratumError> {
        if bytes.len() != HEADER_SIZE {
            return Err(StratumError::Protocol(format!(
                "Invalid header length: expected {} bytes, got {}",
                HEADER_SIZE,
                bytes.len()
            )));
        }

        let version = LittleEndian::read_u32(&bytes[0..4]);
        let prev_hash = Hash256::new(
            bytes[4..36]
                .try_into()
                .map_err(|_| StratumError::Protocol("Invalid prev_hash slice".into()))?,
        );
        let merkle_root = Hash256::new(
            bytes[36..68]
                .try_into()
                .map_err(|_| StratumError::Protocol("Invalid merkle_root slice".into()))?,
        );
        let utxo_root = Hash256::new(
            bytes[68..100]
                .try_into()
                .map_err(|_| StratumError::Protocol("Invalid utxo_root slice".into()))?,
        );
        let timestamp = LittleEndian::read_u64(&bytes[100..108]);
        let bits = LittleEndian::read_u32(&bytes[108..112]);
        let nonce = LittleEndian::read_u64(&bytes[112..120]);

        Ok(Self {
            version,
            prev_hash,
            merkle_root,
            utxo_root,
            timestamp,
            bits,
            nonce,
        })
    }

    /// Computes the cryptographic Proof-of-Work BLAKE3 hash of this 120-byte header.
    pub fn compute_pow_hash(&self) -> Hash256 {
        let serialized = self.serialize();
        let hash = blake3::hash(&serialized);
        Hash256::new(*hash.as_bytes())
    }
}

impl From<RawBlockHeader120> for BlockHeader {
    fn from(raw: RawBlockHeader120) -> Self {
        BlockHeader::new(
            raw.version,
            raw.prev_hash,
            raw.merkle_root,
            raw.utxo_root,
            raw.timestamp,
            raw.bits,
            raw.nonce,
        )
    }
}

impl From<BlockHeader> for RawBlockHeader120 {
    fn from(header: BlockHeader) -> Self {
        Self {
            version: header.version,
            prev_hash: header.previous_block_hash,
            merkle_root: header.transaction_commitment,
            utxo_root: header.utxo_root,
            timestamp: header.timestamp,
            bits: header.difficulty_target,
            nonce: header.nonce,
        }
    }
}
