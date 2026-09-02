use scytale_primitives::{OutPoint, PrimitiveError};
use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum TransactionError {
    #[error("unsupported transaction version: {0}")]
    InvalidVersion(u32),
    #[error("transaction inputs cannot be empty")]
    EmptyInputs,
    #[error("transaction outputs cannot be empty")]
    EmptyOutputs,
    #[error("output value must be greater than zero")]
    ZeroOutputValue,
    #[error("output values sum exceeded u64::MAX")]
    OutputValueOverflow,
    #[error("duplicate input detected: {0:?}")]
    DuplicateInput(OutPoint),
    #[error("input value deficit: total_in={total_in}, total_out={total_out}")]
    InputValueDeficit { total_in: u64, total_out: u64 },
    #[error("arithmetic overflow occurred during fee calculation")]
    ArithmeticOverflow,
    #[error("canonical serialization failure: {0}")]
    SerializationFailure(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum UtxoError {
    #[error("referenced UTXO does not exist: {0:?}")]
    MissingUtxo(OutPoint),
    #[error("output already spent: {0:?}")]
    AlreadySpent(OutPoint),
    #[error("input value deficit: total_in={total_in}, total_out={total_out}")]
    ValueDeficit { total_in: u64, total_out: u64 },
    #[error("arithmetic overflow occurred during balance calculation")]
    ArithmeticOverflow,
    #[error("transaction error: {0}")]
    TxError(#[from] TransactionError),
    #[error("coinbase transaction in invalid location")]
    InvalidCoinbasePlacement,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum AuthorizationError {
    #[error("authorization proof is empty")]
    EmptyAuthorization,
    #[error("authorization proof is malformed: {0}")]
    MalformedProof(String),
    #[error("signature verification failed")]
    SignatureMismatch,
    #[error("locking condition does not match provided proof or key")]
    KeyConditionMismatch,
    #[error("invalid input index {index} for transaction with {total_inputs} inputs")]
    InvalidInputIndex { index: usize, total_inputs: usize },
    #[error("failed to serialize transaction preimage: {0}")]
    PreimageSerializationFailure(String),
    #[error("input count ({input_count}) does not match resolved utxo count ({utxo_count})")]
    MismatchedUtxoCount {
        input_count: usize,
        utxo_count: usize,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum SerializationError {
    #[error("unexpected end of buffer: needed {needed} bytes, got {available}")]
    UnexpectedEof { needed: usize, available: usize },
    #[error("length prefix exceeds maximum allowed: {length} > {max}")]
    LengthExceedsLimit { length: usize, max: usize },
    #[error("trailing unparsed bytes detected: {0} remaining bytes")]
    TrailingBytes(usize),
    #[error("unsupported transaction version: {0}")]
    UnsupportedVersion(u32),
    #[error("invalid integer or boolean encoding")]
    InvalidEncoding,
    #[error("io error during canonical serialization: {0}")]
    Io(String),
}

impl From<std::io::Error> for SerializationError {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            SerializationError::UnexpectedEof {
                needed: 1,
                available: 0,
            }
        } else {
            SerializationError::Io(err.to_string())
        }
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Primitive error: {0}")]
    Primitive(#[from] PrimitiveError),
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),
    #[error("UTXO error: {0}")]
    Utxo(#[from] UtxoError),
    #[error("Authorization error: {0}")]
    Authorization(#[from] AuthorizationError),
    #[error("Serialization codec error: {0}")]
    SerializationCodec(#[from] SerializationError),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
}
