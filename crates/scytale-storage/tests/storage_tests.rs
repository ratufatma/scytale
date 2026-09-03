//! Integration test suites for the Scytale `redb` storage engine.
//!
//! Covers: database/table initialization, block & tx CRUD, UTXO insert/lookup/spend,
//! atomic block commit, aborted-transaction zero-state guarantees, restart durability,
//! and atomic reorganization state transitions.

use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoSet, TRANSACTION_VERSION_1,
};
use scytale_storage::StorageEngine;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Test fixtures
// ─────────────────────────────────────────────────────────────────────────────

const DIFFICULTY_TARGET: u32 = 0x1d00ffff;

fn fixed_hash(byte: u8) -> Hash256 {
    Hash256::new([byte; 32])
}

/// A coinbase transaction paying `value` quanta to a fixed locking condition.
fn coinbase_tx(height: u64, value: u64) -> Transaction {
    Transaction::new_coinbase(height, vec![TxOut::new(value, b"miner-key".to_vec())])
}

/// A simple transfer transaction spending `previous_output` and paying `out_value`
/// to a fresh output, returning `change` (if any) back to the same condition.
/// Value semantics are intentionally loose here because storage does not validate
/// transactions — it only persists whatever validated state consensus hands it.
fn transfer_tx(previous_output: OutPoint, out_value: u64) -> Transaction {
    Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(previous_output, b"sig".to_vec())],
        vec![TxOut::new(out_value, b"recipient-key".to_vec())],
        0,
    )
}

fn make_block(prev_hash: Hash256, timestamp: u64, nonce: u64, txs: Vec<Transaction>) -> Block {
    Block::new(
        BlockHeader::new(
            1,
            prev_hash,
            fixed_hash(0xAB),
            Hash256::ZERO,
            timestamp,
            DIFFICULTY_TARGET,
            nonce,
        ),
        txs,
    )
}

