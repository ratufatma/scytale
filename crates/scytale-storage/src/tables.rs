use redb::TableDefinition;

// ─────────────────────────────────────────────────────────────────────────────
// Canonical Table Schema Definitions
// ─────────────────────────────────────────────────────────────────────────────

/// Stores full Block payloads. Key: BlockHash ([u8; 32]), Value: canonical bytes.
pub const BLOCKS: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("blocks");

/// Stores full Transaction payloads. Key: TxID ([u8; 32]), Value: canonical bytes.
pub const TRANSACTIONS: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("transactions");

/// Stores UTXO entries. Key: OutPoint key ([u8; 36] = 32-byte TxID + 4-byte index LE),
/// Value: canonical bytes of UtxoEntry.
pub const UTXOS: TableDefinition<&[u8; 36], &[u8]> = TableDefinition::new("utxos");

/// Stores per-block metadata (height, cumulative work, timestamp).
/// Key: BlockHash ([u8; 32]), Value: serialized BlockMeta.
pub const BLOCK_INDEX: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("block_index");

/// Stores named chain state scalars.
/// Key: ASCII name string (e.g. "tip_hash", "tip_height"), Value: raw bytes.
pub const CHAIN_STATE: TableDefinition<&str, &[u8]> = TableDefinition::new("chain_state");

/// Indexes transactions by address and block height for fast Passbook queries.
/// Key: [u8; 40] = address_bytes (32B) || block_height (8B Big-Endian)
/// Value: canonical bytes of Vec<AddressTxRecord>
pub const ADDRESS_TX_INDEX: TableDefinition<&[u8; 40], &[u8]> =
    TableDefinition::new("address_tx_index");

// ─────────────────────────────────────────────────────────────────────────────
// Legacy compatibility tables (kept for existing unit test)
// ─────────────────────────────────────────────────────────────────────────────
pub const META_TABLE: redb::TableDefinition<&str, &str> = redb::TableDefinition::new("meta");
pub const BLOCKS_TABLE: redb::TableDefinition<&[u8; 32], &[u8]> =
    redb::TableDefinition::new("blocks");
pub const UTXO_TABLE: redb::TableDefinition<&[u8], &[u8]> = redb::TableDefinition::new("utxos");

// ─────────────────────────────────────────────────────────────────────────────
// CHAIN_STATE key constants
// ─────────────────────────────────────────────────────────────────────────────
pub const KEY_TIP_HASH: &str = "tip_hash";
pub const KEY_TIP_HEIGHT: &str = "tip_height";

// ─────────────────────────────────────────────────────────────────────────────
// BlockMeta: compact index record stored in BLOCK_INDEX
// ─────────────────────────────────────────────────────────────────────────────

/// Compact per-block metadata written to BLOCK_INDEX.
/// Layout (little-endian): [ height: u64 | cumulative_work: [u64; 4] | timestamp: u64 ] = 56 bytes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMeta {
    pub height: u64,
    pub cumulative_work: [u64; 4],
    pub timestamp: u64,
}

impl BlockMeta {
    pub const BYTE_LEN: usize = 8 + 32 + 8; // 48 bytes

