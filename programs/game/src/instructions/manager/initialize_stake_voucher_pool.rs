use crate::constants::{GAME_SEED, STAKE_POOL_SEED, TOKEN_MINT, VOUCHER_MINT_SEED, VOUCHER_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_player_to_vault};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, mint_to, Mint, MintTo, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

///
/// The `InitializeStakeVoucherPool` instruction sets up a new stake pool within the game ecosystem.
/// This stake pool will manage staked tokens and reward distributions, allowing players to stake tokens and earn rewards over time.
///
/// # Steps
/// 1. Derive the stake pool PDA from `STAKE_POOL_SEED`.
/// 2. Create and initialize the `stake_pool` account with appropriate space and payer.
/// 3. Set up the `stake_pool_token_vault` associated token account as the stake pool's token store.
/// 4. Call `stake_pool.initialize` to record the token mint and vault references in the stake pool.
/// 5. Emit an `InitializeStakeVoucherPool` event to log this initialization on-chain.
#[derive(Accounts)]
pub struct InitializeStakeVoucherPool<'info> {
    /// The authority (signer) authorized to initialize the stake pool.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The game account.
    #[account(mut, seeds = [GAME_SEED], bump, has_one = authority @ ErrorCode::AuthorityMismatch)]
    pub game: Box<Account<'info, Game>>,

    /// The stake pool account to be created, managing staking operations and rewards.
    #[account(
        mut,
        seeds = [STAKE_POOL_SEED],
        bump,
    )]
    pub stake_pool: Box<Account<'info, StakePool>>,

    /// The stake pool voucher vault associated token account, holding staked tokens and accrued rewards.
    #[account(
        init,
        payer = authority,
        associated_token::mint = voucher_mint,
        associated_token::authority = stake_pool
    )]
    pub stake_pool_voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The authority's associated token account from which tokens will be deposited.
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The global voucher state, maintaining minted vouchers and total supply.
    /// Verified by `seeds` ensuring uniqueness.
    #[account(
        mut,
        seeds = [VOUCHER_SEED],
        bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The voucher vault account holding voucher tokens or related assets.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The voucher mint account, from which vouchers are minted and sent to the player's voucher_account.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

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

/// Executes the `InitializeStakeVoucherPool` instruction:
///
/// - Creates and initializes the `stake_pool` and its associated vault.
/// - Calls `stake_pool.initialize` to set token mint and vault references.
/// - Emits an `InitializeStakeVoucherPool` event to record the action on-chain.
pub fn initialize_stake_voucher_pool(
    ctx: Context<InitializeStakeVoucherPool>,
    voucher_rewards: u64,
) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and internal logic.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let InitializeStakeVoucherPool {
        game,
        authority,
        stake_pool,
        stake_pool_voucher_vault,
        token_account,
        voucher,
        voucher_mint,
        voucher_vault,
        token_program,
        ..
    } = ctx.accounts;

    require!(
        token_account.amount >= voucher_rewards,
        ErrorCode::InsufficientFunds
    );

    // Initialize the stake pool with the given token mint and vault
    stake_pool.initialize_voucher_pool(stake_pool_voucher_vault.key(), voucher_rewards)?;

    // Transfer tokens from the authority's token account to the game vault.
    transfer_from_player_to_vault(
        authority,
        token_account,
        voucher_vault,
        token_program,
        voucher_rewards,
    )?;

    // Update voucher state to reflect newly minted vouchers
    voucher.mint(voucher_rewards)?;

    // Mint voucher tokens into the player's voucher account
    mint_to(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            MintTo {
                mint: voucher_mint.to_account_info(),
                to: stake_pool_voucher_vault.to_account_info(),
                authority: voucher.to_account_info(),
            },
            &[&[VOUCHER_SEED, &[ctx.bumps.voucher]]],
        ),
        voucher_rewards,
    )?;

    game.increment_event_nonce()?;

    // Emit an event logging the stake pool initialization
    emit!(TransferEvent {
        event_type: EventType::InitializeStakeVoucherPool,
        event_nonce: game.event_nonce,
        data: EventData::InitializeStakeVoucherPool {
            stake_pool: stake_pool.key(),
            voucher: voucher.key(),
        },
        initiator_type: InitiatorType::STAKE,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}
