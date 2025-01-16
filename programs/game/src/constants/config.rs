use anchor_lang::prelude::*;

/// The super admin public key used by the game.
pub const SUPER_ADMIN: Pubkey = pubkey!("3aKZLDP9qQWN1iSUUsvxV5eFsjnG7K162aw1suAiUWyW");

/// The main token mint public key used by the game.
pub const TOKEN_MINT: Pubkey = pubkey!("6mLHbFNMZDFzb3dVEnAthfuwgNuCyAqhGoSzpHtDB5vf");

/// The default player public key used as a baseline or placeholder in the game logic.
pub const DEFAULT_PLAYER: Pubkey = pubkey!("11111111111111111111111111111111");

/// The default team number used as a baseline or placeholder in the game logic.
pub const DEFAULT_TEAM_NUMBER: u32 = 1_000_000;

/// The default round number used as a baseline or placeholder in the game logic.
pub const DEFAULT_ROUND_NUMBER: u16 = 1;

/// The default period number used as a baseline or placeholder in the game logic.
pub const DEFAULT_PERIOD_NUMBER: u16 = 1;

/// The price per ORE in terms of tokens.
pub const PRICE_PER_ORE: u64 = 1_000;

/// Lamports per token, representing the smallest token unit.
pub const LAMPORTS_PER_TOKEN: u64 = 1_000_000;

/// Lamports per ORE, computed as `PRICE_PER_ORE * LAMPORTS_PER_TOKEN`.
pub const LAMPORTS_PER_ORE: u64 = PRICE_PER_ORE * LAMPORTS_PER_TOKEN;

/// Number of seconds in a minute.
pub const SECONDS_PER_MINUTE: u64 = 60;

/// Number of seconds in an hour (60 * SECONDS_PER_MINUTE).
pub const SECONDS_PER_HOUR: u64 = SECONDS_PER_MINUTE * 60;

/// Number of seconds in a day (24 * SECONDS_PER_HOUR).
pub const SECONDS_PER_DAY: u64 = SECONDS_PER_HOUR * 24;
/// For testing or faster rounds, a "day" is reduced to 5 minutes instead of 24 hours.
// pub const SECONDS_PER_DAY: u64 = SECONDS_PER_MINUTE * 5;

/// Number of seconds in a year (365 * SECONDS_PER_DAY).
pub const SECONDS_PER_YEAR: u64 = SECONDS_PER_DAY * 365;
/// For testing or demonstration, a "year" is shortened to 10 minutes.
// pub const SECONDS_PER_YEAR: u64 = SECONDS_PER_MINUTE * 10;

/// Time extension in seconds for each action performed during the round.
pub const ACTION_TIME_EXTENSION: u8 = SECONDS_PER_MINUTE as u8;

/// Maximum countdown time in seconds (e.g., 1 hour).
pub const MAX_COUNTDOWN_SECONDS: u16 = SECONDS_PER_HOUR as u16;

/// The default exit rewards per second, used as a baseline for exit incentives.
pub const EXIT_REWARDS_PER_SECOND: u64 = 1 * LAMPORTS_PER_TOKEN;

/// The default sugar rush rewards, used as a baseline for sugar rush incentives.
pub const SUGAR_RUSH_REWARDS_PER_SECOND: u64 = 10 * LAMPORTS_PER_TOKEN;

/// The cooldown time in seconds for joining a team, defined as one "day" here.
pub const TEAM_JOIN_COOLDOWN_SECONDS: u64 = SECONDS_PER_DAY * 1;

/// Fixed reward amount for new player registration: 1500 FGC
/// Each FGC is represented in lamports, so `REGISTRATION_REWARD` = 1500 * LAMPORTS_PER_TOKEN.
pub const REGISTRATION_REWARD: u64 = 1_500 * LAMPORTS_PER_TOKEN;

/// Maximum daily cap for airdrop rewards: 500,000 FGC.
/// This value limits the total airdrop distribution per day to maintain game economy stability.
pub const DAILY_AIRDROP_REWARDS_CAP: u64 = 500_000 * LAMPORTS_PER_TOKEN;

