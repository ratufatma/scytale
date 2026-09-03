use scytale_consensus::{
    calculate_next_target, validate_block_target, DifficultyConfig, DifficultyError, Target,
    CLAMPING_FACTOR, DEFAULT_DIFFICULTY_EPOCH_BLOCKS, TARGET_BLOCK_INTERVAL_SECS,
};
use scytale_core::{BlockHeader, Hash256};

#[test]
fn test_expected_time_calculation() {
    assert_eq!(DEFAULT_DIFFICULTY_EPOCH_BLOCKS, 1440);
    assert_eq!(TARGET_BLOCK_INTERVAL_SECS, 60);
    assert_eq!(CLAMPING_FACTOR, 4);

    let config = DifficultyConfig::default();
    let expected_time = config.epoch_blocks * config.target_block_interval_secs;
    assert_eq!(expected_time, 1440 * 60);
    assert_eq!(expected_time, 86_400); // Exactly 24 hours in seconds
}

#[test]
fn test_target_unchanged_when_observed_equals_expected() {
    let config = DifficultyConfig::default();
    let initial_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);

    let start_time = 1_000_000;
    let end_time = start_time + 86_400; // Exact 86,400 seconds

    let next_target =
        calculate_next_target(&initial_target, start_time, end_time, &config).unwrap();
    assert_eq!(next_target, initial_target);
}

#[test]
fn test_target_decreases_when_blocks_too_fast() {
    let config = DifficultyConfig::default();
    let initial_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);

    let start_time = 1_000_000;
    let end_time = start_time + 43_200; // Half expected time (86,400 / 2) -> 2x faster

    let next_target =
        calculate_next_target(&initial_target, start_time, end_time, &config).unwrap();

    // Target must decrease (be smaller numerically, increasing difficulty)
    assert!(next_target < initial_target);

    // Expected next target is initial_target / 2
    let expected_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);
    assert_eq!(next_target, expected_target);
}

#[test]
fn test_target_increases_when_blocks_too_slow() {
    let config = DifficultyConfig::default();
    let initial_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);

    let start_time = 1_000_000;
    let end_time = start_time + 172_800; // Double expected time (86,400 * 2) -> 2x slower

    let next_target =
        calculate_next_target(&initial_target, start_time, end_time, &config).unwrap();

    // Target must increase (be larger numerically, decreasing difficulty)
    assert!(next_target > initial_target);

    // Expected next target is initial_target * 2
    let expected_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);
    assert_eq!(next_target, expected_target);
}

#[test]
fn test_clamping_upper_bound() {
    let config = DifficultyConfig::default();
    let initial_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);

    let start_time = 1_000_000;
    let end_time = start_time + 864_000; // 10x expected time -> clamped to exactly 4x

    let next_target =
        calculate_next_target(&initial_target, start_time, end_time, &config).unwrap();

    let expected_clamped_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);
    assert_eq!(next_target, expected_clamped_target);
}

#[test]
fn test_clamping_lower_bound() {
    let config = DifficultyConfig::default();
    let initial_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);

    let start_time = 1_000_000;
    let end_time = start_time + 8_640; // 1/10 expected time -> clamped to exactly 1/4x

    let next_target =
        calculate_next_target(&initial_target, start_time, end_time, &config).unwrap();

    let expected_clamped_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);
    assert_eq!(next_target, expected_clamped_target);
}

#[test]
fn test_ceiling_cap_at_max_target() {
    let config = DifficultyConfig::default();
    // Start near max target
    let near_max_target = config.max_target;

    let start_time = 1_000_000;
    let end_time = start_time + 345_600; // 4x slower

    let next_target =
        calculate_next_target(&near_max_target, start_time, end_time, &config).unwrap();
    assert_eq!(next_target, config.max_target);
}

#[test]
fn test_reject_invalid_observed_time() {
    let config = DifficultyConfig::default();
    let target = Target::max();

    // Equal times (0 duration)
    let err_equal = calculate_next_target(&target, 1_000_000, 1_000_000, &config).unwrap_err();
    assert_eq!(
        err_equal,
        DifficultyError::InvalidObservedTime {
            start_time: 1_000_000,
            end_time: 1_000_000
        }
    );

    // Negative duration (time moved backwards)
    let err_negative = calculate_next_target(&target, 1_000_000, 999_999, &config).unwrap_err();
    assert_eq!(
        err_negative,
        DifficultyError::InvalidObservedTime {
            start_time: 1_000_000,
            end_time: 999_999
        }
    );
}

#[test]
fn test_validate_block_target() {
    let expected_target = Target::from_compact(0x1d00ffff);

    let valid_header = BlockHeader::new(
        1,
        Hash256::ZERO,
        Hash256::ZERO,
        Hash256::ZERO,
        1700000000,
        0x1d00ffff,
        0,
    );

    assert!(validate_block_target(&valid_header, &expected_target).is_ok());

    let invalid_header = BlockHeader::new(
        1,
        Hash256::ZERO,
        Hash256::ZERO,
        Hash256::ZERO,
        1700000000,
        0x1e00ffff, // Different target
        0,
    );

    let err = validate_block_target(&invalid_header, &expected_target).unwrap_err();
    match err {
        DifficultyError::TargetMismatch { expected, actual } => {
            assert_eq!(expected, expected_target);
            assert_eq!(actual, Target::from_compact(0x1e00ffff));
        }
        _ => panic!("expected TargetMismatch error, got {:?}", err),
    }
}
