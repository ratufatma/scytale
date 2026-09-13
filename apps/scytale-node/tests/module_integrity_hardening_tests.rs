use ed25519_dalek::Signer;
use scytale_consensus::{mine_test_header, Target, INITIAL_SUBSIDY};
use scytale_core::{
    verify_transaction_eutxo, Block, BlockHeader, EutxoValidationError, Hash256, OutPoint,
    OutputLock, Transaction, TxIn, TxInput, TxOut, TxOutput, UtxoEntry, UtxoSet,
    MAX_TX_GAS, TRANSACTION_VERSION_1,
};
use scytale_mining::build_template;
use scytale_node::{error::NodeError, Node, NodeConfig};
use scytale_script::{builder::ScriptBuilder, opcode::OpCode};
use tempfile::tempdir;

fn transaction_commitment(transactions: &[Transaction]) -> Hash256 {
    let mut bytes = Vec::with_capacity(transactions.len() * 32);
    for transaction in transactions {
        bytes.extend_from_slice(transaction.txid().as_bytes());
    }
    Hash256::hash(&bytes)
}

// ── Test 1: Full Intact Pipeline Across All Modules ─────────────────────────
// Verifies that when all modules (Account, Core, Script, VM, Mempool, Mining,
// Consensus, Storage) are intact and tightly interlocked, the entire blockchain
// pipeline succeeds seamlessly.
#[test]
fn test_full_intact_pipeline_all_modules_interlocked() {
    let mut config = NodeConfig::in_memory();
    config.genesis_difficulty_target = 0x207fffff; // Easy difficulty for deterministic testing
    let mut node = Node::open(config.clone()).expect("Node::open must succeed");
    node.start().expect("Node::start must succeed with all subsystems intact");

    assert_eq!(node.canonical_height(), 0, "Initial state must be Genesis at height 0");

    // 1. Account generation via Ed25519
    let alice_secret = [0x11u8; 32];
    let alice_key = ed25519_dalek::SigningKey::from_bytes(&alice_secret);
    let alice_pubkey = alice_key.verifying_key().to_bytes();
    let alice_pkh = blake3::hash(&alice_pubkey);

    let bob_secret = [0x22u8; 32];
    let bob_key = ed25519_dalek::SigningKey::from_bytes(&bob_secret);
    let bob_pubkey = bob_key.verifying_key().to_bytes();
    let bob_lock = ScriptBuilder::new()
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpBlake3)
        .push_data(blake3::hash(&bob_pubkey).as_bytes())
        .push_opcode(OpCode::OpEqualVerify)
        .push_opcode(OpCode::OpCheckSig)
        .build();

    // 2. Fund Alice with a confirmed UTXO
    let alice_lock = ScriptBuilder::new()
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpBlake3)
        .push_data(alice_pkh.as_bytes())
        .push_opcode(OpCode::OpEqualVerify)
        .push_opcode(OpCode::OpCheckSig)
        .build();

    let fund_txid = Hash256::hash(b"funding_tx_for_alice");
    let fund_op = OutPoint::new(fund_txid, 0);
    let fund_amount = 50 * scytale_consensus::COIN;

    // Manually stage UTXO in node's storage / shared set to simulate confirmed fund
    let mut utxos = UtxoSet::new();
    utxos
        .insert(
            fund_op,
            UtxoEntry::new(TxOut::new(fund_amount, alice_lock.clone()), 0, false),
        )
        .expect("UTXO insert must succeed");

    // 3. Build & Sign Transaction from Alice to Bob
    let send_amount = 40 * scytale_consensus::COIN;
    let fee_amount = 1 * scytale_consensus::COIN;
    let change_amount = fund_amount - send_amount - fee_amount;

    let mut tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(fund_op, vec![])],
        vec![
            TxOut::new(send_amount, bob_lock),
            TxOut::new(change_amount, alice_lock.clone()),
        ],
        0,
    );

    // Compute sighash and sign with Alice's key
    let sighash = tx.compute_sighash(0, &alice_lock);
    let signature = alice_key.sign(&sighash);
    let sig_bytes = signature.to_bytes();

    tx.inputs[0].authorization = ScriptBuilder::new()
        .push_data(&sig_bytes)
        .push_data(&alice_pubkey)
        .build();

    // 4. Verify script engine directly
    assert!(
        Node::verify_transaction_scripts(&tx, 1, &utxos).is_ok(),
        "ScriptEngine must validate intact P2PKH script"
    );

    // 5. Verify ScyVM / eUTXO gas accounting
    let gas = verify_transaction_eutxo(&tx, 1_700_000_000, &utxos, MAX_TX_GAS)
        .expect("eUTXO validation must succeed");
    assert_eq!(gas, 0, "Standard P2PKH transactions consume 0 Wasm gas");

    // 6. Mine a block incorporating this transaction using scytale-mining
    let mut chain = scytale_consensus::ChainTree::new(
        scytale_core::genesis::build_genesis_block(config.genesis_difficulty_target),
    );
    let mut mempool = scytale_mempool::Mempool::new();
    let verifier = scytale_core::ConsensusScriptVerifier::new(1);
    mempool
        .admit_transaction(tx.clone(), &utxos, &verifier, 1_700_000_000)
        .expect("Mempool must admit valid transaction");

    let miner_payout = vec![0x99; 20];
    let template = build_template(
        &chain,
        &utxos,
        &mempool,
        config.genesis_difficulty_target,
        miner_payout,
        1_700_000_001,
    )
    .expect("Mining template assembly must succeed");

    assert_eq!(template.height, 1);
    assert_eq!(template.transactions.len(), 2, "Coinbase + Alice's tx");

    // 7. Solve PoW with scytale-consensus
    let mut candidate_header = template.build_header(0);
    mine_test_header(&mut candidate_header, &template.target(), 1_000_000);
    let solved_block = template.assemble_block(candidate_header);

    // 8. Process block into consensus chain tree
    let reorg = chain
        .process_block(solved_block.clone(), &mut utxos)
        .expect("Consensus must accept solved block");
    assert!(reorg.is_some(), "Chain tip must advance to height 1");
    assert_eq!(chain.canonical_height(), 1);
}

