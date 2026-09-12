use futures::{SinkExt, StreamExt};
use scytale_core::{BlockHeader, Hash256};
use scytale_stratum::{
    compact_to_target, verify_worker_share, LinesCodec, RawBlockHeader120,
    StratumJob, StratumNotification, StratumRequest,
    StratumResponse, StratumServer, HEADER_SIZE, U256,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

// ── 1. Header Tests ──────────────────────────────────────────────────────────

#[test]
fn test_120_byte_header_binary_layout() {
    let prev_hash = Hash256::hash(b"prev_block");
    let merkle_root = Hash256::hash(b"merkle_root");
    let utxo_root = Hash256::hash(b"utxo_root");
    let raw = RawBlockHeader120::new(
        1,
        prev_hash,
        merkle_root,
        utxo_root,
        1_700_000_000,
        0x1d00ffff,
        0x123456789abcdef0,
    );

    let serialized = raw.serialize();
    assert_eq!(serialized.len(), HEADER_SIZE);

    // Verify fields via LittleEndian offsets
    assert_eq!(&serialized[0..4], &1u32.to_le_bytes());
    assert_eq!(&serialized[4..36], prev_hash.as_bytes());
    assert_eq!(&serialized[36..68], merkle_root.as_bytes());
    assert_eq!(&serialized[68..100], utxo_root.as_bytes());
    assert_eq!(&serialized[100..108], &1_700_000_000u64.to_le_bytes());
    assert_eq!(&serialized[108..112], &0x1d00ffffu32.to_le_bytes());
    assert_eq!(&serialized[112..120], &0x123456789abcdef0u64.to_le_bytes());

    let deserialized = RawBlockHeader120::from_bytes(&serialized).unwrap();
    assert_eq!(deserialized, raw);

    // Verify equivalency with scytale-core BlockHeader canonical hash
    let core_header: BlockHeader = raw.into();
    assert_eq!(raw.compute_pow_hash(), core_header.hash());
}

#[test]
fn test_header_from_bytes_invalid_length() {
    let short_bytes = [0u8; 119];
    assert!(RawBlockHeader120::from_bytes(&short_bytes).is_err());
    let long_bytes = [0u8; 121];
    assert!(RawBlockHeader120::from_bytes(&long_bytes).is_err());
}

// ── 2. Job & Merkle Reconstruction Tests ─────────────────────────────────────

#[test]
fn test_merkle_root_reconstruction_with_extranonces() {
    let prev_hash = Hash256::hash(b"prev");
    let utxo_root = Hash256::hash(b"utxo");

    let cb1 = b"coinbase_part1_".to_vec();
    let cb2 = b"_coinbase_part2".to_vec();
    let branch1 = Hash256::hash(b"tx1");
    let branch2 = Hash256::hash(b"tx2");

    let job = StratumJob::new(
        "job_101",
        prev_hash,
        cb1.clone(),
        cb2.clone(),
        vec![branch1, branch2],
        1,
        0x1d00ffff,
        utxo_root,
        1_700_000_000,
        true,
    );

    let en1 = [0x01, 0x02, 0x03, 0x04];
    let en2 = [0x05, 0x06, 0x07, 0x08];

    let root = job.build_merkle_root(&en1, &en2);

    // Reconstruct manually
    let mut manual_cb = Vec::new();
    manual_cb.extend_from_slice(&cb1);
    manual_cb.extend_from_slice(&en1);
    manual_cb.extend_from_slice(&en2);
    manual_cb.extend_from_slice(&cb2);
    let mut current = Hash256::hash(&manual_cb);

    for branch in &[branch1, branch2] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(current.as_bytes());
        hasher.update(branch.as_bytes());
        current = Hash256::new(*hasher.finalize().as_bytes());
    }

    assert_eq!(root, current);

    let header = job.build_header(root, 1_700_000_000, 42);
    assert_eq!(header.merkle_root, root);
    assert_eq!(header.nonce, 42);
}

// ── 3. Share & Target Verification Tests ─────────────────────────────────────

#[test]
fn test_compact_to_target() {
    let t_floor = compact_to_target(0x1d00ffff);
    assert!(t_floor > U256::zero());

    // Higher difficulty (lower target)
    let t_harder = compact_to_target(0x1c00ffff);
    assert!(t_harder < t_floor);
}

#[test]
fn test_verify_worker_share_thresholds() {
    let prev_hash = Hash256::hash(b"prev");
    let utxo_root = Hash256::hash(b"utxo");
    let job = StratumJob::new(
        "job_test",
        prev_hash,
        b"cb1".to_vec(),
        b"cb2".to_vec(),
        vec![],
        1,
        0x207fffff, // Very easy network target for testing
        utxo_root,
        1_700_000_000,
        false,
    );

    let en1 = [0x00; 4];
    let en2 = [0x00; 4];

    // Difficulty 0.0 means unconstrained share target (all shares accepted for pool testing)
    let res_valid = verify_worker_share(&job, &en1, &en2, 1_700_000_000, 1, 0.0);
    assert!(res_valid.is_ok(), "Difficulty 0.0 must accept any validly formatted share");

    // Impossibly high difficulty must reject random share with HighHash
    let res_invalid = verify_worker_share(&job, &en1, &en2, 1_700_000_000, 1, 1_000_000_000.0);
    assert!(matches!(res_invalid, Err(scytale_stratum::StratumError::HighHash)));
}

