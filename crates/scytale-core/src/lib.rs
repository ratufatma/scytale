//! Scytale Core: Canonical transaction, block, UTXO, authorization, and serialization primitives for Scytale.

pub mod address;
pub mod authorization;
pub mod block;
pub mod codec;
pub mod error;
pub mod genesis;
pub mod transaction;
pub mod utxo;
pub mod vm_adapter;

pub use address::{Address, AddressError};
pub use authorization::{
    verify_transaction_authorization, AuthorizationVerifier, ConsensusScriptVerifier,
};
pub use block::{Block, BlockHeader};
pub use codec::{CanonicalDeserialize, CanonicalSerialize, MAX_VECTOR_LENGTH};
pub use error::{
    AuthorizationError, BlockError, CoreError, SerializationError, TransactionError, UtxoError,
};
pub use scytale_primitives::{
    Hash, Hash256, OutPoint, PrimitiveError, Quanta, TxOut, QUANTA_PER_SCY,
};
pub use transaction::{
    calculate_fee, EutxoWitness, OutputLock, Transaction, TxIn, TxInput, TxOutput,
    TRANSACTION_VERSION_1,
};
pub use utxo::{
    compute_utxo_leaf, compute_utxo_merkle_root, generate_utxo_merkle_proof, UtxoEntry,
    UtxoEntryWithOutpoint, UtxoMerkleProof, UtxoSet,
};
pub use vm_adapter::{
    create_tx_context, verify_transaction_eutxo, EutxoValidationError, MAX_BLOCK_GAS, MAX_TX_GAS,
};
