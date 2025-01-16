use crate::constants::{GAME_SEED, STAKE_POOL_SEED, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_player_to_vault};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

///
/// The `InitializeStakeTokenPool` instruction sets up a new stake pool within the game ecosystem.
/// This stake pool will manage staked tokens and reward distributions, allowing players to stake tokens and earn rewards over time.
///
/// # Steps
/// 1. Derive the stake pool PDA from `STAKE_POOL_SEED`.
/// 2. Create and initialize the `stake_pool` account with appropriate space and payer.
/// 3. Set up the `stake_pool_token_vault` associated token account as the stake pool's token store.
/// 4. Call `stake_pool.initialize` to record the token mint and vault references in the stake pool.
/// 5. Emit an `InitializeStakeTokenPool` event to log this initialization on-chain.
#[derive(Accounts)]
pub struct InitializeStakeTokenPool<'info> {
    /// The authority (signer) authorized to initialize the stake pool.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The game account.
    #[account(mut, seeds = [GAME_SEED], bump, has_one = authority @ ErrorCode::AuthorityMismatch)]
    pub game: Box<Account<'info, Game>>,

    /// The stake pool account to be created, managing staking operations and rewards.
    #[account(
        init,
        payer = authority,
        space = 8 + StakePool::INIT_SPACE,
        seeds = [STAKE_POOL_SEED],
        bump,
    )]
    pub stake_pool: Box<Account<'info, StakePool>>,

    /// The stake pool vault associated token account, holding staked tokens and accrued rewards.
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = stake_pool
    )]
    pub stake_pool_token_vault: Box<Account<'info, TokenAccount>>,

    /// The authority's associated token account from which tokens will be deposited.
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The token mint representing the stakeable token.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program for token operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program for creating the stake_pool_token_vault.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program for account creations and system-level operations.
    pub system_program: Program<'info, System>,
}

/// Executes the `InitializeStakeTokenPool` instruction:
///
/// - Creates and initializes the `stake_pool` and its associated vault.
/// - Calls `stake_pool.initialize` to set token mint and vault references.
/// - Emits an `InitializeStakeTokenPool` event to record the action on-chain.
pub fn initialize_stake_token_pool(
    ctx: Context<InitializeStakeTokenPool>,
    token_rewards: u64,
) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and internal logic.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let InitializeStakeTokenPool {
        game,
        authority,
        stake_pool,
        stake_pool_token_vault,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    require!(
        token_account.amount >= token_rewards,
        ErrorCode::InsufficientFunds
    );

    // Initialize the stake pool with the given token mint and vault
    stake_pool.initialize_token_pool(stake_pool_token_vault.key(), token_rewards)?;

    // Transfer tokens from the authority's token account to the game vault.
    transfer_from_player_to_vault(
        authority,
        token_account,
        stake_pool_token_vault,
        token_program,
        token_rewards,
    )?;

    game.increment_event_nonce()?;

    // Emit an event logging the stake pool initialization
    emit!(TransferEvent {
        event_type: EventType::InitializeStakeTokenPool,
        event_nonce: game.event_nonce,
        data: EventData::InitializeStakeTokenPool {
            stake_pool: stake_pool.key(),
        },
        initiator_type: InitiatorType::STAKE,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}
