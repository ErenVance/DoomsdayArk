use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `RejectTeamApplication` instruction allows a team's captain or manager to decline a player's application to join the team.
/// Once rejected, the player is removed from the team's application list, and the team is removed from the player's application list.
#[derive(Accounts)]
#[instruction(applicant: Pubkey)]
pub struct RejectTeamApplication<'info> {
    /// The signer rejecting the application. Must be the team captain or a manager.
    #[account(mut)]
    pub rejector: Signer<'info>,

    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The team account from which the applicant is being rejected.
    /// This account is mut because the team's application list will be modified.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The rejector's player data account, ensuring the rejector belongs to this team.
    #[account(
        seeds = [PLAYER_DATA_SEED, rejector.key().as_ref()],
        bump,
        has_one = team
    )]
    pub rejector_data: Box<Account<'info, PlayerData>>,

    /// The player data account of the applicant who is being rejected.
    /// This account is mut because the applicant's application list will be updated.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, applicant.as_ref()],
        bump,
    )]
    pub applicant_data: Box<Account<'info, PlayerData>>,
}

/// Rejects a previously made team application:
///
/// Steps:
/// 1. Verify that the `rejector` is either the team captain or a manager, ensuring the authority to reject applications.
/// 2. Remove the applicant from the team's application list.
/// 3. Remove the team from the applicant's application list.
/// 4. Emit a `RejectTeamApplication` event to record the action on-chain.
pub fn reject_team_application(
    ctx: Context<RejectTeamApplication>,
    applicant: Pubkey,
) -> Result<()> {
    // Get the current UNIX timestamp for event logging
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let RejectTeamApplication {
        rejector,
        applicant_data,
        team,
        game,
        ..
    } = ctx.accounts;

    // Ensure the rejector is authorized (captain or manager)
    require!(
        team.is_captain_or_manager(rejector.key()),
        ErrorCode::NotAuthorized
    );

    // Remove the applicant from the team's application list
    team.reject_team_application(applicant)?;

    // Remove the team from the applicant's application list
    applicant_data.reject_team_application(team.key())?;

    game.increment_event_nonce()?;

    // Emit an event recording the rejection of the team application
    emit!(TransferEvent {
        event_type: EventType::RejectTeamApplication,
        event_nonce: game.event_nonce,
        data: EventData::RejectTeamApplication {
            team: team.key(),
            applicant: applicant.key(),
        },
        initiator_type: InitiatorType::TEAM,
        initiator: rejector.key(),
        timestamp,
    });

    Ok(())
}
