use scytale_core::{
    AuthorizationVerifier, Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut,
    UtxoEntry, UtxoSet, TRANSACTION_VERSION_1,
};
use scytale_mempool::{Mempool, MempoolError};

struct MockVerifier;

impl AuthorizationVerifier for MockVerifier {
    fn verify(
        &self,
        _preimage_digest: &Hash256,
        _locking_condition: &[u8],
        authorization_proof: &[u8],
    ) -> Result<(), scytale_core::AuthorizationError> {
        if authorization_proof.is_empty() {
            return Err(scytale_core::AuthorizationError::EmptyAuthorization);
        }
        Ok(())
    }
}

fn create_test_setup() -> (UtxoSet, MockVerifier) {
    let mut utxos = UtxoSet::new();
    let txid = Hash256::hash(b"genesis_tx");
    let op0 = OutPoint::new(txid, 0);
    let op1 = OutPoint::new(txid, 1);
    let op2 = OutPoint::new(txid, 2);

    utxos.insert(
        op0,
        UtxoEntry::new(TxOut::new(10_000_000, vec![]), 0, false),
    );
    utxos.insert(
        op1,
        UtxoEntry::new(TxOut::new(20_000_000, vec![]), 0, false),
    );
    utxos.insert(
        op2,
        UtxoEntry::new(TxOut::new(30_000_000, vec![]), 0, false),
    );

    (utxos, MockVerifier)
}

#[test]
fn test_admit_valid_transaction() {
    let (utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let input = TxIn::new(OutPoint::new(txid_genesis, 0), vec![1]);
    let output = TxOut::new(9_000_000, vec![]); // 10M -> 9M (fee 1M)
    let tx = Transaction::new(TRANSACTION_VERSION_1, vec![input], vec![output], 0);

    let admitted_txid = mempool
        .admit_transaction(tx.clone(), &utxos, &verifier, 1700000000)
        .unwrap();

    assert_eq!(admitted_txid, tx.txid());
    assert_eq!(mempool.len(), 1);
    assert!(mempool.contains(&admitted_txid));

    let entry = mempool.get(&admitted_txid).unwrap();
    assert_eq!(entry.fee, 1_000_000);
    assert_eq!(entry.size_bytes, 73);
    assert_eq!(entry.fee_rate, 1_000_000 / 73);
}

#[test]
fn test_reject_duplicate_txid() {
    let (utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let input = TxIn::new(OutPoint::new(txid_genesis, 0), vec![1]);
    let output = TxOut::new(9_000_000, vec![]);
    let tx = Transaction::new(TRANSACTION_VERSION_1, vec![input], vec![output], 0);

    mempool
        .admit_transaction(tx.clone(), &utxos, &verifier, 1700000000)
        .unwrap();

    let err = mempool
        .admit_transaction(tx.clone(), &utxos, &verifier, 1700000001)
        .unwrap_err();

    assert_eq!(err, MempoolError::DuplicateTx(tx.txid()));
}

#[test]
fn test_reject_pending_double_spend() {
    let (utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let shared_outpoint = OutPoint::new(txid_genesis, 0);

    // Tx 1: spends shared_outpoint -> recipient A
    let tx1 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(shared_outpoint, vec![1])],
        vec![TxOut::new(9_000_000, vec![1])],
        0,
    );

    // Tx 2: attempts to spend shared_outpoint -> recipient B
    let tx2 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(shared_outpoint, vec![2])],
        vec![TxOut::new(8_000_000, vec![2])],
        0,
    );

    mempool
        .admit_transaction(tx1.clone(), &utxos, &verifier, 1700000000)
        .unwrap();

    let err = mempool
        .admit_transaction(tx2, &utxos, &verifier, 1700000001)
        .unwrap_err();

    assert_eq!(
        err,
        MempoolError::ConflictDoubleSpend {
            outpoint: shared_outpoint,
            conflicting_tx: tx1.txid()
        }
    );
}

#[test]
fn test_parent_child_admission() {
    let (utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let op0 = OutPoint::new(txid_genesis, 0);

    // Parent Tx A: spends op0 -> creates output 0
    let tx_a = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op0, vec![1])],
        vec![TxOut::new(9_000_000, vec![10])],
        0,
    );
    let tx_a_id = tx_a.txid();
    mempool
        .admit_transaction(tx_a, &utxos, &verifier, 1700000000)
        .unwrap();

    // Child Tx B: spends unconfirmed output of Tx A
    let tx_b = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(tx_a_id, 0), vec![2])],
        vec![TxOut::new(8_000_000, vec![20])],
        0,
    );
    let tx_b_id = tx_b.txid();

    let admitted_b = mempool
        .admit_transaction(tx_b, &utxos, &verifier, 1700000001)
        .unwrap();

    assert_eq!(admitted_b, tx_b_id);
    assert_eq!(mempool.len(), 2);
}

