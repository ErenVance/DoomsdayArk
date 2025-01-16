use crate::constants::{GAME_SEED, SUPER_ADMIN, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::Game;
use crate::utils::{to_timestamp_u64, transfer_from_player_to_vault};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `Initialize` instruction sets up the initial game state and its main token vault.
/// By calling this instruction, the authorized admin creates and configures the `game` account,
/// and establishes a vault for holding the main token that will be used throughout the game's operations.
///
/// # Steps
/// 1. Create and initialize the `game` account using `GAME_SEED`.
/// 2. Set up the `game_vault` as an associated token account for storing in-game tokens.
/// 3. Link the `authority` and `token_mint` to the `game`.
/// 4. Call `game.initialize` to record initial configuration within the `game` state.
/// 5. Emit an `Initialize` event to log the successful initialization.
#[derive(Accounts)]
pub struct Initialize<'info> {
    /// The admin (signer) authorized to initialize the game.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The game account to be created and initialized.
    #[account(
        init,
        payer = authority,
        space = 8 + Game::INIT_SPACE,
        seeds = [GAME_SEED],
        bump,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The game's token vault, initialized if needed, for holding tokens utilized by the game.
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = game
    )]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The main token mint for the in-game currency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The authority's associated token account from which tokens will be deposited.
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The SPL token program enabling token operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program used for creating the `game_vault`.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program required for account creations and system-level operations.
    pub system_program: Program<'info, System>,
}

/// Executes the `Initialize` instruction:
///
/// - Creates and configures the `game` account.
/// - Sets up `game_vault` as the associated token account for the `game`.
/// - Calls `game.initialize` to record the authority and token mint.
/// - Emits an `Initialize` event, providing an on-chain record of the initialization.
pub fn initialize(
    ctx: Context<Initialize>,
    bot_authority: Pubkey,
    round_rewards: u64,
    period_rewards: u64,
    registration_rewards: u64,
    airdrop_rewards: u64,
    exit_rewards: u64,
    lottery_rewards: u64,
    consumption_rewards: u64,
    sugar_rush_rewards: u64,
) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and configuration reference.
    let timestamp = to_timestamp_u64(Clock::get()?.unix_timestamp)?;

    let Initialize {
        game,
        authority,
        game_vault,
        token_mint,
        token_program,
        token_account,
        ..
    } = ctx.accounts;

    let increase_amount = round_rewards
        .safe_add(period_rewards)?
        .safe_add(registration_rewards)?
        .safe_add(airdrop_rewards)?
        .safe_add(exit_rewards)?
        .safe_add(lottery_rewards)?
        .safe_add(consumption_rewards)?
        .safe_add(sugar_rush_rewards)?;

    require!(
        increase_amount <= token_account.amount,
        ErrorCode::InsufficientFunds
    );

    // Initialize the game account with authority, token_mint, and game_vault
    game.initialize(
        SUPER_ADMIN,
        bot_authority,
        token_mint.key(),
        game_vault.key(),
        round_rewards,
        period_rewards,
        registration_rewards,
        airdrop_rewards,
        exit_rewards,
        lottery_rewards,
        consumption_rewards,
        sugar_rush_rewards,
    )?;

    // Transfer tokens from the authority's token account to the game vault.
    transfer_from_player_to_vault(
        authority,
        token_account,
        game_vault,
        token_program,
        increase_amount,
    )?;

    game.increment_event_nonce()?;

    // Emit an initialization event to log the game setup.
    emit!(TransferEvent {
        event_type: EventType::Initialize,
        event_nonce: game.event_nonce,
        data: EventData::Initialize { game: game.key() },
        initiator_type: InitiatorType::SYSTEM,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}
