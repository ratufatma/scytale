use scytale_consensus::{
    calculate_median_time_past, transaction_commitment, validate_block_authoritative_with_headers,
    ConsensusError,
};
use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxIn, TxOut, UtxoEntry, UtxoSet,
    TRANSACTION_VERSION_1,
};

fn headers() -> Vec<BlockHeader> {
    (0..11)
        .map(|timestamp| {
            BlockHeader::new(
                1,
                Hash256::ZERO,
                Hash256::ZERO,
                Hash256::ZERO,
                100 + timestamp,
                0x207fffff,
                timestamp,
            )
        })
        .collect()
}

fn block_with_lock_time(lock_time: u64) -> (Block, UtxoSet) {
    let funding = OutPoint::new(Hash256::hash(b"locktime-funding"), 0);
    let mut parent_utxo = UtxoSet::new();
    parent_utxo
        .insert(funding, UtxoEntry::new(TxOut::new(10, vec![1]), 1, false))
        .unwrap();

    let spend = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(funding, vec![]).with_sequence(0)],
        vec![TxOut::new(9, vec![2])],
        lock_time,
    );
    let coinbase = Transaction::new_coinbase(2, vec![TxOut::new(25 * 100_000_000, vec![3])]);
    let transactions = vec![coinbase, spend];
    let commitment = transaction_commitment(&Block::new(
        BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, Hash256::ZERO, 200, 0, 0),
        transactions.clone(),
    ));
    let header = BlockHeader::new(
        1,
        Hash256::ZERO,
        commitment,
        Hash256::ZERO,
        200,
        0x207fffff,
        0,
    );
    (Block::new(header, transactions), parent_utxo)
}

#[test]
fn test_mtp_calculation_and_locktime_validation() {
    let history = headers();
    assert_eq!(calculate_median_time_past(&history, history.len()), 105);

    let (height_locked, parent_utxo) = block_with_lock_time(3);
    assert!(matches!(
        validate_block_authoritative_with_headers(&height_locked, 2, &parent_utxo, &history,),
        Err(ConsensusError::LockTimeNotMet)
    ));

    let (timestamp_locked, parent_utxo) = block_with_lock_time(500_000_001);
    assert!(matches!(
        validate_block_authoritative_with_headers(&timestamp_locked, 2, &parent_utxo, &history,),
        Err(ConsensusError::LockTimeNotMet)
    ));
}
