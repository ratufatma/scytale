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

    /// Converts a compact 32-bit integer difficulty target into a 256-bit Target.
    pub fn from_compact(compact: u32) -> Self {
        let exponent = (compact >> 24) as usize;
        let mantissa = compact & 0x007F_FFFF;
        let is_negative = (compact & 0x0080_0000) != 0;

        if is_negative || mantissa == 0 || exponent == 0 {
            return Self::zero();
        }

        let mut bytes = [0u8; 32];
        if exponent <= 3 {
            let shifted = mantissa >> (8 * (3 - exponent));
            let m_bytes = shifted.to_be_bytes();
            bytes[28..32].copy_from_slice(&m_bytes);
        } else {
            let offset = exponent - 3;
            if offset <= 29 {
                let m_bytes = mantissa.to_be_bytes();
                let start_idx = 32 - exponent;
                if start_idx + 2 < 32 {
                    bytes[start_idx] = m_bytes[1];
                    bytes[start_idx + 1] = m_bytes[2];
                    bytes[start_idx + 2] = m_bytes[3];
                }
            } else {
                return Self::max();
            }
        }
        Self(bytes)
    }

    /// Converts a 256-bit Target into a compact 32-bit integer difficulty target.
    pub fn to_compact(&self) -> u32 {
        let mut first_non_zero = 32;
        for (i, &b) in self.0.iter().enumerate() {
            if b != 0 {
                first_non_zero = i;
                break;
            }
        }

        if first_non_zero == 32 {
            return 0;
        }

        let mut exponent = 32 - first_non_zero;
        let mantissa: u32 = if self.0[first_non_zero] > 0x7F {
            exponent += 1;
            let mut m = (self.0[first_non_zero] as u32) << 16;
            if first_non_zero + 1 < 32 {
                m |= (self.0[first_non_zero + 1] as u32) << 8;
            }
            if first_non_zero + 2 < 32 {
                m |= self.0[first_non_zero + 2] as u32;
            }
            m >> 8
        } else {
            let mut m = (self.0[first_non_zero] as u32) << 16;
            if first_non_zero + 1 < 32 {
                m |= (self.0[first_non_zero + 1] as u32) << 8;
            }
            if first_non_zero + 2 < 32 {
                m |= self.0[first_non_zero + 2] as u32;
            }
            m
        };

        ((exponent as u32) << 24) | (mantissa & 0x007F_FFFF)
    }
}
