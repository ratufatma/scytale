use std::sync::Arc;
use tempfile::tempdir;

use ed25519_dalek::Signer;
use scytale_bridge::{NodeRequest, NodeResponse};
use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, TRANSACTION_VERSION_1,
};
use scytale_node::{IpcServer, Node, NodeConfig};

#[allow(dead_code)]
#[path = "../src/client.rs"]
mod client;
#[allow(dead_code)]
#[path = "../src/formatter.rs"]
mod formatter;
#[allow(dead_code)]
#[path = "../src/identity.rs"]
mod identity;
#[allow(dead_code)]
#[path = "../src/wallet.rs"]
mod wallet;

use wallet::WalletFile;

#[tokio::test]
async fn test_wallet_p2pkh_end_to_end() {
    let temp = tempdir().unwrap();
    let sock_path = temp.path().join("scytale_test.sock");
    let wallet_a_path = temp.path().join("wallet_a.json");
    let wallet_b_path = temp.path().join("wallet_b.json");

    // 1. Generate Wallet A and Wallet B
    let wallet_a = WalletFile::generate_new(&wallet_a_path, false).unwrap();
    let wallet_b = WalletFile::generate_new(&wallet_b_path, false).unwrap();
    let lock_a = wallet_a.p2pkh_locking_script().unwrap();
    let lock_b = wallet_b.p2pkh_locking_script().unwrap();

    // 2. Start Test Node with Miner Payout directed to Wallet A P2PKH
    let config = NodeConfig {
        data_dir: ":memory:".into(),
        mining_enabled: false,
        miner_payout_script: lock_a.clone(),
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).unwrap();
    node.start().unwrap();
    let node = Arc::new(node);

    let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server = IpcServer::new(&sock_path, Arc::clone(&node), shutdown_tx);
    tokio::spawn(async move {
        let _ = server.run().await;
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 3. Mine a block with coinbase paying to Wallet A
    let genesis_tip = node.canonical_tip();
    let subsidy = scytale_consensus::calculate_block_reward(1);
    let cb1 = Transaction::new_coinbase(1, vec![TxOut::new(subsidy, lock_a.clone())]);
    let mut staging = node.query_utxo_set();
    staging.insert(
        OutPoint::new(cb1.txid(), 0),
        scytale_core::UtxoEntry::new(cb1.outputs[0].clone(), 1, true),
    );
    let utxo_root1 = staging.compute_utxo_root();
    let h1 = BlockHeader::new(
        1,
        genesis_tip,
        Hash256::ZERO,
        utxo_root1,
        100,
        0x207fffff,
        0,
    );
    let b1 = Block::new(h1, vec![cb1]);
    assert!(node.submit_external_block(b1).unwrap());
    assert_eq!(node.canonical_height(), 1);

    // 4. Query UTXOs for Wallet A via IPC
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::GetUtxosByLock {
            locking_script: lock_a.clone(),
        },
    )
    .await
    .unwrap();

    let utxos_a = match resp {
        NodeResponse::Utxos(u) => u,
        other => panic!("Expected Utxos response, got {other:?}"),
    };
    assert_eq!(utxos_a.len(), 1);
    assert_eq!(utxos_a[0].value_quanta, subsidy);

    // 5. Build, sign, and submit P2PKH transfer from Wallet A to Wallet B
    let send_amount = 200_000_000u64; // 2 SCY
    let fee = 1_000u64;
    let input_txid =
        Hash256::from_slice(&scytale_primitives::from_hex(&utxos_a[0].txid_hex).unwrap()).unwrap();
    let input_op = OutPoint::new(input_txid, utxos_a[0].index);

    let change_amount = subsidy - send_amount - fee;
    let mut transfer_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(input_op, vec![])],
        vec![
            TxOut::new(send_amount, lock_b.clone()),
            TxOut::new(change_amount, lock_a.clone()),
        ],
        0,
    );

    // Sign input 0
    let sighash = transfer_tx.compute_sighash(0, &lock_a);
    let sig_a = wallet_a.signing_key().unwrap().sign(&sighash);
    let pubkey_a = wallet_a.verifying_key_bytes().unwrap();
    transfer_tx.inputs[0].authorization =
        wallet::build_p2pkh_unlocking_script(&sig_a.to_bytes(), &pubkey_a);

    // Submit raw transaction via IPC
    let txid = transfer_tx.txid();
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::SubmitRawTransaction {
            tx: Box::new(transfer_tx.clone()),
        },
    )
    .await
    .unwrap();

    match resp {
        NodeResponse::TransactionSubmitted {
            txid: submitted_txid,
        } => {
            assert_eq!(submitted_txid, txid.to_string());
        }
        other => panic!("Expected TransactionSubmitted, got {other:?}"),
    }
    assert_eq!(node.mempool_len(), 1);

    // 6. Mine block 2 containing the transfer transaction
    let b1_tip = node.canonical_tip();
    let subsidy2 = scytale_consensus::calculate_block_reward(2);
    let cb2 = Transaction::new_coinbase(2, vec![TxOut::new(subsidy2, lock_a.clone())]);
    let mut staging2 = node.query_utxo_set();
    staging2.remove(&transfer_tx.inputs[0].previous_output);
    for (i, out) in transfer_tx.outputs.iter().enumerate() {
        staging2.insert(
            OutPoint::new(transfer_tx.txid(), i as u32),
            scytale_core::UtxoEntry::new(out.clone(), 2, false),
        );
    }
    staging2.insert(
        OutPoint::new(cb2.txid(), 0),
        scytale_core::UtxoEntry::new(cb2.outputs[0].clone(), 2, true),
    );
    let utxo_root2 = staging2.compute_utxo_root();
    let h2 = BlockHeader::new(2, b1_tip, Hash256::ZERO, utxo_root2, 200, 0x207fffff, 0);
    let b2 = Block::new(h2, vec![cb2, transfer_tx]);
    assert!(node.submit_external_block(b2).unwrap());
    assert_eq!(node.canonical_height(), 2);
    assert_eq!(node.mempool_len(), 0);

    // 7. Verify Wallet B has the confirmed UTXO
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::GetUtxosByLock {
            locking_script: lock_b.clone(),
        },
    )
    .await
    .unwrap();

    let utxos_b = match resp {
        NodeResponse::Utxos(u) => u,
        other => panic!("Expected Utxos response, got {other:?}"),
    };
    assert_eq!(utxos_b.len(), 1);
    assert_eq!(utxos_b[0].value_quanta, send_amount);
    assert_eq!(utxos_b[0].txid_hex, txid.to_string());

    // 8. Verify Wallet B can spend its newly received UTXO back to Wallet A
    let spend_b_amount = 150_000_000u64;
    let spend_b_fee = 1_000u64;
    let b_input_op = OutPoint::new(txid, 0);
    let b_change = send_amount - spend_b_amount - spend_b_fee;

    let mut b_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(b_input_op, vec![])],
        vec![
            TxOut::new(spend_b_amount, lock_a.clone()),
            TxOut::new(b_change, lock_b.clone()),
        ],
        0,
    );

    let sighash_b = b_tx.compute_sighash(0, &lock_b);
    let sig_b = wallet_b.signing_key().unwrap().sign(&sighash_b);
    let pubkey_b = wallet_b.verifying_key_bytes().unwrap();
    b_tx.inputs[0].authorization =
        wallet::build_p2pkh_unlocking_script(&sig_b.to_bytes(), &pubkey_b);

    let b_txid = b_tx.txid();
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::SubmitRawTransaction { tx: Box::new(b_tx) },
    )
    .await
    .unwrap();

    match resp {
        NodeResponse::TransactionSubmitted {
            txid: submitted_txid,
        } => {
            assert_eq!(submitted_txid, b_txid.to_string());
        }
        other => panic!("Expected TransactionSubmitted for wallet B spend, got {other:?}"),
    }
    assert_eq!(node.mempool_len(), 1);

    node.shutdown().unwrap();
}

