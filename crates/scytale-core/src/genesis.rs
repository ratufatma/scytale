//! Canonical Genesis Block and Initial Tokenomics Allocation for Scytale.
//!
//! Enforces:
//! - Total Maximum Supply: 42,000,000 SCY (4,200,000,000,000,000 quanta)
//! - Founder Allocation: 21% (8,820,000 SCY / 882,000,000,000,000 quanta)
//! - Developer Fund: 5% (2,100,000 SCY / 210,000,000,000,000 quanta)
//! - Community Reserve: 5% (2,100,000 SCY / 210,000,000,000,000 quanta)
//! - Total Genesis Supply: 31% (13,020,000 SCY / 1,302,000,000,000,000 quanta)
//! - Proof-of-Work Mining Reserve: 69% (28,980,000 SCY / 2,898,000,000,000,000 quanta)

use crate::block::{Block, BlockHeader};
use crate::transaction::Transaction;
use crate::utxo::{compute_utxo_leaf, compute_utxo_merkle_root};
use crate::Quanta;
use scytale_primitives::{from_hex, Hash256, OutPoint, TxOut};

/// Maximum overall supply ceiling in SCY.
pub const MAX_SUPPLY_SCY: u64 = 42_000_000;

/// Maximum overall supply ceiling in integer quanta (4,200,000,000,000,000 quanta).
pub const MAX_SUPPLY_QUANTA: Quanta = 4_200_000_000_000_000;

/// Founder Allocation quota (21% / 8,820,000 SCY).
pub const GENESIS_FOUNDER_QUANTA: Quanta = 882_000_000_000_000;

/// Developer Fund Allocation quota (5% / 2,100,000 SCY).
pub const GENESIS_DEVELOPER_QUANTA: Quanta = 210_000_000_000_000;

/// Community Reserve Allocation quota (5% / 2,100,000 SCY).
pub const GENESIS_COMMUNITY_QUANTA: Quanta = 210_000_000_000_000;

/// Total Genesis Allocation quota (31% / 13,020,000 SCY).
pub const TOTAL_GENESIS_QUANTA: Quanta = 1_302_000_000_000_000;

/// Remaining Public Mining Emission Reserve quota (69% / 28,980,000 SCY).
pub const MINING_RESERVE_QUANTA: Quanta = 2_898_000_000_000_000;

/// Official Founder Bech32 address.
pub const GENESIS_FOUNDER_ADDRESS: &str =
    "scy1nw7vhxmxyz2jlw89vz88tdv938692xk968uxn89787fa4w207s8sddvv3q";

/// Official Developer Bech32 address.
pub const GENESIS_DEVELOPER_ADDRESS: &str =
    "scy1q5nhm4ge3m2myr65x5s8jdfesy5xtm0k0ddkm2qua36rw62z06zswrq8e0";

/// Official Community Bech32 address.
pub const GENESIS_COMMUNITY_ADDRESS: &str =
    "scy1nrlpqplz9f8dvauz2zmmgqcjxr7xvpfc95lewxft5anvgev57kmsxce3kd";

/// Official Founder P2PKH locking script hex bytecode.
pub const GENESIS_FOUNDER_LOCK_HEX: &str =
    "73a0209bbccb9b6620952fb8e5608e75b58589f4551ac5d1f8699cbe3f93dab94ff40f88ac";

/// Official Developer P2PKH locking script hex bytecode.
pub const GENESIS_DEVELOPER_LOCK_HEX: &str =
    "73a02005277dd5198ed5b20f543520793539812865edf67b5b6da81cec743769427e8588ac";

/// Official Community P2PKH locking script hex bytecode.
pub const GENESIS_COMMUNITY_LOCK_HEX: &str =
    "73a02098fe1007e22a4ed6778250b7b4031230fc6605382d3f97192ba766c46594f5b788ac";

/// Decodes the official Founder P2PKH locking script.
pub fn founder_locking_script() -> Vec<u8> {
    from_hex(GENESIS_FOUNDER_LOCK_HEX).expect("valid founder lock hex")
}

/// Decodes the official Developer P2PKH locking script.
pub fn developer_locking_script() -> Vec<u8> {
    from_hex(GENESIS_DEVELOPER_LOCK_HEX).expect("valid developer lock hex")
}

