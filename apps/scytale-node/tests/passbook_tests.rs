//! Passbook presentation-layer integration tests.
//!
//! Covers: zero-balance initialization, multi-UTXO balance summation, sequential
//! entry numbering, confirmed vs pending separation, mining-reward reflection + provenance,
//! reorganization re-projection, restart integrity, multi-asset SCY-20 token tracking,
//! contract interaction datum hash resolution, and inbound/outbound/change mutations.

use scytale_consensus::calculate_block_reward;
use scytale_core::{
    Address, Block, BlockHeader, Hash256, OutPoint, OutputLock, Transaction, TxIn, TxOut, UtxoSet,
    TRANSACTION_VERSION_1,
};
use scytale_node::{
    EntryStatus, Node, NodeConfig, Passbook, PassbookAction, PassbookAsset,
    ProvenanceCategory,
};
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

    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    assert_eq!(passbook.confirmed_balance_quanta(&node).unwrap(), 0);

    let view = passbook.view(&node).unwrap();
    assert_eq!(view.confirmed_native_balance_quanta, 0);
    assert_eq!(view.pending_native_balance_quanta, 0);
    assert!(view.entries.is_empty());
    assert_eq!(view.total_entries(), 0);

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
        .filter(|e| e.action == PassbookAction::MiningReward)
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
    assert_eq!(view.total_entries(), 3);

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

    let h1 = inject_canonical_reward_block(&node, 0, USER_LOCK);
    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    let confirmed_before = calculate_block_reward(1);
    assert_eq!(
        passbook.confirmed_balance_quanta(&node).unwrap(),
        confirmed_before
    );

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

    assert_eq!(
        passbook.confirmed_balance_quanta(&node).unwrap(),
        confirmed_before,
        "pending transaction must not inflate the confirmed balance"
    );

    assert_eq!(
        passbook.pending_balance_delta(&node).unwrap(),
        -(input_value as i64),
        "pending spend subtracts the confirmed input being spent"
    );

    let view = passbook.view(&node).unwrap();
    assert_eq!(
        view.confirmed_native_balance_quanta, confirmed_before,
        "view confirmed balance ignores pending"
    );
    assert_eq!(
        view.pending_native_balance_quanta,
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
                && e.action == PassbookAction::Sent
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

    let genesis_tip = node.canonical_tip();
    let genesis_utxos = node.query_utxo_set();

    let a1 = build_reward_block(genesis_tip, 1, 0, OTHER_LOCK, &genesis_utxos);
    assert!(node.submit_external_block(a1.clone()).unwrap());
    let a1_utxos = node.query_utxo_set();
    let a2 = build_reward_block(a1.header.hash(), 2, 0, OTHER_LOCK, &a1_utxos);
    assert!(node.submit_external_block(a2.clone()).unwrap());
    assert_eq!(node.canonical_height(), 2);

    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    assert_eq!(passbook.confirmed_balance_quanta(&node).unwrap(), 0);
    assert_eq!(passbook.view(&node).unwrap().total_entries(), 0);

    let b1 = build_reward_block(genesis_tip, 1, 1, USER_LOCK, &genesis_utxos);
    assert!(!node.submit_external_block(b1.clone()).unwrap());

    let mut b1_utxos = genesis_utxos.clone();
    b1_utxos.insert(
        scytale_core::OutPoint::new(b1.transactions[0].txid(), 0),
        scytale_core::UtxoEntry::new(b1.transactions[0].outputs[0].clone(), 1, true),
    );
    let b2 = build_reward_block(b1.header.hash(), 2, 1, USER_LOCK, &b1_utxos);
    assert!(!node.submit_external_block(b2.clone()).unwrap());

    let mut b2_utxos = b1_utxos.clone();
    b2_utxos.insert(
        scytale_core::OutPoint::new(b2.transactions[0].txid(), 0),
        scytale_core::UtxoEntry::new(b2.transactions[0].outputs[0].clone(), 2, true),
    );
    let b3 = build_reward_block(b2.header.hash(), 3, 1, USER_LOCK, &b2_utxos);
    assert!(node.submit_external_block(b3.clone()).unwrap());
    assert_eq!(node.canonical_height(), 3);

    let expected =
        calculate_block_reward(1) + calculate_block_reward(2) + calculate_block_reward(3);
    assert_eq!(
        passbook.confirmed_balance_quanta(&node).unwrap(),
        expected,
        "passbook re-projects onto the new canonical branch"
    );
    let view = passbook.view(&node).unwrap();
    assert_eq!(view.total_entries(), 3, "three rewards on branch B");
    let mut heights: Vec<u64> = Vec::new();
    for e in &view.entries {
        assert_eq!(e.action, PassbookAction::MiningReward);
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

    {
        let mut node2 = Node::open(config).unwrap();
        node2.start().unwrap();
        assert_eq!(node2.canonical_height(), saved_height);

        let restored = passbook.view(&node2).unwrap();
        assert_eq!(
            restored.confirmed_native_balance_quanta,
            saved_view.confirmed_native_balance_quanta
        );
        assert_eq!(restored.total_entries(), saved_view.total_entries());
        assert_eq!(
            restored.pending_native_balance_quanta,
            saved_view.pending_native_balance_quanta
        );
        let restored_nums: Vec<u64> = restored.entries.iter().map(|e| e.entry_number).collect();
        let saved_nums: Vec<u64> = saved_view.entries.iter().map(|e| e.entry_number).collect();
        assert_eq!(restored_nums, saved_nums, "identical sequential numbering");
        let restored_types: Vec<PassbookAction> =
            restored.entries.iter().map(|e| e.action.clone()).collect();
        let saved_types: Vec<PassbookAction> =
            saved_view.entries.iter().map(|e| e.action.clone()).collect();
        assert_eq!(restored_types, saved_types, "identical entry types");

        node2.shutdown().unwrap();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Multi-Asset & Contract Mutation Tests (Stage 2 Hardening)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_passbook_native_inbound_outbound_change() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    let user_lock = vec![0xaa, 0xbb, 0xcc];
    let other_lock = vec![0xdd, 0xee, 0xff];

    // Block 1: Fund user with coinbase
    let h1 = inject_canonical_reward_block(&node, 0, &user_lock);
    let h1_block = node.storage_handle().get_block(&h1).unwrap().unwrap();
    let cb_tx = &h1_block.transactions[0];
    let cb_val = cb_tx.outputs[0].value;

    let passbook = Passbook::new(vec![user_lock.clone()]);
    assert_eq!(passbook.confirmed_balance_quanta(&node).unwrap(), cb_val);

    // Build transfer transaction with change output back to user
    let spend_amount = 5_000_000u64;
    let fee = 10_000u64;
    let change_amount = cb_val - spend_amount - fee;
    let tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(cb_tx.txid(), 0), user_lock.clone())],
        vec![
            TxOut::new(spend_amount, other_lock.clone()),
            TxOut::new(change_amount, user_lock.clone()),
        ],
        0,
    );

    let submitted_txid = node.submit_transaction(tx).unwrap();
    assert_eq!(node.mempool_len(), 1);

    // Check pending state before mining
    let view_pending = passbook.view(&node).unwrap();
    assert_eq!(view_pending.confirmed_native_balance_quanta, cb_val);
    assert_eq!(
        view_pending.pending_native_balance_quanta,
        -((spend_amount + fee) as i64)
    );

    // Mine block 2 with the template containing the mempool transaction
    let template = node.build_mining_template(other_lock.clone()).unwrap();
    assert_eq!(template.transactions.len(), 2);
    assert_eq!(template.transactions[1].txid(), submitted_txid);

    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let solved = scytale_mining::run_pow_search(&template, 0, 100_000, &cancel).unwrap();
    let b2 = template.assemble_block(solved);
    assert!(node.submit_external_block(b2).unwrap());
    assert_eq!(node.canonical_height(), 2);
    assert_eq!(node.mempool_len(), 0);

    // Check confirmed state after block 2 confirmation
    let view = passbook.view(&node).unwrap();
    assert_eq!(view.confirmed_native_balance_quanta, change_amount);
    assert_eq!(view.pending_native_balance_quanta, 0);

    let has_reward = view
        .entries
        .iter()
        .any(|e| e.action == PassbookAction::MiningReward && e.amount_quanta == cb_val);
    let has_change = view
        .entries
        .iter()
        .any(|e| e.action == PassbookAction::Change && e.amount_quanta == change_amount);
    assert!(has_reward, "must reflect mining reward");
    assert!(has_change, "must reflect confirmed change output");

    node.shutdown().unwrap();
}