fn make_coinbase_tx(height: u64, value: u64) -> Transaction {
    coinbase_tx(height, value)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Database open & table initialization
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_database_open_and_table_init() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("scytale.db");
    let engine = StorageEngine::open(&path).expect("open should succeed");
    assert!(path.exists(), "database file should exist on disk");

    // Fresh database: no chain tip yet.
    assert!(
        engine.get_canonical_tip().unwrap().is_none(),
        "fresh database must have no canonical tip"
    );
    assert!(
        engine.load_entire_utxo_set().unwrap().is_empty(),
        "fresh database must start with an empty UTXO set"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Block & transaction CRUD roundtrip
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_block_and_tx_crud() {
    let engine = StorageEngine::in_memory().unwrap();

    let cb = make_coinbase_tx(0, 5_000);
    let block = make_block(Hash256::ZERO, 1_700_000_000, 42, vec![cb.clone()]);
    let block_hash = block.header.hash();

    // commit_block is the only mutation path, so use it to populate tables.
    engine
        .commit_block(&block, 0, [1, 0, 0, 0])
        .expect("atomic commit should succeed");

    let fetched = engine
        .get_block(&block_hash)
        .unwrap()
        .expect("block present");
    assert_eq!(fetched, block, "block must roundtrip byte-exact");

    let cb_txid = cb.txid();
    let fetched_tx = engine
        .get_transaction(&cb_txid)
        .unwrap()
        .expect("transaction present");
    assert_eq!(fetched_tx, cb, "transaction must roundtrip byte-exact");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. UTXO insert, lookup, spend
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_utxo_insert_lookup_spend() {
    let engine = StorageEngine::in_memory().unwrap();

    // Build a block whose coinbase creates a spendable UTXO.
    let cb = make_coinbase_tx(0, 1_000);
    let cb_txid = cb.txid();
    let block = make_block(Hash256::ZERO, 1_700_000_000, 7, vec![cb]);
    engine.commit_block(&block, 0, [1, 0, 0, 0]).unwrap();

    // Lookup: the coinbase output at index 0 must be present.
    let outpoint = OutPoint::new(cb_txid, 0);
    let entry = engine
        .get_utxo(&outpoint)
        .unwrap()
        .expect("utxo should exist");
    assert_eq!(entry.output.value, 1_000);
    assert!(entry.is_coinbase);

    // A fresh (unspent) forwarded UTXO from the same block, then spend it.
    let spend = transfer_tx(outpoint, 900);
    let spend_block = make_block(
        block.header.hash(),
        1_700_000_100,
        8,
        vec![make_coinbase_tx(1, 5_000), spend.clone()],
    );
    engine.commit_block(&spend_block, 1, [2, 0, 0, 0]).unwrap();

    // The original coinbase UTXO is now spent -> None.
    assert!(
        engine.get_utxo(&outpoint).unwrap().is_none(),
        "spent input must be removed from UTXO set"
    );

    // The transfer's new output (index 0) is present.
    let new_op = OutPoint::new(spend.txid(), 0);
    let new_entry = engine
        .get_utxo(&new_op)
        .unwrap()
        .expect("created output should be present");
    assert_eq!(new_entry.output.value, 900);
    assert!(!new_entry.is_coinbase);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Atomic block commit success (multi-table simultaneous update)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_atomic_block_commit_success() {
    let engine = StorageEngine::in_memory().unwrap();

    let cb = make_coinbase_tx(0, 5_000);
    let cb_txid = cb.txid();
    let block = make_block(Hash256::ZERO, 1_700_000_000, 1, vec![cb.clone()]);
    let block_hash = block.header.hash();

    engine.commit_block(&block, 0, [1, 0, 0, 0]).unwrap();

    // BLOCKS
    assert!(engine.get_block(&block_hash).unwrap().is_some());
    // TRANSACTIONS
    assert!(engine.get_transaction(&cb_txid).unwrap().is_some());
    // UTXOS
    assert!(engine
        .get_utxo(&OutPoint::new(cb_txid, 0))
        .unwrap()
        .is_some());
    // CHAIN_STATE
    let tip = engine
        .get_canonical_tip()
        .unwrap()
        .expect("tip must be set after commit");
    assert_eq!(tip.0, block_hash);
    assert_eq!(tip.1, 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Aborted transaction leaves zero partial state
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_aborted_transaction_leaves_zero_state() {
    use redb::{Database, ReadableTableMetadata};
    use scytale_storage::tables::{BLOCKS, CHAIN_STATE, TRANSACTIONS, UTXOS};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("abort.db");
    let db = Database::create(&path).unwrap();

    // Pre-initialize the canonical tables so the abort experiment is isolated.
    {
        let wtx = db.begin_write().unwrap();
        wtx.open_table(BLOCKS).unwrap();
        wtx.open_table(TRANSACTIONS).unwrap();
        wtx.open_table(UTXOS).unwrap();
        wtx.open_table(CHAIN_STATE).unwrap();
        wtx.commit().unwrap();
    }

    // Open a single write transaction and perform the FIRST steps of a commit
    // (write a block + a transaction), then deliberately abort WITHOUT committing.
    {
        let wtx = db.begin_write().unwrap();
        let cb = make_coinbase_tx(0, 100);
        let block = make_block(Hash256::ZERO, 1_700_000_000, 99, vec![cb]);
        let block_hash = block.header.hash();

        {
            let mut blk = wtx.open_table(BLOCKS).unwrap();
            blk.insert(block_hash.as_bytes(), "partial-block".as_bytes())
                .unwrap();
        }
        {
            let mut tx_tbl = wtx.open_table(TRANSACTIONS).unwrap();
            let cb_txid = block.transactions[0].txid();
            tx_tbl
                .insert(cb_txid.as_bytes(), "partial-tx".as_bytes())
                .unwrap();
        }
        // NOTE: We do NOT call wtx.commit(). Dropping the guard aborts the tx,
        // mimicking a crash mid-commit.
        drop(wtx);
    }

    // After the aborted transaction, no rows may exist anywhere.
    {
        let rtx = db.begin_read().unwrap();
        let blk = rtx.open_table(BLOCKS).unwrap();
        assert_eq!(
            blk.len().unwrap(),
            0,
            "aborted tx must leave zero block rows"
        );
        let tx_tbl = rtx.open_table(TRANSACTIONS).unwrap();
        assert_eq!(
            tx_tbl.len().unwrap(),
            0,
            "aborted tx must leave zero transaction rows"
        );
        let utxo = rtx.open_table(UTXOS).unwrap();
        assert_eq!(
            utxo.len().unwrap(),
            0,
            "aborted tx must leave zero utxo rows"
        );
        let state = rtx.open_table(CHAIN_STATE).unwrap();
        assert_eq!(
            state.len().unwrap(),
            0,
            "aborted tx must leave zero chain-state rows"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Persistence across restart
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_persistence_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("restart.db");

    // ── Write phase ──────────────────────────────────────────────────────
    let tip_hash;
    {
        let engine = StorageEngine::open(&path).unwrap();

        let cb = make_coinbase_tx(0, 7_000);
        let cb_txid = cb.txid();
        let block = make_block(Hash256::ZERO, 1_700_000_000, 5, vec![cb]);
        tip_hash = block.header.hash();
        engine.commit_block(&block, 0, [1, 0, 0, 0]).unwrap();

        // Sanity: confirm the balance exists before shutdown.
        let before = engine.get_utxo(&OutPoint::new(cb_txid, 0)).unwrap();
        assert_eq!(before.unwrap().output.value, 7_000);

        // Drop the engine here to simulate node shutdown (open file closed).
    }

    // ── Recovery phase: reopen from the same path ─────────────────────────
    let engine = StorageEngine::open(&path).unwrap();

    let tip = engine
        .get_canonical_tip()
        .unwrap()
        .expect("tip must survive restart");
    assert_eq!(tip.0, tip_hash, "tip hash must survive restart");
    assert_eq!(tip.1, 0, "tip height must survive restart");

    // Full UTXO set survives with exact balance (1 coinbase output).
    let utxo_set = engine.load_entire_utxo_set().unwrap();
    assert_eq!(utxo_set.len(), 1, "one committed coinbase UTXO survives");

    // Locate the coinbase UTXO by its tip block's transaction.
    let cb = engine.get_block(&tip_hash).unwrap().unwrap().transactions[0].clone();
    let cb_txid = cb.txid();
    let cb_entry = engine
        .get_utxo(&OutPoint::new(cb_txid, 0))
        .unwrap()
        .expect("coinbase utxo must survive restart");
    assert_eq!(cb_entry.output.value, 7_000);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Atomic reorganization state transition
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_atomic_reorg_state_transition() {
    let engine = StorageEngine::in_memory().unwrap();

    // ── Phase 1: canonical chain A: block0 -> block1a -> block2a ─────────
    let b0 = make_block(
        Hash256::ZERO,
        1_700_000_000,
        1,
        vec![make_coinbase_tx(0, 5_000)],
    );
    let b0_hash = b0.header.hash();
    engine.commit_block(&b0, 0, [1, 0, 0, 0]).unwrap();

    // block1a spends b0's coinbase, paying 4_000 to a fixed recipient.
    let cb0_txid = b0.transactions[0].txid();
    let b1a_spend = transfer_tx(OutPoint::new(cb0_txid, 0), 4_000);
    let b1a_spend_txid = b1a_spend.txid();
    let b1a = make_block(
        b0_hash,
        1_700_000_100,
        2,
        vec![make_coinbase_tx(1, 5_000), b1a_spend],
    );
    let b1a_hash = b1a.header.hash();
    engine.commit_block(&b1a, 1, [2, 0, 0, 0]).unwrap();

    // block2a spends block1a's recipient output.
    let b2a_spend = transfer_tx(OutPoint::new(b1a_spend_txid, 0), 3_000);
    let b2a_spend_txid = b2a_spend.txid();
    let b2a = make_block(
        b1a_hash,
        1_700_000_200,
        3,
        vec![make_coinbase_tx(2, 5_000), b2a_spend],
    );
    let b2a_hash = b2a.header.hash();
    engine.commit_block(&b2a, 2, [3, 0, 0, 0]).unwrap();

    // Chain A active: tip = b2a.
    let (tip_hash, tip_height) = engine.get_canonical_tip().unwrap().unwrap();
    assert_eq!(tip_hash, b2a_hash);
    assert_eq!(tip_height, 2);

    // ── Phase 2: reorg to branch B: block0 -> block1b -> block2b -> block3b ─
    // Disconnect chain A (b2a, b1a) as provided by the caller.
    let disconnected = vec![b2a.clone(), b1a.clone()];

    // Branch B builds a fresh tx spending the SAME coinbase (b0's cb).
    let b1b_spend = transfer_tx(OutPoint::new(cb0_txid, 0), 4_000);
    let b1b_spend_txid = b1b_spend.txid();
    let b1b = make_block(
        b0_hash,
        1_700_000_300,
        20,
        vec![make_coinbase_tx(1, 5_000), b1b_spend],
    );
    let b1b_hash = b1b.header.hash();

    let b2b_spend = transfer_tx(OutPoint::new(b1b_spend_txid, 0), 3_500);
    let b2b_spend_txid = b2b_spend.txid();
    let b2b = make_block(
        b1b_hash,
        1_700_000_400,
        21,
        vec![make_coinbase_tx(2, 5_000), b2b_spend],
    );
    let b2b_hash = b2b.header.hash();

    let b3b_final = transfer_tx(OutPoint::new(b2b_spend_txid, 0), 3_000);
    let b3b_final_txid = b3b_final.txid();
    let b3b = make_block(
        b2b_hash,
        1_700_000_500,
        22,
        vec![make_coinbase_tx(3, 5_000), b3b_final],
    );
    let b3b_hash = b3b.header.hash();

    let connected: Vec<(Block, u64, [u64; 4])> = vec![
        (b1b, 1, [4, 0, 0, 0]),
        (b2b, 2, [5, 0, 0, 0]),
        (b3b, 3, [6, 0, 0, 0]),
    ];

    engine
        .apply_reorganization(&disconnected, &connected)
        .expect("reorg must commit atomically");

    // ── Phase 3: verify branch B is canonical and UTXOs are consistent ────
    let (tip_hash, tip_height) = engine.get_canonical_tip().unwrap().unwrap();
    assert_eq!(tip_hash, b3b_hash, "tip must now point to branch B head");
    assert_eq!(tip_height, 3);

    // Old branch-A blocks must be gone.
    assert!(engine.get_block(&b2a_hash).unwrap().is_none());
    assert!(engine.get_block(&b1a_hash).unwrap().is_none());

    // Branch-B blocks present.
    assert!(engine.get_block(&b1b_hash).unwrap().is_some());
    assert!(engine.get_block(&b2b_hash).unwrap().is_some());
    assert!(engine.get_block(&b3b_hash).unwrap().is_some());

    // UTXO consistency: branch-B final output (index 0 of b3b_final) exists.
    let final_entry = engine
        .get_utxo(&OutPoint::new(b3b_final_txid, 0))
        .unwrap()
        .expect("branch-B final output must be in the UTXO set");
    assert_eq!(final_entry.output.value, 3_000);

    // Branch-A intermediate outputs must be gone (spent/rolled back).
    assert!(engine
        .get_utxo(&OutPoint::new(b2a_spend_txid, 0))
        .unwrap()
        .is_none());

    // The full in-memory set reflects only branch B.
    let utxo_set: UtxoSet = engine.load_entire_utxo_set().unwrap();
    let utxo_count = utxo_set.len();
    // Branch-B canonical state holds: coinbases of b1b,b2b,b3b (3) + b3b final output (1).
    assert_eq!(
        utxo_count, 4,
        "UTXO set must exactly match branch-B canonical state"
    );
}

#[test]
fn test_op_return_omitted_from_utxo_table() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    let genesis_txid = Hash256::hash(b"prev_cb");
    let input = TxIn::new(OutPoint::new(genesis_txid, 0), vec![0x01]);
    let standard_out = TxOut::new(50_000_000, vec![0x01, 0x02, 0x03]);
    let op_return_out = TxOut::new(0, vec![0x6a, 0x04, 0xde, 0xad, 0xbe, 0xef]);

    let tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![input],
        vec![standard_out, op_return_out],
        0,
    );
    let txid = tx.txid();

    let header = BlockHeader::new(
        1,
        Hash256::ZERO,
        Hash256::ZERO,
        Hash256::ZERO,
        1000,
        0x207fffff,
        0,
    );
    let block = Block::new(header, vec![tx]);

    engine.commit_block(&block, 1, [10, 0, 0, 0]).unwrap();

    // Verify transaction is stored in TRANSACTIONS table
    assert!(engine.get_transaction(&txid).unwrap().is_some());

    // Standard output (index 0) is in UTXOS table
    let std_utxo = engine.get_utxo(&OutPoint::new(txid, 0)).unwrap();
    assert!(std_utxo.is_some());

    // OP_RETURN output (index 1) is NOT in UTXOS table
    let op_return_utxo = engine.get_utxo(&OutPoint::new(txid, 1)).unwrap();
    assert!(
        op_return_utxo.is_none(),
        "OP_RETURN output must never be inserted into UTXOS table"
    );
}

#[test]
fn test_utxo_root_and_snapshot_roundtrip() {
    let engine = StorageEngine::in_memory().unwrap();

    // Initially empty
    assert_eq!(engine.compute_utxo_root().unwrap(), Hash256::ZERO);

    // Commit 2 blocks
    let b0 = make_block(
        Hash256::ZERO,
        1_000_000,
        1,
        vec![make_coinbase_tx(0, 1_000_000_000)],
    );
    engine.commit_block(&b0, 0, [1, 0, 0, 0]).unwrap();

    let root0 = engine.compute_utxo_root().unwrap();
    assert_ne!(root0, Hash256::ZERO);

    // Export snapshot
    let snapshot = engine.export_utxo_snapshot().unwrap();
    assert_eq!(snapshot.height, 0);
    assert_eq!(snapshot.block_hash, b0.header.hash());
    assert_eq!(snapshot.utxo_root, root0);
    assert_eq!(snapshot.entries.len(), 1);

    // Apply snapshot to fresh engine
    let fresh_engine = StorageEngine::in_memory().unwrap();
    assert_eq!(fresh_engine.compute_utxo_root().unwrap(), Hash256::ZERO);
    fresh_engine.apply_utxo_snapshot(&snapshot).unwrap();
    assert_eq!(fresh_engine.compute_utxo_root().unwrap(), root0);

    // Test rejection of corrupted root
    let mut corrupted = snapshot.clone();
    corrupted.utxo_root = Hash256::hash(b"tampered_root");
    let corrupt_err = fresh_engine.apply_utxo_snapshot(&corrupted);
    assert!(corrupt_err.is_err());
}
