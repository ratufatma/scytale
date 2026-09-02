//! Scytale Core: Canonical transaction, block, and UTXO primitives for Scytale.

pub mod block;
pub mod error;
pub mod transaction;
pub mod utxo;

pub use block::{Block, BlockHeader};
pub use error::{CoreError, TransactionError, UtxoError};
pub use scytale_primitives::{
    Hash, Hash256, OutPoint, PrimitiveError, Quanta, TxOut, QUANTA_PER_SCY,
};
pub use transaction::{calculate_fee, Transaction, TxIn, TRANSACTION_VERSION_1};
pub use utxo::{UtxoEntry, UtxoSet};
