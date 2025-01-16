use crate::constants::{GAME_SEED, TOKEN_MINT, VAULT_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_player_to_vault};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `InitializeDeposit` instruction sets up a new deposit within the game ecosystem.
/// This deposit will manage staked tokens and reward distributions, allowing players to stake tokens and earn rewards over time.
///
/// # Steps
/// 1. Derive the deposit PDA from `VAULT_SEED`.
/// 2. Create and initialize the `deposit` account with appropriate space and payer.
/// 3. Set up the `deposit_token_vault` associated token account as the deposit's token store.
/// 4. Call `deposit.initialize` to record the token mint and vault references in the deposit.
/// 5. Emit an `InitializeDeposit` event to log this initialization on-chain.
#[derive(Accounts)]
pub struct InitializeVault<'info> {
    /// The authority (signer) authorized to initialize the deposit.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The authority's associated token account from which tokens will be deposited.
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The game account.
    #[account(mut, seeds = [GAME_SEED], bump, has_one = authority @ ErrorCode::AuthorityMismatch)]
    pub game: Box<Account<'info, Game>>,

    /// The stake pool account to be created, managing staking operations and rewards.
    #[account(
        init,
        payer = authority,
        space = 8 + Vault::INIT_SPACE,
        seeds = [VAULT_SEED],
        bump,
    )]
    pub vault: Box<Account<'info, Vault>>,

    /// The token vault associated token account, holding staked tokens and accrued rewards.
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = vault
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    /// The token mint representing the stakeable token.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program for token operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program for creating the deposit_token_vault.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program for account creations and system-level operations.
    pub system_program: Program<'info, System>,
}

/// Executes the `InitializeDeposit` instruction:
///
/// - Creates and initializes the `deposit` and its associated vault.
/// - Calls `deposit.initialize` to set token mint and vault references.
/// - Emits an `InitializeVault` event to record the action on-chain.
pub fn initialize_vault(
    ctx: Context<InitializeVault>,
    token_mint: Pubkey,
    token_amount: u64,
) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and internal logic.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let InitializeVault {
        game,
        authority,
        vault,
        token_account,
        token_vault,
        token_program,
        ..
    } = ctx.accounts;

    require!(
        token_account.amount >= token_amount,
        ErrorCode::InsufficientFunds
    );

    // Initialize the stake pool with the given token mint and vault
    vault.initialize(token_mint, token_vault.key(), token_amount)?;

    // Transfer tokens from the authority's token account to the game vault.
    transfer_from_player_to_vault(
        authority,
        token_account,
        token_vault,
        token_program,
        token_amount,
    )?;

    game.increment_event_nonce()?;

    // Emit an event logging the stake pool initialization
    emit!(TransferEvent {
        event_type: EventType::InitializeVault,
        event_nonce: game.event_nonce,
        data: EventData::InitializeVault {
            vault: vault.key(),
            token_mint: token_mint.key(),
            token_vault: token_vault.key(),
            token_amount,
        },
        initiator_type: InitiatorType::DEPOSIT,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}
