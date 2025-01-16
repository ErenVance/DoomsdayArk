use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `RemoveMemberFromTeam` instruction enables a team captain or manager to forcibly remove a member from the team.
/// Removal might be necessary for inactive members, disruptive players, or reassignments.
/// If the member being removed is a manager, only the team captain can perform this action.
///
/// Steps:
/// 1. Check that the caller (manager) is either a manager or the captain, ensuring proper authority.
/// 2. Prevent the caller from removing themselves, maintaining logical consistency.
/// 3. If removing a manager, ensure the caller is the captain, since only the captain can remove managers.
/// 4. Remove the member from the team's member list and update their player data to revert them to the default team, applying a cooldown before rejoining any team.
/// 5. Emit a `RemoveMemberFromTeam` event recording the action on-chain.
#[derive(Accounts)]
#[instruction(member_to_remove: Pubkey)]
pub struct RemoveMemberFromTeam<'info> {
    /// The individual executing the removal (a manager or the captain). Must sign the transaction.
    #[account(mut)]
    pub manager: Signer<'info>,

    /// The team from which a member is being removed.
    /// This account is mutated to reflect the updated member list.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The player data of the member being removed.
    /// Verified by `has_one = team` to ensure the member currently belongs to this team.
    /// The constraint `member_to_remove_data.team != game.default_team` ensures the member is truly part of this team rather than the default team.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, member_to_remove.as_ref()],
        bump,
        has_one = team,
        constraint = member_to_remove_data.team != game.default_team
    )]
    pub member_to_remove_data: Box<Account<'info, PlayerData>>,

    /// The global game account, providing reference to the `default_team` and other configurations.
    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,
}

pub fn remove_member_from_team(
    ctx: Context<RemoveMemberFromTeam>,
    member_to_remove: Pubkey,
) -> Result<()> {
    // Obtain the current UNIX timestamp for logging and cooldown calculations
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let RemoveMemberFromTeam {
        team,
        manager,
        member_to_remove_data,
        game,
        ..
    } = ctx.accounts;

    // Ensure the caller is authorized (captain or manager)
    require!(
        team.is_captain_or_manager(manager.key()),
        ErrorCode::NotAuthorized
    );

    // Prevent self-removal for logical consistency
    require!(
        member_to_remove != manager.key(),
        ErrorCode::CannotRemoveSelf
    );

    // If the target is a manager, only the captain can remove them
    if team.is_manager(member_to_remove) {
        require!(
            team.is_captain(manager.key()),
            ErrorCode::RemoveManagerMustBeCaptain
        );
    }

    // Remove the member from the team
    team.remove_member(member_to_remove)?;

    // Update the removed member's player data to reflect that they have left the team
    // and apply a cooldown period before they can join another team.
    member_to_remove_data.leave_team(
        game.default_team,
        timestamp + game.team_join_cooldown_seconds,
    )?;

    game.increment_event_nonce()?;

    // Emit an event noting the member removal action
    emit!(TransferEvent {
        event_type: EventType::RemoveMemberFromTeam,
        event_nonce: game.event_nonce,
        data: EventData::RemoveMemberFromTeam {
            member: member_to_remove,
            team: team.key(),
        },
        initiator_type: InitiatorType::TEAM,
        initiator: manager.key(),
        timestamp,
    });

    Ok(())
}
