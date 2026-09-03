//! ScytaleScript: Minimalist stack-based script engine for Scytale blockchain.
//!
//! Features:
//! - Non-Turing-complete, deterministic LIFO stack execution
//! - Arithmetic operations with checked 64-bit integer overflow protections
//! - Cryptographic hashing (BLAKE3) and Ed25519 signature verification (OP_CHECKSIG)
//! - Forward branching logic (OP_IF / OP_ELSE / OP_ENDIF)
//! - Timelock verification (OP_CHECKLOCKTIMEVERIFY)
//! - Backward compatibility with legacy raw byte matching

pub mod builder;
pub mod context;
pub mod engine;
pub mod error;
pub mod opcode;
pub mod stack;

pub use builder::ScriptBuilder;
pub use context::ScriptContext;
pub use engine::{ScriptEngine, DEFAULT_MAX_OPS_BUDGET};
pub use error::ScriptError;
pub use opcode::OpCode;
pub use stack::{ScriptStack, MAX_ITEM_SIZE, MAX_STACK_DEPTH};