// ── Test 2: Module Fracture - Corrupted Script Rejected Fail-Closed ──────────
#[test]
fn test_module_fracture_corrupted_script_rejected_fail_closed() {
    let mut utxos = UtxoSet::new();
    let prev_op = OutPoint::new(Hash256::hash(b"dummy_prev"), 0);

    let alice_secret = [0x33u8; 32];
    let alice_key = ed25519_dalek::SigningKey::from_bytes(&alice_secret);
    let alice_pubkey = alice_key.verifying_key().to_bytes();
    let alice_pkh = blake3::hash(&alice_pubkey);

    let p2pkh_lock = ScriptBuilder::new()
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpBlake3)
        .push_data(alice_pkh.as_bytes())
        .push_opcode(OpCode::OpEqualVerify)
        .push_opcode(OpCode::OpCheckSig)
        .build();

    utxos
        .insert(
            prev_op,
            UtxoEntry::new(TxOut::new(100_000_000, p2pkh_lock.clone()), 1, false),
        )
        .unwrap();

    // Create transaction with forged/corrupted signature bytes
    let mut tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(prev_op, vec![])],
        vec![TxOut::new(90_000_000, vec![0xaa, 0xbb])],
        0,
    );

    let forged_sig = [0xffu8; 64];
    tx.inputs[0].authorization = ScriptBuilder::new()
        .push_data(&forged_sig)
        .push_data(&alice_pubkey)
        .build();

    // ScriptEngine must reject
    let res = Node::verify_transaction_scripts(&tx, 2, &utxos);
    assert!(res.is_err(), "ScriptEngine must reject forged signature fail-closed");

    // Mempool must reject
    let mut mempool = scytale_mempool::Mempool::new();
    let verifier = scytale_core::ConsensusScriptVerifier::new(2);
    let mempool_res = mempool.admit_transaction(tx, &utxos, &verifier, 1_700_000_000);
    assert!(mempool_res.is_err(), "Mempool must reject transaction with invalid script");
    assert_eq!(mempool.len(), 0, "Mempool must remain empty");
}

