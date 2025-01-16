use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `AcceptTeamApplication` instruction allows a team captain or manager to accept a player's application to join the team.
/// Upon acceptance, the applicant is removed from the team's application list and added as a member.
#[derive(Accounts)]
#[instruction(applicant: Pubkey)]
pub struct AcceptTeamApplication<'info> {
    /// The authority accepting the application.
    /// Must be either the team captain or a manager, as verified by `team.is_captain_or_manager()`.
    #[account(mut)]
    pub acceptor: Signer<'info>,

    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The team account to which the applicant is requesting membership.
    /// This account is mutated because the application list will be updated and a new member will be added.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The data account associated with the acceptor, ensuring the acceptor is part of this team.
    /// Uses `PLAYER_DATA_SEED` and checks `has_one = team` to confirm the relationship.
    #[account(
        seeds = [PLAYER_DATA_SEED, acceptor.key().as_ref()],
        bump,
        has_one = team
    )]
    pub acceptor_data: Box<Account<'info, PlayerData>>,

    /// The data account of the applicant, representing the player who requested to join this team.
    /// It will be updated to show that the player has joined the team.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, applicant.as_ref()],
        bump,
    )]
    pub applicant_data: Box<Account<'info, PlayerData>>,
}

/// Accepts the team application from the `applicant`, finalizing their addition to the team membership.
///
/// Steps:
/// 1. Verify that the `acceptor` is either the captain or a manager of the `team`.
/// 2. Remove the applicant from the team's application list and add them as a member.
/// 3. Update the `applicant_data` to show that the applicant has joined the team.
/// 4. Emit an event to record that the applicant has successfully joined the team.
pub fn accept_team_application(
    ctx: Context<AcceptTeamApplication>,
    applicant: Pubkey,
) -> Result<()> {
    // Retrieve the current UNIX timestamp
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to relevant accounts for clarity
    let AcceptTeamApplication {
        game,
        acceptor,
        team,
        applicant_data,
        ..
    } = ctx.accounts;

    // Ensure the acceptor is authorized to accept applications
    // The acceptor must be either the captain or a manager within the team.
    require!(
        team.is_captain_or_manager(acceptor.key()),
        ErrorCode::NotAuthorized
    );

    // Accept the applicant: remove them from the application list and insert them into the member list.
    team.accept_team_application(applicant)?;

    // Reflect the applicant's new team membership in their player data
    applicant_data.join_team(team.key())?;

    game.increment_event_nonce()?;

    // Emit an event to record the successful acceptance of the application
    emit!(TransferEvent {
        event_type: EventType::AcceptTeamApplication,
        event_nonce: game.event_nonce,
        data: EventData::AcceptTeamApplication {
            team: team.key(),
            applicant: applicant.key(),
        },
        initiator_type: InitiatorType::TEAM,
        initiator: acceptor.key(),
        timestamp,
    });

    Ok(())
}
