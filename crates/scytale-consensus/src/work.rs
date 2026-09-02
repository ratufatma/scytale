use crate::target::Target;
use serde::{Deserialize, Serialize};

/// 256-bit unsigned integer representing cumulative Proof-of-Work (big-endian limbs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CumulativeWork(pub [u64; 4]);

impl CumulativeWork {
    /// Zero cumulative work.
    pub const fn zero() -> Self {
        Self([0; 4])
    }

    /// Constructs CumulativeWork from 32 big-endian bytes.
    pub fn from_be_bytes(bytes: [u8; 32]) -> Self {
        let mut limbs = [0u64; 4];
        for (i, limb) in limbs.iter_mut().enumerate() {
            let start = i * 8;
            *limb = u64::from_be_bytes(bytes[start..start + 8].try_into().unwrap());
        }
        Self(limbs)
    }

    /// Converts CumulativeWork to 32 big-endian bytes.
    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (i, limb) in self.0.iter().enumerate() {
            let start = i * 8;
            bytes[start..start + 8].copy_from_slice(&limb.to_be_bytes());
        }
        bytes
    }

    /// Checked addition of two 256-bit work values.
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        let mut res = [0u64; 4];
        let mut carry: u128 = 0;
        for i in (0..4).rev() {
            let sum = (self.0[i] as u128) + (other.0[i] as u128) + carry;
            res[i] = sum as u64;
            carry = sum >> 64;
        }
        if carry > 0 {
            None
        } else {
            Some(Self(res))
        }
    }
}

/// Computes bit-by-bit long division of two 256-bit unsigned integers: `num / den`.
fn div256(num: [u64; 4], den: [u64; 4]) -> [u64; 4] {
    if den == [0; 4] {
        return [u64::MAX; 4];
    }
    if num < den {
        return [0; 4];
    }
    if num == den {
        return [0, 0, 0, 1];
    }

    let mut quotient = [0u64; 4];
    let mut remainder = [0u64; 4];

    // Iterate through all 256 bits from MSB (bit 255) to LSB (bit 0)
    for i in (0..256).rev() {
        // Shift remainder left by 1 bit
        let mut rem_carry = 0u64;
        for j in (0..4).rev() {
            let next_carry = remainder[j] >> 63;
            remainder[j] = (remainder[j] << 1) | rem_carry;
            rem_carry = next_carry;
        }

        // Extract bit i of num and insert into LSB of remainder
        let limb_idx = 3 - (i / 64);
        let bit_idx = i % 64;
        let bit = (num[limb_idx] >> bit_idx) & 1;
        remainder[3] |= bit;

        // If remainder >= den, subtract den from remainder and set bit i of quotient
        if remainder >= den {
            let mut borrow: u128 = 0;
            for j in (0..4).rev() {
                let diff = (remainder[j] as u128).wrapping_sub((den[j] as u128) + borrow);
                if diff > (remainder[j] as u128) {
                    remainder[j] = diff as u64;
                    borrow = 1;
                } else {
                    remainder[j] = diff as u64;
                    borrow = 0;
                }
            }
            quotient[limb_idx] |= 1 << bit_idx;
        }
    }

    quotient
}

/// Calculates the discrete work metric of a block from its target:
/// Work = 2^256 / (Target + 1) = (~Target / (Target + 1)) + 1
pub fn block_work(target: &Target) -> CumulativeWork {
    let mut t_limbs = [0u64; 4];
    for (i, limb) in t_limbs.iter_mut().enumerate() {
        let start = i * 8;
        *limb = u64::from_be_bytes(target.0[start..start + 8].try_into().unwrap());
    }

    // Target + 1
    let mut t_plus_1 = [0u64; 4];
    let mut carry: u128 = 1;
    for i in (0..4).rev() {
        let sum = (t_limbs[i] as u128) + carry;
        t_plus_1[i] = sum as u64;
        carry = sum >> 64;
    }

    // If carry > 0, Target was 2^256 - 1 (all 0xFF), so (Target + 1) = 2^256 -> Work = 1
    if carry > 0 {
        return CumulativeWork([0, 0, 0, 1]);
    }

    // ~Target
    let not_t = [!t_limbs[0], !t_limbs[1], !t_limbs[2], !t_limbs[3]];
    let div_res = div256(not_t, t_plus_1);

    // div_res + 1
    let mut res = [0u64; 4];
    let mut c: u128 = 1;
    for i in (0..4).rev() {
        let sum = (div_res[i] as u128) + c;
        res[i] = sum as u64;
        c = sum >> 64;
    }

    CumulativeWork(res)
}
