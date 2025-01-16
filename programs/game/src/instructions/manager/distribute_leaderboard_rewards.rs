use crate::constants::{GAME_SEED, PERIOD_SEED, PLAYER_DATA_SEED, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, burn, Burn, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
#[instruction(player_leaderboard_winner: Pubkey)]
pub struct DistributeLeaderboardRewards<'info> {
    /// The authority (signer) who initiates the leaderboard rewards distribution.
    #[account(mut)]
    pub bot_authority: Signer<'info>,

    /// The global game account, must reference period and token_mint.
    /// Ensures authority matches the one set in game to prevent unauthorized distributions.
    #[account(mut, seeds = [GAME_SEED], bump,
        has_one = bot_authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The current period account associated with a `period_vault`.
    /// It must contain the final leaderboard standings.
    #[account(mut,
        has_one = period_vault,
    )]
    pub period: Box<Account<'info, Period>>,

    /// The period vault token account holding tokens allocated for this period's leaderboard rewards.
    #[account(mut)]
    pub period_vault: Box<Account<'info, TokenAccount>>,

    /// First-place team's account.
    /// Must match the top_team_list\[0\].team
    #[account(mut,
        address = period.top_team_list[0].team,
    )]
    pub team_first: Box<Account<'info, Team>>,

    /// First-place team's vault token account, receiving first-place team rewards.
    #[account(mut,
        address = team_first.team_vault,
    )]
    pub team_first_vault: Box<Account<'info, TokenAccount>>,

    /// Second-place team's account.
    /// Must match top_team_list\[1\].team
    #[account(mut,
        address = period.top_team_list[1].team,
    )]
    pub team_second: Box<Account<'info, Team>>,

    /// Second-place team's vault token account, receiving second-place team rewards.
    #[account(mut,
        address = team_second.team_vault,
    )]
    pub team_second_vault: Box<Account<'info, TokenAccount>>,

    /// Third-place team's account.
    /// Must match top_team_list\[2\].team
    #[account(mut,
        address = period.top_team_list[2].team
    )]
    pub team_third: Box<Account<'info, Team>>,

    /// Third-place team's vault token account, receiving third-place team rewards.
    #[account(mut,
        address = team_third.team_vault,
    )]
    pub team_third_vault: Box<Account<'info, TokenAccount>>,

    /// The top player on the leaderboard (first place individual winner).
    /// Must match top_player_list\[0\].player and reference a valid token_account.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player_leaderboard_winner.as_ref()],
        bump,
        has_one = token_account,
        constraint = player_leaderboard_winner == period.top_player_list[0].player,
    )]
    pub player_leaderboard_winner_data: Box<Account<'info, PlayerData>>,

    /// The token account of the player leaderboard winner, receiving the individual rewards.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The token mint representing the in-game currency.
    #[account(mut, address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program enabling token transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// The `distribute_leaderboard_rewards` instruction finalizes the leaderboard rewards distribution at the end of a period.
/// It awards the top three teams and the top individual player with their respective token amounts from the period_vault,
/// and updates the corresponding team/player data to reflect the newly allocated rewards.
///
/// Steps:
/// 1. Validate that the authority is authorized to perform this action.
/// 2. Check that the period's distributions haven't been completed yet (no repeated reward distribution).
/// 3. Add the respective team rewards to `distributable_team_rewards` for first, second, and third place teams.
/// 4. Update the top player's data by adding the `individual_rewards` to their `collected_individual_rewards`.
/// 5. Mark the rewards as distributed in the period (mark_distribution_completed).
/// 6. Transfer individual and team rewards from the `period_vault` to their respective token accounts.
/// 7. Emit a `DistributeLeaderboardRewards` event logging the distribution details.