#[test]
fn test_passbook_scy20_token_tracking() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    let user_lock_bytes = vec![0x77; 32];
    let token_id = Hash256::new([0x88; 32]);

    let scy20_payload = scytale_node::passbook::Scy20DatumPayload {
        token_id: *token_id.as_bytes(),
        owner: [0x77; 32],
        amount: 25_000,
    };
    let datum_bytes = bincode::serialize(&scy20_payload).unwrap();
    let script_lock = OutputLock::Script {
        script_hash: [0x33; 32],
        datum: datum_bytes,
    };
    let condition = script_lock.to_locking_condition();

    // Inject block paying to the SCY-20 script condition
    inject_canonical_reward_block(&node, 0, &condition);
    assert_eq!(node.canonical_height(), 1);

    let mut passbook = Passbook::new(vec![user_lock_bytes]);
    passbook.add_owned_lock(condition);

    let view = passbook.view(&node).unwrap();
    assert_eq!(view.token_balances.get(&token_id), Some(&25_000));

    let scy20_entry = view
        .entries
        .iter()
        .find(|e| matches!(e.asset, PassbookAsset::Scy20 { .. }));
    assert!(scy20_entry.is_some(), "must record SCY-20 entry");
    assert_eq!(scy20_entry.unwrap().amount_quanta, 25_000);
    assert_eq!(scy20_entry.unwrap().action, PassbookAction::Scy20Mint);

    node.shutdown().unwrap();
}

