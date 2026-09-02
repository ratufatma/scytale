use crate::error::DifficultyError;
use crate::target::Target;
use scytale_core::BlockHeader;

pub const TARGET_BLOCK_INTERVAL_SECS: u64 = 60;
pub const DEFAULT_DIFFICULTY_EPOCH_BLOCKS: u64 = 1440; // 1 day at 60s intervals
pub const CLAMPING_FACTOR: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DifficultyConfig {
    pub epoch_blocks: u64,
    pub target_block_interval_secs: u64,
    pub clamping_factor: u64,
    pub max_target: Target,
}

impl Default for DifficultyConfig {
    fn default() -> Self {
        Self {
            epoch_blocks: DEFAULT_DIFFICULTY_EPOCH_BLOCKS,
            target_block_interval_secs: TARGET_BLOCK_INTERVAL_SECS,
            clamping_factor: CLAMPING_FACTOR,
            max_target: Target([
                0x00, 0x00, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF,
            ]),
        }
    }
}

/// Multiplies a 256-bit big-endian Target by `multiplier` and divides by `divisor`
/// using pure integer arithmetic (zero floating-point operations).
pub fn scale_target_by_ratio(
    target: &Target,
    multiplier: u64,
    divisor: u64,
) -> Result<Target, DifficultyError> {
    if divisor == 0 {
        return Err(DifficultyError::ArithmeticOverflow);
    }
    if multiplier == 0 {
        return Ok(Target::zero());
    }

    // Convert target (32 bytes big-endian) to 4x 64-bit limbs (MSB at index 0)
    let mut limbs = [0u64; 4];
    for (i, limb) in limbs.iter_mut().enumerate() {
        let start = i * 8;
        *limb = u64::from_be_bytes(
            target.0[start..start + 8]
                .try_into()
                .map_err(|_| DifficultyError::ArithmeticOverflow)?,
        );
    }

    // Multiply [u64; 4] by u64 -> product fits in 5x u64 limbs
    let mut prod = [0u64; 5];
    let mut carry: u128 = 0;
    for i in (0..4).rev() {
        let cur = (limbs[i] as u128) * (multiplier as u128) + carry;
        prod[i + 1] = cur as u64;
        carry = cur >> 64;
    }
    prod[0] = carry as u64;

    // Divide 5x u64 limbs by divisor (u64) -> quotient in 5x u64 limbs
    let mut quot = [0u64; 5];
    let mut rem: u128 = 0;
    for i in 0..5 {
        let cur = (rem << 64) | (prod[i] as u128);
        quot[i] = (cur / (divisor as u128)) as u64;
        rem = cur % (divisor as u128);
    }

    // If quot[0] > 0, result exceeded 256 bits, cap at maximum 256-bit integer
    if quot[0] > 0 {
        return Ok(Target::max());
    }

    // Convert lower 4 limbs [quot[1], quot[2], quot[3], quot[4]] to 32 bytes big-endian
    let mut new_bytes = [0u8; 32];
    for i in 0..4 {
        let limb_bytes = quot[i + 1].to_be_bytes();
        new_bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb_bytes);
    }

    Ok(Target(new_bytes))
}

/// Calculates the next PoW difficulty target based on historical epoch timestamps.
pub fn calculate_next_target(
    current_target: &Target,
    epoch_start_time: u64,
    epoch_end_time: u64,
    config: &DifficultyConfig,
) -> Result<Target, DifficultyError> {
    if epoch_end_time <= epoch_start_time {
        return Err(DifficultyError::InvalidObservedTime {
            start_time: epoch_start_time,
            end_time: epoch_end_time,
        });
    }

    let expected_time = config
        .epoch_blocks
        .checked_mul(config.target_block_interval_secs)
        .ok_or(DifficultyError::ArithmeticOverflow)?;

    if expected_time == 0 || config.clamping_factor == 0 {
        return Err(DifficultyError::ArithmeticOverflow);
    }

    let raw_observed_time = epoch_end_time - epoch_start_time;

    // Clamping limits (max 4x easier or 4x harder)
    let min_time = expected_time / config.clamping_factor;
    let max_time = expected_time
        .checked_mul(config.clamping_factor)
        .ok_or(DifficultyError::ArithmeticOverflow)?;

    let clamped_time = raw_observed_time.clamp(min_time, max_time);

    let calculated = scale_target_by_ratio(current_target, clamped_time, expected_time)?;

    // Upper bound check: new target cannot exceed max_target
    if calculated.0 > config.max_target.0 {
        Ok(config.max_target)
    } else {
        Ok(calculated)
    }
}

/// Validates that a candidate block header specifies the consensus-calculated expected target.
pub fn validate_block_target(
    header: &BlockHeader,
    expected_target: &Target,
) -> Result<(), DifficultyError> {
    let header_target = Target::from_compact(header.difficulty_target);
    if &header_target != expected_target {
        return Err(DifficultyError::TargetMismatch {
            expected: *expected_target,
            actual: header_target,
        });
    }
    Ok(())
}
