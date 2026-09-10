use crate::error::StorageError;
use crate::tables::{
    self, deserialize_address_tx_records, extract_address_from_locking_condition,
    make_address_tx_key, serialize_address_tx_records, AddressTxRecord, BlockMeta, KEY_TIP_HASH,
    KEY_TIP_HEIGHT,
};
use redb::{Database, ReadableTable};
use scytale_core::{
    Address, Block, CanonicalDeserialize, CanonicalSerialize, Hash256, OutPoint, Transaction,
    UtxoEntry, UtxoSet,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;

/// Authenticated snapshot of the active unspent UTXO set at a specific block height.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtxoSnapshotDto {
    pub height: u64,
    pub block_hash: Hash256,
    pub utxo_root: Hash256,
    pub entries: Vec<(OutPoint, UtxoEntry)>,
}

/// Reversible UTXO delta for one committed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockUndo {
    pub spent_utxos: Vec<(OutPoint, UtxoEntry)>,
    pub created_outpoints: Vec<OutPoint>,
}

impl BlockUndo {
    fn to_bytes(&self) -> Result<Vec<u8>, StorageError> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(self.spent_utxos.len() as u32).to_le_bytes());
        for (outpoint, entry) in &self.spent_utxos {
            bytes.extend_from_slice(
                &outpoint
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?,
            );
            bytes.extend_from_slice(
                &entry
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?,
            );
        }
        bytes.extend_from_slice(&(self.created_outpoints.len() as u32).to_le_bytes());
        for outpoint in &self.created_outpoints {
            bytes.extend_from_slice(
                &outpoint
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?,
            );
        }
        Ok(bytes)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, StorageError> {
        let mut cursor = Cursor::new(bytes);
        let read_count = |cursor: &mut Cursor<&[u8]>| -> Result<usize, StorageError> {
            let mut raw = [0u8; 4];
            std::io::Read::read_exact(cursor, &mut raw)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            Ok(u32::from_le_bytes(raw) as usize)
        };
        let spent_count = read_count(&mut cursor)?;
        let mut spent_utxos = Vec::with_capacity(spent_count);
        for _ in 0..spent_count {
            let outpoint = OutPoint::deserialize_canonical(&mut cursor)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            let entry = UtxoEntry::deserialize_canonical(&mut cursor)
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            spent_utxos.push((outpoint, entry));
        }
        let created_count = read_count(&mut cursor)?;
        let mut created_outpoints = Vec::with_capacity(created_count);
        for _ in 0..created_count {
            created_outpoints.push(
                OutPoint::deserialize_canonical(&mut cursor)
                    .map_err(|e| StorageError::serialization(e.to_string()))?,
            );
        }
        if cursor.position() != bytes.len() as u64 {
            return Err(StorageError::serialization("trailing BlockUndo bytes"));
        }
        Ok(Self {
            spent_utxos,
            created_outpoints,
        })
    }
}

// Helper: encode OutPoint as fixed 36-byte key (TxID[32] || index_LE[4])
pub fn outpoint_to_key(outpoint: &OutPoint) -> [u8; 36] {
    let mut key = [0u8; 36];
    key[..32].copy_from_slice(outpoint.txid.as_bytes());
    key[32..].copy_from_slice(&outpoint.index.to_le_bytes());
    key
}

// Helper: decode 36-byte key back to OutPoint
fn key_to_outpoint(key: &[u8; 36]) -> OutPoint {
    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(&key[..32]);
    let index = u32::from_le_bytes(key[32..36].try_into().unwrap());
    OutPoint::new(Hash256::new(hash_bytes), index)
}

/// The primary Scytale embedded storage engine.
///
/// All write operations are executed inside a single `redb::WriteTransaction` guaranteeing
/// all-or-nothing ACID atomicity: either all tables are updated or none.
pub struct StorageEngine {
    db: Database,
}

