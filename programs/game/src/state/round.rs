use crate::constants::{ACTION_TIME_EXTENSION, MAX_COUNTDOWN_SECONDS};
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

/// Maximum number of last active participants to track for this round.
const MAX_LAST_ACTIVE_PARTICIPANT_LIST: usize = 10;

/// Total number of grand prize winners in a round.
const TOTAL_WINNERS: u8 = 10;

#[account]
#[derive(Debug, Default, InitSpace)]
/// The `Round` account represents the state and configuration of a single game round.
/// Each round has a start time, end time, reward pools, participant records, and can be dynamically updated as the game progresses.
///
/// # Fields
/// - `round_number`: A sequential identifier for this round.
/// - `round_vault`: The public key of the token vault holding funds for this round.
/// - `start_time`: The UNIX timestamp marking the beginning of the round.
/// - `end_time`: The UNIX timestamp marking when the round ends. The end time can be extended based on certain actions or conditions.
/// - `last_call_slot`: Tracks a specific slot number associated with the last action or "call" event (used for timing logic).
/// - `call_count`: How many times the final countdown has been triggered or extended.
/// - `earnings_per_ore`: The current rate of earnings per ore unit for players participating in this round.
/// - `sold_ores`: The total number of ores sold during the round, indicative of player participation.
/// - `available_ores`: How many ores are still available for purchase or allocation.
/// - `grand_prize_pool_balance`: The total balance dedicated to grand prizes.
/// - `construction_pool_balance`: The balance dedicated to construction rewards in this round.
/// - `first_grand_prizes`: The amount allocated for the first set of grand prize winners (e.g., first place).
/// - `second_grand_prizes`: The amount allocated for the remaining grand prize winners (e.g., runner-ups).
/// - `distributed_grand_prizes`: How many grand prizes have already been distributed.
/// - `distributed_construction_rewards`: How many construction rewards have been allocated so far.
/// - `grand_prize_distribution_index`: An index tracking how many winners have been awarded grand prizes.
/// - `last_active_participant_list`: A list of public keys representing the most recent active participants.
///   Maintained in order, with the most recent participant inserted at the front.
/// - `auto_reinvesting_players`: How many players have opted for auto-reinvestment of their rewards.
/// - `is_over`: Indicates whether the round is completed.
/// - `is_grand_prize_distribution_completed`: Indicates whether all grand prizes have been fully distributed.
/// - `exit_rewards_per_second`: The rate at which exit rewards accrue per second.
/// - `last_collected_exit_reward_timestamp`: The last timestamp at which exit rewards were claimed or adjusted.
/// - `bump`: A PDA bump seed for this round account.
pub struct Round {
    pub round_number: u16,
    pub round_vault: Pubkey,

    pub start_time: u64,
    pub end_time: u64,

    pub last_call_slot: u64,
    pub call_count: u8,

    pub earnings_per_ore: u64,

    pub sold_ores: u32,
    pub available_ores: u32,

    pub grand_prize_pool_balance: u64,

    pub first_grand_prizes: u64,
    pub second_grand_prizes: u64,

    pub distributed_grand_prizes: u64,
    pub grand_prize_distribution_index: u8,

    #[max_len(MAX_LAST_ACTIVE_PARTICIPANT_LIST)]
    pub last_active_participant_list: Vec<Pubkey>,

    pub auto_reinvesting_players: u16,

    pub is_over: bool,
    pub is_grand_prize_distribution_completed: bool,

    pub last_collected_exit_reward_timestamp: u64,
    pub last_collected_sugar_rush_reward_timestamp: u64,

    pub bump: u8,
}

impl Round {
    /// Initializes a new round with the provided configuration parameters.
    ///
    /// # Arguments
    /// - `round_number`: A sequential identifier for the round.
    /// - `round_vault`: The public key of the token vault for this round.
    /// - `grand_prize_pool_balance`: The initial balance allocated to grand prizes.
    /// - `start_time`: The UNIX timestamp marking when this round starts.
    /// - `countdown_duration`: The duration of the round in seconds before it ends, absent extensions.
    /// - `default_player`: A default player public key used to initialize the `last_active_participant_list`.
    /// - `bump`: The PDA bump seed.
    ///
    /// # Returns
    /// Returns `Ok(())` if successful, otherwise `InvalidTimestamp` if the end time computation fails.
    pub fn initialize(
        &mut self,
        round_number: u16,
        round_vault: Pubkey,
        grand_prize_pool_balance: u64,
        start_time: u64,
        countdown_duration: u64,
        default_player: Pubkey,
        bump: u8,
    ) -> Result<()> {
        let end_time = start_time
            .checked_add(countdown_duration)
            .ok_or(ErrorCode::InvalidTimestamp)?;

        *self = Round {
            round_number,
            round_vault,
            grand_prize_pool_balance,
            start_time,
            end_time,
            last_active_participant_list: vec![default_player; MAX_LAST_ACTIVE_PARTICIPANT_LIST],
            last_collected_exit_reward_timestamp: start_time,
            last_collected_sugar_rush_reward_timestamp: start_time,
            bump,
            ..Default::default()
        };

        Ok(())
    }

