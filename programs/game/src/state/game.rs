use crate::constants::{
    DAILY_AIRDROP_REWARDS_CAP, DEFAULT_PERIOD_NUMBER, DEFAULT_ROUND_NUMBER, DEFAULT_TEAM_NUMBER,
    EXIT_REWARDS_PER_SECOND, REGISTRATION_REWARD, SUGAR_RUSH_REWARDS_PER_SECOND,
    TEAM_JOIN_COOLDOWN_SECONDS,
};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

/// The `Game` account holds the global state of the entire platform. It tracks all
/// key parameters and balances, including reward pool balances, default entities,
/// and various nonces for PDA derivations. This structure is crucial for maintaining
/// the integrity and configurability of the game environment.
///
/// # Fields
/// - `authority`: The public key authorized to manage key aspects of the game (e.g., initializing rounds, updating configs).
/// - `token_mint`: The public key of the token mint that represents the in-game currency.
/// - `game_vault`: A vault account holding game funds. It stores tokens used for rewards and payouts.
/// - `default_team`: A default team entity assigned to newly registered players before they join or create their own team.
/// - `default_player`: A default player profile assigned during initial player creation.
/// - `current_round`: The public key of the currently active round in the game.
/// - `current_period`: The public key of the currently active leaderboard period.
/// - `current_day`: The current day index, computed as a timestamp-based day number (e.g., `timestamp / 86400`).
/// - Various pool balances for different reward categories (mining, bonus, lottery, developer, referral, registration, airdrop, consumption, exit).
///   These track available resources to be distributed to players and teams.
/// - Various counters (`distributed_*_rewards`) tracking the total amount of distributed rewards per category, aiding in analytics and caps enforcement.
/// - `current_day_distributed_airdrop_rewards`: Keeps track of how much airdrop reward has been distributed today to ensure it does not exceed the daily cap.
/// - `current_day_cap_airdrop_rewards`: The daily airdrop cap, usually set to `DAILY_AIRDROP_REWARDS_CAP`.
/// - `registration_rewards`: The fixed amount allocated for each player registration.
/// - `remaining_registration_slots`: How many registration rewards are still available to new players, enabling a limited incentive system.
/// - `team_nonce`, `round_nonce`, `period_nonce`: Incrementing counters used for PDA (Program Derived Address) derivation to ensure uniqueness of program accounts.
#[account]
#[derive(Debug, Default, InitSpace)]
pub struct Game {
    pub authority: Pubkey,
    pub bot_authority: Pubkey,
    pub token_mint: Pubkey,
    pub game_vault: Pubkey,

    pub default_team: Pubkey,
    pub default_player: Pubkey,

    pub current_round: Pubkey,
    pub current_period: Pubkey,

    // Pool balances
    pub construction_rewards_pool_balance: u64,
    pub bonus_rewards_pool_balance: u64,
    pub lottery_rewards_pool_balance: u64,
    pub developer_rewards_pool_balance: u64,
    pub referral_rewards_pool_balance: u64,

    pub round_rewards_pool_balance: u64,
    pub period_rewards_pool_balance: u64,
    pub registration_rewards_pool_balance: u64,
    pub airdrop_rewards_pool_balance: u64,
    pub consumption_rewards_pool_balance: u64,
    pub exit_rewards_pool_balance: u64,
    pub sugar_rush_rewards_pool_balance: u64,

    pub distributable_consumption_rewards: u64,

    // Distributed rewards tracking
    pub distributed_registration_rewards: u64,
    pub distributed_airdrop_rewards: u64,
    pub distributed_consumption_rewards: u64,
    pub distributed_exit_rewards: u64,
    pub distributed_stake_rewards: u64,
    pub distributed_construction_rewards: u64,
    pub distributed_bonus_rewards: u64,
    pub distributed_lottery_rewards: u64,
    pub distributed_developer_rewards: u64,
    pub distributed_referral_rewards: u64,
    pub distributed_grand_prizes: u64,
    pub distributed_individual_rewards: u64,
    pub distributed_team_rewards: u64,

