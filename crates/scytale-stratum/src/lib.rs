//! High-performance Stratum mining pool server (SSP-1) for Scytale Layer-1.
//!
//! Provides:
//! - 120-byte raw block header framing & BLAKE3 Proof-of-Work validation.
//! - JSON-RPC 2.0 Stratum protocol stream codec and worker session state machine.
//! - Extranonce1 & Extranonce2 Merkle tree reconstitution.
//! - Share verification, PPLNS difficulty accounting, and instant block broadcast pipeline.

pub mod codec;
pub mod error;
pub mod header;
pub mod job;
pub mod protocol;
pub mod server;
pub mod session;
pub mod shares;

pub use codec::{LinesCodec, StratumCodec, MAX_FRAME_LENGTH};
pub use error::{StratumError, StratumRpcError};
pub use header::{RawBlockHeader120, HEADER_SIZE};
pub use job::StratumJob;
pub use protocol::{
    StratumNotification, StratumRequest, StratumResponse, StratumRpcMessage,
};
pub use server::{BlockFoundEvent, StratumServer};
pub use session::WorkerSession;
pub use shares::{compact_to_target, verify_worker_share, ShareVerificationResult, U256};