    /// Updates the end time of the round, potentially extending it based on current conditions.
    /// This function resets `last_call_slot` and `call_count`, and applies logic to ensure the round
    /// does not extend indefinitely beyond `MAX_COUNTDOWN_SECONDS`.
    ///
    /// # Arguments
    /// - `current_time`: The current UNIX timestamp.
    pub fn update_end_time(&mut self, current_time: u64) -> Result<()> {
        self.last_call_slot = 0;
        self.call_count = 0;

        // If current time surpasses the end time, extend by ACTION_TIME_EXTENSION
        if current_time > self.end_time {
            self.end_time = current_time.safe_add(ACTION_TIME_EXTENSION as u64)?;
            return Ok(());
        }

        // If there's already more than MAX_COUNTDOWN_SECONDS remaining, do not shorten or extend.
        if self.end_time.safe_sub(current_time)? > MAX_COUNTDOWN_SECONDS as u64 {
            return Ok(());
        }

        // Extend end_time by ACTION_TIME_EXTENSION, but do not exceed MAX_COUNTDOWN_SECONDS beyond current_time
        let extended_time = self
            .end_time
            .max(current_time)
            .safe_add(ACTION_TIME_EXTENSION as u64)?;
        let max_end_time = current_time.safe_add(MAX_COUNTDOWN_SECONDS as u64)?;
        self.end_time = extended_time.min(max_end_time);

        Ok(())
    }

    /// Updates the list of the last active participants by inserting the new participant
    /// at the front and removing the oldest if the list exceeds the maximum length.
    ///
    /// # Arguments
    /// - `player`: The public key of the active participant to add.
    pub fn update_last_active_participant_list(&mut self, player: Pubkey) -> Result<()> {
        self.last_active_participant_list.retain(|&x| x != player);

        if self.last_active_participant_list.len() >= MAX_LAST_ACTIVE_PARTICIPANT_LIST {
            self.last_active_participant_list.pop();
        }

        self.last_active_participant_list.insert(0, player);

        Ok(())
    }

    /// Distributes grand prizes to winners, one distribution at a time, until all `TOTAL_WINNERS`
    /// are awarded. The first winner receives `first_grand_prizes` amount, subsequent winners receive
    /// `second_grand_prizes` amount each.
    ///
    /// # Returns
    /// Returns the `reward_amount` distributed in this call.
    /// If insufficient grand prize pool balance is available, returns `InsufficientGrandPrizePoolBalance`.
    pub fn distribute_grand_prizes(&mut self) -> Result<u64> {
        // If no distribution has started, compute the prize amounts.
        if self.grand_prize_distribution_index == 0 {
            self.calculate_prize_amounts()?;
        }

        let reward_amount = if self.grand_prize_distribution_index == 0 {
            self.first_grand_prizes
        } else {
            self.second_grand_prizes
        };

        require!(
            self.grand_prize_pool_balance >= reward_amount,
            RoundError::InsufficientGrandPrizePoolBalance
        );

        // Deduct the reward from the grand prize pool and record distribution
        self.grand_prize_pool_balance = self.grand_prize_pool_balance.safe_sub(reward_amount)?;
        self.distributed_grand_prizes = self.distributed_grand_prizes.safe_add(reward_amount)?;

        // Increment the distribution index to move to the next winner
        self.grand_prize_distribution_index = self.grand_prize_distribution_index.safe_add(1)?;

        // Once we've reached `TOTAL_WINNERS`, mark prize distribution as complete
        if self.grand_prize_distribution_index == TOTAL_WINNERS {
            self.is_grand_prize_distribution_completed = true;
        }

        Ok(reward_amount)
    }

    /// Calculates the amounts allocated to the top winner and the subsequent winners.
    /// Splits the `grand_prize_pool_balance` into `first_grand_prizes` and `second_grand_prizes`.
    fn calculate_prize_amounts(&mut self) -> Result<()> {
        let half_prize = self.grand_prize_pool_balance.safe_div(2)?;
        let shared_prize = half_prize.safe_div(TOTAL_WINNERS as u64)?;

        self.first_grand_prizes = half_prize.safe_add(shared_prize)?;
        self.second_grand_prizes = shared_prize;

        Ok(())
    }
}

/// Error codes related to round operations.
/// These errors help identify why certain actions within a round failed.
#[error_code]
#[derive(PartialEq)]
pub enum RoundError {
    /// Emitted when the grand prize pool does not have enough funds to pay a prize.
    #[msg("Insufficient grand prize pool balance")]
    InsufficientGrandPrizePoolBalance,

    /// Emitted when the construction reward pool lacks sufficient funds.
    /// Note: Construction reward management may occur in other round-related operations.
    #[msg("Insufficient balance in construction reward pool")]
    InsufficientConstructionRewardBalance,

    /// Emitted when there are not enough ores available for a requested operation.
    #[msg("Insufficient ores for subtraction")]
    InsufficientOres,
}
