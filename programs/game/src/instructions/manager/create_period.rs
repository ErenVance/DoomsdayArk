use crate::constants::{GAME_SEED, PERIOD_SEED, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::{Game, Period};
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
pub struct CreatePeriod<'info> {
    /// The authority initializing the period, must sign the transaction.
    #[account(mut)]
    pub bot_authority: Signer<'info>,

    /// The global game account, providing references to token mint, game vault, and current settings.
    /// Ensures correct authority is used and that initial reward pools are available.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = game_vault,
        has_one = bot_authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The period account to be created, storing leaderboard duration, rewards distribution, etc.
    /// Initialized with payer = bot_authority, ensuring the period is linked to the game via period_nonce.
    #[account(
        init,
        payer = bot_authority,
        space = 8 + Period::INIT_SPACE,
        seeds = [PERIOD_SEED, game.period_nonce.to_le_bytes().as_ref()],
        bump,
    )]
    pub period: Box<Account<'info, Period>>,

    /// The main game vault token account holding tokens for various distributions.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The period's associated token vault, created to hold tokens allocated for this period's rewards.
    #[account(
        init,
        payer = bot_authority,
        associated_token::mint = token_mint,
        associated_token::authority = period
    )]
    pub period_vault: Box<Account<'info, TokenAccount>>,

    /// The token mint representing the in-game currency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The associated token program used to create the period_vault account.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The SPL token program, enabling token transfers and related operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The system program for basic Solana operations, required for account initializations.
    pub system_program: Program<'info, System>,
}

/// The `create_period` instruction sets up a new leaderboard period within the game.
/// It allocates a portion of the initial period leaderboard reward pool into a newly created period,
/// defining start times, reward pools (team and individual), and associated token vaults.
///
/// Steps:
/// 1. Validate that the authority is authorized and that the game has sufficient reward balances.
/// 2. Ensure start_time is valid and that requested `team_rewards` and `individual_rewards` are non-zero.
/// 3. Subtract the total allocated rewards from the game's `period_rewards_pool_balance`.
/// 4. Initialize the `Period` account with the provided parameters and increment `period_nonce` in the `game`.
/// 5. Transfer the allocated rewards from `game_vault` to `period_vault`.
/// 6. Emit a `CreatePeriod` event to log the new period creation.

pub fn create_period(
    ctx: Context<CreatePeriod>,
    start_time: u64,
    leaderboard_duration: u64,
    team_rewards: u64,
    individual_rewards: u64,
) -> Result<()> {
    // Fetch current UNIX timestamp for logical checks and event timestamping.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let CreatePeriod {
        bot_authority,
        game,
        period,
        period_vault,
        game_vault,
        token_program,
        ..
    } = ctx.accounts;

    // Validate input parameters and ensure sufficient game resources.
    require!(team_rewards > 0, ErrorCode::InvalidAmount);
    require!(individual_rewards > 0, ErrorCode::InvalidAmount);
    require!(start_time >= timestamp, ErrorCode::InvalidAmount);

    let total_rewards = team_rewards.safe_add(individual_rewards)?;
    require!(
        total_rewards <= game_vault.amount,
        ErrorCode::InsufficientFunds
    );
    require!(
        total_rewards <= game.period_rewards_pool_balance,
        ErrorCode::InsufficientFunds
    );

    // Update game state: set current_period and deduct from initial leaderboard reward pool.
    game.current_period = period.key();
    game.period_rewards_pool_balance = game.period_rewards_pool_balance.safe_sub(total_rewards)?;

    // Initialize the period account with provided arguments.
    period.initialize(
        game.period_nonce,
        period_vault.key(),
        start_time,
        leaderboard_duration,
        team_rewards,
        individual_rewards,
        game.default_player,
        game.default_team,
        ctx.bumps.period,
    )?;

    // Increment period_nonce for future period derivations.
    game.increment_period_nonce()?;

    // Transfer the allocated rewards from game_vault to period_vault.
    transfer_from_token_vault_to_token_account(
        game,
        game_vault,
        period_vault,
        token_program,
        total_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    game.increment_event_nonce()?;

    // Emit the event logging period creation.
    emit!(TransferEvent {
        event_type: EventType::CreatePeriod,
        event_nonce: game.event_nonce,
        data: EventData::CreatePeriod {
            game: game.key(),
            period: period.key(),
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: bot_authority.key(),
        timestamp,
    });

    Ok(())
}
