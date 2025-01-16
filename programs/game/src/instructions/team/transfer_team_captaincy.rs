use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `TransferTeamCaptaincy` instruction allows the current team captain to transfer their captain role to another member.
/// This can be used to delegate leadership when the current captain becomes inactive or wishes to hand over responsibilities.
///
/// Steps:
/// 1. Verify that the signer is indeed the current team captain.
/// 2. Prevent the captain from transferring the captaincy to themselves.
/// 3. Check that the recipient is a member of the team (handled by team's internal logic).
/// 4. Update the team account to reflect the new captain.
/// 5. Emit a `TransferTeamCaptaincy` event to record this leadership change on-chain.
#[derive(Accounts)]
#[instruction(member: Pubkey)]
pub struct TransferTeamCaptaincy<'info> {
    /// The team account whose captaincy is being transferred.
    /// Mutated to reflect the change in captain.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The current captain, must be the signer of the transaction and cannot be the same as `member`.
    #[account(mut, constraint = captain.key() != member @ ErrorCode::CantTransferToSelf)]
    pub captain: Signer<'info>,

    /// The captain's player data account, ensuring the captain is tied to this team.
    #[account(
        seeds = [PLAYER_DATA_SEED, captain.key().as_ref()],
        bump,
        has_one = team
    )]
    pub captain_player_data: Box<Account<'info, PlayerData>>,

    /// The player data of the new captain, who will receive captaincy.
    /// Must be a member of the team for the transfer to succeed.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, member.as_ref()],
        bump,
    )]
    pub member_player_data: Box<Account<'info, PlayerData>>,
}

pub fn transfer_team_captaincy(ctx: Context<TransferTeamCaptaincy>, member: Pubkey) -> Result<()> {
    // Fetch the current UNIX timestamp for event logging
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let TransferTeamCaptaincy {
        captain,
        team,
        game,
        ..
    } = ctx.accounts;

    // Ensure the caller is indeed the current team captain
    require!(team.is_captain(captain.key()), ErrorCode::NotAuthorized);

    // Perform the captaincy transfer within the team account
    team.transfer_captaincy(member)?;

    game.increment_event_nonce()?;

    // Emit an event logging the leadership change
    emit!(TransferEvent {
        event_type: EventType::TransferTeamCaptaincy,
        event_nonce: game.event_nonce,
        data: EventData::TransferTeamCaptaincy {
            team: team.key(),
            captain: captain.key(),
            new_captain: member,
        },
        initiator_type: InitiatorType::TEAM,
        initiator: captain.key(),
        timestamp,
    });

    Ok(())
}