#[tokio::test]
async fn test_wallet_op_return_embed_data() {
    let temp = tempdir().unwrap();
    let sock_path = temp.path().join("scytale_op_return.sock");
    let wallet_path = temp.path().join("wallet.json");

    let wallet = WalletFile::generate_new(&wallet_path, false).unwrap();
    let lock = wallet.p2pkh_locking_script().unwrap();

    let config = NodeConfig {
        data_dir: ":memory:".into(),
        mining_enabled: false,
        miner_payout_script: lock.clone(),
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).unwrap();
    node.start().unwrap();
    let node = Arc::new(node);

    let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server = IpcServer::new(&sock_path, Arc::clone(&node), shutdown_tx);
    tokio::spawn(async move {
        let _ = server.run().await;
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Mine block 1 to give wallet funds
    let genesis_tip = node.canonical_tip();
    let subsidy = scytale_consensus::calculate_block_reward(1);
    let cb = Transaction::new_coinbase(1, vec![TxOut::new(subsidy, lock.clone())]);
    let mut staging = node.query_utxo_set();
    staging.insert(
        OutPoint::new(cb.txid(), 0),
        scytale_core::UtxoEntry::new(cb.outputs[0].clone(), 1, true),
    );
    let utxo_root1 = staging.compute_utxo_root();
    let h1 = BlockHeader::new(
        1,
        genesis_tip,
        Hash256::ZERO,
        utxo_root1,
        100,
        0x207fffff,
        0,
    );
    let b1 = Block::new(h1, vec![cb]);
    assert!(node.submit_external_block(b1).unwrap());

    // Fetch UTXO
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::GetUtxosByLock {
            locking_script: lock.clone(),
        },
    )
    .await
    .unwrap();
    let utxos = match resp {
        NodeResponse::Utxos(u) => u,
        other => panic!("Expected Utxos response, got {other:?}"),
    };

    // Construct OP_RETURN transaction
    let payload = b"SCYTALE_PROTOCOL_PROOF_V1";
    let op_return_lock = wallet::build_op_return_script(payload);
    let fee = 10_000u64;
    let input_txid =
        Hash256::from_slice(&scytale_primitives::from_hex(&utxos[0].txid_hex).unwrap()).unwrap();
    let input_op = OutPoint::new(input_txid, utxos[0].index);
    let change = subsidy - fee;

    let mut embed_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(input_op, vec![])],
        vec![
            TxOut::new(0, op_return_lock.clone()),
            TxOut::new(change, lock.clone()),
        ],
        0,
    );

    let sighash = embed_tx.compute_sighash(0, &lock);
    let sig = wallet.signing_key().unwrap().sign(&sighash);
    let pubkey = wallet.verifying_key_bytes().unwrap();
    embed_tx.inputs[0].authorization =
        wallet::build_p2pkh_unlocking_script(&sig.to_bytes(), &pubkey);

    let embed_txid = embed_tx.txid();
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::SubmitRawTransaction {
            tx: Box::new(embed_tx.clone()),
        },
    )
    .await
    .unwrap();

    assert!(matches!(resp, NodeResponse::TransactionSubmitted { .. }));

    // Mine block 2 containing the OP_RETURN transaction
    let tip = node.canonical_tip();
    let subsidy2 = scytale_consensus::calculate_block_reward(2);
    let cb2 = Transaction::new_coinbase(2, vec![TxOut::new(subsidy2, lock.clone())]);
    let mut staging2 = node.query_utxo_set();
    staging2.remove(&embed_tx.inputs[0].previous_output);
    // output 0 is OP_RETURN, omitted from UTXO set
    staging2.insert(
        OutPoint::new(embed_tx.txid(), 1),
        scytale_core::UtxoEntry::new(embed_tx.outputs[1].clone(), 2, false),
    );
    staging2.insert(
        OutPoint::new(cb2.txid(), 0),
        scytale_core::UtxoEntry::new(cb2.outputs[0].clone(), 2, true),
    );
    let utxo_root2 = staging2.compute_utxo_root();
    let h2 = BlockHeader::new(2, tip, Hash256::ZERO, utxo_root2, 200, 0x207fffff, 0);
    let b2 = Block::new(h2, vec![cb2, embed_tx]);
    assert!(node.submit_external_block(b2).unwrap());

    // Verify OP_RETURN output is NOT in UTXOs table
    let utxo_set = node.query_utxo_set();
    assert!(utxo_set.get(&OutPoint::new(embed_txid, 0)).is_none());
    // But change output is in UTXOs table
    assert!(utxo_set.get(&OutPoint::new(embed_txid, 1)).is_some());

    node.shutdown().unwrap();
}
