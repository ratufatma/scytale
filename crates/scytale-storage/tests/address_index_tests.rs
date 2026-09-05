//! Integration test suite for the Passbook address-transaction index (`ADDRESS_TX_INDEX`).
//!
//! Validates:
//! 1. Atomic indexing of inbound (output) and outbound (input) transactions per address.
//! 2. Range query traversal with limit bounds across block heights.
//! 3. Clean index rollback during single-block unwind and atomic multi-block chain reorganizations.

use scytale_core::{
    Address, Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut,
    TRANSACTION_VERSION_1,
};
use scytale_storage::StorageEngine;

const DIFFICULTY_TARGET: u32 = 0x1d00ffff;

fn fixed_hash(byte: u8) -> Hash256 {
    Hash256::new([byte; 32])
}

fn make_p2pkh_script(addr: &Address) -> Vec<u8> {
    let mut script = Vec::with_capacity(37);
    script.push(0x73); // OP_DUP
    script.push(0xa0); // OP_BLAKE3
    script.push(0x20); // OP_PUSHBYTES_32
    script.extend_from_slice(addr.hash());
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG
    script
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

#[test]
fn test_address_tx_index_inbound_outbound() {
    let engine = StorageEngine::in_memory().expect("in-memory db should open");

    let addr_alice = Address::new([0x11; 32]);
    let addr_bob = Address::new([0x22; 32]);

    let alice_script = make_p2pkh_script(&addr_alice);
    let bob_script = make_p2pkh_script(&addr_bob);

    // ── Block 1: Fund Alice with 10,000 quanta via coinbase ──────────────
    let cb_tx = Transaction::new_coinbase(
        1,
        vec![TxOut::new(10_000, alice_script.clone())],
    );
    let cb_txid = cb_tx.txid();
    let block1 = make_block(Hash256::ZERO, 1_700_000_000, 1, vec![cb_tx]);

    engine
        .commit_block(&block1, 1, [1, 0, 0, 0])
        .expect("commit block 1 must succeed");

    // Query Alice at height 1: should have 1 inbound record (is_output: true)
    let alice_records_b1 = engine
        .get_address_transactions(&addr_alice, 1, 1, 10)
        .expect("get address transactions should succeed");
    assert_eq!(alice_records_b1.len(), 1);
    assert_eq!(alice_records_b1[0].txid, cb_txid);
    assert!(!alice_records_b1[0].is_input);
    assert!(alice_records_b1[0].is_output);
    assert_eq!(alice_records_b1[0].value_quanta, 10_000);
    assert_eq!(alice_records_b1[0].token_id, None);

    // Query Bob at height 1: should have 0 records
    let bob_records_b1 = engine
        .get_address_transactions(&addr_bob, 1, 1, 10)
        .expect("get address transactions should succeed");
    assert!(bob_records_b1.is_empty());

    // ── Block 2: Alice sends 6,000 to Bob and returns 4,000 change to Alice ──
    let spend_input = OutPoint::new(cb_txid, 0);
    let transfer_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(spend_input, vec![0xfe; 64])],
        vec![
            TxOut::new(6_000, bob_script),
            TxOut::new(4_000, alice_script),
        ],
        0,
    );
    let transfer_txid = transfer_tx.txid();
    let block2 = make_block(block1.header.hash(), 1_700_000_010, 2, vec![transfer_tx]);

    engine
        .commit_block(&block2, 2, [2, 0, 0, 0])
        .expect("commit block 2 must succeed");

    // Query Alice at height 2: should have 2 records:
    // 1 input record (spending 10,000) and 1 output record (receiving 4,000 change)
    let alice_records_b2 = engine
        .get_address_transactions(&addr_alice, 2, 2, 10)
        .expect("query alice at height 2 should succeed");
    assert_eq!(alice_records_b2.len(), 2);

    let input_rec = alice_records_b2.iter().find(|r| r.is_input).expect("must contain input record");
    assert_eq!(input_rec.txid, transfer_txid);
    assert_eq!(input_rec.value_quanta, 10_000);
    assert!(!input_rec.is_output);

    let output_rec = alice_records_b2.iter().find(|r| r.is_output).expect("must contain output record");
    assert_eq!(output_rec.txid, transfer_txid);
    assert_eq!(output_rec.value_quanta, 4_000);
    assert!(!output_rec.is_input);

    // Query Bob at height 2: should have 1 inbound record (receiving 6,000)
    let bob_records_b2 = engine
        .get_address_transactions(&addr_bob, 2, 2, 10)
        .expect("query bob at height 2 should succeed");
    assert_eq!(bob_records_b2.len(), 1);
    assert_eq!(bob_records_b2[0].txid, transfer_txid);
    assert!(!bob_records_b2[0].is_input);
    assert!(bob_records_b2[0].is_output);
    assert_eq!(bob_records_b2[0].value_quanta, 6_000);

    // Cumulative query for Alice across blocks 1..=2
    let alice_all = engine
        .get_address_transactions(&addr_alice, 1, 2, 10)
        .expect("query alice 1..=2 should succeed");
    assert_eq!(alice_all.len(), 3);
}

