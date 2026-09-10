use ed25519_dalek::Signer;
use scytale_consensus::{mine_test_header, Target};
use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoEntry, UtxoSet,
    TRANSACTION_VERSION_1,
};
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

#[test]
fn test_node_verify_legacy_script() {
    let mut utxos = UtxoSet::new();
    let prev_txid = Hash256::hash(b"prev_tx");
    let prev_op = OutPoint::new(prev_txid, 0);
    let legacy_lock = vec![0x01, 0x02, 0x03];
    utxos
        .insert(
            prev_op,
            UtxoEntry::new(TxOut::new(100_000_000, legacy_lock.clone()), 1, false),
        )
        .unwrap();

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

    utxos
        .insert(
            prev_op,
            UtxoEntry::new(TxOut::new(500_000_000, p2pkh_lock.clone()), 10, false),
        )
        .unwrap();

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
        genesis_difficulty_target: 0x207fffff,
        miner_payout_script: vec![0x01, 0x02, 0x03],
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).unwrap();
    node.start().unwrap();

    // 1. Initial Genesis state has 3 outputs
    let genesis_tip = node.canonical_tip();
    let subsidy1 = scytale_consensus::calculate_block_reward(1);

    // 2. Mine Block 1 with a coinbase paying to test lock 010203
    let cb1 = Transaction::new_coinbase(1, vec![TxOut::new(subsidy1, vec![0x01, 0x02, 0x03])]);
    let mut staging1 = node.query_utxo_set();
    staging1.insert(
        OutPoint::new(cb1.txid(), 0),
        scytale_core::UtxoEntry::new(TxOut::new(subsidy1, vec![0x01, 0x02, 0x03]), 1, true),
    );
    let utxo_root1 = staging1.compute_utxo_root();
    let mut header1 = BlockHeader::new(
        1,
        genesis_tip,
        transaction_commitment(std::slice::from_ref(&cb1)),
        utxo_root1,
        100,
        0x207fffff,
        0,
    );
    assert!(mine_test_header(
        &mut header1,
        &Target::from_compact(0x207fffff),
        10_000_000
    ));
    let block1 = Block::new(header1, vec![cb1.clone()]);
    assert!(node.submit_external_block(block1).unwrap());
    assert_eq!(node.canonical_height(), 1);

    // 3. In Block 2, spend Block 1 coinbase with standard output and OP_RETURN data carrier
    let tip1 = node.canonical_tip();
    let subsidy2 = scytale_consensus::calculate_block_reward(2);
    let cb2 = Transaction::new_coinbase(2, vec![TxOut::new(subsidy2, vec![0x01, 0x02, 0x03])]);
    let prev_op = OutPoint::new(cb1.txid(), 0);

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

    let mut staging2 = node.query_utxo_set();
    staging2.remove(&prev_op);
    staging2.insert(
        OutPoint::new(transfer_txid, 0),
        scytale_core::UtxoEntry::new(TxOut::new(50_000_000, vec![0xaa, 0xbb]), 2, false),
    );
    staging2.insert(
        OutPoint::new(cb2.txid(), 0),
        scytale_core::UtxoEntry::new(TxOut::new(subsidy2, vec![0x01, 0x02, 0x03]), 2, true),
    );
    let utxo_root2 = staging2.compute_utxo_root();
    let transactions2 = vec![cb2, transfer_tx];
    let mut header2 = BlockHeader::new(
        2,
        tip1,
        transaction_commitment(&transactions2),
        utxo_root2,
        200,
        0x207fffff,
        0,
    );
    assert!(mine_test_header(
        &mut header2,
        &Target::from_compact(0x207fffff),
        10_000_000
    ));
    let block2 = Block::new(header2, transactions2);

    assert!(node.submit_external_block(block2).unwrap());
    assert_eq!(node.canonical_height(), 2);

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

