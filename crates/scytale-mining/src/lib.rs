//! Scytale Mining: Block candidate template assembly and Proof-of-Work mining loop.

pub mod error;
pub mod worker;

pub use error::{MinerState, MiningError};
pub use worker::{build_template, run_pow_search, BlockTemplate};

// Re-export verify_pow for external integration convenience
pub use scytale_consensus::verify_pow;
pub use scytale_core::{BlockHeader, Hash256};