#[test]
fn test_passbook_contract_interaction_with_datum_hash() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    let datum = b"scytale-smart-contract-state-payload".to_vec();
    let expected_datum_hash = Hash256::hash(&datum);
    let contract_lock = OutputLock::Script {
        script_hash: [0x55; 32],
        datum,
    };
    let condition = contract_lock.to_locking_condition();

    // Inject block paying to the contract condition
    inject_canonical_reward_block(&node, 0, &condition);
    assert_eq!(node.canonical_height(), 1);

    let passbook = Passbook::new(vec![condition]);
    let view = passbook.view(&node).unwrap();

    let entry = view
        .entries
        .iter()
        .find(|e| matches!(e.action, PassbookAction::ContractInteraction { .. }));
    assert!(entry.is_some(), "must record ContractInteraction action");
    assert_eq!(entry.unwrap().datum_hash, Some(expected_datum_hash));

    node.shutdown().unwrap();
}

#[test]
fn test_passbook_confirmed_vs_pending_balances() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    let h1 = inject_canonical_reward_block(&node, 0, USER_LOCK);
    let h1_block = node.storage_handle().get_block(&h1).unwrap().unwrap();
    let cb_tx = &h1_block.transactions[0];
    let cb_val = cb_tx.outputs[0].value;

    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);

    let initial_view = passbook.view(&node).unwrap();
    assert_eq!(initial_view.confirmed_native_balance_quanta, cb_val);
    assert_eq!(initial_view.pending_native_balance_quanta, 0);

    let fee = 5_000u64;
    let send_val = 1_000_000u64;
    let tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(cb_tx.txid(), 0), USER_LOCK.to_vec())],
        vec![
            TxOut::new(send_val, OTHER_LOCK.to_vec()),
            TxOut::new(cb_val - send_val - fee, USER_LOCK.to_vec()),
        ],
        0,
    );
    node.submit_transaction(tx).unwrap();

    let pending_view = passbook.view(&node).unwrap();
    // Confirmed balance must remain strictly unmutated
    assert_eq!(pending_view.confirmed_native_balance_quanta, cb_val);
    // Net pending outflow: - (send_val + fee)
    assert_eq!(
        pending_view.pending_native_balance_quanta,
        -((send_val + fee) as i64)
    );

    node.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Cryptographic Passbook Statement & Merkle Proof Integrity (Stage 3)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_passbook_cryptographic_statement_generation_and_verification() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    // Mine 3 canonical blocks to user lock
    inject_canonical_reward_block(&node, 0, USER_LOCK);
    inject_canonical_reward_block(&node, 1, USER_LOCK);
    inject_canonical_reward_block(&node, 2, USER_LOCK);
    assert_eq!(node.canonical_height(), 3);

    let user_addr_hash =
        scytale_storage::extract_address_from_locking_condition(USER_LOCK).unwrap();
    let user_address = Address::new(user_addr_hash);

    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    let statement = passbook.generate_statement(&node, &user_address).unwrap();

    assert_eq!(statement.account, user_address);
    assert_eq!(statement.generated_at_height, 3);
    assert_eq!(statement.block_hash, node.canonical_tip());
    assert_eq!(statement.active_utxo_proofs.len(), 3);

    let expected_balance =
        calculate_block_reward(1) + calculate_block_reward(2) + calculate_block_reward(3);
    assert_eq!(
        statement.confirmed_native_balance_quanta,
        expected_balance
    );

    // Cryptographic offline verification
    assert!(
        statement.verify_integrity(),
        "cryptographic passbook statement must pass offline integrity verification"
    );

    node.shutdown().unwrap();
}

