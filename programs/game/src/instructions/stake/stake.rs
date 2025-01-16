use crate::constants::{
    GAME_SEED, PLAYER_DATA_SEED, STAKE_ORDER_SEED, STAKE_POOL_SEED, TOKEN_MINT,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{
    calculate_proportion, to_timestamp_u64, transfer_from_player_to_vault,
    transfer_from_token_vault_to_token_account,
};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `Stake` instruction allows a player to stake tokens (shards) into the staking pool, create a stake order,
/// and receive voucher tokens representing their staked assets. It involves transferring tokens from the player's
/// account to a newly created stake order vault, then into the pool vault. Additionally, the player receives minted
/// vouchers to confirm their stake, and rewards are allocated based on the configured APR.
#[derive(Accounts)]
pub struct Stake<'info> {
    /// The player initiating the stake, must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The global game account.
    #[account(mut,
        seeds = [GAME_SEED], bump,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The player's data account, storing player-specific state.
    /// Verified by `seeds` and `has_one` constraints ensuring token_account and voucher_account association.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = token_account,
        has_one = voucher_account,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The global staking pool account, maintaining state of APR, total staked amount, and reward distribution.
    /// Verified by `seeds` and associations to `stake_pool_token_vault` and `token_mint`.
    #[account(mut,
        seeds = [STAKE_POOL_SEED],
        bump,
        has_one = stake_pool_voucher_vault
    )]
    pub stake_pool: Box<Account<'info, StakePool>>,

    /// The stake order account to be created for this staking operation.
    /// Represents a single stake position belonging to the player.
    #[account(init,
        payer = player,
        space = 8 + StakeOrder::INIT_SPACE,
        seeds = [STAKE_ORDER_SEED, player.key().as_ref(), player_data.nonce.to_le_bytes().as_ref()],
        bump,
    )]
    pub stake_order: Box<Account<'info, StakeOrder>>,

    /// The associated token account (vault) for the stake order.
    /// Holds the staked tokens for this particular order.
    #[account(
        init,
        payer = player,
        associated_token::mint = token_mint,
        associated_token::authority = stake_order
    )]
    pub stake_order_vault: Box<Account<'info, TokenAccount>>,

    /// The player's token account, from which staked tokens will be deducted.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The player's voucher account, where newly minted vouchers will be credited.
    #[account(mut)]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The stake pool's voucher vault holding the staked assets and available rewards.
    #[account(mut)]
    pub stake_pool_voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The token mint for the stake token.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program, used for token operations like minting and transferring.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program, used for creating associated token accounts (like stake_order_vault).
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program, required for certain account creation operations.
    pub system_program: Program<'info, System>,
}

/// Executes the staking logic:
/// 1. Validates the input `shards_amount`.
/// 2. Converts `shards_amount` into `stake_amount` using predefined constants (`ONE_MILLION` and `LAMPORTS_PER_TOKEN`).
/// 3. Ensures the player has sufficient tokens.
/// 4. Creates a stake order and allocates reward tokens from the pool.
/// 5. Transfers the staked tokens from the player's token account to the `stake_order_vault`,
///    then from `stake_order_vault` to the `stake_pool_token_vault`.
/// 6. Mints voucher tokens to the player's voucher account and moves corresponding tokens to the `voucher_vault`.
/// 7. Emits a `TransferEvent` logging the stake operation.
pub fn stake(ctx: Context<Stake>, shards_amount: u64) -> Result<()> {
    // Fetch the current UNIX timestamp for record keeping
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to accounts for easier manipulation
    let Stake {
        game,
        player,
        player_data,
        stake_pool,
        stake_order,
        stake_order_vault,
        token_account,
        voucher_account,
        stake_pool_voucher_vault,
        token_program,
        ..
    } = ctx.accounts;

    // Validate that the player is staking a positive amount
    require!(shards_amount > 0, ErrorCode::InvalidAmount);

    // Compute one shard and full stake amount in lamports
    let one_shard = stake_pool.one_shard;
    let stake_amount = shards_amount.safe_mul(one_shard)?;

    // Ensure the player has enough tokens in their token account
    require!(
        token_account.amount >= stake_amount,
        ErrorCode::InsufficientFundsToPayFee
    );

    // Use player's nonce as the stake_number for this new order
    let stake_number = player_data.nonce;
    let annual_rate = stake_pool.annual_rate;

    // Allocate rewards for this order and update pool state
    let token_rewards = calculate_proportion(stake_amount, annual_rate)?;
    let voucher_rewards = token_rewards;

    require!(
        token_rewards <= stake_pool.distributable_token_rewards,
        ErrorCode::InsufficientRemainingTokenRewards
    );
    require!(
        voucher_rewards <= stake_pool.voucher_rewards_pool_balance,
        ErrorCode::InsufficientRemainingVoucherRewards
    );

    stake_pool.staked_amount = stake_pool.staked_amount.safe_add(stake_amount)?;
    stake_pool.active_orders = stake_pool.active_orders.safe_add(1)?;

    stake_pool.distributable_token_rewards = stake_pool
        .distributable_token_rewards
        .safe_sub(token_rewards)?;
    stake_pool.voucher_rewards_pool_balance = stake_pool
        .voucher_rewards_pool_balance
        .safe_sub(voucher_rewards)?;
    stake_pool.distributed_voucher_rewards = stake_pool
        .distributed_voucher_rewards
        .safe_add(voucher_rewards)?;

    game.distributed_stake_rewards = game.distributed_stake_rewards.safe_add(voucher_rewards)?;

    // Initialize the stake order with the calculated values and vault info
    stake_order.initialize(
        stake_number,
        stake_amount,
        annual_rate,
        stake_pool.lock_duration,
        token_rewards,
        voucher_rewards,
        stake_order_vault.key(),
        timestamp,
        ctx.bumps.stake_order,
    )?;

    // Increment the player_data nonce to ensure uniqueness for future orders
    player_data.increment_nonce()?;

    // Transfer the stake_amount from player's token account to the order vault
    transfer_from_player_to_vault(
        player,
        token_account,
        stake_order_vault,
        token_program,
        stake_amount,
    )?;

    // Transfer the equivalent of staked tokens from the pool vault to the voucher vault,
    // representing the locked value behind the vouchers just minted.
    transfer_from_token_vault_to_token_account(
        stake_pool,
        stake_pool_voucher_vault,
        voucher_account,
        token_program,
        voucher_rewards,
        &[STAKE_POOL_SEED, &[ctx.bumps.stake_pool]],
    )?;

    game.increment_event_nonce()?;

    // Emit an event to record the staking action on-chain
    emit!(TransferEvent {
        event_type: EventType::Stake,
        event_nonce: game.event_nonce,
        data: EventData::Stake {
            player: player.key(),
            stake_order: stake_order.key(),
            stake_pool: stake_pool.key(),
            stake_amount,
            annual_rate,
            lock_duration: stake_pool.lock_duration,
            token_rewards,
            voucher_rewards,
        },
        initiator_type: InitiatorType::STAKE,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
