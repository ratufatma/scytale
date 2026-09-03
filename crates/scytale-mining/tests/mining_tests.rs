/// Mining Lifecycle Integration Test Suite
/// =========================================
/// Covers:
///   - T1: Candidate block template assembly (coinbase + fee aggregation)
///   - T2: Coinbase output value exactness (subsidy + fees, no inflation)
///   - T3: Zero-balance user bootstrapping (node with zero UTXOs builds valid candidate)
///   - T4: PoW nonce search finds solution within easy target
///   - T5: Cancellation token aborts PoW search immediately
///   - T6: Stale candidate abort on simulated new-block arrival
///   - T7: End-to-end: solve block, assemble Block, assert coinbase UTXO present
use scytale_consensus::{calculate_block_reward, ChainTree, Target};
use scytale_core::{Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoSet};
use scytale_mining::{build_template, run_pow_search, MiningError};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// ──────────────────────────────────────────────
// Test Helpers
// ──────────────────────────────────────────────

/// Compact target for maximum difficulty (all ones = trivially easy).
const EASY_COMPACT_TARGET: u32 = 0x2100ffff;

/// Build a genesis block with an easy target for testing.
fn make_genesis_block() -> Block {
    let coinbase = Transaction::new_coinbase(0, vec![TxOut::new(1_000_000_000, vec![])]);
    let genesis_outpoint = OutPoint::new(coinbase.txid(), 0);
    let genesis_utxo_root =
        scytale_core::compute_utxo_leaf(&genesis_outpoint, &coinbase.outputs[0]);
    let header = BlockHeader::new(
        1,
        Hash256::ZERO,
        coinbase.txid(),
        genesis_utxo_root,
        1_700_000_000,
        EASY_COMPACT_TARGET,
        0,
    );
    Block::new(header, vec![coinbase])
}

/// Build a UtxoSet populated with one spendable UTXO.
fn make_utxo_set_with_one_output() -> (UtxoSet, OutPoint) {
    use scytale_core::UtxoEntry;
    let txid = Hash256::hash(b"funded_genesis");
    let op = OutPoint::new(txid, 0);
    let mut utxos = UtxoSet::new();
    utxos.insert(
        op,
        UtxoEntry::new(TxOut::new(5_000_000, vec![0x01]), 0, false),
    );
    (utxos, op)
}

/// Build a non-cancelling cancel token.
fn no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// Build an already-cancelled cancel token.
fn already_cancelled() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(true))
}

// ──────────────────────────────────────────────
// T1: Candidate block template assembly
// ──────────────────────────────────────────────

#[test]
fn test_candidate_block_assembly() {
    let genesis = make_genesis_block();
    let chain = ChainTree::new(genesis.clone());
    let utxos = UtxoSet::new(); // empty canonical UTXO set (miner has no spendable outputs)
    let mempool = scytale_mempool::Mempool::new(); // empty mempool

    let miner_script = vec![0xDE, 0xAD];
    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        EASY_COMPACT_TARGET,
        miner_script.clone(),
        1_700_000_001,
    )
    .expect("template must be built successfully");

    // Height must be genesis+1 = 1
    assert_eq!(template.height, 1);

    // Must have exactly one transaction (coinbase only, no mempool txs)
    assert_eq!(template.transactions.len(), 1);
    assert!(
        template.transactions[0].is_coinbase(),
        "first transaction must be coinbase"
    );

    // Coinbase locking condition must match miner_script
    assert_eq!(
        template.transactions[0].outputs[0].locking_condition,
        miner_script
    );

    // Parent must equal genesis hash
    assert_eq!(template.previous_block_hash, genesis.header.hash());
}

// ──────────────────────────────────────────────
// T2: Coinbase output value exactness
// ──────────────────────────────────────────────

#[test]
fn test_coinbase_output_exactness() {
    let genesis = make_genesis_block();
    let chain = ChainTree::new(genesis.clone());
    let (utxos, op) = make_utxo_set_with_one_output();
    let mut mempool = scytale_mempool::Mempool::new();

    // Admit a transaction that spends the UTXO and pays a 500_000 quanta fee
    use scytale_core::{AuthorizationError, AuthorizationVerifier};

    struct MockVerifier;
    impl AuthorizationVerifier for MockVerifier {
        fn verify(
            &self,
            _digest: &Hash256,
            _locking_condition: &[u8],
            authorization_proof: &[u8],
        ) -> Result<(), AuthorizationError> {
            if authorization_proof.is_empty() {
                return Err(AuthorizationError::EmptyAuthorization);
            }
            Ok(())
        }
    }

    let tx = Transaction::new(
        scytale_core::TRANSACTION_VERSION_1,
        vec![TxIn::new(op, vec![0x01])],
        vec![TxOut::new(4_500_000, vec![0x02])], // 5_000_000 - 4_500_000 = 500_000 fee
        0,
    );
    mempool
        .admit_transaction(tx.clone(), &utxos, &MockVerifier, 1_700_000_000)
        .unwrap();

    let expected_subsidy = calculate_block_reward(1);
    let expected_fee: u64 = 500_000;

    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        EASY_COMPACT_TARGET,
        vec![0xAB],
        1_700_000_001,
    )
    .unwrap();

    let coinbase_value = template.transactions[0].outputs[0].value;
    assert_eq!(
        coinbase_value,
        expected_subsidy + expected_fee,
        "coinbase value must equal subsidy + fees exactly"
    );
}

// ──────────────────────────────────────────────
// T3: Zero-balance user bootstrapping
// ──────────────────────────────────────────────

