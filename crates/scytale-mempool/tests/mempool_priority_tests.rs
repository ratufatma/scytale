use scytale_core::{
    AuthorizationVerifier, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoEntry, UtxoSet,
    TRANSACTION_VERSION_1,
};
use scytale_mempool::{Mempool, MempoolEntry, MempoolError, PriorityKey};

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

fn create_setup_with_outputs(values: &[u64]) -> (UtxoSet, MockVerifier) {
    let mut utxos = UtxoSet::new();
    let txid = Hash256::hash(b"genesis_funding");
    for (i, &val) in values.iter().enumerate() {
        utxos.insert(
            OutPoint::new(txid, i as u32),
            UtxoEntry::new(TxOut::new(val, vec![]), 0, false),
        );
    }
    (utxos, MockVerifier)
}

#[test]
fn test_fee_rate_ordering() {
    let (utxos, verifier) = create_setup_with_outputs(&[10_000_000, 10_000_000, 10_000_000]);
    let mut mempool = Mempool::new();
    let genesis_txid = Hash256::hash(b"genesis_funding");

    // Tx 1: Fee 10,000 quanta, size ~73 bytes => fee_rate ≈ 136,986 milli-quanta/byte
    let tx1 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 0), vec![1])],
        vec![TxOut::new(9_990_000, vec![])],
        0,
    );

    // Tx 2: Fee 50,000 quanta, size ~73 bytes => fee_rate ≈ 684,931 milli-quanta/byte
    let tx2 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 1), vec![2])],
        vec![TxOut::new(9_950_000, vec![])],
        0,
    );

    // Tx 3: Fee 30,000 quanta, size ~73 bytes => fee_rate ≈ 410,958 milli-quanta/byte
    let tx3 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 2), vec![3])],
        vec![TxOut::new(9_970_000, vec![])],
        0,
    );

    let id1 = mempool
        .admit_transaction(tx1.clone(), &utxos, &verifier, 100)
        .unwrap();
    let id2 = mempool
        .admit_transaction(tx2.clone(), &utxos, &verifier, 100)
        .unwrap();
    let id3 = mempool
        .admit_transaction(tx3.clone(), &utxos, &verifier, 100)
        .unwrap();

    let sorted = mempool.get_entries_sorted_by_fee_rate();
    assert_eq!(sorted.len(), 3);
    // Highest fee rate (tx2) -> mid (tx3) -> lowest (tx1)
    assert_eq!(sorted[0].txid, id2);
    assert_eq!(sorted[1].txid, id3);
    assert_eq!(sorted[2].txid, id1);

    // select_transactions_for_block should select tx2 first
    let (selected, total_fees) = mempool.select_transactions_for_block(sorted[0].size_bytes);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].txid(), id2);
    assert_eq!(total_fees, 50_000);
}

#[test]
fn test_min_relay_fee_rejection() {
    let (utxos, verifier) = create_setup_with_outputs(&[10_000_000]);
    let mut mempool = Mempool::new();
    let genesis_txid = Hash256::hash(b"genesis_funding");

    // Size ~73 bytes.
    // To have fee_rate < 1,000 milli-quanta/byte:
    // (fee * 1000) / 73 < 1000 => fee * 1000 < 73000 => fee < 73 quanta.
    // Let's set fee = 50 quanta.
    let tx_low_fee = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 0), vec![1])],
        vec![TxOut::new(9_999_950, vec![])], // 10M - 9,999,950 = 50 quanta fee
        0,
    );

    let err = mempool
        .admit_transaction(tx_low_fee, &utxos, &verifier, 100)
        .unwrap_err();

    match err {
        MempoolError::FeeTooLow {
            fee_rate,
            min_relay_fee,
        } => {
            assert!(fee_rate < min_relay_fee);
            assert_eq!(min_relay_fee, 1_000);
        }
        other => panic!("Expected FeeTooLow error, got {other:?}"),
    }
}

#[test]
fn test_mempool_eviction_when_full() {
    let (utxos, verifier) =
        create_setup_with_outputs(&[10_000_000, 10_000_000, 10_000_000, 10_000_000]);
    // Create mempool with capacity of exactly 2 transactions
    let mut mempool = Mempool::with_config(2, 10_000, 1_000);
    let genesis_txid = Hash256::hash(b"genesis_funding");

    // Tx 1: Fee 10,000 quanta (fee_rate ≈ 136,986)
    let tx1 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 0), vec![1])],
        vec![TxOut::new(9_990_000, vec![])],
        0,
    );

    // Tx 2: Fee 20,000 quanta (fee_rate ≈ 273,972)
    let tx2 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 1), vec![2])],
        vec![TxOut::new(9_980_000, vec![])],
        0,
    );

    let id1 = mempool
        .admit_transaction(tx1, &utxos, &verifier, 100)
        .unwrap();
    let id2 = mempool
        .admit_transaction(tx2, &utxos, &verifier, 100)
        .unwrap();
    assert_eq!(mempool.len(), 2);

    // Tx 3 (LOWER than lowest tx1: Fee 5,000 quanta): should be rejected with MempoolFull
    let tx_low = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 2), vec![3])],
        vec![TxOut::new(9_995_000, vec![])],
        0,
    );
    let err = mempool
        .admit_transaction(tx_low, &utxos, &verifier, 100)
        .unwrap_err();
    assert!(matches!(err, MempoolError::MempoolFull { .. }));
    assert_eq!(mempool.len(), 2);
    assert!(mempool.contains(&id1));
    assert!(mempool.contains(&id2));

    // Tx 4 (HIGHER than lowest tx1: Fee 50,000 quanta): should evict tx1 and be admitted
    let tx_high = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(genesis_txid, 3), vec![4])],
        vec![TxOut::new(9_950_000, vec![])],
        0,
    );
    let id4 = mempool
        .admit_transaction(tx_high, &utxos, &verifier, 100)
        .unwrap();

    assert_eq!(mempool.len(), 2);
    // tx1 (lowest) was evicted!
    assert!(!mempool.contains(&id1));
    // tx2 and tx4 are present!
    assert!(mempool.contains(&id2));
    assert!(mempool.contains(&id4));
}

#[test]
fn test_zero_float_accuracy() {
    let tx = Transaction::new(TRANSACTION_VERSION_1, vec![], vec![], 0);
    let size = tx.serialized_size().max(1);

    // Test large fee near u64 boundaries without overflow
    let large_fee: u64 = 1_000_000_000_000; // 1 trillion quanta
    let entry = MempoolEntry::new(tx.clone(), large_fee, 1000);
    assert_eq!(
        entry.fee_rate,
        large_fee.saturating_mul(1000) / (size as u64)
    );

    // Verify deterministic priority comparison
    let key1 = PriorityKey::new(5000, 100, Hash256::from_slice(&[1u8; 32]).unwrap());
    let key2 = PriorityKey::new(5000, 200, Hash256::from_slice(&[2u8; 32]).unwrap());
    // key1 arrived at 100, key2 arrived at 200.
    // key1 should be GREATER than key2 because earlier arrival has higher priority
    assert!(key1 > key2);
}