/// Decodes the official Community P2PKH locking script.
pub fn community_locking_script() -> Vec<u8> {
    from_hex(GENESIS_COMMUNITY_LOCK_HEX).expect("valid community lock hex")
}

/// Constructs the canonical Genesis Bootstrap Transaction (Height 0) with exactly 3 outputs:
/// - Output 0: Founder Allocation (21% / 8,820,000 SCY)
/// - Output 1: Developer Fund (5% / 2,100,000 SCY)
/// - Output 2: Community Reserve (5% / 2,100,000 SCY)
pub fn build_genesis_coinbase() -> Transaction {
    let outputs = vec![
        TxOut::new(GENESIS_FOUNDER_QUANTA, founder_locking_script()),
        TxOut::new(GENESIS_DEVELOPER_QUANTA, developer_locking_script()),
        TxOut::new(GENESIS_COMMUNITY_QUANTA, community_locking_script()),
    ];
    Transaction::new_coinbase(0, outputs)
}

/// Computes the canonical balanced binary Merkle root across all Genesis UTXO outputs.
pub fn compute_genesis_utxo_root(coinbase: &Transaction) -> Hash256 {
    let txid = coinbase.txid();
    let leaves: Vec<Hash256> = coinbase
        .outputs
        .iter()
        .enumerate()
        .map(|(index, output)| {
            let outpoint = OutPoint::new(txid, index as u32);
            compute_utxo_leaf(&outpoint, output)
        })
        .collect();
    compute_utxo_merkle_root(leaves)
}

/// Assembles the canonical Genesis Block 0 with:
/// - Version: 1
/// - Previous Block Hash: Hash256::ZERO
/// - Transaction Commitment: BLAKE3 hash of the Genesis Bootstrap Transaction
/// - UTXO Root: Balanced binary Merkle root across the 3 genesis outputs
/// - Timestamp: 0
/// - Nonce: 0
pub fn build_genesis_block(difficulty_target: u32) -> Block {
    let coinbase = build_genesis_coinbase();
    let commitment = Hash256::hash(coinbase.txid().as_bytes());
    let utxo_root = compute_genesis_utxo_root(&coinbase);
    let header = BlockHeader::new(
        1,
        Hash256::ZERO,
        commitment,
        utxo_root,
        0,
        difficulty_target,
        0,
    );
    Block::new(header, vec![coinbase])
}

#[cfg(test)]
mod tests {
    use super::*;
    use scytale_primitives::QUANTA_PER_SCY;

    #[test]
    fn test_tokenomics_exact_integer_reconciliation() {
        assert_eq!(
            GENESIS_FOUNDER_QUANTA + GENESIS_DEVELOPER_QUANTA + GENESIS_COMMUNITY_QUANTA,
            TOTAL_GENESIS_QUANTA
        );
        assert_eq!(
            TOTAL_GENESIS_QUANTA + MINING_RESERVE_QUANTA,
            MAX_SUPPLY_QUANTA
        );
        assert_eq!(
            MAX_SUPPLY_QUANTA,
            MAX_SUPPLY_SCY * QUANTA_PER_SCY
        );
    }

    #[test]
    fn test_genesis_block_structure() {
        let block = build_genesis_block(0x1d00_ffff);
        assert_eq!(block.transactions.len(), 1);
        let cb = &block.transactions[0];
        assert!(cb.is_coinbase());
        assert_eq!(cb.outputs.len(), 3);
        assert_eq!(cb.outputs[0].value, GENESIS_FOUNDER_QUANTA);
        assert_eq!(cb.outputs[1].value, GENESIS_DEVELOPER_QUANTA);
        assert_eq!(cb.outputs[2].value, GENESIS_COMMUNITY_QUANTA);

        let total: Quanta = cb.outputs.iter().map(|o| o.value).sum();
        assert_eq!(total, TOTAL_GENESIS_QUANTA);

        let expected_root = compute_genesis_utxo_root(cb);
        assert_eq!(block.header.utxo_root, expected_root);
        assert_eq!(block.header.previous_block_hash, Hash256::ZERO);
    }
}