    pub fn to_bytes(&self) -> [u8; Self::BYTE_LEN] {
        let mut buf = [0u8; Self::BYTE_LEN];
        buf[0..8].copy_from_slice(&self.height.to_le_bytes());
        for (i, word) in self.cumulative_work.iter().enumerate() {
            buf[8 + i * 8..16 + i * 8].copy_from_slice(&word.to_le_bytes());
        }
        buf[40..48].copy_from_slice(&self.timestamp.to_le_bytes());
        buf
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < Self::BYTE_LEN {
            return None;
        }
        let height = u64::from_le_bytes(b[0..8].try_into().ok()?);
        let mut cumulative_work = [0u64; 4];
        for (i, word) in cumulative_work.iter_mut().enumerate() {
            *word = u64::from_le_bytes(b[8 + i * 8..16 + i * 8].try_into().ok()?);
        }
        let timestamp = u64::from_le_bytes(b[40..48].try_into().ok()?);
        Some(Self {
            height,
            cumulative_work,
            timestamp,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AddressTxRecord: Per-address transaction record
// ─────────────────────────────────────────────────────────────────────────────

use scytale_core::{CanonicalDeserialize, CanonicalSerialize, Hash256, SerializationError};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};

/// Financial transaction record for a specific address at a given block height.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressTxRecord {
    pub txid: Hash256,
    pub is_input: bool,
    pub is_output: bool,
    pub value_quanta: u64,
    pub token_id: Option<[u8; 32]>,
}

impl CanonicalSerialize for AddressTxRecord {
    fn serialize_canonical<W: Write>(&self, writer: &mut W) -> Result<(), SerializationError> {
        self.txid.serialize_canonical(writer)?;
        self.is_input.serialize_canonical(writer)?;
        self.is_output.serialize_canonical(writer)?;
        self.value_quanta.serialize_canonical(writer)?;
        match &self.token_id {
            Some(token) => {
                1u8.serialize_canonical(writer)?;
                writer.write_all(token)?;
            }
            None => {
                0u8.serialize_canonical(writer)?;
            }
        }
        Ok(())
    }
}

impl CanonicalDeserialize for AddressTxRecord {
    fn deserialize_canonical<R: Read>(reader: &mut R) -> Result<Self, SerializationError> {
        let txid = Hash256::deserialize_canonical(reader)?;
        let is_input = bool::deserialize_canonical(reader)?;
        let is_output = bool::deserialize_canonical(reader)?;
        let value_quanta = u64::deserialize_canonical(reader)?;
        let has_token = u8::deserialize_canonical(reader)?;
        let token_id = match has_token {
            0 => None,
            1 => {
                let mut token = [0u8; 32];
                reader.read_exact(&mut token)?;
                Some(token)
            }
            _ => return Err(SerializationError::InvalidEncoding),
        };
        Ok(Self {
            txid,
            is_input,
            is_output,
            value_quanta,
            token_id,
        })
    }
}

/// Encodes an address hash and block height into the fixed 40-byte key for `ADDRESS_TX_INDEX`.
/// Key: [u8; 40] = address_bytes (32B) || block_height (8B Big-Endian).
pub fn make_address_tx_key(address_hash: &[u8; 32], height: u64) -> [u8; 40] {
    let mut key = [0u8; 40];
    key[..32].copy_from_slice(address_hash);
    key[32..].copy_from_slice(&height.to_be_bytes());
    key
}

/// Serializes a slice of `AddressTxRecord` into canonical byte payload.
pub fn serialize_address_tx_records(
    records: &[AddressTxRecord],
) -> Result<Vec<u8>, SerializationError> {
    let mut buf = Vec::new();
    (records.len() as u32).serialize_canonical(&mut buf)?;
    for record in records {
        record.serialize_canonical(&mut buf)?;
    }
    Ok(buf)
}

/// Deserializes a canonical byte payload back into a `Vec<AddressTxRecord>`.
pub fn deserialize_address_tx_records(
    bytes: &[u8],
) -> Result<Vec<AddressTxRecord>, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let count = u32::deserialize_canonical(&mut cursor)? as usize;
    if count > scytale_core::MAX_VECTOR_LENGTH {
        return Err(SerializationError::LengthExceedsLimit {
            length: count,
            max: scytale_core::MAX_VECTOR_LENGTH,
        });
    }
    let mut records = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        records.push(AddressTxRecord::deserialize_canonical(&mut cursor)?);
    }
    let pos = cursor.position() as usize;
    if pos < bytes.len() {
        return Err(SerializationError::TrailingBytes(bytes.len() - pos));
    }
    Ok(records)
}

/// Extracts a 32-byte public key hash or script hash address from a transaction locking condition.
pub fn extract_address_from_locking_condition(script: &[u8]) -> Option<[u8; 32]> {
    // 1. Check standard P2PKH:
    // OP_DUP(0x73) OP_BLAKE3(0xa0) OP_PUSHBYTES_32(0x20) [32B hash] OP_EQUALVERIFY(0x88) OP_CHECKSIG(0xac)
    if script.len() == 37
        && script[0] == 0x73
        && script[1] == 0xa0
        && script[2] == 0x20
        && script[35] == 0x88
        && script[36] == 0xac
    {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&script[3..35]);
        return Some(hash);
    }

    // 2. Check OutputLock format
    if let Some(lock) = scytale_core::OutputLock::from_locking_condition(script) {
        match lock {
            scytale_core::OutputLock::PublicKey(pk) => {
                return Some(*blake3::hash(&pk).as_bytes());
            }
            scytale_core::OutputLock::Script { script_hash, .. } => {
                return Some(script_hash);
            }
        }
    }

    // 3. Raw 32-byte address hash
    if script.len() == 32 {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(script);
        return Some(hash);
    }

    // 4. Any other non-empty script: derive 32-byte address hash via BLAKE3
    if !script.is_empty() {
        return Some(*blake3::hash(script).as_bytes());
    }

    None
}
