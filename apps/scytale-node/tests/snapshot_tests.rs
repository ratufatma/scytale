//! Fast sync snapshot export and apply integration tests.

use scytale_node::{Node, NodeConfig};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const EASY_TARGET: u32 = 0x217F_FFFF;

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

#[test]
fn test_export_and_apply_snapshot_roundtrip() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();

    let mut node_a = Node::open(test_config(dir_a.path().to_path_buf(), true)).unwrap();
    node_a.start().unwrap();

    // Mine at least 3 blocks
    assert!(
        wait_for_height(&node_a, 3, 5000),
        "node_a failed to mine 3 blocks"
    );

    let tip_hash = node_a.canonical_tip().to_string();
    let expected_utxo_count = node_a.query_utxo_set().len();
    assert!(expected_utxo_count >= 3);
    let expected_quanta = node_a.total_utxo_quanta();

    // Export in chunks of size 1
    let mut all_entries = Vec::new();
    let mut chunk_idx = 0;
    loop {
        let (hash_hex, idx, total_chunks, chunk_entries) = node_a
            .export_snapshot_chunk(&tip_hash, chunk_idx, 1)
            .expect("export failed");
        assert_eq!(hash_hex, tip_hash);
        assert_eq!(idx, chunk_idx);
        assert_eq!(total_chunks, expected_utxo_count as u32);
        if chunk_entries.is_empty() {
            break;
        }
        all_entries.extend(chunk_entries);
        chunk_idx += 1;
        if chunk_idx >= total_chunks {
            break;
        }
    }

    assert_eq!(all_entries.len(), expected_utxo_count);

    // Initialize node B (non-mining, clean at genesis)
    let mut node_b = Node::open(test_config(dir_b.path().to_path_buf(), false)).unwrap();
    node_b.start().unwrap();
    assert_eq!(node_b.query_utxo_set().len(), 3); // 3 genesis allocation UTXOs (Founder, Dev, Community)

    // Apply snapshot to Node B
    let applied_count = node_b
        .apply_snapshot(&tip_hash, &all_entries)
        .expect("apply snapshot to node B failed");
    assert_eq!(applied_count, expected_utxo_count);

    // Verify node B now has identical active state
    assert_eq!(node_b.query_utxo_set().len(), expected_utxo_count);
    assert_eq!(node_b.total_utxo_quanta(), expected_quanta);

    node_a.shutdown().unwrap();
    node_b.shutdown().unwrap();
}

#[test]
fn test_apply_snapshot_fail_closed_on_root_mismatch() {
    let dir_a = tempfile::tempdir().unwrap();
    let mut node = Node::open(test_config(dir_a.path().to_path_buf(), true)).unwrap();
    node.start().unwrap();

    assert!(wait_for_height(&node, 1, 5000), "failed to mine 1 block");
    let tip_hash = node.canonical_tip().to_string();

    let (_, _, _, mut entries) = node
        .export_snapshot_chunk(&tip_hash, 0, 100)
        .expect("export failed");
    assert!(!entries.is_empty());

    // Corrupt one entry's value
    entries[0].value_quanta += 1000;

    // Applying corrupted entries against the known block tip MUST fail closed
    let err = node.apply_snapshot(&tip_hash, &entries);
    assert!(
        err.is_err(),
        "corrupted snapshot root should have been rejected"
    );

    node.shutdown().unwrap();
}
