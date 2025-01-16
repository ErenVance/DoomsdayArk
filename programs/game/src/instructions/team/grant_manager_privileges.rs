use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `GrantManagerPrivileges` instruction allows the team captain to promote a team member to a manager.
/// Managers can have additional privileges such as accepting team applications, distributing rewards, or other administrative tasks defined by the program.
#[derive(Accounts)]
#[instruction(member: Pubkey)]
pub struct GrantManagerPrivileges<'info> {
    /// The team captain who is granting the manager privileges. Must sign the transaction.
    #[account(mut)]
    pub captain: Signer<'info>,

    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The team account being updated.
    /// Includes references to the captain and the list of members and managers.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The captain's player data account, ensuring the captain is part of this team.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, captain.key().as_ref()],
        bump,
        has_one = team
    )]
    pub captain_data: Box<Account<'info, PlayerData>>,

    /// The player data account of the member receiving manager privileges.
    /// Used to verify that the member is associated with a valid `PlayerData` account.
    #[account(
        seeds = [PLAYER_DATA_SEED, member.as_ref()],
        bump,
    )]
    pub member_data: Box<Account<'info, PlayerData>>,
}

/// Grants manager privileges to a specific team member:
///
/// Steps:
/// 1. Verify that `captain` is indeed the team captain, ensuring they have the authority to modify team roles.
/// 2. Ensure the captain is not granting privileges to themselves, maintaining proper delegation.
/// 3. Update the team account to add this member to the manager list.
/// 4. Emit a `GrantManagerPrivileges` event to log the action on-chain.
pub fn grant_manager_privileges(
    ctx: Context<GrantManagerPrivileges>,
    member: Pubkey,
) -> Result<()> {
    // Get the current UNIX timestamp for event recording
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let GrantManagerPrivileges {
        captain,
        team,
        game,
        ..
    } = ctx.accounts;

    // Ensure the caller is the team captain, granting them authorization to assign managers
    require!(team.is_captain(captain.key()), ErrorCode::NotAuthorized);

    // Ensure the captain is not granting privileges to themselves
    require!(member != captain.key(), ErrorCode::TeamCannotGrantSelf);

    // Grant manager privileges to the specified member
    team.grant_manager_privileges(member)?;

    game.increment_event_nonce()?;

    // Emit an event logging the action
    emit!(TransferEvent {
        event_type: EventType::GrantManagerPrivileges,
        event_nonce: game.event_nonce,
        data: EventData::GrantManagerPrivileges {
            member,
            team: team.key(),
        },
        initiator_type: InitiatorType::TEAM,
        initiator: captain.key(),
        timestamp,
    });

    Ok(())
}