    pub current_day_distributed_airdrop_rewards: u64,
    pub current_day_cap_airdrop_rewards: u64,

    // Registration reward configuration
    pub registration_rewards: u64,
    // Sugar rush reward configuration
    pub sugar_rush_rewards_per_second: u64,
    pub exit_rewards_per_second: u64,

    pub team_join_cooldown_seconds: u64,

    // PDAs nonces
    pub team_nonce: u32,
    pub event_nonce: u32,
    pub round_nonce: u16,
    pub period_nonce: u16,
    pub current_day: u32,
}

impl Game {
    /// Initializes a new game instance with default values and configuration.
    ///
    /// # Arguments
    /// - `authority`: The public key of the entity controlling and configuring the game.
    /// - `token_mint`: The public key of the token mint used as the in-game currency.
    /// - `game_vault`: The public key of the vault holding game funds.
    ///
    /// # Returns
    /// Returns `Ok(())` on success, otherwise returns an error code indicating the issue.
    pub fn initialize(
        &mut self,
        authority: Pubkey,
        bot_authority: Pubkey,
        token_mint: Pubkey,
        game_vault: Pubkey,
        round_rewards: u64,
        period_rewards: u64,
        registration_rewards: u64,
        airdrop_rewards: u64,
        exit_rewards: u64,
        lottery_rewards: u64,
        consumption_rewards: u64,
        sugar_rush_rewards: u64,
    ) -> Result<()> {
        *self = Game {
            authority,
            bot_authority,
            token_mint,
            game_vault,
            team_nonce: DEFAULT_TEAM_NUMBER,
            round_nonce: DEFAULT_ROUND_NUMBER,
            period_nonce: DEFAULT_PERIOD_NUMBER,
            registration_rewards: REGISTRATION_REWARD,
            sugar_rush_rewards_per_second: SUGAR_RUSH_REWARDS_PER_SECOND,
            exit_rewards_per_second: EXIT_REWARDS_PER_SECOND,
            team_join_cooldown_seconds: TEAM_JOIN_COOLDOWN_SECONDS,
            current_day_cap_airdrop_rewards: DAILY_AIRDROP_REWARDS_CAP,

            lottery_rewards_pool_balance: lottery_rewards,
            round_rewards_pool_balance: round_rewards,
            period_rewards_pool_balance: period_rewards,
            registration_rewards_pool_balance: registration_rewards,
            airdrop_rewards_pool_balance: airdrop_rewards,
            consumption_rewards_pool_balance: consumption_rewards,
            exit_rewards_pool_balance: exit_rewards,
            sugar_rush_rewards_pool_balance: sugar_rush_rewards,

            distributable_consumption_rewards: consumption_rewards,

            ..Default::default()
        };

        Ok(())
    }

    /// Increments the `team_nonce` by one, ensuring new unique team PDAs.
    pub fn increment_team_nonce(&mut self) -> Result<()> {
        self.team_nonce = self.team_nonce.safe_add(1)?;
        Ok(())
    }

    /// Increments the `round_nonce` by one, ensuring each new round PDA is uniquely derived.
    pub fn increment_round_nonce(&mut self) -> Result<()> {
        self.round_nonce = self.round_nonce.safe_add(1)?;
        Ok(())
    }

    /// Increments the `period_nonce` by one, ensuring unique period PDAs for leaderboard events.
    pub fn increment_period_nonce(&mut self) -> Result<()> {
        self.period_nonce = self.period_nonce.safe_add(1)?;
        Ok(())
    }

    /// Increments the `event_nonce` by one, ensuring unique event IDs.
    pub fn increment_event_nonce(&mut self) -> Result<()> {
        self.event_nonce = self.event_nonce.safe_add(1)?;
        Ok(())
    }
}