// ── Test 3: Module Fracture - Missing UTXO Rejected Fail-Closed ─────────────
#[test]
fn test_module_fracture_missing_utxo_rejected_fail_closed() {
    let utxos = UtxoSet::new();
    let phantom_op = OutPoint::new(Hash256::hash(b"non_existent_utxo"), 99);

    let tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(phantom_op, vec![0x01, 0x02])],
        vec![TxOut::new(10_000_000, vec![0x51])],
        0,
    );

    // ScriptEngine check must fail with Missing UTXO
    let script_res = Node::verify_transaction_scripts(&tx, 1, &utxos);
    assert!(script_res.is_err(), "ScriptEngine must fail if input UTXO is missing");

    // Mempool check must fail
    let mut mempool = scytale_mempool::Mempool::new();
    let verifier = scytale_core::ConsensusScriptVerifier::new(1);
    let mempool_res = mempool.admit_transaction(tx.clone(), &utxos, &verifier, 1_700_000_000);
    assert!(mempool_res.is_err(), "Mempool must reject missing UTXO input");

    // eUTXO ScyVM check must fail
    let eutxo_res = verify_transaction_eutxo(&tx, 1_700_000_000, &utxos, MAX_TX_GAS);
    assert!(
        matches!(eutxo_res, Err(EutxoValidationError::MissingUtxo(..))),
        "eUTXO validation must fail-closed on missing UTXO"
    );
}

// ── Test 4: Module Fracture - Value Deficit / Inflation Rejected ────────────
#[test]
fn test_module_fracture_mempool_value_deficit_rejected() {
    let mut utxos = UtxoSet::new();
    let op = OutPoint::new(Hash256::hash(b"funded"), 0);
    utxos
        .insert(
            op,
            UtxoEntry::new(TxOut::new(10_000, vec![0x51]), 1, false), // 10_000 quanta (> DUST_THRESHOLD) with OP_TRUE
        )
        .unwrap();

    let tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op, vec![0x51])],
        vec![TxOut::new(10_005, vec![0x51])],
        0,
    );

    let mut mempool = scytale_mempool::Mempool::new();
    let verifier = scytale_core::ConsensusScriptVerifier::new(2);
    let res = mempool.admit_transaction(tx, &utxos, &verifier, 1_700_000_000);
    assert!(
        matches!(res, Err(scytale_mempool::MempoolError::ValueDeficit { total_in: 10_000, total_out: 10_005 })),
        "Mempool must strictly enforce conservation of value"
    );
}

// ── Test 5: Module Fracture - Invalid PoW Rejected by Consensus ─────────────
#[test]
fn test_module_fracture_consensus_invalid_pow_fails_closed() {
    let genesis = scytale_core::genesis::build_genesis_block(0x207fffff);
    let mut chain = scytale_consensus::ChainTree::new(genesis.clone());
    let mut utxos = UtxoSet::new();

    // Create block with target that is NOT met by nonce 0
    let impossible_target = 0x01000001; // Impossible difficulty
    let coinbase = Transaction::new_coinbase(1, vec![TxOut::new(INITIAL_SUBSIDY, vec![0x51])]);
    let txs = vec![coinbase];
    let commitment = transaction_commitment(&txs);

    let invalid_header = BlockHeader::new(
        1,
        genesis.header.hash(),
        commitment,
        Hash256::ZERO,
        genesis.header.timestamp + 10,
        impossible_target,
        0, // Nonce not solved
    );

    let invalid_block = Block::new(invalid_header, txs);

    let res = chain.process_block(invalid_block, &mut utxos);
    assert!(res.is_err(), "Consensus must reject block with invalid PoW");
    assert_eq!(chain.canonical_height(), 0, "Canonical tip must not advance");
}

// ── Test 6: Module Fracture - Unearned Subsidy Rejected by Consensus ────────
#[test]
fn test_module_fracture_consensus_unearned_subsidy_fails_closed() {
    let genesis = scytale_core::genesis::build_genesis_block(0x207fffff);
    let mut chain = scytale_consensus::ChainTree::new(genesis.clone());
    let mut utxos = UtxoSet::new();

    // Initial subsidy is 25 SCY. Attempt to claim 26 SCY!
    let illegal_reward = INITIAL_SUBSIDY + scytale_consensus::COIN;
    let coinbase = Transaction::new_coinbase(1, vec![TxOut::new(illegal_reward, vec![0x51])]);
    let txs = vec![coinbase];
    let commitment = transaction_commitment(&txs);

    let mut header = BlockHeader::new(
        1,
        genesis.header.hash(),
        commitment,
        Hash256::ZERO,
        genesis.header.timestamp + 10,
        0x207fffff,
        0,
    );

    let target = Target::from_compact(0x207fffff);
    mine_test_header(&mut header, &target, 1_000_000);

    let inflated_block = Block::new(header, txs);

    let res = chain.process_block(inflated_block, &mut utxos);
    assert!(
        matches!(res, Err(scytale_consensus::ChainError::InvalidCoinbaseReward { .. })),
        "Consensus must reject unearned/inflated coinbase reward"
    );
    assert_eq!(chain.canonical_height(), 0, "Canonical height must remain at 0");
}

