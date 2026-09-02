use scytale_core::{AuthorizationError, Hash256, OutPoint, TransactionError};
use thiserror::Error;

/// Strongly typed errors returned during transaction admission and mempool operations.
#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum MempoolError {
    #[error("transaction already exists in mempool: {0:?}")]
    DuplicateTx(Hash256),
    #[error("referenced input UTXO not found: {0:?}")]
    MissingInputUtxo(OutPoint),
    #[error(
        "conflict detected: input {outpoint:?} is already spent by pending tx {conflicting_tx:?}"
    )]
    ConflictDoubleSpend {
        outpoint: OutPoint,
        conflicting_tx: Hash256,
    },
    #[error("authorization verification failed: {0}")]
    AuthorizationFailed(String),
    #[error("value deficit: total input ({total_in}) < total output ({total_out})")]
    ValueDeficit { total_in: u64, total_out: u64 },
    #[error("structural transaction validation failed: {0}")]
    StructuralError(String),
    #[error("coinbase transactions cannot be admitted to mempool")]
    CoinbaseNotAllowed,
    #[error("arithmetic overflow occurred during fee calculation")]
    ArithmeticOverflow,
}

impl From<TransactionError> for MempoolError {
    fn from(err: TransactionError) -> Self {
        Self::StructuralError(err.to_string())
    }
}

impl From<AuthorizationError> for MempoolError {
    fn from(err: AuthorizationError) -> Self {
        Self::AuthorizationFailed(err.to_string())
    }
}
