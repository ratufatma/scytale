use axum::{body::to_bytes, body::Body, http::Request, http::StatusCode};
use scytale_consensus::calculate_block_reward;
use scytale_core::{OutPoint, Transaction, TxIn, TxOut, TRANSACTION_VERSION_1};
use scytale_mining::worker::run_pow_search;
use scytale_node::{http_gateway::router, Node, NodeConfig};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn test_miner_fee_accrual_and_http_mempool_telemetry() {
    let miner_lock = vec![0x01, 0x02, 0x03];
    let recipient_lock = vec![0x09, 0x08, 0x07];

    let config = NodeConfig {
        data_dir: ":memory:".into(),
        mining_enabled: false,
        miner_payout_script: miner_lock.clone(),
        genesis_difficulty_target: 0x207fffff,
        ..NodeConfig::default()
    };

    let mut node = Node::open(config).unwrap();
    node.start().unwrap();
    let node = Arc::new(node);
    let app = router(Arc::clone(&node));

    // 1. Initial mempool query via HTTP
    let req = Request::builder()
        .uri("/api/v1/mempool")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let initial_mp: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(initial_mp["count"], 0);
    assert_eq!(initial_mp["total_bytes"], 0);
    assert_eq!(initial_mp["total_fees_quanta"], 0);

    // 2. Mine block 1 (genesis subsidy) paying to miner
    let template1 = node.build_mining_template(miner_lock.clone()).unwrap();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let solved1 = run_pow_search(&template1, 0, 100_000, &cancel).unwrap();
    let b1 = template1.assemble_block(solved1);
    assert!(node.submit_external_block(b1.clone()).unwrap());
    assert_eq!(node.canonical_height(), 1);

    // 3. Assemble and submit a transaction with 50,000 quanta fee
    let fee_quanta = 50_000u64;
    let send_quanta = 200_000_000u64; // 2 SCY
    let subsidy1 = calculate_block_reward(1);
    let change_quanta = subsidy1 - send_quanta - fee_quanta;

    let input_op = OutPoint::new(b1.transactions[0].txid(), 0);
    let tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(input_op, miner_lock.clone())],
        vec![
            TxOut::new(send_quanta, recipient_lock.clone()),
            TxOut::new(change_quanta, miner_lock.clone()),
        ],
        0,
    );
    let txid = tx.txid();

    let submitted_id = node.submit_transaction(tx.clone()).unwrap();
    assert_eq!(submitted_id, txid);
    assert_eq!(node.mempool_len(), 1);
    assert_eq!(node.mempool_total_fees(), fee_quanta);
    assert!(node.mempool_total_bytes() > 0);

    // 4. Verify HTTP mempool telemetry reflects fee density and metadata
    let req = Request::builder()
        .uri("/api/v1/mempool")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let mp: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(mp["count"], 1);
    assert_eq!(mp["total_fees_quanta"], fee_quanta);
    assert_eq!(mp["total_bytes"], node.mempool_total_bytes());
    assert_eq!(mp["transactions"][0]["txid"], format!("0x{txid}"));
    assert_eq!(mp["transactions"][0]["fee_quanta"], fee_quanta);
    assert!(mp["transactions"][0]["fee_rate_milli"].as_u64().unwrap() >= 1_000);

    // 5. Build block template using node's mining template builder
    let template = node.build_mining_template(miner_lock.clone()).unwrap();

    // Verify candidate block transactions: coinbase at 0, transfer at 1
    assert_eq!(template.transactions.len(), 2);
    assert_eq!(template.transactions[1].txid(), txid);

    // Verify miner fee accrual in Coinbase value
    let subsidy2 = calculate_block_reward(2);
    let expected_coinbase_value = subsidy2 + fee_quanta;
    assert_eq!(
        template.transactions[0].outputs[0].value,
        expected_coinbase_value
    );

    // 6. Mine block 2 and submit to node
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let solved_header = run_pow_search(&template, 0, 100_000, &cancel).unwrap();
    let b2 = template.assemble_block(solved_header);
    assert!(node.submit_external_block(b2).unwrap());
    assert_eq!(node.canonical_height(), 2);

    // 7. Verify mempool is emptied upon block confirmation
    assert_eq!(node.mempool_len(), 0);
    assert_eq!(node.mempool_total_bytes(), 0);
    assert_eq!(node.mempool_total_fees(), 0);

    // 8. Verify UTXO set contains recipient output and miner change
    let final_utxos = node.query_utxo_set();
    assert_eq!(
        final_utxos
            .get(&OutPoint::new(txid, 0))
            .unwrap()
            .output
            .value,
        send_quanta
    );
    assert_eq!(
        final_utxos
            .get(&OutPoint::new(txid, 1))
            .unwrap()
            .output
            .value,
        change_quanta
    );

    node.shutdown().unwrap();
}