#[test]
fn test_parent_removal_evicts_child() {
    let (utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let op0 = OutPoint::new(txid_genesis, 0);

    // Parent Tx A
    let tx_a = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op0, vec![1])],
        vec![TxOut::new(9_000_000, vec![10])],
        0,
    );
    let tx_a_id = tx_a.txid();
    mempool
        .admit_transaction(tx_a, &utxos, &verifier, 1700000000)
        .unwrap();

    // Child Tx B
    let tx_b = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(tx_a_id, 0), vec![2])],
        vec![TxOut::new(8_000_000, vec![20])],
        0,
    );
    let tx_b_id = tx_b.txid();
    mempool
        .admit_transaction(tx_b, &utxos, &verifier, 1700000001)
        .unwrap();

    assert_eq!(mempool.len(), 2);

    // Removing parent Tx A must cascade remove Child Tx B
    let removed = mempool.remove_transaction_and_descendants(&tx_a_id);
    assert!(removed.contains(&tx_a_id));
    assert!(removed.contains(&tx_b_id));
    assert!(mempool.is_empty());
}

#[test]
fn test_block_inclusion_removes_transactions() {
    let (mut utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let op0 = OutPoint::new(txid_genesis, 0);

    let tx_a = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op0, vec![1])],
        vec![TxOut::new(9_000_000, vec![10])],
        0,
    );
    let tx_a_id = tx_a.txid();
    mempool
        .admit_transaction(tx_a.clone(), &utxos, &verifier, 1700000000)
        .unwrap();

    assert_eq!(mempool.len(), 1);

    // Block confirming Tx A
    let coinbase = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![])]);
    let block = Block::new(
        BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, 1700000060, 0x1f00ffff, 0),
        vec![coinbase, tx_a.clone()],
    );

    // Update canonical UTXO set
    utxos
        .apply_block_transactions(&block.transactions[0], &block.transactions[1..], 1)
        .unwrap();

    mempool.on_block_connected(&block, &utxos);

    // Tx A should be removed from mempool
    assert!(!mempool.contains(&tx_a_id));
    assert!(mempool.is_empty());
}

#[test]
fn test_block_inclusion_evicts_conflicting_pending_tx() {
    let (mut utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let op0 = OutPoint::new(txid_genesis, 0);

    // Pending Tx in mempool spending op0
    let pending_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op0, vec![1])],
        vec![TxOut::new(9_000_000, vec![10])],
        0,
    );
    let pending_tx_id = pending_tx.txid();
    mempool
        .admit_transaction(pending_tx, &utxos, &verifier, 1700000000)
        .unwrap();

    // A block arrives with a conflicting tx that also spent op0
    let conflicting_block_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op0, vec![2])],
        vec![TxOut::new(8_500_000, vec![99])],
        0,
    );

    let coinbase = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![])]);
    let block = Block::new(
        BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, 1700000060, 0x1f00ffff, 0),
        vec![coinbase, conflicting_block_tx],
    );

    // Apply to canonical UTXO set
    utxos
        .apply_block_transactions(&block.transactions[0], &block.transactions[1..], 1)
        .unwrap();

    mempool.on_block_connected(&block, &utxos);

    // Pending tx should be evicted because op0 is no longer in canonical UTXO set
    assert!(!mempool.contains(&pending_tx_id));
    assert!(mempool.is_empty());
}

#[test]
fn test_reorg_readmission() {
    let (utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");
    let op0 = OutPoint::new(txid_genesis, 0);

    let disconnected_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op0, vec![1])],
        vec![TxOut::new(9_000_000, vec![10])],
        0,
    );
    let disconnected_txid = disconnected_tx.txid();

    // Simulate reorg: on_reorg is given disconnected transactions
    mempool.on_reorg(vec![disconnected_tx], &utxos, &verifier, 1700000000);

    assert!(mempool.contains(&disconnected_txid));
    assert_eq!(mempool.len(), 1);
}

#[test]
fn test_fee_metadata_calculation() {
    let (utxos, verifier) = create_test_setup();
    let mut mempool = Mempool::new();

    let txid_genesis = Hash256::hash(b"genesis_tx");

    // Tx 1: spends op0 (10M) -> outputs 8M, fee 2M
    let tx1 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(txid_genesis, 0), vec![1])],
        vec![TxOut::new(8_000_000, vec![1])],
        0,
    );

    // Tx 2: spends op1 (20M) -> outputs 15M, fee 5M
    let tx2 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(txid_genesis, 1), vec![2])],
        vec![TxOut::new(15_000_000, vec![2])],
        0,
    );

    let id1 = mempool
        .admit_transaction(tx1, &utxos, &verifier, 1700000000)
        .unwrap();
    let id2 = mempool
        .admit_transaction(tx2, &utxos, &verifier, 1700000000)
        .unwrap();

    let sorted = mempool.get_entries_sorted_by_fee_rate();
    assert_eq!(sorted.len(), 2);
    // Tx 2 has higher fee (5M) and similar size -> higher fee rate
    assert_eq!(sorted[0].txid, id2);
    assert_eq!(sorted[1].txid, id1);
    assert!(sorted[0].fee_rate >= sorted[1].fee_rate);
}
