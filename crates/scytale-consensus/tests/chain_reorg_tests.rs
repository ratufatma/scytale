use scytale_consensus::{block_work, ChainTree, CumulativeWork, Target};
use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoSet, TRANSACTION_VERSION_1,
};

#[allow(clippy::too_many_arguments)]
fn make_test_block(
    version: u32,
    prev_hash: Hash256,
    timestamp: u64,
    target: u32,
    nonce: u64,
    txs: Vec<Transaction>,
    parent_utxos: &UtxoSet,
    height: u64,
) -> Block {
    let mut staging = parent_utxos.clone();
    let _ = staging.apply_block_transactions(&txs[0], &txs[1..], height);
    let utxo_root = staging.compute_utxo_root();
    let header = BlockHeader::new(
        version,
        prev_hash,
        Hash256::ZERO,
        utxo_root,
        timestamp,
        target,
        nonce,
    );
    Block::new(header, txs)
}

fn create_genesis() -> (Block, UtxoSet) {
    let coinbase = Transaction::new_coinbase(
        0,
        vec![TxOut::new(10_000_000_000, vec![1, 2, 3])], // 100 SCY
    );
    let mut utxo_set = UtxoSet::new();
    utxo_set
        .apply_block_transactions(&coinbase, &[], 0)
        .unwrap();
    let utxo_root = utxo_set.compute_utxo_root();
    let header = BlockHeader::new(
        1,
        Hash256::ZERO,
        Hash256::ZERO,
        utxo_root,
        1700000000,
        0x1f00ffff, // Easy target
        0,
    );
    let block = Block::new(header, vec![coinbase]);
    (block, utxo_set)
}

#[test]
fn test_cumulative_work_accumulation() {
    let (genesis, _) = create_genesis();
    let tree = ChainTree::new(genesis);

    let t1 = Target::from_compact(0x1f00ffff);
    let w1 = block_work(&t1);
    assert_eq!(tree.canonical_work(), w1);
    assert_ne!(w1, CumulativeWork::zero());

    let w_sum = w1.checked_add(&w1).unwrap();
    assert!(w_sum > w1);
}

#[test]
fn test_linear_extension() {
    let (genesis, mut utxo_set) = create_genesis();
    let genesis_hash = genesis.header.hash();
    let mut tree = ChainTree::new(genesis);

    // Block 1
    let coinbase1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![])]);
    let block1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1f00ffff,
        1,
        vec![coinbase1],
        &utxo_set,
        1,
    );
    let block1_hash = block1.header.hash();

    let result = tree.process_block(block1, &mut utxo_set).unwrap();
    assert!(result.is_some());
    let reorg = result.unwrap();

    assert_eq!(reorg.old_tip, genesis_hash);
    assert_eq!(reorg.new_tip, block1_hash);
    assert!(reorg.disconnected_blocks.is_empty());
    assert_eq!(reorg.connected_blocks.len(), 1);
    assert_eq!(tree.canonical_tip(), block1_hash);
    assert_eq!(tree.canonical_height(), 1);
}