#[test]
fn test_address_tx_index_range_and_limit() {
    let engine = StorageEngine::in_memory().expect("in-memory db should open");

    let addr = Address::new([0x33; 32]);
    let script = make_p2pkh_script(&addr);

    // Generate 5 consecutive blocks, each granting an output to `addr`
    let mut prev_hash = Hash256::ZERO;
    for h in 1..=5 {
        let val = h * 1_000;
        let tx = Transaction::new_coinbase(h, vec![TxOut::new(val, script.clone())]);
        let block = make_block(prev_hash, 1_700_000_000 + h * 10, h, vec![tx]);
        prev_hash = block.header.hash();
        engine
            .commit_block(&block, h, [h, 0, 0, 0])
            .expect("commit block should succeed");
    }

    // 1. Range query from height 2 to 4 (inclusive): expects 3 records (2000, 3000, 4000)
    let range_2_to_4 = engine
        .get_address_transactions(&addr, 2, 4, 100)
        .expect("range 2..=4 should succeed");
    assert_eq!(range_2_to_4.len(), 3);
    assert_eq!(range_2_to_4[0].value_quanta, 2_000);
    assert_eq!(range_2_to_4[1].value_quanta, 3_000);
    assert_eq!(range_2_to_4[2].value_quanta, 4_000);

    // 2. Limit test: query heights 1..=5 with limit 2: expects only the first 2 records
    let limit_2 = engine
        .get_address_transactions(&addr, 1, 5, 2)
        .expect("limit query should succeed");
    assert_eq!(limit_2.len(), 2);
    assert_eq!(limit_2[0].value_quanta, 1_000);
    assert_eq!(limit_2[1].value_quanta, 2_000);

    // 3. Limit 0: expects 0 records
    let limit_0 = engine
        .get_address_transactions(&addr, 1, 5, 0)
        .expect("limit 0 should succeed");
    assert!(limit_0.is_empty());

    // 4. Inverted range (from_height > to_height): expects empty
    let inverted = engine
        .get_address_transactions(&addr, 5, 2, 10)
        .expect("inverted range should return empty");
    assert!(inverted.is_empty());

    // 5. Non-existent range: expects empty
    let out_of_range = engine
        .get_address_transactions(&addr, 10, 20, 10)
        .expect("out of range query should return empty");
    assert!(out_of_range.is_empty());
}

#[test]
fn test_address_tx_index_unwind_and_reorg() {
    let engine = StorageEngine::in_memory().expect("in-memory db should open");

    let addr_target = Address::new([0x44; 32]);
    let addr_alt = Address::new([0x55; 32]);

    let target_script = make_p2pkh_script(&addr_target);
    let alt_script = make_p2pkh_script(&alt_script_addr(&addr_alt));

    fn alt_script_addr(addr: &Address) -> Address {
        addr.clone()
    }

    // ── Part 1: Test unwind_block ─────────────────────────────────────────
    let tx1 = Transaction::new_coinbase(1, vec![TxOut::new(5_000, target_script.clone())]);
    let block1 = make_block(Hash256::ZERO, 1_700_000_000, 1, vec![tx1]);
    engine
        .commit_block(&block1, 1, [1, 0, 0, 0])
        .expect("commit block 1 should succeed");

    // Verify record exists
    let records = engine
        .get_address_transactions(&addr_target, 1, 1, 10)
        .expect("query must succeed");
    assert_eq!(records.len(), 1);

    // Unwind block 1
    engine
        .unwind_block(&block1, 1)
        .expect("unwind_block must succeed");

    // Index must be cleanly wiped
    let records_after_unwind = engine
        .get_address_transactions(&addr_target, 1, 1, 10)
        .expect("query after unwind must succeed");
    assert!(
        records_after_unwind.is_empty(),
        "address index records must be purged after unwind"
    );

    // ── Part 2: Test apply_reorganization ─────────────────────────────────
    // Re-commit Block 1
    engine
        .commit_block(&block1, 1, [1, 0, 0, 0])
        .expect("re-commit block 1 must succeed");

    // Create Block 2A (Chain A) paying target_addr
    let tx2a = Transaction::new_coinbase(2, vec![TxOut::new(7_777, target_script)]);
    let block2a = make_block(block1.header.hash(), 1_700_000_020, 2, vec![tx2a]);
    engine
        .commit_block(&block2a, 2, [2, 0, 0, 0])
        .expect("commit block 2a must succeed");

    let target_b2_records = engine
        .get_address_transactions(&addr_target, 2, 2, 10)
        .expect("query target at height 2 must succeed");
    assert_eq!(target_b2_records.len(), 1);
    assert_eq!(target_b2_records[0].value_quanta, 7_777);

    // Create Block 2B (Chain B) paying alt_addr instead
    let tx2b = Transaction::new_coinbase(2, vec![TxOut::new(9_999, alt_script)]);
    let block2b = make_block(block1.header.hash(), 1_700_000_030, 3, vec![tx2b]);

    // Reorganize: disconnect Block 2A, connect Block 2B
    engine
        .apply_reorganization(&[block2a], &[(block2b, 2, [3, 0, 0, 0])])
        .expect("apply_reorganization must succeed");

    // Verify addr_target records at height 2 have been pruned
    let target_b2_after_reorg = engine
        .get_address_transactions(&addr_target, 2, 2, 10)
        .expect("query target at height 2 after reorg");
    assert!(
        target_b2_after_reorg.is_empty(),
        "orphaned block records for target_addr must be removed"
    );

    // Verify addr_alt records at height 2 are now present
    let alt_b2_records = engine
        .get_address_transactions(&addr_alt, 2, 2, 10)
        .expect("query alt at height 2 after reorg");
    assert_eq!(alt_b2_records.len(), 1);
    assert_eq!(alt_b2_records[0].value_quanta, 9_999);

    // Block 1 record for addr_target must remain intact
    let target_b1_records = engine
        .get_address_transactions(&addr_target, 1, 1, 10)
        .expect("query target at height 1");
    assert_eq!(target_b1_records.len(), 1);
}
