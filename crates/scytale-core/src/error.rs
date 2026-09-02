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

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Primitive error: {0}")]
    Primitive(#[from] PrimitiveError),
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
}
