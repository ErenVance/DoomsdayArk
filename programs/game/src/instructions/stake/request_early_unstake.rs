use crate::constants::{
    GAME_SEED, PLAYER_DATA_SEED, STAKE_ORDER_SEED, STAKE_POOL_SEED, TOKEN_MINT, VOUCHER_MINT_SEED,
    VOUCHER_SEED,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, burn, Burn, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `RequestEarlyUnstake` instruction enables a player to initiate an early unlock of their staked tokens before the normal lock period ends.
/// Early unlocking comes at a reduced APR, resulting in fewer rewards. This process involves adjusting the stake order, burning vouchers,
/// and reallocating unused rewards back to the game pool.
#[derive(Accounts)]
#[instruction(order_number: u16)]
pub struct RequestEarlyUnstake<'info> {
    /// The global `Game` account, which maintains overarching game state and reward pools.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The player requesting the early unlock, must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, tracking individual player state such as their nonce and voucher account association.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = voucher_account
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The player's voucher token account, holding vouchers representing staked value.
    #[account(mut)]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The stake order to be unlocked early.
    /// Verified by `seeds` to ensure it belongs to the `player`.
    #[account(mut,
        seeds = [STAKE_ORDER_SEED, player.key().as_ref(), order_number.to_le_bytes().as_ref()],
        bump,
    )]
    pub stake_order: Box<Account<'info, StakeOrder>>,

    /// The associated token vault for the stake order, initially holding staked tokens and allocated rewards.
    #[account(mut)]
    pub stake_pool_token_vault: Box<Account<'info, TokenAccount>>,

    /// The global stake pool account managing staking rates, rewards distribution, and total staked amounts.
    #[account(mut,
        seeds = [STAKE_POOL_SEED],
        bump,
        has_one = stake_pool_token_vault,
    )]
    pub stake_pool: Box<Account<'info, StakePool>>,

    /// The global voucher account tracking voucher mint and supply.
    /// Verified by `seeds` and associated with `voucher_vault`.
    #[account(
        mut,
        seeds = [VOUCHER_SEED],
        bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The voucher vault holding tokens that back the voucher supply.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The token mint account used to issue and burn voucher tokens.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The token mint account used to issue and burn underlying tokens.
    #[account(mut, address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program used for all token operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// Processes an early unlock request for a stake order:
///
/// 1. Verifies that the stake order is still locked and not completed.
/// 2. Ensures the player holds enough vouchers corresponding to the staked amount.
/// 3. Adjusts the stake order's APR to the early unlock rate and recalculates rewards based on the elapsed time.
/// 4. Burns the player's vouchers equal to the staked amount and redeems the underlying tokens.
/// 5. Returns unused rewards to the game's mining pool and updates relevant accounts.
/// 6. Emits a `RequestEarlyUnstake` event for record-keeping.
pub fn request_early_unstake(ctx: Context<RequestEarlyUnstake>, order_number: u16) -> Result<()> {
    // Fetch the current UNIX timestamp from the clock sysvar
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to critical accounts for convenience
    let RequestEarlyUnstake {
        game,
        player,
        player_data,
        voucher_account,
        stake_pool,
        stake_order,
        stake_pool_token_vault,
        voucher,
        voucher_vault,
        voucher_mint,
        token_mint,
        token_program,
        ..
    } = ctx.accounts;

    // Validate that the order_number is valid for this player
    require!(
        player_data.nonce >= order_number,
        ErrorCode::StakeOrderNotFound
    );

    // Check that the order is not already completed or early unlocked
    require!(
        !stake_order.is_completed,
        ErrorCode::StakeOrderAlreadyCompleted
    );
    require!(
        !stake_order.is_early_unstaked,
        ErrorCode::StakeOrderAlreadyEarlyUnstaked
    );

    // Verify that the current time is before the natural unlock time, ensuring early unlock conditions apply
    require!(
        timestamp < stake_order.unstaked_timestamp,
        ErrorCode::StakeOrderCannotUnstake
    );

    // Confirm that the player holds enough vouchers corresponding to the staked amount
    require!(
        stake_order.stake_amount <= voucher_account.amount,
        ErrorCode::InsufficientVoucherBalance
    );

    let token_rewards = stake_order.token_rewards;
    let voucher_rewards = stake_order.voucher_rewards;

    // Request early unlock, recomputing rewards at the reduced APR
    stake_order.request_early_unstake(
        timestamp,
        stake_pool.early_unlock_rate,
        stake_pool.early_unlock_duration,
    )?;

    let burned_token_rewards = token_rewards.safe_sub(stake_order.token_rewards)?;
    let burned_voucher_rewards = voucher_rewards.safe_sub(stake_order.voucher_rewards)?;

    game.distributed_stake_rewards = game
        .distributed_stake_rewards
        .safe_sub(burned_voucher_rewards)?;

    stake_pool.distributed_voucher_rewards = stake_pool
        .distributed_voucher_rewards
        .safe_sub(burned_voucher_rewards)?;

    stake_pool.token_rewards_pool_balance = stake_pool
        .token_rewards_pool_balance
        .safe_sub(burned_token_rewards)?;

    stake_pool.burned_token_rewards = stake_pool
        .burned_token_rewards
        .safe_add(burned_token_rewards)?;
    stake_pool.burned_voucher_rewards = stake_pool
        .burned_voucher_rewards
        .safe_add(burned_voucher_rewards)?;

    // Update the voucher state by "burning" the corresponding staked amount (removing vouchers from circulation)
    voucher.burn(burned_voucher_rewards)?;

    burn(
        CpiContext::new(
            token_program.to_account_info(),
            Burn {
                mint: voucher_mint.to_account_info(),
                from: voucher_account.to_account_info(),
                authority: player.to_account_info(),
            },
        ),
        burned_voucher_rewards,
    )?;

    burn(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            Burn {
                mint: token_mint.to_account_info(),
                from: voucher_vault.to_account_info(),
                authority: voucher.to_account_info(),
            },
            &[&[VOUCHER_SEED, &[ctx.bumps.voucher]]],
        ),
        burned_voucher_rewards,
    )?;

    burn(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            Burn {
                mint: token_mint.to_account_info(),
                from: stake_pool_token_vault.to_account_info(),
                authority: stake_pool.to_account_info(),
            },
            &[&[STAKE_POOL_SEED, &[ctx.bumps.stake_pool]]],
        ),
        burned_token_rewards,
    )?;

    game.increment_event_nonce()?;

    // Emit an event capturing the early unlock request
    emit!(TransferEvent {
        event_type: EventType::RequestEarlyUnstake,
        event_nonce: game.event_nonce,
        data: EventData::RequestEarlyUnstake {
            stake_order: stake_order.key(),
            player: player.key(),
            voucher: voucher.key(),
            voucher_rewards,
        },
        initiator_type: InitiatorType::STAKE,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
