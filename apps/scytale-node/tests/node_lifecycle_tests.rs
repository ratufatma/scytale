//! Node lifecycle and subsystem-orchestration integration tests.
//!
//! Covers: state-machine transitions, fresh zero-balance bootstrap, graceful
//! shutdown ordering, restart state continuity, and incoming-block mining
//! template cancellation.

use scytale_node::{Node, NodeConfig, NodeState};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A compact difficulty target that maps to `Target::max()` (easiest), so a PoW
/// nonce search succeeds on the very first nonce regardless of hash value.
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

fn build_extending_block(
    prev: scytale_core::Hash256,
    height: u64,
    payout: &[u8],
) -> scytale_core::Block {
    use scytale_consensus::calculate_block_reward;
    use scytale_core::{Block, BlockHeader, Hash256, Transaction, TxOut};
    let coinbase = Transaction::new_coinbase(
        height,
        vec![TxOut::new(calculate_block_reward(height), payout.to_vec())],
    );
    let commitment = Hash256::hash(coinbase.txid().as_bytes());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let header = BlockHeader::new(1, prev, commitment, now, EASY_TARGET, 0);
    Block::new(header, vec![coinbase])
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Node state transitions
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_node_state_transitions() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), false)).unwrap();
    assert_eq!(node.state(), NodeState::Starting);

    node.start().unwrap();
    let mid = node.state();
    assert!(
        matches!(
            mid,
            NodeState::Ready | NodeState::Running | NodeState::Recovering
        ),
        "after a mining-disabled start node should be Running/Ready, got {mid:?}"
    );
    assert_eq!(
        node.state(),
        NodeState::Running,
        "non-mining node reaches Running"
    );

    node.shutdown().unwrap();
    assert_eq!(node.state(), NodeState::Stopped);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Fresh node zero-balance bootstrap
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fresh_node_zero_balance_bootstrap() {
    use scytale_consensus::calculate_block_reward;
    let dir = tempfile::TempDir::new().unwrap();

    // Fresh node, mining disabled: reaches Running. The only value present is the
    // defined genesis emission; no user or wallet account is credited — a freshly
    // started node holds a spendable balance of 0 SCY associated with any user key.
    let mut node = Node::open(test_config(dir.path().join("db1"), false)).unwrap();
    node.start().unwrap();
    assert_eq!(node.state(), NodeState::Running);
    let genesis_emission: u64 = calculate_block_reward(0);
    assert_eq!(
        node.total_utxo_quanta(),
        genesis_emission,
        "only the protocol-defined genesis emission exists; zero user balance"
    );
    assert_eq!(node.canonical_height(), 0);
    assert_eq!(node.mempool_len(), 0);
    node.shutdown().unwrap();

    // Fresh node with mining enabled: without any user-supplied balance, the node
    // mines its first block and earns a spendable coinbase — permissionless bootstrap.
    let mut miner = Node::open(test_config(dir.path().join("db2"), true)).unwrap();
    miner.start().unwrap();
    assert!(miner.mining_running(), "mining worker should be spawned");
    assert!(
        wait_for_height(&miner, 1, 5000),
        "a fresh node with no balance must mine a first block"
    );
    let balance_after_first_block = miner.total_utxo_quanta();
    assert!(
        balance_after_first_block > genesis_emission,
        "mining must yield a new spendable coinbase on top of genesis emission"
    );
    miner.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Graceful shutdown ordering
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_graceful_shutdown_order() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut node = Node::open(test_config(dir.path().join("db"), true)).unwrap();
    node.start().unwrap();
    assert!(node.mining_running());
    assert!(
        wait_for_height(&node, 1, 5000),
        "miner active and producing"
    );

    let cancel = node.mining_cancel_flag();
    assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));

    node.shutdown().unwrap();

    // The cancellation flag is set before the worker is joined and storage closed.
    assert!(
        cancel.load(std::sync::atomic::Ordering::Relaxed),
        "shutdown must set the mining cancellation flag"
    );
    assert!(
        !node.mining_running(),
        "mining worker must be joined and released before shutdown completes"
    );
    assert_eq!(node.state(), NodeState::Stopped);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Restart state continuity
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_restart_state_continuity() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("db");

    let (saved_tip, saved_height, saved_balance);
    {
        let mut node = Node::open(test_config(path.clone(), true)).unwrap();
        node.start().unwrap();
        assert!(wait_for_height(&node, 2, 10_000), "mine two blocks");

        saved_tip = node.canonical_tip();
        saved_height = node.canonical_height();
        saved_balance = node.total_utxo_quanta();
        assert!(saved_height >= 2);

        // Graceful shutdown closes the database handle before dropping.
        node.shutdown().unwrap();
    } // node (and thus storage) dropped here

    // Reopen from the same data_dir: state must resume at the exact point.
    let mut node2 = Node::open(test_config(path, false)).unwrap();
    node2.start().unwrap();

    assert_eq!(
        node2.canonical_tip(),
        saved_tip,
        "canonical tip must resume exactly"
    );
    assert_eq!(
        node2.canonical_height(),
        saved_height,
        "canonical height must resume exactly"
    );
    assert_eq!(
        node2.total_utxo_quanta(),
        saved_balance,
        "UTXO balance must resume exactly"
    );

    // The persisted tip block is recoverable from the same on-disk store.
    let store = node2.storage_handle();
    assert!(
        store.get_block(&saved_tip).unwrap().is_some(),
        "tip block present in reopened storage"
    );
    node2.shutdown().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Incoming block cancels mining template
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_incoming_block_cancels_mining_template() {
    let dir = tempfile::TempDir::new().unwrap();
    let payout = vec![0xAA, 0xBB, 0xCC];
    let mut node = Node::open(test_config(dir.path().join("db"), true)).unwrap();
    node.start().unwrap();
    assert!(wait_for_height(&node, 1, 5000), "miner active");

    // Inject an external, valid, canonical-extending block. Because the autonomous
    // miner races concurrently, retry building on the live tip until our block wins
    // the canonical slot; on winning, the miner's in-flight template is invalidated.
    let mut injected_hash = None;
    let mut injected_height = None;
    for _ in 0..5000 {
        let tip = node.canonical_tip();
        let height = node.canonical_height();
        let ext = build_extending_block(tip, height + 1, &payout);
        if node.submit_external_block(ext.clone()).unwrap() {
            injected_hash = Some(ext.header.hash());
            injected_height = Some(height + 1);
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    let injected_hash = injected_hash.expect("external block should become canonical");
    let injected_height = injected_height.expect("external block height known");
    assert!(
        node.canonical_height() >= injected_height,
        "injected block must be (or exceed) the recorded canonical height"
    );

    // The miner must rebuild its template on the NEW tip and continue mining past it.
    assert!(
        wait_for_height(&node, injected_height + 1, 5000),
        "miner must refresh template and mine a child of the injected block"
    );

    // Walk the current canonical tip backwards and confirm it descends from the
    // injected block, proving the miner refreshed its template onto the new tip
    // (robust even if the fast miner races ahead by several blocks).
    let store = node.storage_handle();
    let mut cur = node.canonical_tip();
    let mut descends_from_injected = false;
    for _ in 0..(node.canonical_height() + 1) {
        if cur == injected_hash {
            descends_from_injected = true;
            break;
        }
        match store.get_block(&cur).unwrap() {
            Some(b) => cur = b.header.previous_block_hash,
            None => break,
        }
    }
    assert!(
        descends_from_injected,
        "current tip must descend from the injected canonical block"
    );

    node.shutdown().unwrap();
}
