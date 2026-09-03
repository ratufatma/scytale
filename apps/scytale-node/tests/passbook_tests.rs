//! Passbook presentation-layer integration tests.
//!
//! Covers the Task 16 acceptance suites: zero-balance initialization, multi-UTXO
//! balance summation, sequential entry numbering, confirmed vs pending separation,
//! mining-reward reflection + provenance, reorganization re-projection, and restart
//! integrity. All projections go through the node's read-only query interface.

use scytale_consensus::calculate_block_reward;
use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoSet, TRANSACTION_VERSION_1,
};
use scytale_node::{EntryStatus, EntryType, Node, NodeConfig, Passbook, ProvenanceCategory};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Easiest compact target: any nonce satisfies Proof-of-Work.
const EASY_TARGET: u32 = 0x217F_FFFF;

const USER_LOCK: &[u8] = &[0x11, 0x22, 0x33];
const OTHER_LOCK: &[u8] = &[0x44, 0x55, 0x66];

fn test_config(data_dir: PathBuf, mining: bool) -> NodeConfig {
    NodeConfig {
        data_dir,
        mining_enabled: mining,
        genesis_difficulty_target: EASY_TARGET,
        shutdown_timeout_secs: 10,
        ..NodeConfig::default()
    }
}

fn wait_for_height(node: &Node, target: u64, timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if node.canonical_height() >= target {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Builds a coinbase-only block paying `value` to `lock`, extending `prev`.
fn build_reward_block(
    prev: Hash256,
    height: u64,
    nonce: u64,
    lock: &[u8],
    parent_utxos: &UtxoSet,
) -> Block {
    let coinbase = Transaction::new_coinbase(
        height,
        vec![TxOut::new(calculate_block_reward(height), lock.to_vec())],
    );
    let commitment = Hash256::hash(coinbase.txid().as_bytes());
    let mut staging = parent_utxos.clone();
    staging.insert(
        scytale_core::OutPoint::new(coinbase.txid(), 0),
        scytale_core::UtxoEntry::new(coinbase.outputs[0].clone(), height, true),
    );
    let utxo_root = staging.compute_utxo_root();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let header = BlockHeader::new(1, prev, commitment, utxo_root, now, EASY_TARGET, nonce);
    Block::new(header, vec![coinbase])
}

/// Injects a block extending the live canonical tip, retrying until it wins the
/// canonical slot (guards against ambient mining races). Returns the block hash.
fn inject_canonical_reward_block(node: &Node, nonce: u64, lock: &[u8]) -> Hash256 {
    loop {
        let tip = node.canonical_tip();
        let height = node.canonical_height();
        let utxos = node.query_utxo_set();
        let ext = build_reward_block(tip, height + 1, nonce, lock, &utxos);
        if node.submit_external_block(ext.clone()).unwrap() {
            return ext.header.hash();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Zero-balance initialization
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_zero_balance_initialization() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    // The user owns a lock that carries no on-chain value; the node's genesis
    // payout is a different lock, so the fresh user sees exactly 0 SCY.
    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    assert_eq!(passbook.confirmed_balance_quanta(&node).unwrap(), 0);

    let view = passbook.view(&node).unwrap();
    assert_eq!(view.confirmed_balance_quanta, 0);
    assert_eq!(view.pending_balance_quanta, 0);
    assert!(view.entries.is_empty());
    assert_eq!(view.total_entries, 0);

    node.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Mining reward reflection + provenance lineage
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mining_reward_reflection() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    // Two externally-injected canonical blocks both pay the user.
    let h1 = inject_canonical_reward_block(&node, 0, USER_LOCK);
    let h2 = inject_canonical_reward_block(&node, 1, USER_LOCK);
    assert_eq!(node.canonical_height(), 2);

    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    let expected = calculate_block_reward(1) + calculate_block_reward(2);
    assert_eq!(
        passbook.confirmed_balance_quanta(&node).unwrap(),
        expected,
        "confirmed balance equals the two mining rewards"
    );

    let view = passbook.view(&node).unwrap();
    let mining_entries: Vec<_> = view
        .entries
        .iter()
        .filter(|e| e.entry_type == EntryType::MiningReward)
        .collect();
    assert_eq!(
        mining_entries.len(),
        2,
        "two confirmed mining-reward entries exist"
    );
    for (idx, e) in mining_entries.iter().enumerate() {
        assert_eq!(
            e.status,
            EntryStatus::Confirmed {
                confirmations: 2 - idx as u64
            },
            "entry at height {} has the correct confirmation depth",
            idx + 1
        );
        assert_eq!(e.block_height, Some(idx as u64 + 1));
        assert!(
            e.outpoint.is_some(),
            "mining reward maps to its coinbase outpoint"
        );
    }

    // Provenance: trace the height-1 coinbase outpoint back to the coinbase origin.
    let h1_block = node.storage_handle().get_block(&h1).unwrap().unwrap();
    let coinbase = &h1_block.transactions[0];
    let origin = OutPoint::new(coinbase.txid(), 0);
    let lineage = passbook.provenance(&node, &origin).unwrap();
    assert!(!lineage.is_empty());
    assert_eq!(lineage[0].category, ProvenanceCategory::Coinbase);
    assert_eq!(lineage[0].txid, coinbase.txid());

    let _ = h2;
    node.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Sequential passbook entry numbering
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_passbook_entry_numbering() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    inject_canonical_reward_block(&node, 0, USER_LOCK);
    inject_canonical_reward_block(&node, 1, USER_LOCK);
    inject_canonical_reward_block(&node, 2, USER_LOCK);
    assert_eq!(node.canonical_height(), 3);

    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    let view = passbook.view(&node).unwrap();

    let mut numbers: Vec<u64> = view.entries.iter().map(|e| e.entry_number).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, vec![1, 2, 3], "sequential entry numbers #1 #2 #3");
    assert_eq!(view.total_entries, 3);

    // Numerically-named entries reference increasing block heights.
    let mut heights: Vec<u64> = view
        .entries
        .iter()
        .map(|e| e.block_height.unwrap())
        .collect();
    heights.sort_unstable();
    assert_eq!(heights, vec![1, 2, 3]);

    node.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Confirmed vs pending separation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_confirmed_vs_pending_separation() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    // Fund the user with one confirmed mining reward (height 1).
    let h1 = inject_canonical_reward_block(&node, 0, USER_LOCK);
    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    let confirmed_before = calculate_block_reward(1);
    assert_eq!(
        passbook.confirmed_balance_quanta(&node).unwrap(),
        confirmed_before
    );

    // Build a pending transaction that spends the user's confirmed coinbase and
    // sends the proceeds to OTHER_LOCK. This is a net outflow for the user.
    let h1_block = node.storage_handle().get_block(&h1).unwrap().unwrap();
    let coinbase = &h1_block.transactions[0];
    let input_op = OutPoint::new(coinbase.txid(), 0);
    let input_value = coinbase.outputs[0].value;

    let fee = 10_000u64;
    let pending_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(input_op, USER_LOCK.to_vec())],
        vec![TxOut::new(input_value - fee, OTHER_LOCK.to_vec())],
        0,
    );
    let pending_txid = node.submit_transaction(pending_tx).unwrap();
    assert_eq!(node.mempool_len(), 1);

    // The confirmed balance must be UNCHANGED — pending spends do not touch it.
    assert_eq!(
        passbook.confirmed_balance_quanta(&node).unwrap(),
        confirmed_before,
        "pending transaction must not inflate the confirmed balance"
    );

    // The pending delta reflects the outflow of the yet-unconfirmed spend.
    assert_eq!(
        passbook.pending_balance_delta(&node).unwrap(),
        -(input_value as i64),
        "pending spend subtracts the confirmed input being spent"
    );

    let view = passbook.view(&node).unwrap();
    assert_eq!(
        view.confirmed_balance_quanta, confirmed_before,
        "view confirmed balance ignores pending"
    );
    assert_eq!(
        view.pending_balance_quanta,
        -(input_value as i64),
        "view pending delta matches"
    );
    let has_pending = view
        .entries
        .iter()
        .any(|e| e.status == EntryStatus::Pending);
    assert!(has_pending, "the unconfirmed spend yields a Pending entry");
    assert!(
        view.entries.iter().any(|e| {
            e.status == EntryStatus::Pending
                && e.entry_type == EntryType::Sent
                && e.txid == pending_txid
        }),
        "pending entry classified as Sent with the correct TxID"
    );

    node.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Reorganization re-projection
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_reorganization_updates_passbook() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    // Branch A (pays OTHER_LOCK) grows to height 2 and is canonical at first.
    let genesis_tip = node.canonical_tip();
    let genesis_utxos = node.query_utxo_set();

    let a1 = build_reward_block(genesis_tip, 1, 0, OTHER_LOCK, &genesis_utxos);
    assert!(node.submit_external_block(a1.clone()).unwrap());
    let a1_utxos = node.query_utxo_set();
    let a2 = build_reward_block(a1.header.hash(), 2, 0, OTHER_LOCK, &a1_utxos);
    assert!(node.submit_external_block(a2.clone()).unwrap());
    assert_eq!(node.canonical_height(), 2);

    // The user owns USER_LOCK; branch A pays OTHER_LOCK, so the user sees 0.
    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    assert_eq!(passbook.confirmed_balance_quanta(&node).unwrap(), 0);
    assert_eq!(passbook.view(&node).unwrap().total_entries, 0);

    // Branch B (pays USER_LOCK) forks at height 1 and extends to height 3.
    let b1 = build_reward_block(genesis_tip, 1, 1, USER_LOCK, &genesis_utxos);
    assert!(!node.submit_external_block(b1.clone()).unwrap()); // side branch

    let mut b1_utxos = genesis_utxos.clone();
    b1_utxos.insert(
        scytale_core::OutPoint::new(b1.transactions[0].txid(), 0),
        scytale_core::UtxoEntry::new(b1.transactions[0].outputs[0].clone(), 1, true),
    );
    let b2 = build_reward_block(b1.header.hash(), 2, 1, USER_LOCK, &b1_utxos);
    assert!(!node.submit_external_block(b2.clone()).unwrap()); // equal work -> side

    let mut b2_utxos = b1_utxos.clone();
    b2_utxos.insert(
        scytale_core::OutPoint::new(b2.transactions[0].txid(), 0),
        scytale_core::UtxoEntry::new(b2.transactions[0].outputs[0].clone(), 2, true),
    );
    let b3 = build_reward_block(b2.header.hash(), 3, 1, USER_LOCK, &b2_utxos);
    assert!(node.submit_external_block(b3.clone()).unwrap()); // heavier -> reorg
    assert_eq!(node.canonical_height(), 3);

    // The passbook auto-re-projects against the new canonical branch: the user
    // now sees exactly the three USER_LOCK mined rewards, not branch A's.
    let expected =
        calculate_block_reward(1) + calculate_block_reward(2) + calculate_block_reward(3);
    assert_eq!(
        passbook.confirmed_balance_quanta(&node).unwrap(),
        expected,
        "passbook re-projects onto the new canonical branch"
    );
    let view = passbook.view(&node).unwrap();
    assert_eq!(view.total_entries, 3, "three rewards on branch B");
    let mut heights: Vec<u64> = Vec::new();
    for e in &view.entries {
        assert_eq!(e.entry_type, EntryType::MiningReward);
        let h = e.block_height.expect("confirmed entry has a block height");
        heights.push(h);
        assert_eq!(
            e.status,
            EntryStatus::Confirmed {
                confirmations: 3 + 1 - h
            },
            "confirmation depth computed from canonical tip height"
        );
    }
    heights.sort_unstable();
    assert_eq!(
        heights,
        vec![1, 2, 3],
        "branch B heights 1,2,3 are canonical"
    );

    node.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Restart preserves passbook integrity
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_restart_preserves_passbook_integrity() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("db");
    let mut config = test_config(path.clone(), true);
    // The miner pays the passbook user, so mined rewards project into the book.
    config.miner_payout_script = USER_LOCK.to_vec();
    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);

    let (saved_view, saved_height);
    {
        let mut node = Node::open(config.clone()).unwrap();
        node.start().unwrap();
        assert!(
            wait_for_height(&node, 3, 10_000),
            "mine at least three blocks"
        );
        saved_height = node.canonical_height();
        saved_view = passbook.view(&node).unwrap();
        node.shutdown().unwrap();
    }

    // Reopen the same data directory: the projection must reproduce identically.
    {
        let mut node2 = Node::open(config).unwrap();
        node2.start().unwrap();
        assert_eq!(node2.canonical_height(), saved_height);

        let restored = passbook.view(&node2).unwrap();
        assert_eq!(
            restored.confirmed_balance_quanta,
            saved_view.confirmed_balance_quanta
        );
        assert_eq!(restored.total_entries, saved_view.total_entries);
        assert_eq!(
            restored.pending_balance_quanta,
            saved_view.pending_balance_quanta
        );
        let restored_nums: Vec<u64> = restored.entries.iter().map(|e| e.entry_number).collect();
        let saved_nums: Vec<u64> = saved_view.entries.iter().map(|e| e.entry_number).collect();
        assert_eq!(restored_nums, saved_nums, "identical sequential numbering");
        let restored_types: Vec<EntryType> =
            restored.entries.iter().map(|e| e.entry_type).collect();
        let saved_types: Vec<EntryType> = saved_view.entries.iter().map(|e| e.entry_type).collect();
        assert_eq!(restored_types, saved_types, "identical entry types");

        node2.shutdown().unwrap();
    }
}