#[allow(clippy::result_large_err)]
impl StorageEngine {
    /// Opens (or creates) a persistent database at the given filesystem path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = Database::create(path).map_err(StorageError::Database)?;
        let engine = Self { db };
        engine.init_tables()?;
        Ok(engine)
    }

    /// Creates an ephemeral in-memory database. Ideal for fast unit test execution.
    pub fn in_memory() -> Result<Self, StorageError> {
        let db = Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .map_err(StorageError::Database)?;
        let engine = Self { db };
        engine.init_tables()?;
        Ok(engine)
    }

    /// Ensures all canonical tables exist, creating them if necessary.
    fn init_tables(&self) -> Result<(), StorageError> {
        let write_tx = self.db.begin_write()?;
        write_tx.open_table(tables::BLOCKS)?;
        write_tx.open_table(tables::TRANSACTIONS)?;
        write_tx.open_table(tables::UTXOS)?;
        write_tx.open_table(tables::BLOCK_INDEX)?;
        write_tx.open_table(tables::CHAIN_STATE)?;
        write_tx.open_table(tables::ADDRESS_TX_INDEX)?;
        write_tx.open_table(tables::BLOCK_UNDO_TABLE)?;
        write_tx.commit()?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Read API
    // ─────────────────────────────────────────────────────────────────────

    /// Returns the canonical-serialized block for the given hash, if present.
    pub fn get_block(&self, hash: &Hash256) -> Result<Option<Block>, StorageError> {
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::BLOCKS)?;
        let key: [u8; 32] = *hash.as_bytes();
        match table.get(&key)? {
            None => Ok(None),
            Some(v) => {
                let block = Block::from_canonical_bytes(v.value())
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                Ok(Some(block))
            }
        }
    }

    /// Returns the canonical-serialized transaction for the given TxID, if present.
    pub fn get_transaction(&self, txid: &Hash256) -> Result<Option<Transaction>, StorageError> {
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::TRANSACTIONS)?;
        let key: [u8; 32] = *txid.as_bytes();
        match table.get(&key)? {
            None => Ok(None),
            Some(v) => {
                let tx = Transaction::from_canonical_bytes(v.value())
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                Ok(Some(tx))
            }
        }
    }

    /// Returns the UTXO entry for the given OutPoint, if unspent.
    pub fn get_utxo(&self, outpoint: &OutPoint) -> Result<Option<UtxoEntry>, StorageError> {
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::UTXOS)?;
        let key = outpoint_to_key(outpoint);
        match table.get(&key)? {
            None => Ok(None),
            Some(v) => {
                let entry = UtxoEntry::from_canonical_bytes(v.value())
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                Ok(Some(entry))
            }
        }
    }

    /// Returns the current canonical tip (hash, height) from CHAIN_STATE, if set.
    pub fn get_canonical_tip(&self) -> Result<Option<(Hash256, u64)>, StorageError> {
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::CHAIN_STATE)?;

        let tip_hash = match table.get(KEY_TIP_HASH)? {
            None => return Ok(None),
            Some(v) => {
                let bytes: &[u8] = v.value();
                if bytes.len() != 32 {
                    return Err(StorageError::InconsistentState(
                        "tip_hash has wrong length".into(),
                    ));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(bytes);
                Hash256::new(arr)
            }
        };

        let tip_height = match table.get(KEY_TIP_HEIGHT)? {
            None => {
                return Err(StorageError::InconsistentState(
                    "tip_hash present but tip_height missing".into(),
                ))
            }
            Some(v) => {
                let bytes: &[u8] = v.value();
                if bytes.len() != 8 {
                    return Err(StorageError::InconsistentState(
                        "tip_height has wrong length".into(),
                    ));
                }
                u64::from_le_bytes(bytes.try_into().unwrap())
            }
        };

        Ok(Some((tip_hash, tip_height)))
    }

    /// Loads the entire unspent UTXO set into memory.
    /// Used during node startup to reconstruct the in-memory `UtxoSet`.
    pub fn load_entire_utxo_set(&self) -> Result<UtxoSet, StorageError> {
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::UTXOS)?;
        let mut utxo_set = UtxoSet::new();
        for result in table.iter()? {
            let (key_guard, value_guard) = result?;
            let key: &[u8; 36] = key_guard.value();
            let outpoint = key_to_outpoint(key);
            let entry = UtxoEntry::from_canonical_bytes(value_guard.value())
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            utxo_set
                .insert(outpoint, entry)
                .map_err(|error| StorageError::InconsistentState(error.to_string()))?;
        }
        Ok(utxo_set)
    }

    /// Computes the canonical active UTXO Merkle root directly from the stored UTXOS table.
    /// Since redb iterates keys in lexicographical B-Tree order, this visits entries
    /// in canonical OutPoint order (txid ASC, index ASC).
    pub fn compute_utxo_root(&self) -> Result<Hash256, StorageError> {
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::UTXOS)?;
        let mut leaves = Vec::new();
        for result in table.iter()? {
            let (key_guard, value_guard) = result?;
            let key: &[u8; 36] = key_guard.value();
            let outpoint = key_to_outpoint(key);
            let entry = UtxoEntry::from_canonical_bytes(value_guard.value())
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            leaves.push(scytale_core::compute_utxo_leaf(&outpoint, &entry.output));
        }
        Ok(scytale_core::compute_utxo_merkle_root(leaves))
    }

    /// Exports an authenticated snapshot of the active unspent UTXO set.
    pub fn export_utxo_snapshot(&self) -> Result<UtxoSnapshotDto, StorageError> {
        let (block_hash, height) = self.get_canonical_tip()?.unwrap_or((Hash256::ZERO, 0));
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::UTXOS)?;
        let mut entries = Vec::new();
        let mut leaves = Vec::new();
        for result in table.iter()? {
            let (key_guard, value_guard) = result?;
            let key: &[u8; 36] = key_guard.value();
            let outpoint = key_to_outpoint(key);
            let entry = UtxoEntry::from_canonical_bytes(value_guard.value())
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            leaves.push(scytale_core::compute_utxo_leaf(&outpoint, &entry.output));
            entries.push((outpoint, entry));
        }
        let utxo_root = scytale_core::compute_utxo_merkle_root(leaves);
        Ok(UtxoSnapshotDto {
            height,
            block_hash,
            utxo_root,
            entries,
        })
    }

    /// Atomically applies an authenticated UTXO snapshot to the UTXOS table.
    /// Verifies the snapshot's calculated Merkle root matches `snapshot.utxo_root`.
    pub fn apply_utxo_snapshot(&self, snapshot: &UtxoSnapshotDto) -> Result<(), StorageError> {
        // 1. Verify Merkle root of snapshot entries
        let mut sorted_entries = snapshot.entries.clone();
        sorted_entries.sort_by(|(a_op, _), (b_op, _)| {
            a_op.txid
                .cmp(&b_op.txid)
                .then_with(|| a_op.index.cmp(&b_op.index))
        });
        let leaves: Vec<Hash256> = sorted_entries
            .iter()
            .map(|(op, entry)| scytale_core::compute_utxo_leaf(op, &entry.output))
            .collect();
        let calculated_root = scytale_core::compute_utxo_merkle_root(leaves);
        if calculated_root != snapshot.utxo_root {
            return Err(StorageError::InconsistentState(format!(
                "UTXO snapshot root mismatch: expected {}, calculated {}",
                snapshot.utxo_root, calculated_root
            )));
        }

        // 2. Atomically clear old UTXOS and populate with snapshot
        let write_tx = self.db.begin_write()?;
        {
            let mut utxo_tbl = write_tx.open_table(tables::UTXOS)?;
            let existing_keys: Vec<[u8; 36]> = {
                let mut keys = Vec::new();
                for result in utxo_tbl.iter()? {
                    let (k, _) = result?;
                    keys.push(*k.value());
                }
                keys
            };
            for k in existing_keys {
                utxo_tbl.remove(&k)?;
            }

            for (outpoint, entry) in &sorted_entries {
                let key = outpoint_to_key(outpoint);
                let entry_bytes = entry
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                utxo_tbl.insert(&key, entry_bytes.as_slice())?;
            }
        }
        write_tx.commit()?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Atomic Block Commit Pipeline
    // ─────────────────────────────────────────────────────────────────────

    /// Atomically commits a fully-validated block to all storage tables.
    ///
    /// All writes happen inside a single `WriteTransaction`; if any step fails,
    /// the entire transaction is aborted — leaving zero partial state on disk.
    pub fn commit_block(
        &self,
        block: &Block,
        height: u64,
        cumulative_work: [u64; 4],
    ) -> Result<(), StorageError> {
        let block_hash = block.header.hash();
        let block_bytes = block
            .to_canonical_bytes()
            .map_err(|e| StorageError::serialization(e.to_string()))?;

        let write_tx = self.db.begin_write()?;

        // ── Step 1: Store block ──────────────────────────────────────────
        {
            let mut tbl = write_tx.open_table(tables::BLOCKS)?;
            tbl.insert(block_hash.as_bytes(), block_bytes.as_slice())?;
        }

        // ── Step 2 & 3: Store transactions, mutate UTXOs, and update ADDRESS_TX_INDEX ──
        {
            let mut tx_tbl = write_tx.open_table(tables::TRANSACTIONS)?;
            let mut utxo_tbl = write_tx.open_table(tables::UTXOS)?;
            let mut addr_idx_tbl = write_tx.open_table(tables::ADDRESS_TX_INDEX)?;
            let mut undo_tbl = write_tx.open_table(tables::BLOCK_UNDO_TABLE)?;

            let mut block_addr_records: HashMap<[u8; 32], Vec<AddressTxRecord>> = HashMap::new();
            let mut undo = BlockUndo {
                spent_utxos: Vec::new(),
                created_outpoints: Vec::new(),
            };

            for tx in &block.transactions {
                let txid = tx.txid();
                let tx_bytes = tx
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                tx_tbl.insert(txid.as_bytes(), tx_bytes.as_slice())?;

                // Spend inputs (skip for coinbase — inputs are sentinel)
                if !tx.is_coinbase() {
                    for input in &tx.inputs {
                        let key = outpoint_to_key(&input.previous_output);

                        // Lookup spent UTXO to record address index before removal
                        let spent_output = if let Some(guard) = utxo_tbl.get(&key)? {
                            let entry = UtxoEntry::from_canonical_bytes(guard.value())
                                .map_err(|e| StorageError::serialization(e.to_string()))?;
                            undo.spent_utxos
                                .push((input.previous_output, entry.clone()));
                            Some(entry.output)
                        } else if let Some(tx_bytes) =
                            tx_tbl.get(input.previous_output.txid.as_bytes())?
                        {
                            let prev_tx = Transaction::from_canonical_bytes(tx_bytes.value())
                                .map_err(|e| StorageError::serialization(e.to_string()))?;
                            prev_tx
                                .outputs
                                .get(input.previous_output.index as usize)
                                .cloned()
                        } else {
                            None
                        };

                        if let Some(spent_out) = spent_output {
                            if let Some(addr) =
                                extract_address_from_locking_condition(&spent_out.locking_condition)
                            {
                                block_addr_records
                                    .entry(addr)
                                    .or_default()
                                    .push(AddressTxRecord {
                                        txid,
                                        is_input: true,
                                        is_output: false,
                                        value_quanta: spent_out.value,
                                        token_id: None,
                                    });
                            }
                        }

                        utxo_tbl.remove(&key)?;
                    }
                }

                // Create new UTXOs for all non-OP_RETURN outputs and record address index
                for (idx, output) in tx.outputs.iter().enumerate() {
                    // Record output in ADDRESS_TX_INDEX if address is resolvable
                    if let Some(addr) =
                        extract_address_from_locking_condition(&output.locking_condition)
                    {
                        block_addr_records
                            .entry(addr)
                            .or_default()
                            .push(AddressTxRecord {
                                txid,
                                is_input: false,
                                is_output: true,
                                value_quanta: output.value,
                                token_id: None,
                            });
                    }

                    // Consensus rule: OP_RETURN outputs (0x6a) are data carriers and omitted from UTXOS table
                    if output.locking_condition.first() == Some(&0x6a) {
                        continue;
                    }
                    let new_op = OutPoint::new(txid, idx as u32);
                    undo.created_outpoints.push(new_op);
                    let key = outpoint_to_key(&new_op);
                    let entry = UtxoEntry::new(output.clone(), height, tx.is_coinbase());
                    let entry_bytes = entry
                        .to_canonical_bytes()
                        .map_err(|e| StorageError::serialization(e.to_string()))?;
                    utxo_tbl.insert(&key, entry_bytes.as_slice())?;
                }
            }

            // Write all accumulated address records for this block
            for (addr, new_records) in block_addr_records {
                let key = make_address_tx_key(&addr, height);
                let merged_records = if let Some(guard) = addr_idx_tbl.get(&key)? {
                    let mut records = deserialize_address_tx_records(guard.value())
                        .map_err(|e| StorageError::serialization(e.to_string()))?;
                    records.extend(new_records);
                    records
                } else {
                    new_records
                };
                let payload = serialize_address_tx_records(&merged_records)
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                addr_idx_tbl.insert(&key, payload.as_slice())?;
            }

            let undo_bytes = undo.to_bytes()?;
            undo_tbl.insert(block_hash.as_bytes(), undo_bytes.as_slice())?;
        }

        // ── Step 4: Update BLOCK_INDEX ────────────────────────────────────
        {
            let mut idx_tbl = write_tx.open_table(tables::BLOCK_INDEX)?;
            let meta = BlockMeta {
                height,
                cumulative_work,
                timestamp: block.header.timestamp,
            };
            let meta_bytes = meta.to_bytes();
            idx_tbl.insert(block_hash.as_bytes(), meta_bytes.as_slice())?;
        }

        // ── Step 5: Update CHAIN_STATE ────────────────────────────────────
        {
            let mut state_tbl = write_tx.open_table(tables::CHAIN_STATE)?;
            state_tbl.insert(KEY_TIP_HASH, block_hash.as_bytes().as_slice())?;
            state_tbl.insert(KEY_TIP_HEIGHT, height.to_le_bytes().as_slice())?;
        }

        // ── Step 6: Atomic commit ─────────────────────────────────────────
        write_tx.commit()?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Atomic Reorganization & Unwind
    // ─────────────────────────────────────────────────────────────────────

    /// Helper to remove all address index records created by a block at the given height.
    fn remove_block_address_records_internal(
        addr_idx_tbl: &mut redb::Table<&[u8; 40], &[u8]>,
        tx_tbl: &redb::Table<&[u8; 32], &[u8]>,
        block: &Block,
        height: u64,
    ) -> Result<(), StorageError> {
        let mut touched_addrs = HashSet::new();

        let block_tx_map: HashMap<Hash256, &Transaction> = block
            .transactions
            .iter()
            .map(|tx| (tx.txid(), tx))
            .collect();

        for tx in &block.transactions {
            for output in &tx.outputs {
                if let Some(addr) =
                    extract_address_from_locking_condition(&output.locking_condition)
                {
                    touched_addrs.insert(addr);
                }
            }

            if !tx.is_coinbase() {
                for input in &tx.inputs {
                    let prev_output = if let Some(local_tx) =
                        block_tx_map.get(&input.previous_output.txid)
                    {
                        local_tx
                            .outputs
                            .get(input.previous_output.index as usize)
                            .cloned()
                    } else if let Some(tx_guard) =
                        tx_tbl.get(input.previous_output.txid.as_bytes())?
                    {
                        if let Ok(prev_tx) = Transaction::from_canonical_bytes(tx_guard.value()) {
                            prev_tx
                                .outputs
                                .get(input.previous_output.index as usize)
                                .cloned()
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(out) = prev_output {
                        if let Some(addr) =
                            extract_address_from_locking_condition(&out.locking_condition)
                        {
                            touched_addrs.insert(addr);
                        }
                    }
                }
            }
        }

        for addr in touched_addrs {
            let key = make_address_tx_key(&addr, height);
            addr_idx_tbl.remove(&key)?;
        }

        Ok(())
    }

    /// Atomically rolls back a single block: removes its transactions, UTXOs,
    /// address index entries, and block index metadata.
    pub fn unwind_block(&self, block: &Block, height: u64) -> Result<(), StorageError> {
        let write_tx = self.db.begin_write()?;
        {
            let mut utxo_tbl = write_tx.open_table(tables::UTXOS)?;
            let mut tx_tbl = write_tx.open_table(tables::TRANSACTIONS)?;
            let mut blk_tbl = write_tx.open_table(tables::BLOCKS)?;
            let mut idx_tbl = write_tx.open_table(tables::BLOCK_INDEX)?;
            let mut addr_idx_tbl = write_tx.open_table(tables::ADDRESS_TX_INDEX)?;
            let mut undo_tbl = write_tx.open_table(tables::BLOCK_UNDO_TABLE)?;

            Self::remove_block_address_records_internal(&mut addr_idx_tbl, &tx_tbl, block, height)?;

            let bh = block.header.hash();
            let undo = undo_tbl
                .get(bh.as_bytes())?
                .ok_or_else(|| {
                    StorageError::InconsistentState(format!("missing undo delta for block {bh}"))
                })
                .and_then(|guard| BlockUndo::from_bytes(guard.value()))?;
            for outpoint in &undo.created_outpoints {
                utxo_tbl.remove(&outpoint_to_key(outpoint))?;
            }
            for (outpoint, entry) in &undo.spent_utxos {
                let entry_bytes = entry
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                utxo_tbl.insert(&outpoint_to_key(outpoint), entry_bytes.as_slice())?;
            }
            for tx in &block.transactions {
                let txid = tx.txid();
                tx_tbl.remove(txid.as_bytes())?;
            }
            blk_tbl.remove(bh.as_bytes())?;
            idx_tbl.remove(bh.as_bytes())?;
            undo_tbl.remove(bh.as_bytes())?;

            // If tip matches this block, revert tip to parent
            let mut state_tbl = write_tx.open_table(tables::CHAIN_STATE)?;
            let is_tip = state_tbl
                .get(KEY_TIP_HASH)?
                .is_some_and(|guard| guard.value() == bh.as_bytes());
            if is_tip {
                state_tbl.insert(
                    KEY_TIP_HASH,
                    block.header.previous_block_hash.as_bytes().as_slice(),
                )?;
                let new_height = height.saturating_sub(1);
                state_tbl.insert(KEY_TIP_HEIGHT, new_height.to_le_bytes().as_slice())?;
            }
        }
        write_tx.commit()?;
        Ok(())
    }

    /// Atomically rolls back disconnected blocks and applies connected blocks.
    ///
    /// All mutations execute inside a single `WriteTransaction`.
    /// The new canonical tip is the last block in `connected_blocks`.
    pub fn apply_reorganization(
        &self,
        disconnected_blocks: &[Block],
        connected_blocks: &[(Block, u64, [u64; 4])],
    ) -> Result<(), StorageError> {
        let write_tx = self.db.begin_write()?;

        {
            let mut utxo_tbl = write_tx.open_table(tables::UTXOS)?;
            let mut tx_tbl = write_tx.open_table(tables::TRANSACTIONS)?;
            let mut blk_tbl = write_tx.open_table(tables::BLOCKS)?;
            let mut idx_tbl = write_tx.open_table(tables::BLOCK_INDEX)?;
            let mut addr_idx_tbl = write_tx.open_table(tables::ADDRESS_TX_INDEX)?;
            let mut undo_tbl = write_tx.open_table(tables::BLOCK_UNDO_TABLE)?;

            // Rollback disconnected blocks
            for block in disconnected_blocks {
                let bh = block.header.hash();
                let height = if let Some(guard) = idx_tbl.get(bh.as_bytes())? {
                    BlockMeta::from_bytes(guard.value()).map(|m| m.height)
                } else {
                    None
                };

                if let Some(h) = height {
                    Self::remove_block_address_records_internal(
                        &mut addr_idx_tbl,
                        &tx_tbl,
                        block,
                        h,
                    )?;
                }

                let undo = undo_tbl
                    .get(bh.as_bytes())?
                    .ok_or_else(|| {
                        StorageError::InconsistentState(format!(
                            "missing undo delta for block {bh}"
                        ))
                    })
                    .and_then(|guard| BlockUndo::from_bytes(guard.value()))?;
                for outpoint in &undo.created_outpoints {
                    utxo_tbl.remove(&outpoint_to_key(outpoint))?;
                }
                for (outpoint, entry) in &undo.spent_utxos {
                    let entry_bytes = entry
                        .to_canonical_bytes()
                        .map_err(|e| StorageError::serialization(e.to_string()))?;
                    utxo_tbl.insert(&outpoint_to_key(outpoint), entry_bytes.as_slice())?;
                }
                for tx in &block.transactions {
                    let txid = tx.txid();
                    tx_tbl.remove(txid.as_bytes())?;
                }
                blk_tbl.remove(bh.as_bytes())?;
                idx_tbl.remove(bh.as_bytes())?;
                undo_tbl.remove(bh.as_bytes())?;
            }

            // Apply connected blocks
            for (block, height, cumulative_work) in connected_blocks {
                let block_hash = block.header.hash();
                let block_bytes = block
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                blk_tbl.insert(block_hash.as_bytes(), block_bytes.as_slice())?;

                let mut block_addr_records: HashMap<[u8; 32], Vec<AddressTxRecord>> =
                    HashMap::new();
                let mut undo = BlockUndo {
                    spent_utxos: Vec::new(),
                    created_outpoints: Vec::new(),
                };

                for tx in &block.transactions {
                    let txid = tx.txid();
                    let tx_bytes = tx
                        .to_canonical_bytes()
                        .map_err(|e| StorageError::serialization(e.to_string()))?;
                    tx_tbl.insert(txid.as_bytes(), tx_bytes.as_slice())?;
                    if !tx.is_coinbase() {
                        for input in &tx.inputs {
                            let key = outpoint_to_key(&input.previous_output);
                            let spent_output = if let Some(guard) = utxo_tbl.get(&key)? {
                                let entry = UtxoEntry::from_canonical_bytes(guard.value())
                                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                                undo.spent_utxos
                                    .push((input.previous_output, entry.clone()));
                                Some(entry.output)
                            } else if let Some(tx_bytes) =
                                tx_tbl.get(input.previous_output.txid.as_bytes())?
                            {
                                let prev_tx =
                                    Transaction::from_canonical_bytes(tx_bytes.value())
                                        .map_err(|e| StorageError::serialization(e.to_string()))?;
                                prev_tx
                                    .outputs
                                    .get(input.previous_output.index as usize)
                                    .cloned()
                            } else {
                                None
                            };

                            if let Some(spent_out) = spent_output {
                                if let Some(addr) = extract_address_from_locking_condition(
                                    &spent_out.locking_condition,
                                ) {
                                    block_addr_records.entry(addr).or_default().push(
                                        AddressTxRecord {
                                            txid,
                                            is_input: true,
                                            is_output: false,
                                            value_quanta: spent_out.value,
                                            token_id: None,
                                        },
                                    );
                                }
                            }

                            utxo_tbl.remove(&key)?;
                        }
                    }
                    for (idx, output) in tx.outputs.iter().enumerate() {
                        if let Some(addr) =
                            extract_address_from_locking_condition(&output.locking_condition)
                        {
                            block_addr_records
                                .entry(addr)
                                .or_default()
                                .push(AddressTxRecord {
                                    txid,
                                    is_input: false,
                                    is_output: true,
                                    value_quanta: output.value,
                                    token_id: None,
                                });
                        }

                        if output.locking_condition.first() == Some(&0x6a) {
                            continue;
                        }
                        let op = OutPoint::new(txid, idx as u32);
                        undo.created_outpoints.push(op);
                        let entry = UtxoEntry::new(output.clone(), *height, tx.is_coinbase());
                        let entry_bytes = entry
                            .to_canonical_bytes()
                            .map_err(|e| StorageError::serialization(e.to_string()))?;
                        utxo_tbl.insert(&outpoint_to_key(&op), entry_bytes.as_slice())?;
                    }
                }

                let undo_bytes = undo.to_bytes()?;
                undo_tbl.insert(block_hash.as_bytes(), undo_bytes.as_slice())?;

                // Write address records for connected block
                for (addr, new_records) in block_addr_records {
                    let key = make_address_tx_key(&addr, *height);
                    let merged_records = if let Some(guard) = addr_idx_tbl.get(&key)? {
                        let mut records = deserialize_address_tx_records(guard.value())
                            .map_err(|e| StorageError::serialization(e.to_string()))?;
                        records.extend(new_records);
                        records
                    } else {
                        new_records
                    };
                    let payload = serialize_address_tx_records(&merged_records)
                        .map_err(|e| StorageError::serialization(e.to_string()))?;
                    addr_idx_tbl.insert(&key, payload.as_slice())?;
                }

                let meta = BlockMeta {
                    height: *height,
                    cumulative_work: *cumulative_work,
                    timestamp: block.header.timestamp,
                };
                idx_tbl.insert(block_hash.as_bytes(), meta.to_bytes().as_slice())?;
            }
        }

        // Update CHAIN_STATE to the new tip (last connected block)
        if let Some((last_block, last_height, _)) = connected_blocks.last() {
            let mut state_tbl = write_tx.open_table(tables::CHAIN_STATE)?;
            let tip_hash = last_block.header.hash();
            state_tbl.insert(KEY_TIP_HASH, tip_hash.as_bytes().as_slice())?;
            state_tbl.insert(KEY_TIP_HEIGHT, last_height.to_le_bytes().as_slice())?;
        }

        write_tx.commit()?;
        Ok(())
    }

    /// Replaces the persisted UTXO snapshot after a consensus-validated reorg.
    ///
    /// The consensus layer computes the complete post-reorg UTXO set before this
    /// method is called. Replacing the table in one write transaction prevents
    /// stale spent outputs from surviving a disconnect/connect sequence.
    pub fn replace_utxo_set(&self, utxo_set: &UtxoSet) -> Result<(), StorageError> {
        let write_tx = self.db.begin_write()?;
        {
            let mut table = write_tx.open_table(tables::UTXOS)?;
            let keys: Vec<[u8; 36]> = table
                .iter()?
                .map(|entry| entry.map(|(key, _)| *key.value()))
                .collect::<Result<_, _>>()?;
            for key in keys {
                table.remove(&key)?;
            }
            for (outpoint, entry) in utxo_set.entries() {
                let bytes = entry
                    .to_canonical_bytes()
                    .map_err(|e| StorageError::serialization(e.to_string()))?;
                table.insert(&outpoint_to_key(outpoint), bytes.as_slice())?;
            }
        }
        write_tx.commit()?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Address Index Queries (Passbook)
    // ─────────────────────────────────────────────────────────────────────

    /// Queries transaction records paired with their confirmed block height associated
    /// with `address` between `from_height` and `to_height` (inclusive), up to `limit` records.
    ///
    /// Traverses the canonical `ADDRESS_TX_INDEX` in ascending block height order.
    pub fn get_address_transactions_with_height(
        &self,
        address: &Address,
        from_height: u64,
        to_height: u64,
        limit: usize,
    ) -> Result<Vec<(u64, AddressTxRecord)>, StorageError> {
        if limit == 0 || from_height > to_height {
            return Ok(Vec::new());
        }

        let addr_bytes = address.hash();
        let start_key = make_address_tx_key(addr_bytes, from_height);
        let end_key = make_address_tx_key(addr_bytes, to_height);

        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_table(tables::ADDRESS_TX_INDEX)?;

        let mut results = Vec::new();
        for item in table.range::<&[u8; 40]>(&start_key..=&end_key)? {
            let (key_guard, val_guard) = item?;
            let key = key_guard.value();
            let height = u64::from_be_bytes(key[32..40].try_into().unwrap());
            let records = deserialize_address_tx_records(val_guard.value())
                .map_err(|e| StorageError::serialization(e.to_string()))?;
            for record in records {
                results.push((height, record));
                if results.len() >= limit {
                    return Ok(results);
                }
            }
        }

        Ok(results)
    }

    /// Queries transaction records associated with `address` between `from_height`
    /// and `to_height` (inclusive), up to `limit` records.
    ///
    /// Traverses the canonical `ADDRESS_TX_INDEX` in ascending block height order.
    pub fn get_address_transactions(
        &self,
        address: &Address,
        from_height: u64,
        to_height: u64,
        limit: usize,
    ) -> Result<Vec<AddressTxRecord>, StorageError> {
        Ok(self
            .get_address_transactions_with_height(address, from_height, to_height, limit)?
            .into_iter()
            .map(|(_, rec)| rec)
            .collect())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Legacy meta API (backward-compatible with existing test)
    // ─────────────────────────────────────────────────────────────────────

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let write_tx = self.db.begin_write()?;
        {
            let mut tbl = write_tx.open_table(tables::META_TABLE)?;
            tbl.insert(key, value)?;
        }
        write_tx.commit()?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>, StorageError> {
        let read_tx = self.db.begin_read()?;
        let tbl = read_tx.open_table(tables::META_TABLE)?;
        Ok(tbl.get(key)?.map(|v| v.value().to_string()))
    }
}
