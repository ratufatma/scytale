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
