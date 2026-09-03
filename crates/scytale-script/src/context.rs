//! Execution context for script verification.

/// Context provided to the script interpreter during execution.
#[derive(Debug, Clone, Copy)]
pub struct ScriptContext<'a> {
    /// 32-byte Blake3/SHA digest of the transaction being authorized.
    pub sighash: &'a [u8; 32],
    /// Height of the block currently containing or evaluating the transaction.
    pub current_block_height: u64,
}

impl<'a> ScriptContext<'a> {
    /// Creates a new ScriptContext with the given sighash digest and block height.
    pub const fn new(sighash: &'a [u8; 32], current_block_height: u64) -> Self {
        Self {
            sighash,
            current_block_height,
        }
    }
}