#[test]
fn test_zero_balance_mining_bootstrap() {
    let genesis = make_genesis_block();
    let chain = ChainTree::new(genesis.clone());
    // Empty UTXO set — miner has NO spendable outputs at all
    let utxos = UtxoSet::new();
    let mempool = scytale_mempool::Mempool::new();

    // Must succeed even with zero prior balance
    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        EASY_COMPACT_TARGET,
        vec![0xFF],
        1_700_000_001,
    )
    .expect("zero-balance node must be able to construct a valid mining candidate");

    assert!(
        !template.transactions.is_empty(),
        "template must contain at least the coinbase"
    );
    assert!(
        template.transactions[0].is_coinbase(),
        "first tx must be coinbase"
    );
    // Coinbase value must be >= subsidy (no fees in empty mempool)
    let subsidy = calculate_block_reward(1);
    assert_eq!(template.transactions[0].outputs[0].value, subsidy);
}

// ──────────────────────────────────────────────
// T4: PoW nonce search finds solution within easy target
// ──────────────────────────────────────────────

#[test]
fn test_pow_search_finds_solution_easy_target() {
    let genesis = make_genesis_block();
    let chain = ChainTree::new(genesis.clone());
    let utxos = UtxoSet::new();
    let mempool = scytale_mempool::Mempool::new();

    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        EASY_COMPACT_TARGET,
        vec![0x01],
        1_700_000_001,
    )
    .unwrap();

    let cancel = no_cancel();
    let result = run_pow_search(&template, 0, 1_000_000, &cancel);

    // With the easy target (0x2100ffff ≈ max target), must find solution quickly
    assert!(
        result.is_ok(),
        "PoW search must find a solution for the easy target: {:?}",
        result
    );

    // Verify the solved header actually satisfies PoW
    let solved_header = result.unwrap();
    let target = Target::from_compact(EASY_COMPACT_TARGET);
    assert!(
        scytale_consensus::verify_pow(&solved_header, &target).is_ok(),
        "solved header must satisfy PoW invariant"
    );
}

// ──────────────────────────────────────────────
// T5: Cancellation token aborts PoW search immediately
// ──────────────────────────────────────────────

#[test]
fn test_cancel_aborts_pow_search() {
    let genesis = make_genesis_block();
    let chain = ChainTree::new(genesis.clone());
    let utxos = UtxoSet::new();
    let mempool = scytale_mempool::Mempool::new();

    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        EASY_COMPACT_TARGET,
        vec![0x01],
        1_700_000_001,
    )
    .unwrap();

    let cancel = already_cancelled();
    let result = run_pow_search(&template, 0, 1_000_000, &cancel);

    assert!(
        matches!(result, Err(MiningError::Cancelled { height: 1 })),
        "a pre-cancelled token must abort immediately with MiningError::Cancelled"
    );
}

// ──────────────────────────────────────────────
// T6: Stale candidate abort on simulated new-block arrival
// ──────────────────────────────────────────────

#[test]
fn test_cancel_mining_on_new_block_arrival() {
    let genesis = make_genesis_block();
    let chain = ChainTree::new(genesis.clone());
    let utxos = UtxoSet::new();
    let mempool = scytale_mempool::Mempool::new();

    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        EASY_COMPACT_TARGET,
        vec![0x01],
        1_700_000_001,
    )
    .unwrap();

    // Shared cancellation token between "mining loop" and "block arrival handler"
    let cancel = Arc::new(AtomicBool::new(false));

    // Simulate a competing block arriving from the network: signal cancellation
    let cancel_for_network = Arc::clone(&cancel);
    cancel_for_network.store(true, Ordering::Relaxed);

    // Mining loop checks and sees cancellation immediately
    let result = run_pow_search(&template, 0, u64::MAX, &cancel);

    assert!(
        matches!(result, Err(MiningError::Cancelled { .. })),
        "mining worker must abort when cancellation token is signalled"
    );
}

// ──────────────────────────────────────────────
// T7: End-to-end solve block and assert coinbase UTXO recognized
// ──────────────────────────────────────────────

#[test]
fn test_end_to_end_mine_and_validate_coinbase_utxo() {
    let genesis = make_genesis_block();
    let chain = ChainTree::new(genesis.clone());
    let mut utxos = UtxoSet::new();
    let mempool = scytale_mempool::Mempool::new();

    let miner_script = vec![0xBE, 0xEF];
    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        EASY_COMPACT_TARGET,
        miner_script.clone(),
        1_700_000_001,
    )
    .unwrap();

    let cancel = no_cancel();
    let solved_header = run_pow_search(&template, 0, 1_000_000, &cancel)
        .expect("must solve block with easy target");

    // Assemble the full block from the template
    let block = template.assemble_block(solved_header.clone());

    // Pre-broadcast local validation: verify PoW
    let target = Target::from_compact(EASY_COMPACT_TARGET);
    assert!(
        scytale_consensus::verify_pow(&solved_header, &target).is_ok(),
        "pre-broadcast local PoW validation must pass"
    );

    // Apply coinbase to UTXO state (simulates block acceptance)
    utxos
        .apply_coinbase(&block.transactions[0], template.height)
        .expect("coinbase must be applied to UTXO set without error");

    // Assert that coinbase UTXO is now spendable
    let coinbase_outpoint = OutPoint::new(block.transactions[0].txid(), 0);
    assert!(
        utxos.contains(&coinbase_outpoint),
        "coinbase UTXO must be recognized in the canonical UTXO set after block acceptance"
    );

    // Assert the miner's locking condition is preserved
    let entry = utxos.get(&coinbase_outpoint).unwrap();
    assert_eq!(entry.output.locking_condition, miner_script);

    // Assert value is correct (subsidy only, no fees)
    let expected_subsidy = calculate_block_reward(template.height);
    assert_eq!(entry.output.value, expected_subsidy);
}