#[test]
fn test_fork_choice_heavier_branch_wins() {
    let (genesis, mut utxo_set) = create_genesis();
    let genesis_hash = genesis.header.hash();
    let mut tree = ChainTree::new(genesis);

    let utxo_genesis = utxo_set.clone();

    // Branch A: 2 blocks with easy target (0x1f00ffff)
    let cb_a1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![1])]);
    let block_a1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1f00ffff,
        1,
        vec![cb_a1],
        &utxo_genesis,
        1,
    );
    let hash_a1 = block_a1.header.hash();
    tree.process_block(block_a1, &mut utxo_set).unwrap();

    let utxo_a1 = utxo_set.clone();
    let cb_a2 = Transaction::new_coinbase(2, vec![TxOut::new(1_000_000_000, vec![2])]);
    let block_a2 = make_test_block(
        1,
        hash_a1,
        1700000120,
        0x1f00ffff,
        2,
        vec![cb_a2],
        &utxo_a1,
        2,
    );
    let hash_a2 = block_a2.header.hash();
    tree.process_block(block_a2, &mut utxo_set).unwrap();

    assert_eq!(tree.canonical_tip(), hash_a2);
    assert_eq!(tree.canonical_height(), 2);

    // Branch B: 1 block directly off Genesis with very difficult target (0x1000ffff -> much higher work)
    let cb_b1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![3])]);
    let block_b1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1000ffff,
        100,
        vec![cb_b1],
        &utxo_genesis,
        1,
    );
    let hash_b1 = block_b1.header.hash();

    // Work of B1 exceeds 2 blocks of A
    let work_b = block_work(&Target::from_compact(0x1000ffff));
    let work_a_total = tree.canonical_work();
    assert!(work_b > work_a_total);

    let result = tree.process_block(block_b1, &mut utxo_set).unwrap();
    assert!(result.is_some());
    let reorg = result.unwrap();

    // Branch B becomes canonical despite height being 1 vs Branch A height 2
    assert_eq!(reorg.old_tip, hash_a2);
    assert_eq!(reorg.new_tip, hash_b1);
    assert_eq!(reorg.disconnected_blocks.len(), 2); // Block A2, Block A1
    assert_eq!(reorg.connected_blocks.len(), 1); // Block B1
    assert_eq!(tree.canonical_tip(), hash_b1);
    assert_eq!(tree.canonical_height(), 1);
}

#[test]
fn test_reject_invalid_branch_even_with_high_work() {
    let (genesis, mut utxo_set) = create_genesis();
    let genesis_hash = genesis.header.hash();
    let mut tree = ChainTree::new(genesis);

    let utxo_genesis = utxo_set.clone();

    // Branch A: valid linear block
    let cb_a = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![1])]);
    let block_a = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1f00ffff,
        1,
        vec![cb_a],
        &utxo_genesis,
        1,
    );
    let hash_a = block_a.header.hash();
    tree.process_block(block_a, &mut utxo_set).unwrap();

    let initial_utxo_snapshot = utxo_set.clone();

    // Branch B: huge work (0x1000ffff) but contains double-spending invalid transaction
    let cb_b = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![2])]);

    // Phantom input that doesn't exist
    let phantom_op = OutPoint::new(Hash256::hash(b"non_existent"), 0);
    let invalid_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(phantom_op, vec![])],
        vec![TxOut::new(500_000_000, vec![])],
        0,
    );

    let block_b = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1000ffff,
        99,
        vec![cb_b, invalid_tx],
        &utxo_genesis,
        1,
    );

    // Processing invalid branch must fail
    let res = tree.process_block(block_b, &mut utxo_set);
    assert!(res.is_err());

    // Canonical tip and UTXO set MUST remain 100% untouched
    assert_eq!(tree.canonical_tip(), hash_a);
    assert_eq!(utxo_set, initial_utxo_snapshot);
}

#[test]
fn test_common_ancestor_discovery() {
    let (genesis, mut utxo_set) = create_genesis();
    let genesis_hash = genesis.header.hash();
    let mut tree = ChainTree::new(genesis);

    let utxo_genesis = utxo_set.clone();

    // Branch A: 2 blocks
    let b_a1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1f00ffff,
        1,
        vec![Transaction::new_coinbase(1, vec![TxOut::new(100, vec![])])],
        &utxo_genesis,
        1,
    );
    let hash_a1 = b_a1.header.hash();
    tree.process_block(b_a1, &mut utxo_set).unwrap();

    let utxo_a1 = utxo_set.clone();
    let b_a2 = make_test_block(
        1,
        hash_a1,
        1700000120,
        0x1f00ffff,
        2,
        vec![Transaction::new_coinbase(2, vec![TxOut::new(100, vec![])])],
        &utxo_a1,
        2,
    );
    let hash_a2 = b_a2.header.hash();
    tree.process_block(b_a2, &mut utxo_set).unwrap();

    // Branch B: off A1
    let b_b2 = make_test_block(
        1,
        hash_a1,
        1700000120,
        0x1f00ffff,
        3,
        vec![Transaction::new_coinbase(2, vec![TxOut::new(100, vec![])])],
        &utxo_a1,
        2,
    );
    let hash_b2 = b_b2.header.hash();
    tree.process_block(b_b2, &mut utxo_set).unwrap();

    let ancestor = tree.find_common_ancestor(&hash_a2, &hash_b2).unwrap();
    assert_eq!(ancestor, hash_a1);
}

