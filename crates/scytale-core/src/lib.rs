//! Scytale Core: Canonical transaction, block, UTXO, authorization, and serialization primitives for Scytale.

pub mod authorization;
pub mod block;
pub mod codec;
pub mod error;
pub mod transaction;
pub mod utxo;

pub use authorization::{verify_transaction_authorization, AuthorizationVerifier};
pub use block::{Block, BlockHeader};
pub use codec::{CanonicalDeserialize, CanonicalSerialize, MAX_VECTOR_LENGTH};
pub use error::{
    AuthorizationError, BlockError, CoreError, SerializationError, TransactionError, UtxoError,
};
pub use scytale_primitives::{
    Hash, Hash256, OutPoint, PrimitiveError, Quanta, TxOut, QUANTA_PER_SCY,
};
pub use transaction::{calculate_fee, Transaction, TxIn, TRANSACTION_VERSION_1};
pub use utxo::{UtxoEntry, UtxoSet};
