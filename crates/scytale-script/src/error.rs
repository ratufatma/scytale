//! Error types for ScytaleScript execution.

use thiserror::Error;

/// Errors that can occur during script interpretation and stack operations.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ScriptError {
    #[error("stack underflow: attempted to pop from empty stack")]
    StackUnderflow,

    #[error("stack overflow: stack depth exceeded maximum limit ({0})")]
    StackOverflow(usize),

    #[error("item too large: element size {0} bytes exceeded limit of {1} bytes")]
    ItemTooLarge(usize, usize),

    #[error("invalid integer encoding: expected 8-byte little-endian integer, found {0} bytes")]
    InvalidInteger(usize),

    #[error("arithmetic overflow during operation")]
    ArithmeticOverflow,

    #[error("OP_EQUALVERIFY failed: elements are not equal")]
    EqualVerifyFailed,

    #[error("OP_CHECKSIGVERIFY failed: signature verification failed")]
    CheckSigVerifyFailed,

    #[error("OP_CHECKLOCKTIMEVERIFY failed: required lock height {lock_height} > current height {current_height}")]
    LockTimeNotMet {
        lock_height: u64,
        current_height: u64,
    },

    #[error("OP_RETURN encountered: transaction output is provably unspendable")]
    OpReturnEncountered,

    #[error("execution budget exceeded: maximum {0} opcodes allowed")]
    BudgetExceeded(usize),

    #[error("unbalanced conditional: mismatched OP_IF/OP_ELSE/OP_ENDIF")]
    UnbalancedConditionals,

    #[error("invalid opcode: 0x{0:02x}")]
    InvalidOpCode(u8),

    #[error("script truncated: expected {expected} bytes for pushdata, found {found}")]
    ScriptTruncated { expected: usize, found: usize },

    #[error("script evaluation failed: stack is empty or top item is false")]
    ScriptFailed,
}