// ── 4. Protocol Frame Tests ──────────────────────────────────────────────────

#[test]
fn test_protocol_json_serialization() {
    let req = StratumRequest::new(
        Some(serde_json::json!(1)),
        "mining.subscribe",
        serde_json::json!(["miner/1.0.0"]),
    );
    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: StratumRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(req, deserialized);

    let resp = StratumResponse::success(Some(serde_json::json!(1)), serde_json::json!(true));
    let resp_str = serde_json::to_string(&resp).unwrap();
    assert!(resp_str.contains("\"result\":true"));

    let notify = StratumNotification::notify(
        "job_1",
        "0000",
        "aabb",
        "ccdd",
        &["1122".into()],
        1,
        0x1d00ffff,
        1_700_000_000,
        true,
        "eeee",
    );
    let notify_str = serde_json::to_string(&notify).unwrap();
    assert!(notify_str.contains("\"mining.notify\""));
}

// ── 5. End-to-End Server TCP Integration Test ────────────────────────────────

#[tokio::test]
async fn test_stratum_server_tcp_lifecycle() {
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = Arc::new(StratumServer::new(bind_addr, 0.0));

    // Bind actual listener
    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    let actual_addr = listener.local_addr().unwrap();

    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        loop {
            if let Ok((stream, peer)) = listener.accept().await {
                let srv = Arc::clone(&server_clone);
                tokio::spawn(async move {
                    let _ = srv.handle_connection(stream, peer).await;
                });
            }
        }
    });

    // Setup active job
    let test_job = Arc::new(StratumJob::new(
        "job_e2e_1",
        Hash256::hash(b"genesis"),
        b"coinbase_head_".to_vec(),
        b"_coinbase_tail".to_vec(),
        vec![],
        1,
        0x207fffff, // very easy
        Hash256::hash(b"utxo_root"),
        1_700_000_000,
        true,
    ));
    server.broadcast_job(Arc::clone(&test_job)).await;

    // Connect mock worker client
    let stream = TcpStream::connect(actual_addr).await.unwrap();
    let mut framed = Framed::new(stream, LinesCodec::new());

    async fn read_response(
        framed: &mut Framed<TcpStream, LinesCodec>,
        expected_id: u64,
    ) -> serde_json::Value {
        while let Some(Ok(line)) = framed.next().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("id").and_then(|v| v.as_u64()) == Some(expected_id) {
                    return val;
                }
            }
        }
        panic!("Did not receive response for id {}", expected_id);
    }

    // 1. Send mining.subscribe
    let sub_req = serde_json::json!({
        "id": 1,
        "method": "mining.subscribe",
        "params": ["ScytaleRig/1.0"]
    });
    framed.send(sub_req.to_string()).await.unwrap();
    let sub_resp = read_response(&mut framed, 1).await;
    let result = sub_resp.get("result").unwrap().as_array().unwrap();
    let en1_hex = result[1].as_str().unwrap();
    assert_eq!(en1_hex.len(), 8, "Extranonce1 must be 4 bytes hex (8 chars)");

    // 2. Send mining.authorize
    let auth_req = serde_json::json!({
        "id": 2,
        "method": "mining.authorize",
        "params": ["scy1testnetaddr.worker1", "x"]
    });
    framed.send(auth_req.to_string()).await.unwrap();
    let auth_resp = read_response(&mut framed, 2).await;
    assert_eq!(auth_resp.get("result"), Some(&serde_json::json!(true)));

    // 3. Send mining.submit
    let submit_req = serde_json::json!({
        "id": 3,
        "method": "mining.submit",
        "params": [
            "scy1testnetaddr.worker1",
            "job_e2e_1",
            "00000001", // extranonce2 (4 bytes hex)
            "0x655455a0", // timestamp (1700000000)
            "0x0000000000000001" // nonce
        ]
    });
    framed.send(submit_req.to_string()).await.unwrap();
    let submit_resp = read_response(&mut framed, 3).await;
    assert_eq!(submit_resp.get("result"), Some(&serde_json::json!(true)));

    // 4. Send duplicate share -> should reject with error
    let dup_req = serde_json::json!({
        "id": 4,
        "method": "mining.submit",
        "params": [
            "scy1testnetaddr.worker1",
            "job_e2e_1",
            "00000001",
            "0x655455a0",
            "0x0000000000000001"
        ]
    });
    framed.send(dup_req.to_string()).await.unwrap();
    let dup_resp = read_response(&mut framed, 4).await;
    assert!(dup_resp.get("error").is_some());
    assert_eq!(
        dup_resp.get("error").unwrap().get("code"),
        Some(&serde_json::json!(22)) // DUPLICATE_SHARE
    );
}
