use crate::constants::{
    ANNUAL_RATE, EARLY_UNLOCK_APR, EARLY_UNLOCK_DURATION, LAMPORTS_PER_TOKEN, LOCK_DURATION,
    ONE_MILLION,
};
use crate::errors::ErrorCode;
use crate::utils::calculate_prorated_interest;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

/// The `StakePool` account stores global state for the staking system.
/// The `StakePool` account stores global state for the staking system.
/// It tracks total staked amounts, available and distributed rewards, as well as interest rates.
/// This account underpins the logic for creating, managing, and completing stake orders, ensuring
/// that enough rewards are available and that staked amounts are properly accounted for.
#[account]
#[derive(Debug, Default, InitSpace)]
pub struct StakePool {
    /// The vault holding all staked funds and their corresponding rewards.
    pub stake_pool_token_vault: Pubkey,

    /// The vault holding all staked vouchers and their corresponding rewards.
    pub stake_pool_voucher_vault: Pubkey,

    /// The total amount of tokens currently staked in this pool.
    pub staked_amount: u64,

    /// The total amount of rewards that have been allocated (distributed) to stake orders from this pool.
    pub distributed_token_rewards: u64,

    /// The total amount of rewards that have been burned from this pool.
    pub burned_token_rewards: u64,

    /// The total amount of vouchers that have been issued (distributed) to orders.
    pub distributed_voucher_rewards: u64,

    /// The total amount of vouchers that have been burned from this pool.
    pub burned_voucher_rewards: u64,

    /// The remaining reward balance that can still be allocated to new or ongoing orders.
    pub token_rewards_pool_balance: u64,

    /// The remaining voucher balance that can still be allocated.
    pub voucher_rewards_pool_balance: u64,

    /// The remaining reward balance that can still be allocated to new or ongoing orders.
    pub distributable_token_rewards: u64,

    /// The value of one shard in lamports.
    pub one_shard: u64,

    /// The standard annual interest rate (APR) applied to new stake orders (in basis points).
    pub annual_rate: u8,

    /// The annual interest rate applied if early unlock is requested (in basis points).
    pub early_unlock_rate: u8,

    /// The duration (in seconds) for which funds remain locked under normal conditions.
    pub lock_duration: u64,

    /// The duration (in seconds) for which funds remain locked under early unlock conditions.
    pub early_unlock_duration: u64,

    /// The number of active stake orders currently outstanding.
    pub active_orders: u32,
}

impl StakePool {
    /// Initializes the `StakePool` with specified token mint and vault.
    /// Sets default APR and early unlock APR based on predefined constants.
    ///
    /// # Arguments
    /// - `stake_token_mint`: The public key of the staking token's mint.
    /// - `stake_pool_vault`: The public key of the vault holding staked funds.
    ///
    /// # Returns
    /// `Ok(())` if initialization succeeds.
    pub fn initialize_token_pool(
        &mut self,
        stake_pool_token_vault: Pubkey,
        token_rewards: u64,
    ) -> Result<()> {
        *self = StakePool {
            stake_pool_token_vault,
            one_shard: ONE_MILLION.safe_mul(LAMPORTS_PER_TOKEN)?,
            annual_rate: ANNUAL_RATE,
            early_unlock_rate: EARLY_UNLOCK_APR,
            lock_duration: LOCK_DURATION,
            early_unlock_duration: EARLY_UNLOCK_DURATION,

            token_rewards_pool_balance: token_rewards,
            distributable_token_rewards: token_rewards,
            ..Default::default()
        };

        Ok(())
    }

    pub fn initialize_voucher_pool(
        &mut self,
        stake_pool_voucher_vault: Pubkey,
        voucher_rewards: u64,
    ) -> Result<()> {
        self.stake_pool_voucher_vault = stake_pool_voucher_vault;
        self.voucher_rewards_pool_balance = voucher_rewards;

        Ok(())
    }

    /// Updates the pool's annual and early unlock interest rates.
    ///
    /// # Arguments
    /// - `annual_rate`: New standard APR (in basis points).
    /// - `early_unlock_rate`: New early unlock APR (in basis points).
    pub fn update_rates(&mut self, annual_rate: u8, early_unlock_rate: u8) -> Result<()> {
        self.annual_rate = annual_rate;
        self.early_unlock_rate = early_unlock_rate;
        Ok(())
    }

    /// Adds additional rewards to the pool, increasing its capacity to handle future orders.
    ///
    /// # Arguments
    /// - `amount`: The amount of additional rewards to add.
    pub fn add_rewards(&mut self, amount: u64) -> Result<()> {
        self.token_rewards_pool_balance = self.token_rewards_pool_balance.safe_add(amount)?;
        Ok(())
    }

    /// Completes a stake order by removing its staked amount and recording its final rewards as mined.
    /// Decrements the number of active orders and updates the mined rewards total.
    ///
    /// # Arguments
    /// - `staked_amount`: The principal amount originally staked in the order.
    pub fn complete_order(&mut self, staked_amount: u64) -> Result<()> {
        require!(
            self.staked_amount >= staked_amount,
            ErrorCode::StakeOrderInsufficientBalance
        );
        self.staked_amount = self.staked_amount.safe_sub(staked_amount)?;
        self.active_orders = self.active_orders.safe_sub(1)?;
        Ok(())
    }
}

