use crate::constants::{
    CONSTRUCTION_POOL_SHARE, CONSUMPTION_POOL_SHARE, GAME_SEED, GRAND_PRIZES_POOL_SHARE,
    LAMPORTS_PER_ORE, LOTTERY_POOL_SHARE, PLAYER_DATA_SEED, REFERRAL_POOL_SHARE, TOKEN_MINT,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::{Game, Period, PlayerData, Round, Team};
use crate::utils::{
    calculate_proportion, timestamp_to_days, to_timestamp_u64,
    transfer_from_token_vault_to_token_account,
};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, burn, Burn, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
#[instruction(player: Pubkey)]
pub struct AutoReinvest<'info> {
    /// The authority account required to sign this transaction.
    /// Typically an admin or a system authority that triggers auto reinvest actions.
    #[account(mut)]
    pub bot_authority: Signer<'info>,

    /// The main game account, referencing current_round and game_vault.
    /// Also ensures that authority matches the one specified in the game for security.
    #[account(mut,
        seeds = [GAME_SEED], bump,
        has_one = current_round,
        has_one = current_period,
        has_one = game_vault,
        has_one = bot_authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The current round must be ongoing (not ended),
    /// and must have an associated round_vault.
    #[account(mut,
        constraint = !current_round.is_over @ ErrorCode::RoundAlreadyEnded,
        has_one = round_vault,
    )]
    pub current_round: Box<Account<'info, Round>>,

    /// The player's data account, storing pending rewards, current round participation, ORE holdings, etc.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player.as_ref()], bump,
        has_one = team,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The referrer's data account, tracking pending referral rewards due to them.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player_data.referrer.as_ref()],
        bump
    )]
    pub referrer_data: Box<Account<'info, PlayerData>>,

    /// The current period account representing a leaderboard period.
    #[account(mut)]
    pub current_period: Box<Account<'info, Period>>,

    /// The team account the player belongs to, or the default team if none.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The main game vault where aggregated tokens are stored.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The round-specific vault token account.
    #[account(mut)]
    pub round_vault: Box<Account<'info, TokenAccount>>,

    /// The token mint account used for issuing and burning token tokens.
    #[account(mut, address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL token program, enabling token transfers and operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// The `auto_reinvest` instruction allows a designated authority to automatically reinvest a player's accumulated pending rewards into ORE,
/// without requiring the player's direct interaction. This mechanism ensures continuous growth of the player's ORE holdings
/// if they have enabled auto-reinvest.
///
/// Steps:
/// 1. Confirm the round has started and is not ended, and that the player is still participating in this round (not exited).
/// 2. Check that the player has auto-reinvest enabled.
/// 3. Settle any pending construction rewards to determine the final amount available for reinvestment.
/// 4. Calculate how many ORE can be purchased using the player's pending rewards.
/// 5. Ensure at least one ORE is purchased to justify the reinvest action.
/// 6. Compute proportional distributions (construction, bonus, lottery, grand prizes) from the total cost of purchased ORE.
/// 7. Update the round's earnings_per_ore, available_ores, and possibly end_time if needed.
/// 8. Deduct the cost from the player's collectable_construction_rewards, effectively turning them into ORE holdings.
/// 9. Move bonus and lottery portions from the round_vault to the game_vault.
/// 10. Emit an `AutoReinvest` event logging the performed reinvest action.

