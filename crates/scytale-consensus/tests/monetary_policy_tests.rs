use scytale_consensus::{get_block_subsidy, COIN, HALVING_INTERVAL, MAX_SUPPLY};

const GENESIS_SUPPLY: u128 = 33_000_000 * COIN as u128;

#[test]
fn test_total_supply_never_exceeds_66_million() {
    let mut accumulated_mining: u128 = 0;
    for era in 0u64..64 {
        let subsidy = get_block_subsidy(era * HALVING_INTERVAL);
        if subsidy == 0 {
            break;
        }
        accumulated_mining += (HALVING_INTERVAL as u128) * (subsidy as u128);
    }

    let total_supply = GENESIS_SUPPLY + accumulated_mining;

    assert!(
        total_supply <= MAX_SUPPLY as u128,
        "total supply {} quanta must not exceed MAX_SUPPLY {} quanta",
        total_supply,
        MAX_SUPPLY
    );
    assert!(
        total_supply > 65_999_990 * COIN as u128,
        "total supply {} quanta must exceed 65_999_990 SCY in quanta",
        total_supply
    );
}
