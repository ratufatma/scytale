use ed25519_dalek::Signer;
use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoEntry, UtxoSet,
    TRANSACTION_VERSION_1,
};
use scytale_node::{error::NodeError, Node, NodeConfig};
use scytale_script::{builder::ScriptBuilder, opcode::OpCode};
use tempfile::tempdir;

#[test]
fn test_node_verify_legacy_script() {
    let mut utxos = UtxoSet::new();
    let prev_txid = Hash256::hash(b"prev_tx");
    let prev_op = OutPoint::new(prev_txid, 0);
    let legacy_lock = vec![0x01, 0x02, 0x03];
    utxos.insert(
        prev_op,
        UtxoEntry::new(TxOut::new(100_000_000, legacy_lock.clone()), 1, false),
    );

    // 1. Valid legacy authorization matching locking script
    let valid_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(prev_op, legacy_lock.clone())],
        vec![TxOut::new(90_000_000, vec![0x04, 0x05])],
        0,
    );
    assert!(Node::verify_transaction_scripts(&valid_tx, 2, &utxos).is_ok());

    // 2. Mismatched legacy authorization fails closed
    let invalid_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(prev_op, vec![0x01, 0x02, 0x04])],
        vec![TxOut::new(90_000_000, vec![0x04, 0x05])],
        0,
    );
    let err = Node::verify_transaction_scripts(&invalid_tx, 2, &utxos).unwrap_err();
    assert!(matches!(
        err,
        NodeError::InvalidScript(_) | NodeError::ScriptEvaluationFailed
    ));
}

#[test]
fn test_node_verify_p2pkh_script() {
    let mut utxos = UtxoSet::new();
    let prev_txid = Hash256::hash(b"p2pkh_tx");
    let prev_op = OutPoint::new(prev_txid, 0);

    // Generate real Ed25519 keypair
    let secret = [0x42u8; 32];
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes = verifying_key.to_bytes();
    let pubkey_hash = blake3::hash(&pubkey_bytes);

    // Locking script (P2PKH): OP_DUP OP_BLAKE3 <hash> OP_EQUALVERIFY OP_CHECKSIG
    let p2pkh_lock = ScriptBuilder::new()
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpBlake3)
        .push_data(pubkey_hash.as_bytes())
        .push_opcode(OpCode::OpEqualVerify)
        .push_opcode(OpCode::OpCheckSig)
        .build();

    utxos.insert(
        prev_op,
        UtxoEntry::new(TxOut::new(500_000_000, p2pkh_lock.clone()), 10, false),
    );

    // Build unsigned transaction
    let recipient_lock = vec![0x0a, 0x0b];
    let mut spending_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(prev_op, vec![])],
        vec![TxOut::new(490_000_000, recipient_lock)],
        0,
    );

    // Compute sighash for input 0
    let sighash = spending_tx.compute_sighash(0, &p2pkh_lock);
    let signature = signing_key.sign(&sighash);
    let sig_bytes = signature.to_bytes();

    // Set unlocking script: <sig> <pubkey>
    let unlocking = ScriptBuilder::new()
        .push_data(&sig_bytes)
        .push_data(&pubkey_bytes)
        .build();
    spending_tx.inputs[0].authorization = unlocking;

    // Verify valid P2PKH script
    assert!(Node::verify_transaction_scripts(&spending_tx, 11, &utxos).is_ok());

    // Corrupted signature must fail closed
    let mut bad_sig_tx = spending_tx.clone();
    let mut corrupted_sig = sig_bytes;
    corrupted_sig[0] ^= 0xff;
    bad_sig_tx.inputs[0].authorization = ScriptBuilder::new()
        .push_data(&corrupted_sig)
        .push_data(&pubkey_bytes)
        .build();

    let err = Node::verify_transaction_scripts(&bad_sig_tx, 11, &utxos).unwrap_err();
    assert!(matches!(
        err,
        NodeError::InvalidScript(_) | NodeError::ScriptEvaluationFailed
    ));
}

#[test]
fn test_node_op_return_output_handling() {
    let temp = tempdir().unwrap();
    let config = NodeConfig {
        data_dir: temp.path().to_path_buf(),
        mining_enabled: false,
        miner_payout_script: vec![0x01, 0x02, 0x03],
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).unwrap();
    node.start().unwrap();

    // 1. Initial coinbase (Height 0) in genesis
    let genesis_tip = node.canonical_tip();
    let subsidy = scytale_consensus::calculate_block_reward(1);

    // 2. Mine a block with a standard output and an OP_RETURN data carrier
    let coinbase = Transaction::new_coinbase(1, vec![TxOut::new(subsidy, vec![0x01, 0x02, 0x03])]);
    let prev_op = OutPoint::new(
        node.query_utxo_set().entries().keys().next().unwrap().txid,
        0,
    );

    let op_return_lock = vec![0x6a, 0x08, b'S', b'C', b'Y', b'T', b'A', b'L', b'E', b'1'];
    let transfer_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(prev_op, vec![0x01, 0x02, 0x03])],
        vec![
            TxOut::new(50_000_000, vec![0xaa, 0xbb]),
            TxOut::new(0, op_return_lock.clone()),
        ],
        0,
    );
    let transfer_txid = transfer_tx.txid();

    let mut staging = node.query_utxo_set();
    staging.remove(&prev_op);
    staging.insert(
        OutPoint::new(transfer_txid, 0),
        scytale_core::UtxoEntry::new(TxOut::new(50_000_000, vec![0xaa, 0xbb]), 1, false),
    );
    staging.insert(
        OutPoint::new(coinbase.txid(), 0),
        scytale_core::UtxoEntry::new(TxOut::new(subsidy, vec![0x01, 0x02, 0x03]), 1, true),
    );
    let utxo_root = staging.compute_utxo_root();
    let header = BlockHeader::new(1, genesis_tip, Hash256::ZERO, utxo_root, 100, 0x207fffff, 0);
    let block = Block::new(header, vec![coinbase, transfer_tx]);

    assert!(node.submit_external_block(block).unwrap());
    assert_eq!(node.canonical_height(), 1);

    // 3. Verify standard output is present in the UTXO set
    let utxo_set = node.query_utxo_set();
    assert!(utxo_set.get(&OutPoint::new(transfer_txid, 0)).is_some());

    // 4. Verify OP_RETURN output is NOT present in the UTXO set
    assert!(utxo_set.get(&OutPoint::new(transfer_txid, 1)).is_none());

    // 5. Verify transaction itself is saved in storage
    let stored_tx = node
        .storage_handle()
        .get_transaction(&transfer_txid)
        .unwrap();
    assert!(stored_tx.is_some());
    assert_eq!(
        stored_tx.unwrap().outputs[1].locking_condition,
        op_return_lock
    );

    node.shutdown().unwrap();
}