pub fn distribute_leaderboard_rewards(
    ctx: Context<DistributeLeaderboardRewards>,
    player_leaderboard_winner: Pubkey,
) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let DistributeLeaderboardRewards {
        bot_authority,
        game,
        period,
        period_vault,
        team_first,
        team_first_vault,
        team_second,
        team_second_vault,
        team_third,
        team_third_vault,
        token_account,
        token_program,
        player_leaderboard_winner_data,
        token_mint,
        ..
    } = ctx.accounts;

    // Mark the period distribution as completed to prevent repeated distributions.
    period.mark_distribution_completed()?;

    if player_leaderboard_winner == game.default_player {
        burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: token_mint.to_account_info(),
                    from: period_vault.to_account_info(),
                    authority: period.to_account_info(),
                },
                &[&[
                    PERIOD_SEED,
                    period.period_number.to_le_bytes().as_ref(),
                    &[period.bump],
                ]],
            ),
            period.individual_rewards,
        )?;
    } else {
        game.distributed_individual_rewards = game
            .distributed_individual_rewards
            .safe_add(period.individual_rewards)?;

        // Add individual rewards to the top player winner's data.
        player_leaderboard_winner_data.collect_individual_rewards(period.individual_rewards)?;

        // Transfer individual rewards to the top player's token account.
        transfer_from_token_vault_to_token_account(
            period,
            &period_vault,
            &token_account,
            &token_program,
            period.individual_rewards,
            &[
                PERIOD_SEED,
                period.period_number.to_le_bytes().as_ref(),
                &[period.bump],
            ],
        )?;
    }

    if period.top_team_list[0].team == game.default_team {
        burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: token_mint.to_account_info(),
                    from: period_vault.to_account_info(),
                    authority: period.to_account_info(),
                },
                &[&[
                    PERIOD_SEED,
                    period.period_number.to_le_bytes().as_ref(),
                    &[period.bump],
                ]],
            ),
            period.team_first_place_rewards,
        )?;
    } else {
        game.distributed_team_rewards = game
            .distributed_team_rewards
            .safe_add(period.team_first_place_rewards)?;

        // Distribute team rewards to the top three teams.
        team_first.distributable_team_rewards = team_first
            .distributable_team_rewards
            .safe_add(period.team_first_place_rewards)?;

        // Transfer first place team rewards.
        transfer_from_token_vault_to_token_account(
            period,
            &period_vault,
            &team_first_vault,
            &token_program,
            period.team_first_place_rewards,
            &[
                PERIOD_SEED,
                period.period_number.to_le_bytes().as_ref(),
                &[period.bump],
            ],
        )?;
    }

    if period.top_team_list[1].team == game.default_team {
        burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: token_mint.to_account_info(),
                    from: period_vault.to_account_info(),
                    authority: period.to_account_info(),
                },
                &[&[
                    PERIOD_SEED,
                    period.period_number.to_le_bytes().as_ref(),
                    &[period.bump],
                ]],
            ),
            period.team_second_place_rewards,
        )?;
    } else {
        game.distributed_team_rewards = game
            .distributed_team_rewards
            .safe_add(period.team_second_place_rewards)?;

        // Distribute team rewards to the top three teams.
        team_second.distributable_team_rewards = team_second
            .distributable_team_rewards
            .safe_add(period.team_second_place_rewards)?;

        // Transfer second place team rewards.
        transfer_from_token_vault_to_token_account(
            period,
            &period_vault,
            &team_second_vault,
            &token_program,
            period.team_second_place_rewards,
            &[
                PERIOD_SEED,
                period.period_number.to_le_bytes().as_ref(),
                &[period.bump],
            ],
        )?;
    }

    if period.top_team_list[2].team == game.default_team {
        burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: token_mint.to_account_info(),
                    from: period_vault.to_account_info(),
                    authority: period.to_account_info(),
                },
                &[&[
                    PERIOD_SEED,
                    period.period_number.to_le_bytes().as_ref(),
                    &[period.bump],
                ]],
            ),
            period.team_third_place_rewards,
        )?;
    } else {
        game.distributed_team_rewards = game
            .distributed_team_rewards
            .safe_add(period.team_third_place_rewards)?;

        // Distribute team rewards to the top three teams.
        team_third.distributable_team_rewards = team_third
            .distributable_team_rewards
            .safe_add(period.team_third_place_rewards)?;

        // Transfer third place team rewards.
        transfer_from_token_vault_to_token_account(
            period,
            &period_vault,
            &team_third_vault,
            &token_program,
            period.team_third_place_rewards,
            &[
                PERIOD_SEED,
                period.period_number.to_le_bytes().as_ref(),
                &[period.bump],
            ],
        )?;
    }

    game.increment_event_nonce()?;

    // Emit event logging the distribution of leaderboard rewards.
    emit!(TransferEvent {
        event_type: EventType::DistributeLeaderboardRewards,
        event_nonce: game.event_nonce,
        data: EventData::DistributeLeaderboardRewards {
            period: period.key(),
            team_first: team_first.key(),
            team_first_place_rewards: period.team_first_place_rewards,
            team_second: team_second.key(),
            team_second_place_rewards: period.team_second_place_rewards,
            team_third: team_third.key(),
            team_third_place_rewards: period.team_third_place_rewards,
            player_leaderboard_winner,
            individual_rewards: period.individual_rewards,
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: bot_authority.key(),
        timestamp,
    });

    Ok(())
}
