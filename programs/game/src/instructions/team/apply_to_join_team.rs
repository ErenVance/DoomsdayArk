use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `ApplyToJoinTeam` instruction allows a player to request membership in an existing team.
/// This involves two primary actions:
/// 1. Adding the team to the player's application list.
/// 2. Adding the player's public key to the team's application list.
///
/// To mitigate frequent switching and spamming of team applications, a cooldown mechanism (`can_apply_to_team_timestamp`) is enforced.
#[derive(Accounts)]
pub struct ApplyToJoinTeam<'info> {
    /// The target team that the player wishes to join.
    /// This account will be mutated to include the player's application.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The player initiating the team join application.
    /// Must be the signer of the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, which stores personal state such as cooldown timestamps, current team, etc.
    /// Uses `PLAYER_DATA_SEED` and `bump` to ensure correct derivation.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,
}

/// Attempts to apply to join a given team:
///
/// Steps:
/// 1. Verify that the player's cooldown period has passed, ensuring they are allowed to apply again.
/// 2. Add the team to the player's team application list, recording the player's intent to join.
/// 3. Add the player to the team's application list, waiting for captain or manager approval.
/// 4. Emit a `ApplyToJoinTeam` event to record this action on-chain.
pub fn apply_to_join_team(ctx: Context<ApplyToJoinTeam>) -> Result<()> {
    // Get the current UNIX timestamp
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to accounts for ease of use
    let ApplyToJoinTeam {
        game,
        player,
        player_data,
        team,
        ..
    } = ctx.accounts;

    // Enforce the cooldown period to prevent immediate reapplication after leaving a team
    require!(
        player_data.can_apply_to_team_timestamp <= timestamp,
        ErrorCode::TeamJoinCooldown
    );

    // Add the team to the player's application list
    player_data.apply_to_join_team(team.key())?;

    // Add the player to the team's application list
    team.apply_to_join_team(player.key())?;

    game.increment_event_nonce()?;

    // Emit an event capturing the team application action
    emit!(TransferEvent {
        event_type: EventType::ApplyToJoinTeam,
        event_nonce: game.event_nonce,
        data: EventData::ApplyToJoinTeam {
            team: team.key(),
            player: player.key()
        },
        initiator_type: InitiatorType::TEAM,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