#[test]
fn test_atomic_reorg_utxo_state() {
    let (genesis, mut utxo_set) = create_genesis();
    let genesis_hash = genesis.header.hash();
    let genesis_cb_op = OutPoint::new(genesis.transactions[0].txid(), 0);
    let mut tree = ChainTree::new(genesis);

    let utxo_genesis = utxo_set.clone();

    // Branch A: spends genesis coinbase
    let cb_a1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![1])]);
    let spend_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(genesis_cb_op, vec![])],
        vec![TxOut::new(9_000_000_000, vec![99])], // 100 SCY -> 90 SCY (fee 10 SCY)
        0,
    );
    let spend_tx_outpoint = OutPoint::new(spend_tx.txid(), 0);
    let b_a1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1f00ffff,
        1,
        vec![cb_a1, spend_tx],
        &utxo_genesis,
        1,
    );
    tree.process_block(b_a1, &mut utxo_set).unwrap();

    // In Branch A, genesis coinbase is spent, and spend_tx_outpoint is present
    assert!(!utxo_set.contains(&genesis_cb_op));
    assert!(utxo_set.contains(&spend_tx_outpoint));

    // Branch B: does not spend genesis coinbase, but has higher cumulative work
    let cb_b1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![2])]);
    let cb_b1_op = OutPoint::new(cb_b1.txid(), 0);
    let b_b1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1000ffff,
        20,
        vec![cb_b1],
        &utxo_genesis,
        1,
    );
    tree.process_block(b_b1, &mut utxo_set).unwrap();

    // After reorg to Branch B:
    // 1. Genesis coinbase is restored (unspent)
    assert!(utxo_set.contains(&genesis_cb_op));
    // 2. spend_tx_outpoint from Branch A is removed
    assert!(!utxo_set.contains(&spend_tx_outpoint));
    // 3. Branch B coinbase is present
    assert!(utxo_set.contains(&cb_b1_op));
}

#[test]
fn test_mempool_reconciliation_list() {
    let (genesis, mut utxo_set) = create_genesis();
    let genesis_hash = genesis.header.hash();
    let genesis_cb_op = OutPoint::new(genesis.transactions[0].txid(), 0);
    let mut tree = ChainTree::new(genesis);

    let utxo_genesis = utxo_set.clone();

    // Branch A: includes a standard non-coinbase tx
    let cb_a1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![1])]);
    let regular_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(genesis_cb_op, vec![])],
        vec![TxOut::new(9_000_000_000, vec![])],
        0,
    );
    let b_a1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1f00ffff,
        1,
        vec![cb_a1, regular_tx.clone()],
        &utxo_genesis,
        1,
    );
    tree.process_block(b_a1, &mut utxo_set).unwrap();

    // Branch B: higher work (0x1000ffff), only coinbase
    let cb_b1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![2])]);
    let b_b1 = make_test_block(
        1,
        genesis_hash,
        1700000060,
        0x1000ffff,
        20,
        vec![cb_b1],
        &utxo_genesis,
        1,
    );

    let res = tree.process_block(b_b1, &mut utxo_set).unwrap().unwrap();

    // Assert regular_tx is returned for mempool reconciliation
    assert_eq!(res.transactions_for_mempool.len(), 1);
    assert_eq!(res.transactions_for_mempool[0], regular_tx);

    // Assert coinbase tx from Branch A is NOT in transactions_for_mempool
    assert!(!res
        .transactions_for_mempool
        .iter()
        .any(|tx| tx.is_coinbase()));
}
