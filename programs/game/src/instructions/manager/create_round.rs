use crate::constants::{GAME_SEED, ROUND_SEED, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
pub struct CreateRound<'info> {
    /// The authority (signer) who initiates the creation of a new round.
    #[account(mut)]
    pub bot_authority: Signer<'info>,

    /// The main game account, referencing token mint and main vault.
    /// Must have the correct authority and ensure sufficient initial grand prize pool balance.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = game_vault,
        has_one = bot_authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The main game vault holding tokens allocated for different in-game pools.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The round account to be created. Each round is uniquely derived using `game.round_nonce`.
    #[account(
        init,
        payer = bot_authority,
        space = 8 + Round::INIT_SPACE,
        seeds = [ROUND_SEED, game.round_nonce.to_le_bytes().as_ref()],
        bump,
    )]
    pub round: Box<Account<'info, Round>>,

    /// The token mint representing the in-game currency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The associated token account for the round, serving as the `round_vault`.
    /// Stores tokens specifically allocated to this round.
    #[account(
        init,
        payer = bot_authority,
        associated_token::mint = token_mint,
        associated_token::authority = round
    )]
    pub round_vault: Box<Account<'info, TokenAccount>>,

    /// The associated token program used for creating the `round_vault`.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The SPL token program enabling token transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The system program required for account creations and other operations.
    pub system_program: Program<'info, System>,
}

/// The `create_round` instruction sets up a new game round in the platform.
/// It allocates initial grand prize tokens to the round vault and updates the global state accordingly.
/// This enables players to participate in a fresh round with a defined start time and countdown duration.
///
/// Steps:
/// 1. Validate inputs (e.g., `start_time` >= current time, `countdown_duration` > 0) and ensure the game has sufficient funds.
/// 2. Deduct the `initial_grand_prizes` from the `round_rewards_pool_balance`.
/// 3. Initialize the `Round` account with the provided parameters and increment `round_nonce` in the game account.
/// 4. Transfer the allocated grand prize tokens from `game_vault` to the `round_vault`.
/// 5. Emit a `CreateRound` event to record the creation of the new round on-chain.
pub fn create_round(
    ctx: Context<CreateRound>,
    start_time: u64,
    countdown_duration: u64,
    initial_grand_prizes: u64,
) -> Result<()> {
    // Get the current timestamp for validation and event logging.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let CreateRound {
        game,
        game_vault,
        round,
        round_vault,
        bot_authority,
        token_program,
        ..
    } = ctx.accounts;

    // Validate input parameters and ensure the game has enough resources.
    require!(start_time >= timestamp, ErrorCode::InvalidAmount);
    require!(countdown_duration > 0, ErrorCode::InvalidAmount);
    require!(
        initial_grand_prizes <= game_vault.amount,
        ErrorCode::InsufficientBalance
    );
    require!(
        initial_grand_prizes <= game.round_rewards_pool_balance,
        ErrorCode::InsufficientBalance
    );

    let grand_prizes = initial_grand_prizes.safe_add(game.bonus_rewards_pool_balance)?;

    // Initialize the round with given parameters.
    round.initialize(
        game.round_nonce,
        round_vault.key(),
        grand_prizes,
        start_time,
        countdown_duration,
        game.default_player,
        ctx.bumps.round,
    )?;

    // Update game state: set current_round, deduct initial_grand_prizes, and adjust mining and bonus pool balances.
    game.current_round = round.key();
    game.round_rewards_pool_balance = game
        .round_rewards_pool_balance
        .safe_sub(initial_grand_prizes)?;
    game.bonus_rewards_pool_balance = 0;

    // Transfer the initial grand prize amount from game_vault to round_vault.
    transfer_from_token_vault_to_token_account(
        game,
        &game_vault,
        &round_vault,
        &token_program,
        grand_prizes,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    // Increment the round_nonce for future round derivations.
    game.increment_round_nonce()?;

    game.increment_event_nonce()?;

    // Emit event logging the creation of a new round.
    emit!(TransferEvent {
        event_type: EventType::CreateRound,
        event_nonce: game.event_nonce,
        data: EventData::CreateRound {
            game: game.key(),
            round: round.key(),
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: bot_authority.key(),
        timestamp,
    });

    Ok(())
}
