use crate::constants::SECONDS_PER_YEAR;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

/// Basis points denominator (100%)
const BASIS_POINTS_DENOMINATOR: u8 = 100;

/// Calculate interest based on duration and annual rate
///
/// # Arguments
/// * `principal` - The initial amount
/// * `actual_duration` - Actual time duration (in seconds)
/// * `annual_rate` - Annual interest rate in basis points
///
/// # Returns
/// * `Result<u64>` - Calculated interest amount
pub fn calculate_prorated_interest(
    principal: u64,
    actual_duration: u64,
    annual_rate: u8,
) -> Result<u64> {
    let interest = principal
        .safe_div(BASIS_POINTS_DENOMINATOR as u64)?
        .safe_mul(annual_rate as u64)?
        .safe_mul(actual_duration)?
        .safe_div(SECONDS_PER_YEAR)?;

    Ok(interest)
}

/// Calculate proportional amount
///
/// # Arguments
/// * `amount` - The amount to calculate from
/// * `proportion` - The proportion in basis points
///
/// # Returns
/// * `Result<u64>` - Calculated proportional amount
pub fn calculate_proportion(amount: u64, proportion: u8) -> Result<u64> {
    let proportional_amount = amount
        .safe_div(BASIS_POINTS_DENOMINATOR as u64)?
        .safe_mul(proportion as u64)?;

    Ok(proportional_amount)
}

pub fn calculate_multiplier(symbols: [u8; 3]) -> u16 {
    let (s1, s2, s3) = (symbols[0], symbols[1], symbols[2]);

    if s1 == s2 && s2 == s3 {
        return match s1 {
            0 => 1000,
            x if x == 1 || x == 2 => 100,
            x if (3..=5).contains(&x) => 50,
            x if (6..=9).contains(&x) => 20,
            _ => 0,
        };
    }

    let cherry_count = [s1, s2, s3].iter().filter(|&&x| x == 1 || x == 2).count();
    if cherry_count > 0 && cherry_count < 3 {
        return 3 * (cherry_count as u16);
    }

    let bell_count = [s1, s2, s3]
        .iter()
        .filter(|&&x| (3..=5).contains(&x))
        .count();
    if bell_count == 2 {
        return 6;
    }

    let lemon_count = [s1, s2, s3]
        .iter()
        .filter(|&&x| (6..=9).contains(&x))
        .count();
    if lemon_count == 2 {
        return 3;
    }

    0
}

pub const REEL_SYMBOLS: [u8; 32] = [
    0, // 0:7
    1, 2, // 1,2
    3, 4, 5, // 3,4,5
    6, 7, 8, 9, // 6..9
    10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
];

pub fn get_symbol_id(random_byte: u8) -> u8 {
    let idx = (random_byte % 32) as usize;
    REEL_SYMBOLS[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_prorated_interest() {
        // Test case: 1000 tokens, 30 days, 100% APR
        let principal = 1000;
        let duration = 30 * 24 * 60 * 60; // 30 days in seconds
        let rate = 100; // 100% in basis points

        let interest = calculate_prorated_interest(principal, duration, rate).unwrap();
        assert_eq!(interest, 82); // Approximately 8.2% for 30 days
    }

    #[test]
    fn test_calculate_proportion() {
        // Test case: 1000 tokens, 25% proportion
        let total = 1000;
        let proportion = 25; // 25% in basis points

        let amount = calculate_proportion(total, proportion).unwrap();
        assert_eq!(amount, 250);
    }
}
