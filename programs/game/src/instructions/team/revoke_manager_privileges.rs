use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

/// The `RevokeManagerPrivileges` instruction allows the team captain to remove manager privileges from a specific member.
/// This might be necessary when a manager becomes inactive, violates rules, or if the team's structure changes.
///
/// Steps:
/// 1. Ensure the caller is the team captain, as only they can revoke manager privileges.
/// 2. Prevent the captain from revoking their own privileges, maintaining logical consistency.
/// 3. Remove the specified manager from the team's manager list.
/// 4. Emit a `RevokeManagerPrivileges` event to record the action on-chain.
#[derive(Accounts)]
#[instruction(manager: Pubkey)]
pub struct RevokeManagerPrivileges<'info> {
    /// The captain of the team, who has the authority to revoke manager privileges. Must sign the transaction.
    #[account(mut)]
    pub captain: Signer<'info>,

    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The team account where manager privileges are being revoked.
    /// Mutated to reflect the updated manager list.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The captain's player data account, ensuring the captain belongs to this team.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, captain.key().as_ref()],
        bump,
        has_one = team
    )]
    pub captain_data: Box<Account<'info, PlayerData>>,

    /// The player data account of the manager losing their privileges.
    #[account(
        seeds = [PLAYER_DATA_SEED, manager.as_ref()],
        bump,
    )]
    pub manager_data: Box<Account<'info, PlayerData>>,
}

pub fn revoke_manager_privileges(
    ctx: Context<RevokeManagerPrivileges>,
    manager: Pubkey,
) -> Result<()> {
    // Obtain the current UNIX timestamp to log the event time
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let RevokeManagerPrivileges {
        captain,
        team,
        game,
        ..
    } = ctx.accounts;

    // Ensure the caller is the team captain, giving them authorization to modify manager roles
    require!(team.is_captain(captain.key()), ErrorCode::NotAuthorized);

    // Prevent the captain from revoking their own privileges
    require!(captain.key() != manager, ErrorCode::CannotRemoveSelf);

    // Remove the specified manager from the team's manager list
    team.revoke_manager_privileges(manager)?;

    game.increment_event_nonce()?;

    // Emit an event to record the revocation action
    emit!(TransferEvent {
        event_type: EventType::RevokeManagerPrivileges,
        event_nonce: game.event_nonce,
        data: EventData::RevokeManagerPrivileges {
            manager,
            team: team.key(),
        },
        initiator_type: InitiatorType::TEAM,
        initiator: captain.key(),
        timestamp,
    });

    Ok(())
}
