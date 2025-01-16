use crate::constants::{GAME_SEED, TEAM_SEED, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
pub struct InitializeDefaultTeam<'info> {
    /// The authority (signer) who is authorized to initialize the default team.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The global game account, which ensures the authority matches and references token_mint.
    /// Used to increment team_nonce and set the default_team.
    #[account(
        mut,
        seeds = [GAME_SEED], bump,
        has_one = authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The default team account to be initialized, derived from TEAM_SEED and game.team_nonce.
    #[account(
        init,
        payer = authority,
        space = 8 + Team::INIT_SPACE,
        seeds = [TEAM_SEED, game.team_nonce.to_le_bytes().as_ref()],
        bump,
    )]
    pub default_team: Box<Account<'info, Team>>,

    /// The default team's token vault, created as an associated token account for the team.
    #[account(
        init,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = default_team
    )]
    pub default_team_vault: Box<Account<'info, TokenAccount>>,

    /// The token mint representing the in-game currency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program used for token transfers and related operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program used for creating the default_team_vault.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program required for account creations and other operations.
    pub system_program: Program<'info, System>,
}

/// The `initialize_default_team` instruction sets up a default team within the game ecosystem.
/// This team can serve as a baseline or fallback team, ensuring all players have a reference point if they have not joined a custom team.
///
/// Steps:
/// 1. Obtain the `default_team_number` from `game.team_nonce` and increment this nonce after creating the team.
/// 2. Initialize the `default_team` account with the provided timestamp, vault, and default player.
/// 3. Update `game.default_team` to reference this newly created default team.
/// 4. Emit an `InitializeDefaultTeam` event to log the team creation.
pub fn initialize_default_team(ctx: Context<InitializeDefaultTeam>) -> Result<()> {
    // Obtain current UNIX timestamp for event logging and reference.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let InitializeDefaultTeam {
        authority,
        default_team,
        default_team_vault,
        game,
        ..
    } = ctx.accounts;

    // Assign a unique team_number from game.team_nonce and increment it for future teams.
    let default_team_number = game.team_nonce;
    game.increment_team_nonce()?;

    // Initialize the team with the default player as captain.
    default_team.initialize(
        default_team_number,
        default_team_vault.key(),
        game.default_player,
        timestamp,
        ctx.bumps.default_team,
    )?;

    // Set the game's default_team to the newly created team.
    game.default_team = default_team.key();

    game.increment_event_nonce()?;

    // Emit an event logging the creation of the default team.
    emit!(TransferEvent {
        event_type: EventType::InitializeDefaultTeam,
        event_nonce: game.event_nonce,
        data: EventData::InitializeDefaultTeam {
            team: default_team.key(),
            team_number: default_team_number,
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}