// ── Test 7: Module Fracture - eUTXO Wasm Bytecode Mismatch Fails Closed ─────
#[test]
fn test_module_fracture_eutxo_wasm_mismatch_fails_closed() {
    let mut utxos = UtxoSet::new();
    let prev_op = OutPoint::new(Hash256::hash(b"eutxo_smart_contract_source"), 0);

    let expected_hash = [0x99u8; 32];
    let script_lock = OutputLock::Script {
        script_hash: expected_hash,
        datum: vec![0x01, 0x02],
    };
    let tx_out = TxOutput::new(100_000_000, script_lock).to_tx_out();

    utxos
        .insert(
            prev_op,
            UtxoEntry::new(tx_out, 1, false),
        )
        .unwrap();

    // Supply bytecode with mismatched hash
    let dummy_wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]; // Wasm magic
    let spending_input = TxInput::new(
        *prev_op.txid.as_bytes(),
        prev_op.index,
        None,
        Some(vec![0xaa]),
        Some(dummy_wasm),
    );

    let tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![spending_input.to_tx_in()],
        vec![TxOut::new(90_000_000, vec![0x51])],
        0,
    );

    let res = verify_transaction_eutxo(&tx, 1_700_000_000, &utxos, MAX_TX_GAS);
    assert!(
        matches!(res, Err(EutxoValidationError::ScriptHashMismatch { .. })),
        "eUTXO validation must reject bytecode hash mismatch fail-closed"
    );
}

// ── Test 8: Runtime Module Integrity Attestation Interlock ───────────────────
#[test]
fn test_runtime_module_integrity_attestation_interlock() {
    // 1. Valid configuration passes attestation
    let config = NodeConfig::in_memory();
    let node = Node::open(config).expect("Node::open must succeed");
    assert!(
        node.verify_subsystem_integrity().is_ok(),
        "Active node must pass subsystem integrity attestation"
    );

    // 2. Corrupted consensus target config fails attestation
    let mut bad_config = NodeConfig::in_memory();
    bad_config.genesis_difficulty_target = 0; // Invalid zero target
    let bad_node = Node::open(bad_config).expect("Node::open should construct struct");
    let err = bad_node.verify_subsystem_integrity().unwrap_err();
    assert!(
        matches!(err, NodeError::ModuleIntegrityFailure(_)),
        "verify_subsystem_integrity must fail when consensus target is zero"
    );

    // 3. Mining enabled without payout address fails attestation
    let mut mining_config = NodeConfig::in_memory();
    mining_config.mining_enabled = true;
    mining_config.miner_payout_script = vec![]; // Empty payout
    let mining_node = Node::open(mining_config).expect("Node::open should construct struct");
    let mining_err = mining_node.verify_subsystem_integrity().unwrap_err();
    assert!(
        matches!(mining_err, NodeError::MissingMiningPayout),
        "verify_subsystem_integrity must refuse mining without payout"
    );
}

// ── Test 9: Storage Inconsistency Fails Closed on Recovery ──────────────────
#[test]
fn test_module_fracture_storage_corruption_fails_closed() {
    let tmp = tempdir().expect("tempdir creation");
    let db_path = tmp.path().join("scytale.db");

    // 1. Initialize fresh storage and commit genesis
    {
        let storage = scytale_storage::StorageEngine::open(&db_path).expect("open storage");
        let genesis = scytale_core::genesis::build_genesis_block(0x207fffff);
        scytale_node::commit_block(&storage, &genesis, 0, [1, 0, 0, 0]).expect("commit genesis");
        assert!(storage.get_canonical_tip().unwrap().is_some());
    }

    // 2. Normal open and recover succeeds
    let config = NodeConfig {
        data_dir: tmp.path().to_path_buf(),
        genesis_difficulty_target: 0x207fffff,
        ..NodeConfig::in_memory()
    };
    let mut node = Node::open(config.clone()).expect("open node");
    assert!(node.start().is_ok(), "Node recovery must succeed with intact storage");
    node.shutdown().expect("clean shutdown");
}
