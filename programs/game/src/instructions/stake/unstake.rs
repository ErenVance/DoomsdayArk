use crate::constants::{GAME_SEED, PLAYER_DATA_SEED, STAKE_ORDER_SEED, STAKE_POOL_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `Unstake` instruction allows a player to withdraw their originally staked tokens plus accrued rewards from a completed or fully vested stake order.
/// Once the lock-up period (or early unlock duration) has passed, the player can unstake their tokens and claim rewards directly to their token account.
#[derive(Accounts)]
#[instruction(order_number: u16)]
pub struct Unstake<'info> {
    /// The player initiating the unstake operation. Must be the signer of the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The global game account.
    #[account(mut,
        seeds = [GAME_SEED], bump,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The player's data account tracking their state, including orders created.
    /// Verified by `seeds` to ensure the correct association with the player.
    #[account(
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = token_account,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The specific stake order to be unstaked.
    /// Verified by `seeds` to ensure uniqueness and ownership by the `player`.
    /// The order must not be completed yet.
    #[account(mut,
        seeds = [
            STAKE_ORDER_SEED,
            player.key().as_ref(),
            order_number.to_le_bytes().as_ref()
        ],
        bump,
        has_one = stake_order_vault,
        constraint = stake_order.is_completed == false,
    )]
    pub stake_order: Box<Account<'info, StakeOrder>>,

    /// The associated token vault for this stake order, holding the staked tokens and accumulated rewards.
    #[account(mut)]
    pub stake_order_vault: Box<Account<'info, TokenAccount>>,

    /// The player's token account, where the unstaked tokens and rewards will be transferred.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The global stake pool account, tracking total staked amounts and rewards distribution.
    /// Verified by `seeds` for correct program-derived address derivation.
    #[account(mut,
        seeds = [STAKE_POOL_SEED],
        bump,
        has_one = stake_pool_token_vault,
    )]
    pub stake_pool: Box<Account<'info, StakePool>>,

    /// The stake pool's token vault, holding the rewards pool tokens.
    #[account(mut)]
    pub stake_pool_token_vault: Box<Account<'info, TokenAccount>>,

    /// The SPL token program, required for token transfer operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// The `unstake` function completes an existing stake order and returns the staked principal plus accrued rewards to the player.
///
/// Steps:
/// 1. Validate that the order number is valid and the order is associated with the player.
/// 2. Check that the current time allows unstaking (the lock-up period or early unlock duration has passed).
/// 3. Mark the order as completed and adjust the pool state accordingly.
/// 4. Transfer the combined principal (stake_amount) and locked_rewards back to the player's token account.
/// 5. Emit an `Unstake` event to record this operation on-chain.
pub fn unstake(ctx: Context<Unstake>, order_number: u16) -> Result<()> {
    // Obtain the current UNIX timestamp
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to the relevant accounts
    let Unstake {
        game,
        player,
        player_data,
        stake_pool,
        token_account,
        stake_order,
        stake_order_vault,
        token_program,
        stake_pool_token_vault,
        ..
    } = ctx.accounts;

    // Confirm that the order_number is valid. Player_data nonce should be at least the order_number,
    // ensuring that this order_number has been created previously.
    require!(
        player_data.nonce >= order_number,
        ErrorCode::StakeOrderNotFound
    );

    // Check if the order can be unstaked at the current timestamp
    require!(
        stake_order.can_unstake(timestamp),
        ErrorCode::StakeOrderCannotUnstake
    );

    // Calculate the amount to transfer: principal + rewards
    let stake_amount = stake_order.stake_amount;
    let token_rewards = stake_order.token_rewards;

    // Mark the order as completed and update the stake pool state.
    stake_order.complete()?;
    stake_pool.complete_order(stake_amount)?;

    stake_pool.token_rewards_pool_balance = stake_pool
        .token_rewards_pool_balance
        .safe_sub(token_rewards)?;
    stake_pool.distributed_token_rewards = stake_pool
        .distributed_token_rewards
        .safe_add(token_rewards)?;

    game.distributed_stake_rewards = game.distributed_stake_rewards.safe_add(token_rewards)?;

    // Transfer tokens from the stake_order_vault back to the player's token_account.
    // This returns the player's initial staked tokens plus accrued rewards.
    transfer_from_token_vault_to_token_account(
        stake_order,
        stake_order_vault,
        token_account,
        token_program,
        stake_amount,
        &[
            STAKE_ORDER_SEED,
            player.key().as_ref(),
            order_number.to_le_bytes().as_ref(),
            &[ctx.bumps.stake_order],
        ],
    )?;

    // Move reward tokens from the order vault to the pool vault, finalizing the staking process
    transfer_from_token_vault_to_token_account(
        stake_pool,
        stake_pool_token_vault,
        token_account,
        token_program,
        token_rewards,
        &[STAKE_POOL_SEED, &[ctx.bumps.stake_pool]],
    )?;

    game.increment_event_nonce()?;

    // Emit an event logging the unstake action
    emit!(TransferEvent {
        event_type: EventType::Unstake,
        event_nonce: game.event_nonce,
        data: EventData::Unstake {
            player: player.key(),
            stake_order: stake_order.key(),
            stake_amount,
            token_rewards: stake_order.token_rewards,
            voucher_rewards: stake_order.voucher_rewards,
            stake_pool: stake_pool.key(),
        },
        initiator_type: InitiatorType::STAKE,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