/// The duration (in seconds) for which funds remain locked under normal conditions.
/// Set to one year (`SECONDS_PER_YEAR`) for a long-term staking scenario.
pub const LOCK_DURATION: u64 = SECONDS_PER_YEAR;

/// The duration (in seconds) during which an early unlock can be requested.
/// Set to one day (`SECONDS_PER_DAY`) to allow short-term early exits with reduced rewards.
pub const EARLY_UNLOCK_DURATION: u64 = SECONDS_PER_DAY;

/// The standard Annual Percentage Rate (APR) in basis points.
/// `APR = 100` means a 100% annual rate.
pub const ANNUAL_RATE: u8 = 100; // 100% APR

/// The reduced APR for early unlock scenarios in basis points.
/// `EARLY_UNLOCK_APR = 20` means a 20% annual rate for early unlocking.
pub const EARLY_UNLOCK_APR: u8 = 20; // 20% APR

/// One million constant for calculations and scaling.
pub const ONE_MILLION: u64 = 1_000_000;

/// Seed used to derive the game's Program Derived Address (PDA).
pub const GAME_SEED: &[u8] = b"game";

/// Seed used to derive the round's Program Derived Address (PDA).
pub const ROUND_SEED: &[u8] = b"round";

/// Seed used to derive the period's Program Derived Address (PDA).
pub const PERIOD_SEED: &[u8] = b"period";

/// Seed used to derive the voucher's Program Derived Address (PDA).
pub const VOUCHER_SEED: &[u8] = b"voucher";

/// Seed used to derive the voucher mint's Program Derived Address (PDA).
pub const VOUCHER_MINT_SEED: &[u8] = b"voucher_mint";

/// Seed used to derive the team's Program Derived Address (PDA).
pub const TEAM_SEED: &[u8] = b"team";

/// Seed used to derive the player's Program Derived Address (PDA).
pub const PLAYER_DATA_SEED: &[u8] = b"player_data";

/// Seed used to derive the stake order's Program Derived Address (PDA).
pub const STAKE_ORDER_SEED: &[u8] = b"stake_order";

/// Seed used to derive the pool's Program Derived Address (PDA).
pub const STAKE_POOL_SEED: &[u8] = b"stake_pool";

/// Seed used to derive the team name's Program Derived Address (PDA).
pub const TEAM_NAME_SEED: &[u8] = b"team_name";

/// Seed used to derive the deposit's Program Derived Address (PDA).
pub const VAULT_SEED: &[u8] = b"vault";

/// Percentage of total purchase allocated to construction worker rewards (25%).
pub const CONSTRUCTION_POOL_SHARE: u8 = 25;

/// Percentage of total purchase allocated to purchase lottery rewards (8%).
pub const LOTTERY_POOL_SHARE: u8 = 10;

/// Percentage of total purchase allocated to referrer rewards (10%).
pub const REFERRAL_POOL_SHARE: u8 = 10;

/// Percentage of total purchase allocated to grand prizes (30%).
pub const GRAND_PRIZES_POOL_SHARE: u8 = 30;

/// Percentage of total purchase allocated to consumption rewards (10%).
pub const CONSUMPTION_POOL_SHARE: u8 = 10;

/// Exchange collateral rate used in collateral exchange calculations.
pub const EXCHANGE_COLLATERAL_RATE: u8 = 100;

/// Redeem voucher rate used when converting vouchers back into tokens.
pub const REDEEM_VOUCHER_RATE: u8 = 10;

/// Cost in vouchers for one lottery draw (1000 FGV).
pub const ONCE_DRAW_LOTTERY_VOUCHER_COST: u64 = 1000 * LAMPORTS_PER_TOKEN;

/// Minimum required lottery pool balance for allowing draws.
pub const MIN_LOTTERY_REWARDS_POOL_BALANCE: u64 = 100_0000 * LAMPORTS_PER_TOKEN;
