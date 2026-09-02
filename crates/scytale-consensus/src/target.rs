use scytale_primitives::Hash256;
use serde::{Deserialize, Serialize};

/// 256-bit Proof-of-Work Target threshold (big-endian).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Target(pub [u8; 32]);

impl Target {
    /// Creates a Target from big-endian bytes.
    pub const fn from_be_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the big-endian byte array representation.
    pub const fn to_be_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Maximum target (all 32 bytes 0xFF, easiest difficulty for testing).
    pub const fn max() -> Self {
        Self([0xFF; 32])
    }

    /// Minimum target (all 32 bytes 0x00, impossible difficulty).
    pub const fn zero() -> Self {
        Self([0x00; 32])
    }

    /// Evaluates whether a 32-byte BLAKE3 hash satisfies this target threshold.
    ///
    /// Both Hash256 and Target are evaluated as unsigned 256-bit big-endian integers.
    /// In big-endian byte array representation, numerical ordering is equivalent to
    /// lexicographical ordering from byte index 0 (MSB) to byte index 31 (LSB).
    pub fn is_met_by(&self, hash: &Hash256) -> bool {
        hash.as_bytes() <= &self.0
    }
}
