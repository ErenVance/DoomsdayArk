use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `LeaveTeam` instruction allows a player to leave their current team and return to the default team.
/// This might be used by players who want to switch teams or temporarily opt out from their current team.
///
/// Steps:
/// 1. Ensure that the player leaving is not the team captain, as a captain cannot leave their own team.
/// 2. Remove the player from the current team's member list.
/// 3. Update the player's data to reflect that they have left the team and apply a cooldown period before they can join another team.
/// 4. Emit a `LeaveTeam` event to record the action on-chain.
#[derive(Accounts)]
pub struct LeaveTeam<'info> {
    /// The player leaving the team. Must be the transaction signer.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The team that the player is leaving.
    /// Will be mutated to remove the player's membership.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The player's data account, indicating their current team membership and other personal state.
    /// Verified by `seeds` to ensure correct association and `has_one = team` to confirm the player's current team.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = team
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The global game account, providing default team references and other configuration.
    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,
}

pub fn leave_team(ctx: Context<LeaveTeam>) -> Result<()> {
    // Retrieve the current UNIX timestamp for record keeping and cooldown calculations
    let timestamp = to_timestamp_u64(Clock::get()?.unix_timestamp)?;

    // Extract the relevant accounts for clarity
    let LeaveTeam {
        player,
        player_data,
        team,
        game,
        ..
    } = ctx.accounts;

    // Ensure the player is not the team captain, as the captain cannot leave the team
    require!(
        team.captain != player.key(),
        ErrorCode::TeamCaptainCannotLeave
    );

    // Remove the player from the team's member list
    team.remove_member(player.key())?;

    // Update the player's data to return them to the default team and set a cooldown period
    // preventing immediate reapplication to another team.
    player_data.leave_team(
        game.default_team,
        timestamp + game.team_join_cooldown_seconds,
    )?;

    game.increment_event_nonce()?;
    // Emit an event logging the player's departure from the team
    emit!(TransferEvent {
        event_type: EventType::LeaveTeam,
        event_nonce: game.event_nonce,
        data: EventData::LeaveTeam {
            player: player.key(),
            team: team.key(),
        },
        initiator_type: InitiatorType::TEAM,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
