//! Architectural Boundary Tests: Separation of L3 Indexer Authority from L1 Sovereign Runtime.
//!
//! Verifies:
//! - Test A: Node opens without Indexer
//! - Test B: Node does not create Indexer storage
//! - Test C: commit_block() has no Indexer dependency
//! - Test D: Canonical commit survives observer failure

use scytale_node::{commit_block, BlockCommitted, ExplorerObserver, Node, NodeConfig, NodeState};
use scytale_storage::StorageEngine;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const EASY_TARGET: u32 = 0x217F_FFFF;

fn test_config(data_dir: std::path::PathBuf) -> NodeConfig {
    NodeConfig {
        data_dir,
        mining_enabled: false,
        miner_payout_script: Vec::new(),
        genesis_difficulty_target: EASY_TARGET,
        shutdown_timeout_secs: 10,
        ..NodeConfig::default()
    }
}

/// Test A: Node startup succeeds when the Indexer database does not exist.
/// Node::open() == success; no indexer.sqlite is required.
#[test]
fn test_a_node_opens_without_indexer() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("db");

    // Explicitly ensure no indexer.sqlite exists
    assert!(!db_path.join("indexer.sqlite").exists());

    let node = Node::open(test_config(db_path.clone()));
    assert!(node.is_ok(), "Node::open must succeed without any indexer present");

    let mut node = node.unwrap();
    assert_eq!(node.state(), NodeState::Starting);

    let start_res = node.start();
    assert!(start_res.is_ok(), "Node::start must succeed independently of indexer");
    assert_eq!(node.state(), NodeState::Running);

    node.shutdown().expect("clean shutdown");
}

/// Test B: Opening a fresh Node must not implicitly create an Indexer database.
/// Verify that Node startup does not create: indexer.sqlite or equivalent L3 storage.
#[test]
fn test_b_node_does_not_create_indexer_storage() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("db");

    let mut node = Node::open(test_config(db_path.clone())).expect("open node");
    node.start().expect("start node");

    // Check directory contents: canonical scytale.db and alias sidecar should exist,
    // but indexer.sqlite must NEVER be created.
    let sqlite_path = db_path.join("indexer.sqlite");
    assert!(
        !sqlite_path.exists(),
        "Node must NOT create indexer.sqlite in its data_dir"
    );

    // Also check for any sqlite temporary files (*.sqlite, *.sqlite-wal, *.sqlite-shm)
    for entry in std::fs::read_dir(&db_path).expect("read dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.contains("sqlite") && !name.contains("indexer"),
            "Disallowed L3 storage file detected in L1 node data directory: {name}"
        );
    }

    node.shutdown().expect("clean shutdown");
}

/// Test C: commit_block() has no Indexer dependency.
/// Compile-time & runtime verification: commit_block() accepts only storage, block,
/// height, and cumulative_work. No indexer parameters exist.
#[test]
fn test_c_commit_block_has_no_indexer_dependency() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("scytale.db");
    let storage = StorageEngine::open(&db_path).expect("open storage");

    let genesis = scytale_core::genesis::build_genesis_block(EASY_TARGET);
    let work = [1, 0, 0, 0];

    // Strictly 4 arguments: storage, block, height, cumulative_work
    let commit_res = commit_block(&storage, &genesis, 0, work);
    assert!(commit_res.is_ok(), "commit_block must succeed with canonical arguments only");

    // Confirm block is canonical in storage
    let tip = storage.get_canonical_tip().expect("tip").expect("has tip");
    assert_eq!(tip.0, genesis.header.hash());
    assert_eq!(tip.1, 0);
}

/// Test D: Canonical commit survives observer failure.
/// Simulate an observer/event consumer failure after canonical persistence.
/// Expected:
///   canonical commit = success
///   observer = failure
///   Node canonical state = preserved
/// Observer failure must not roll back or invalidate the canonical block.
#[test]
fn test_d_canonical_commit_survives_observer_failure() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("db");

    let mut cfg = test_config(db_path.clone());
    cfg.mining_enabled = true;
    cfg.miner_payout_script = vec![0x51]; // OP_TRUE

    let mut node = Node::open(cfg).expect("open node");

    // Subscribe an observer that will intentionally fail/disconnect immediately
    let mut observer_rx = node.subscribe_blocks();
    let observer_failed = Arc::new(AtomicBool::new(false));
    let observer_failed_clone = Arc::clone(&observer_failed);

    // Observer task: reads first block, then panics or terminates with failure
    let observer_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            if let Ok(event) = observer_rx.recv().await {
                // Simulate failure on block 0 (or first received block)
                observer_failed_clone.store(true, Ordering::SeqCst);
                drop(observer_rx); // Disconnect consumer abruptly
                // Simulate observer error
                tracing::warn!("Simulated L3 observer failure for block {}", event.height);
            }
        });
    });

    // Start node and allow mining to proceed past height 2
    node.start().expect("start node");

    let deadline = Instant::now() + Duration::from_millis(5000);
    while Instant::now() < deadline {
        if node.canonical_height() >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        node.canonical_height() >= 2,
        "L1 node must continue advancing canonical chain even when observer failed"
    );

    // Verify observer failed as expected
    let _ = observer_handle.join();
    assert!(
        observer_failed.load(Ordering::SeqCst),
        "Observer failure simulation occurred"
    );

    // Confirm L1 canonical state is intact and preserved
    assert!(node.storage_handle().get_canonical_tip().unwrap().is_some());
    let tip = node.canonical_tip();
    let tip_height = node.canonical_height();
    assert!(tip_height >= 2);
    assert_ne!(tip, scytale_core::Hash256::ZERO);

    // Verify explorer observer queue saturation / disconnect doesn't affect commit
    let dead_handle = scytale_node::start_indexer("http://127.0.0.1:9".into(), None);
    let explorer_obs = ExplorerObserver::new(dead_handle);
    let dummy_block = scytale_core::genesis::build_genesis_block(EASY_TARGET);
    let event = BlockCommitted {
        block: Arc::new(dummy_block),
        block_hash: scytale_core::Hash256::ZERO,
        height: 99,
    };
    // Dispatching to broken/closed explorer observer must never panic or return error
    explorer_obs.handle_event(&event);

    node.shutdown().expect("clean shutdown");
}