#[test]
fn test_passbook_statement_tamper_detection() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    node.start().unwrap();

    inject_canonical_reward_block(&node, 0, USER_LOCK);
    inject_canonical_reward_block(&node, 1, USER_LOCK);

    let user_addr_hash =
        scytale_storage::extract_address_from_locking_condition(USER_LOCK).unwrap();
    let user_address = Address::new(user_addr_hash);

    let passbook = Passbook::new(vec![USER_LOCK.to_vec()]);
    let valid_statement = passbook.generate_statement(&node, &user_address).unwrap();
    assert!(valid_statement.verify_integrity());

    // 1. Tamper: balance amount inflated
    let mut tampered_balance = valid_statement.clone();
    tampered_balance.confirmed_native_balance_quanta += 100_000;
    assert!(
        !tampered_balance.verify_integrity(),
        "statement with inflated confirmed balance must be rejected"
    );

    // 2. Tamper: UTXO root swapped
    let mut tampered_root = valid_statement.clone();
    tampered_root.utxo_root = Hash256::new([0xee; 32]);
    assert!(
        !tampered_root.verify_integrity(),
        "statement with forged utxo_root must be rejected"
    );

    // 3. Tamper: proof quanta modified
    let mut tampered_proof_val = valid_statement.clone();
    tampered_proof_val.active_utxo_proofs[0].value_quanta += 1;
    assert!(
        !tampered_proof_val.verify_integrity(),
        "statement with tampered proof value must be rejected"
    );

    // 4. Tamper: proof leaf hash modified
    let mut tampered_leaf = valid_statement.clone();
    tampered_leaf.active_utxo_proofs[0].leaf_hash = Hash256::new([0x11; 32]);
    assert!(
        !tampered_leaf.verify_integrity(),
        "statement with tampered proof leaf hash must be rejected"
    );

    // 5. Tamper: omitted active UTXO proof
    let mut tampered_omission = valid_statement.clone();
    tampered_omission.active_utxo_proofs.pop();
    assert!(
        !tampered_omission.verify_integrity(),
        "statement with missing active UTXO proof must fail balance reconciliation"
    );

    node.shutdown().unwrap();
}