pub fn auto_reinvest(ctx: Context<AutoReinvest>, player: Pubkey) -> Result<()> {
    // Obtain the current UNIX timestamp for logging and timing checks.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to accounts for clarity.
    let AutoReinvest {
        game,
        current_round,
        player_data,
        referrer_data,
        team,
        current_period,
        game_vault,
        round_vault,
        token_program,
        bot_authority,
        token_mint,
        ..
    } = ctx.accounts;

    // The round must have started.
    require!(
        current_round.start_time <= timestamp,
        ErrorCode::RoundNotStarted
    );

    // The player must be in the current round and not require settling a previous round.
    require!(
        player_data.current_round == current_round.key(),
        ErrorCode::NeedToSettlePreviousRound
    );

    // Player must not have exited already.
    require!(!player_data.is_exited, ErrorCode::PlayerAlreadyExited);

    // Check if auto-reinvesting is enabled for this player.
    require!(
        player_data.is_auto_reinvesting,
        ErrorCode::AutoReinvestNotEnabled
    );

    // Settle pending construction rewards first.
    player_data.settle_collectable_construction_rewards(current_round.earnings_per_ore)?;

    let rewards = player_data.collectable_construction_rewards;

    // Determine how many ORE can be purchased from the player's pending construction rewards.
    let purchased_ores = rewards.safe_mul(2)?.safe_div(LAMPORTS_PER_ORE)? as u32;

    // Ensure at least one ORE can be purchased.
    require!(
        purchased_ores > 0,
        ErrorCode::InsufficientSalaryToAutoReinvest
    );

    let total_cost = LAMPORTS_PER_ORE.safe_mul(purchased_ores as u64)?;
    let half_cost = total_cost.safe_div(2)?;

    // Deduct total_cost from player's collectable_construction_rewards after reinvesting.
    player_data.collectable_construction_rewards = player_data
        .collectable_construction_rewards
        .safe_sub(half_cost)?;

    game.construction_rewards_pool_balance =
        game.construction_rewards_pool_balance.safe_sub(half_cost)?;
    game.bonus_rewards_pool_balance = game.bonus_rewards_pool_balance.safe_sub(half_cost)?;
    game.distributed_construction_rewards =
        game.distributed_construction_rewards.safe_add(half_cost)?;
    game.distributed_bonus_rewards = game.distributed_bonus_rewards.safe_add(half_cost)?;

    // Update the player to reflect they are now in the current round and period
    player_data.current_round = current_round.key();
    if player_data.current_period != current_period.key() {
        player_data.current_period = current_period.key();
        player_data.current_period_purchased_ores = 0;
    }

    // Update consecutive purchase days if needed
    let current_day = timestamp_to_days(timestamp)?;
    if player_data.last_purchased_day != current_day {
        if player_data.last_purchased_day + 1 == current_day {
            player_data.consecutive_purchased_days =
                player_data.consecutive_purchased_days.safe_add(1)?;
        } else {
            player_data.consecutive_purchased_days = 1;
        }
        player_data.last_purchased_day = current_day;
    }

    // Calculate proportional rewards for various pools
    let construction_rewards = calculate_proportion(total_cost, CONSTRUCTION_POOL_SHARE)?;
    let bonus_rewards = construction_rewards;
    let lottery_rewards = calculate_proportion(total_cost, LOTTERY_POOL_SHARE)?;
    let referral_rewards = calculate_proportion(total_cost, REFERRAL_POOL_SHARE)?;
    let grand_prizes_rewards = calculate_proportion(total_cost, GRAND_PRIZES_POOL_SHARE)?;
    let consumption_rewards = calculate_proportion(total_cost, CONSUMPTION_POOL_SHARE)?;
    let developer_rewards = calculate_proportion(total_cost, CONSUMPTION_POOL_SHARE)?;

    // Update game-level pools
    game.construction_rewards_pool_balance = game
        .construction_rewards_pool_balance
        .safe_add(construction_rewards)?;
    game.bonus_rewards_pool_balance = game.bonus_rewards_pool_balance.safe_add(bonus_rewards)?;
    game.lottery_rewards_pool_balance = game
        .lottery_rewards_pool_balance
        .safe_add(lottery_rewards)?;
    if player_data.referrer != game.default_player {
        game.referral_rewards_pool_balance = game
            .referral_rewards_pool_balance
            .safe_add(referral_rewards)?;
    }

    // Update round-level pools
    current_round.grand_prize_pool_balance = current_round
        .grand_prize_pool_balance
        .safe_add(grand_prizes_rewards)?;

    if player_data.referrer != game.default_player {
        // Add referral rewards to the referrer's pending rewards
        referrer_data.collectable_referral_rewards = referrer_data
            .collectable_referral_rewards
            .safe_add(referral_rewards)?;
    }

    // Update earnings_per_ore in the round
    let available_ores = current_round.available_ores.max(1);
    let earnings_per_ore_increment = construction_rewards.safe_div(available_ores as u64)?;
    current_round.earnings_per_ore = current_round
        .earnings_per_ore
        .safe_add(earnings_per_ore_increment)?;

    // Update round state: sold ORE, participant list, end time
    current_round.available_ores = current_round.available_ores.safe_add(purchased_ores)?;
    current_round.sold_ores = current_round.sold_ores.safe_add(purchased_ores)?;
    current_round.update_last_active_participant_list(player.key())?;
    current_round.update_end_time(timestamp)?;

    // Settle any pending construction rewards before adding newly purchased ORE
    player_data.settle_collectable_construction_rewards(current_round.earnings_per_ore)?;

    // Update player ORE holdings and earnings rate
    player_data.available_ores = player_data.available_ores.safe_add(purchased_ores)?;
    player_data.purchased_ores = player_data.purchased_ores.safe_add(purchased_ores)?;

    // If the player is part of a team, update team ORE and period data
    team.update_current_period(current_period.key());
    team.purchased_ores = team.purchased_ores.safe_add(purchased_ores)?;
    team.last_updated_timestamp = timestamp;

    // If the current period is ongoing, update leaderboards
    if current_period.is_ongoing(timestamp) {
        player_data.current_period_purchased_ores = player_data
            .current_period_purchased_ores
            .safe_add(purchased_ores)?;
        current_period
            .update_top_player(player.key(), player_data.current_period_purchased_ores)?;

        team.current_period_purchased_ores = team
            .current_period_purchased_ores
            .safe_add(purchased_ores)?;
        if player_data.team != game.default_team {
            current_period.update_top_team_list(team.key(), team.current_period_purchased_ores)?;
        }
    }

    // If mining pool balance is enough, add developer rewards
    if game.consumption_rewards_pool_balance >= developer_rewards {
        game.consumption_rewards_pool_balance = game
            .consumption_rewards_pool_balance
            .safe_sub(developer_rewards)?;
        game.distributable_consumption_rewards = game
            .distributable_consumption_rewards
            .safe_sub(developer_rewards)?;
        game.developer_rewards_pool_balance = game
            .developer_rewards_pool_balance
            .safe_add(developer_rewards)?;
        msg!(
            "Developer consumption pool increased by {}.",
            developer_rewards
        );
    }

    // If tokens are used (token_cost > 0), add consumption rewards
    if game.distributable_consumption_rewards >= consumption_rewards {
        game.distributable_consumption_rewards = game
            .distributable_consumption_rewards
            .safe_sub(consumption_rewards)?;
        player_data.collectable_consumption_rewards = player_data
            .collectable_consumption_rewards
            .safe_add(consumption_rewards)?;
        msg!(
            "Player earned {} consumption rewards for spending {} tokens.",
            consumption_rewards,
            total_cost
        );
    }

    // Transfer grand prizes rewards from the game_vault to the round_vault, reflecting resource redistribution.
    transfer_from_token_vault_to_token_account(
        game,
        &game_vault,
        &round_vault,
        &token_program,
        grand_prizes_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    if player_data.referrer == game.default_player {
        burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: token_mint.to_account_info(),
                    from: game_vault.to_account_info(),
                    authority: game.to_account_info(),
                },
                &[&[GAME_SEED, &[ctx.bumps.game]]],
            ),
            referral_rewards,
        )?;
    }

    game.increment_event_nonce()?;

    // Emit an AutoReinvest event, logging the reinvest action and purchased ORE count.
    emit!(TransferEvent {
        event_type: EventType::AutoReinvest,
        event_nonce: game.event_nonce,
        data: EventData::AutoReinvest {
            game: game.key(),
            round: current_round.key(),
            period: current_period.key(),
            player,
            team: player_data.team,
            purchased_ores,
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: bot_authority.key(),
        timestamp,
    });

    Ok(())
}