#[test]
fn test_utxo_root_template_matches_canonical_block_application() {
    let make_block =
        |prev_hash: Hash256, height: u64, utxos: &UtxoSet, transactions: Vec<Transaction>| {
            let mut staged = utxos.clone();
            let fee_total = staged.apply_block(&transactions, height).unwrap();
            let _ = fee_total;
            let utxo_root = staged.compute_utxo_root();
            let header = BlockHeader::new(
                1,
                prev_hash,
                transaction_commitment(&transactions),
                utxo_root,
                200 + height,
                0x207fffff,
                0,
            );
            Block::new(header, transactions)
        };

    let prev_txid = Hash256::hash(b"root_equiv_prev");
    let spend_1 = OutPoint::new(prev_txid, 0);
    let spend_2 = OutPoint::new(prev_txid, 1);

    // Scenario 1: coinbase + single spends to a standard output.
    let coinbase1 = Transaction::new_coinbase(1, vec![TxOut::new(10_000_000, vec![0x10])]);
    let spend_tx1 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(spend_1, vec![0x11])],
        vec![TxOut::new(9_000_000, vec![0x22])],
        0,
    );

    let mut utxos1 = UtxoSet::new();
    utxos1.insert(
        spend_1,
        UtxoEntry::new(TxOut::new(10_000_000, vec![0x10]), 1, false),
    );
    let block1 = make_block(
        Hash256::ZERO,
        1,
        &utxos1,
        vec![coinbase1.clone(), spend_tx1.clone()],
    );
    let mut staged1 = utxos1.clone();
    staged1.apply_block(&block1.transactions, 1).unwrap();
    assert_eq!(block1.header.utxo_root, staged1.compute_utxo_root());

    // Scenario 2: intra-block dependent payments in the same block.
    let coinbase2 = Transaction::new_coinbase(2, vec![TxOut::new(10_000_000, vec![0x30])]);
    let tx_a = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(spend_2, vec![0x41])],
        vec![TxOut::new(7_000_000, vec![0x50])],
        0,
    );
    let tx_b = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(tx_a.txid(), 0), vec![0x51])],
        vec![TxOut::new(6_900_000, vec![0x60])],
        0,
    );

    let mut utxos2 = UtxoSet::new();
    utxos2.insert(
        spend_2,
        UtxoEntry::new(TxOut::new(10_000_000, vec![0x30]), 2, false),
    );
    let block2 = make_block(
        Hash256::ZERO,
        2,
        &utxos2,
        vec![coinbase2.clone(), tx_a.clone(), tx_b.clone()],
    );
    let mut staged2 = utxos2.clone();
    staged2.apply_block(&block2.transactions, 2).unwrap();
    assert_eq!(block2.header.utxo_root, staged2.compute_utxo_root());

    // Scenario 3: OP_RETURN + multi-input/output transaction.
    let coinbase3 = Transaction::new_coinbase(3, vec![TxOut::new(10_000_000, vec![0x70])]);
    let tx_c = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![
            TxIn::new(OutPoint::new(prev_txid, 2), vec![0x81]),
            TxIn::new(OutPoint::new(prev_txid, 3), vec![0x82]),
        ],
        vec![
            TxOut::new(3_000_000, vec![0x90]),
            TxOut::new(0, vec![0x6a, 0x04, b'X', b'Y']),
            TxOut::new(2_000_000, vec![0x91]),
        ],
        0,
    );

    let mut utxos3 = UtxoSet::new();
    utxos3.insert(
        OutPoint::new(prev_txid, 2),
        UtxoEntry::new(TxOut::new(2_000_000, vec![0x81]), 3, false),
    );
    utxos3.insert(
        OutPoint::new(prev_txid, 3),
        UtxoEntry::new(TxOut::new(3_000_000, vec![0x82]), 3, false),
    );
    let block3 = make_block(
        Hash256::ZERO,
        3,
        &utxos3,
        vec![coinbase3.clone(), tx_c.clone()],
    );
    let mut staged3 = utxos3.clone();
    staged3.apply_block(&block3.transactions, 3).unwrap();
    assert_eq!(block3.header.utxo_root, staged3.compute_utxo_root());
}
