use crate::constants::{GAME_SEED, PLAYER_DATA_SEED, TEAM_SEED, TOKEN_MINT};
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `CreateTeam` instruction enables a player to establish a new team.
/// This involves creating a new `Team` account, initializing its vault, and updating the player's state to indicate that they have formed (and joined) this team as the captain.
#[derive(Accounts)]
pub struct CreateTeam<'info> {
    /// The player who initiates the team creation. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account storing their personal state.
    /// The player must currently be in the default team (`game.default_team`) to form a new team.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        constraint = player_data.team == game.default_team
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The new team account to be created.
    /// Uses `TEAM_SEED` and `game.team_nonce` to generate a unique Program Derived Address (PDA).
    /// Space allocated for `Team` is defined by `Team::INIT_SPACE`.
    #[account(
        init,
        payer = player,
        space = 8 + Team::INIT_SPACE,
        seeds = [TEAM_SEED, game.team_nonce.to_le_bytes().as_ref()],
        bump,
    )]
    pub team: Box<Account<'info, Team>>,

    /// The team vault, an associated token account holding tokens for the team.
    /// Initialized with the `team` as its authority.
    #[account(
        init,
        payer = player,
        associated_token::mint = token_mint,
        associated_token::authority = team
    )]
    pub team_vault: Box<Account<'info, TokenAccount>>,

    /// The global game state, maintaining references to token mint, and the `team_nonce` used to name new teams.
    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The mint for the in-game token. The `game` has a `has_one` relationship ensuring consistency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program, used for token-related operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program, used to create associated token accounts (like `team_vault`).
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program used for account creations and other Solana native operations.
    pub system_program: Program<'info, System>,
}

/// Creates a new team and updates the player's state to reflect their captaincy of this newly formed team:
///
/// Steps:
/// 1. Validate that the player is currently in the default team, ensuring they are "free" to create a new one.
/// 2. Obtain a new `team_number` from `game.team_nonce`, then increment `team_nonce` for future teams.
/// 3. Initialize the new `Team` account, setting the current player as captain and creating a `team_vault`.
/// 4. Update `player_data` so that the player now belongs to the newly created team.
/// 5. Emit a `CreateTeam` event to record the action on-chain.
pub fn create_team(ctx: Context<CreateTeam>) -> Result<()> {
    // Fetch the current UNIX timestamp for record keeping
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to relevant accounts for clarity
    let CreateTeam {
        game,
        player,
        player_data,
        team,
        team_vault,
        ..
    } = ctx.accounts;

    // Assign a unique team number from the game's `team_nonce`
    let team_number = game.team_nonce;
    game.increment_team_nonce()?;

    // Initialize the team with the given number, vault, and the player as the captain
    team.initialize(
        team_number,
        team_vault.key(),
        player.key(),
        timestamp,
        ctx.bumps.team,
    )?;

    // Update the player's data to reflect that they have joined this newly created team as captain
    player_data.join_team(team.key())?;

    game.increment_event_nonce()?;

    // Emit an event logging the creation of a new team
    emit!(TransferEvent {
        event_type: EventType::CreateTeam,
        event_nonce: game.event_nonce,
        data: EventData::CreateTeam {
            team: team.key(),
            player: player.key()
        },
        initiator_type: InitiatorType::TEAM,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
