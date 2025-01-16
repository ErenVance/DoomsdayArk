use crate::constants::{GAME_SEED, PLAYER_DATA_SEED, TEAM_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `DistributeTeamRewards` instruction allows the team captain to distribute team-level rewards to a specific team member.
/// This involves transferring a specified `team_rewards` amount from the team vault to the member's token account,
/// and updating both the team and the member's player data to reflect the distribution.
#[derive(Accounts)]
#[instruction(member: Pubkey)]
pub struct DistributeTeamRewards<'info> {
    /// The team account holding references to team resources, including the `team_vault` and the `captain`.
    /// Must have `has_one = captain` and `has_one = team_vault` to ensure consistency.
    #[account(mut,
        has_one = captain @ ErrorCode::AuthorityMismatch,
        has_one = team_vault
    )]
    pub team: Box<Account<'info, Team>>,

    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The captain (signer) of the team who is authorizing the reward distribution.
    /// Must be the team captain to ensure correct authorization.
    #[account(mut)]
    pub captain: Signer<'info>,

    /// The captain's player data account, ensuring that the captain belongs to this team.
    #[account(
        seeds = [PLAYER_DATA_SEED, captain.key().as_ref()],
        bump,
        has_one = team
    )]
    pub captain_data: Box<Account<'info, PlayerData>>,

    /// The member's player data account, who will receive the distributed team rewards.
    /// Must have a `token_account` associated to receive the funds.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, member.as_ref()],
        bump,
        has_one = token_account,
    )]
    pub member_player_data: Box<Account<'info, PlayerData>>,

    /// The team vault token account holding tokens allocated to the team.
    #[account(mut)]
    pub team_vault: Box<Account<'info, TokenAccount>>,

    /// The member's token account where the team rewards will be deposited.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The token program, enabling token-related CPI calls (transfers, etc.).
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// Distributes `team_rewards` amount of tokens from the team vault to a specific team member's token account.
///
/// Steps:
/// 1. Ensure the caller (`captain`) is authorized by verifying their captain role in the team.
/// 2. Update the team's internal records to deduct from the `distributable_team_rewards`.
/// 3. Update the member's player data to record the newly collected team rewards.
/// 4. Transfer the requested `team_rewards` from the `team_vault` to the member's `token_account`.
/// 5. Emit a `DistributeTeamRewards` event to log the transaction on-chain.
pub fn distribute_team_rewards(
    ctx: Context<DistributeTeamRewards>,
    member: Pubkey,
    team_rewards: u64,
) -> Result<()> {
    // Fetch the current UNIX timestamp to record the operation time
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let DistributeTeamRewards {
        game,
        captain,
        member_player_data,
        team,
        team_vault,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    // Update the team's reward pool to reflect the distribution
    team.distribute_team_rewards(team_rewards)?;

    // Add the distributed rewards to the member's collected team rewards
    member_player_data.collect_team_rewards(team_rewards)?;

    // Transfer the specified `team_rewards` tokens from the team vault to the member's token account
    transfer_from_token_vault_to_token_account(
        team,
        &team_vault,
        &token_account,
        &token_program,
        team_rewards,
        &[
            TEAM_SEED,
            team.team_number.to_le_bytes().as_ref(),
            &[team.bump],
        ],
    )?;

    game.increment_event_nonce()?;

    // Emit an event logging the team rewards distribution
    emit!(TransferEvent {
        event_type: EventType::DistributeTeamRewards,
        event_nonce: game.event_nonce,
        data: EventData::DistributeTeamRewards {
            team: team.key(),
            member,
            team_rewards
        },
        initiator_type: InitiatorType::TEAM,
        initiator: captain.key(),
        timestamp,
    });

    Ok(())
}