/// The `StakeOrder` account represents a single staking position.
/// It tracks the principal staked amount, the associated rewards, timestamps, and state flags for early unlocks.
/// Each `StakeOrder` can either run its full course (LOCK_DURATION) at the full APR
/// or be unlocked early at a reduced APR for fewer rewards.
#[account]
#[derive(Debug, Default, InitSpace)]
pub struct StakeOrder {
    /// A unique identifying number for the stake order.
    pub stake_number: u16,

    /// The amount staked in this order (principal). This amount is immutable after creation.
    pub stake_amount: u64,

    /// The total amount of rewards initially locked in this order at creation time.
    pub token_rewards: u64,

    /// The total amount of rewards initially locked in this order at creation time.
    pub voucher_rewards: u64,

    /// A vault specifically associated with this stake order for holding staked assets.
    pub stake_order_vault: Pubkey,

    /// The UNIX timestamp when the order was created.
    pub created_timestamp: u64,

    /// The UNIX timestamp when the order becomes eligible for withdrawal without penalty,
    /// normally `created_timestamp + LOCK_DURATION`.
    pub unstaked_timestamp: u64,

    /// The annual rate (in basis points) applied to this order. Defaults to `APR`,
    /// but may be reduced to `EARLY_UNLOCK_APR` if early unstake is requested.
    pub annual_rate: u8,

    /// The lock duration (in seconds) for this order.
    pub lock_duration: u64,

    /// A flag indicating if early unstake has been requested, reducing the APR and locking period.
    pub is_early_unstaked: bool,

    /// A flag indicating whether this order is fully completed and rewards have been claimed.
    pub is_completed: bool,

    /// A PDA bump seed for the stake order account.
    pub bump: u8,
}

impl StakeOrder {
    /// Initializes a new stake order with the provided parameters.
    ///
    /// # Arguments
    /// - `stake_number`: A unique identifier for this order.
    /// - `stake_amount`: The principal staked amount.
    /// - `reward_amount`: The initial computed rewards for the order.
    /// - `stake_order_vault`: The vault holding the staked tokens for this order.
    /// - `created_timestamp`: The UNIX timestamp at order creation.
    /// - `bump`: PDA bump seed.
    ///
    /// # Returns
    /// `Ok(())` if successfully initialized.
    pub fn initialize(
        &mut self,
        stake_number: u16,
        stake_amount: u64,
        annual_rate: u8,
        lock_duration: u64,
        token_rewards: u64,
        voucher_rewards: u64,
        stake_order_vault: Pubkey,
        created_timestamp: u64,
        bump: u8,
    ) -> Result<()> {
        *self = StakeOrder {
            stake_number,
            stake_amount,
            token_rewards,
            voucher_rewards,
            stake_order_vault,
            created_timestamp,
            unstaked_timestamp: created_timestamp.safe_add(lock_duration)?,
            annual_rate,
            lock_duration,
            is_early_unstaked: false,
            is_completed: false,
            bump,
            ..Default::default()
        };

        Ok(())
    }

    /// Requests an early unlock for this stake order.
    /// Reduces the APR and recalculates rewards based on the elapsed time.
    /// Adjusts `locked_rewards` to reflect the new, reduced reward amount and sets a shorter `unstaked_timestamp`.
    ///
    /// # Arguments
    /// - `current_timestamp`: The current UNIX timestamp to determine elapsed time.
    /// - `early_unstake_rate`: The reduced APR to apply for early unlocking.
    ///
    /// # Returns
    /// Returns `unused_rewards`, the portion of initially locked rewards that become unused due to the reduced interest calculation.
    pub fn request_early_unstake(
        &mut self,
        current_timestamp: u64,
        early_unstake_rate: u8,
        early_unlock_duration: u64,
    ) -> Result<()> {
        require!(
            !self.is_early_unstaked,
            ErrorCode::EarlyUnlockAlreadyRequested
        );
        let elapsed_time = current_timestamp.safe_sub(self.created_timestamp)?;
        let new_token_rewards =
            calculate_prorated_interest(self.stake_amount, elapsed_time, early_unstake_rate)?;

        self.lock_duration = elapsed_time;
        self.token_rewards = new_token_rewards;
        self.voucher_rewards = 0;
        self.annual_rate = early_unstake_rate;
        self.unstaked_timestamp = current_timestamp.safe_add(early_unlock_duration)?;
        self.is_early_unstaked = true;

        Ok(())
    }

    /// Completes the stake order, setting `is_completed = true`.
    /// This usually occurs when the staking period ends and rewards are claimed.
    pub fn complete(&mut self) -> Result<()> {
        require!(!self.is_completed, ErrorCode::StakeOrderAlreadyCompleted);

        self.is_completed = true;
        Ok(())
    }

    /// Checks if the order can be unstaked at the given `current_timestamp`.
    /// The order is unstakeable if `current_timestamp >= unstaked_timestamp`.
    pub fn can_unstake(&self, current_timestamp: u64) -> bool {
        current_timestamp >= self.unstaked_timestamp
    }
}
