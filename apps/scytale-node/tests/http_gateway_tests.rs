use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use scytale_node::{
    http_gateway::{router, StatusResponse},
    Node, NodeConfig,
};
use std::sync::Arc;
use tempfile::tempdir;
use tower::util::ServiceExt;

fn setup_test_node() -> Arc<Node> {
    let temp = tempdir().unwrap();
    let config = NodeConfig {
        data_dir: temp.path().to_path_buf(),
        mining_enabled: false,
        miner_payout_script: vec![0x01, 0x02, 0x03],
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).unwrap();
    node.start().unwrap();
    Arc::new(node)
}

#[tokio::test]
async fn test_http_status_endpoint() {
    let node = setup_test_node();
    let app = router(Arc::clone(&node));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let status: StatusResponse = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(status.runtime_state, "Running");
    assert_eq!(status.canonical_height, 0);
    assert!(status.canonical_tip.starts_with("0x"));
    assert_eq!(status.canonical_tip, format!("0x{}", node.canonical_tip()));
    assert!(status.utxo_root.starts_with("0x"));
    assert_eq!(status.peer_count, 0);
    assert_eq!(status.mempool_tx_count, 0);
    assert!(!status.mining_active);
}

#[tokio::test]
async fn test_http_blocks_endpoints() {
    let node = setup_test_node();
    let app = router(Arc::clone(&node));

    // 1. Get tip block
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/blocks/tip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let block: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(block["height"], 0);
    assert_eq!(block["tx_count"], 1);
    let tip_hash = block["hash"].as_str().unwrap().to_string();

    // 2. Get block by height 0
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/blocks/0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let block_by_height: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(block_by_height["hash"], tip_hash);

    // 3. Get block by hash
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/blocks/{}", tip_hash))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 4. Query list of blocks
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/blocks?limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["height"], 0);

    // 5. Query list of blocks with order=asc and from_height=0
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/blocks?from_height=0&limit=10&order=asc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let asc_list: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(asc_list.len(), 1);
    assert_eq!(asc_list[0]["height"], 0);

    // 6. Non-existent block returns 404
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/blocks/999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_tx_endpoint() {
    let node = setup_test_node();
    let app = router(Arc::clone(&node));

    let chain = node.query_canonical_chain().unwrap();
    let genesis_txid = chain[0].0.transactions[0].txid();

    // 1. Query genesis transaction
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tx/{}", genesis_txid))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let tx: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(tx["txid"], format!("0x{}", genesis_txid));
    assert_eq!(tx["is_coinbase"], true);
    assert_eq!(tx["status"], "Confirmed");
    assert_eq!(tx["block_height"], 0);
    assert_eq!(tx["total_output_quanta"], scytale_core::genesis::TOTAL_GENESIS_QUANTA);
    assert_eq!(tx["total_output_scy"], "13020000.00000000");

    // 2. Non-existent transaction returns 404
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/tx/0000000000000000000000000000000000000000000000000000000000000001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_passbook_endpoint() {
    let node = setup_test_node();
    let app = router(Arc::clone(&node));

    // Genesis rewards are paid to Founder lock
    let founder_lock = scytale_core::genesis::GENESIS_FOUNDER_LOCK_HEX;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/passbook/{}", founder_lock))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let pb: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(pb["account_lock_hex"], founder_lock);
    assert_eq!(pb["confirmed_balance_quanta"], scytale_core::genesis::GENESIS_FOUNDER_QUANTA);
    assert_eq!(pb["total_entries"], 1);

    // Empty passbook for unused lock
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/passbook/deadbeef")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let pb: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(pb["confirmed_balance_quanta"], 0);
    assert_eq!(pb["total_entries"], 0);

    // Bech32 address passbook query
    let dummy_hash = [0x55u8; 32];
    let bech32_addr = scytale_core::Address::new(dummy_hash).to_bech32().unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/passbook/{}", bech32_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let pb: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(pb["confirmed_balance_quanta"], 0);
    assert_eq!(pb["total_entries"], 0);
}

#[tokio::test]
async fn test_http_mempool_and_provenance() {
    let node = setup_test_node();
    let app = router(Arc::clone(&node));

    // 1. Mempool is initially empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/mempool")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let mp: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(mp["count"], 0);
    assert_eq!(mp["max_count"], 5000);
    assert_eq!(mp["max_bytes"], 5_000_000);
    assert_eq!(mp["min_relay_fee_milli"], 1000);

    // 2. Provenance of genesis coinbase
    let chain = node.query_canonical_chain().unwrap();
    let genesis_txid = chain[0].0.transactions[0].txid();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/provenance/{}/0", genesis_txid))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let prov: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(prov["steps"].as_array().unwrap().len(), 1);
    assert_eq!(prov["steps"][0]["category"], "Genesis");
    assert_eq!(
        prov["steps"][0]["value_quanta"],
        scytale_core::genesis::GENESIS_FOUNDER_QUANTA
    );
}

#[tokio::test]
async fn test_http_gateway_server_live_lifecycle() {
    let node = setup_test_node();
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    // Bind to OS-assigned port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let bind_addr = format!("127.0.0.1:{port}");
    let bind_copy = bind_addr.clone();
    let node_copy = Arc::clone(&node);

    let server_handle = tokio::spawn(async move {
        let _ = scytale_node::run_http_gateway(&bind_copy, node_copy, shutdown_rx).await;
    });

    // Wait briefly for server to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Send HTTP GET request via TcpStream
    let mut stream = tokio::net::TcpStream::connect(&bind_addr).await.unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let response_str = String::from_utf8_lossy(&buf);
    assert!(response_str.contains("200 OK"));
    assert!(response_str.contains("{\"status\":\"ok\"}"));

    // Trigger graceful shutdown
    let _ = shutdown_tx.send(());
    let res = tokio::time::timeout(tokio::time::Duration::from_secs(2), server_handle).await;
    assert!(res.is_ok(), "Server should shutdown cleanly within timeout");
}

#[tokio::test]
async fn test_embedded_explorer_endpoint() {
    let node = setup_test_node();
    let app = router(Arc::clone(&node));

    // Test GET /
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/html"),
        "Expected text/html content-type, got: {content_type}"
    );

    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body_bytes);
    assert!(
        html.contains("Scytale Block Explorer"),
        "HTML must contain 'Scytale Block Explorer'"
    );

    // Test GET /index.html
    let response_index = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_index.status(), StatusCode::OK);

    // Test GET /favicon.svg
    let response_favicon = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_favicon.status(), StatusCode::OK);
    assert_eq!(
        response_favicon.headers().get("content-type").unwrap(),
        "image/svg+xml; charset=utf-8"
    );

    // Test GET /gemini-svg.svg
    let response_gemini = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/gemini-svg.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_gemini.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_tx_output_script_analysis() {
    let node = setup_test_node();
    let app = router(Arc::clone(&node));

    // Genesis tx output 0 is Founder P2PKH script
    let chain = node.query_canonical_chain().unwrap();
    let genesis_txid = chain[0].0.transactions[0].txid();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tx/{}", genesis_txid))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let tx: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(tx["outputs"][0]["script_type"], "p2pkh");
    assert_eq!(
        tx["outputs"][0]["address"],
        scytale_core::genesis::GENESIS_FOUNDER_ADDRESS
    );
    assert_eq!(
        tx["outputs"][0]["op_return_payload"],
        serde_json::Value::Null
    );

}
