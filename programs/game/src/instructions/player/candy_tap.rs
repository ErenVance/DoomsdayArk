use crate::constants::{
    CONSTRUCTION_POOL_SHARE, CONSUMPTION_POOL_SHARE, GAME_SEED, GRAND_PRIZES_POOL_SHARE,
    LOTTERY_POOL_SHARE, PLAYER_DATA_SEED, REFERRAL_POOL_SHARE,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{
    calculate_proportion, to_timestamp_u64, transfer_from_token_vault_to_token_account,
};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `Purchase` instruction enables players to buy ORE tokens within the current round, affecting various in-game pools and distributions.
/// Through this action, players potentially earn wages, access continuous purchase rewards, and contribute to multiple reward pools (bonus, lottery, construction, etc.).
///
/// Steps:
/// 1. Validate that the current round has started and handle edge cases if the round end conditions are met.
/// 2. Ensure the player has sufficient funds (vouchers + tokens) to cover the ORE purchase cost.
/// 3. Calculate proportional allocations (construction, bonus, lottery, developer, referral, and grand prize pools) from the purchase amount.
/// 4. Update round and game-level states, adjusting earnings_per_ore, sold_ores, participant lists, and round end time.
/// 5. Manage player state: update consecutive purchase days, settle pending construction rewards, and adjust ORE holdings and earnings_per_ore.
/// 6. If vouchers are used as payment, burn them and redeem underlying tokens. Also, transfer funds from player accounts to game and round vaults as required.
/// 7. Emit a `Purchase` event to record the transaction on-chain.
#[derive(Accounts)]
#[instruction(last_active_participant: Pubkey)]
pub struct CandyTap<'info> {
    /// The player making the purchase. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, linking to their token and voucher accounts, and indicating their current team.
    #[account(mut, seeds = [PLAYER_DATA_SEED, player.key().as_ref()], bump)]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The global `Game` account, referencing current round, period, and main vault.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = current_round,
        has_one = game_vault,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The current round account, must be active (not ended), referencing its `round_vault`.
    #[account(
        mut,
        constraint = !current_round.is_over @ ErrorCode::RoundAlreadyEnded,
        has_one = round_vault,
    )]
    pub current_round: Box<Account<'info, Round>>,

    /// The referrer's data account, tracking pending referral rewards due to them.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, last_active_participant.as_ref()],
        bump,
        constraint = last_active_participant == current_round.last_active_participant_list[0] @ ErrorCode::WrongLastActiveParticipant,
    )]
    pub last_active_participant_data: Box<Account<'info, PlayerData>>,

    /// The main game vault holding the platform's aggregated funds.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The round-specific vault holding tokens allocated for the current round.
    #[account(mut)]
    pub round_vault: Box<Account<'info, TokenAccount>>,

    /// The SPL Token program used for token operations like minting, burning, and transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// Handles the `Purchase` logic, applying cost calculations, distribution of funds to various pools,
/// updating leaderboards and player states, and managing the round lifecycle if conditions warrant ending the round.
pub fn candy_tap(ctx: Context<CandyTap>, last_active_participant: Pubkey) -> Result<()> {
    // Obtain current Solana time for logic and event logging
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let CandyTap {
        player,
        player_data,
        last_active_participant_data,
        game,
        game_vault,
        current_round,
        round_vault,
        token_program,
        ..
    } = ctx.accounts;

    // Validate that the current round is active (has started)
    require!(
        current_round.start_time <= timestamp,
        ErrorCode::RoundNotStarted
    );

    require!(player_data.available_ores > 0, ErrorCode::NoOresAvailable);

    // Calculate total cost in lamports for the requested ORE quantity
    let elapsed_time =
        timestamp.safe_sub(current_round.last_collected_sugar_rush_reward_timestamp)?;
    let total_cost = game.sugar_rush_rewards_per_second.safe_mul(elapsed_time)?;
    current_round.last_collected_sugar_rush_reward_timestamp = timestamp;

    game.sugar_rush_rewards_pool_balance =
        game.sugar_rush_rewards_pool_balance.safe_sub(total_cost)?;

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
    game.referral_rewards_pool_balance = game
        .referral_rewards_pool_balance
        .safe_add(referral_rewards)?;

    // Update round-level pools
    current_round.grand_prize_pool_balance = current_round
        .grand_prize_pool_balance
        .safe_add(grand_prizes_rewards)?;

    // Update earnings_per_ore in the round
    let available_ores = current_round.available_ores.max(1);
    let earnings_per_ore_increment = construction_rewards.safe_div(available_ores as u64)?;
    current_round.earnings_per_ore = current_round
        .earnings_per_ore
        .safe_add(earnings_per_ore_increment)?;

    // Update round state: sold ORE, participant list, end time
    current_round.update_end_time(timestamp)?;

    // Add referral rewards to the referrer's pending rewards
    last_active_participant_data.collectable_referral_rewards = last_active_participant_data
        .collectable_referral_rewards
        .safe_add(referral_rewards)?;

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
        if player.key() == last_active_participant {
            last_active_participant_data.collectable_consumption_rewards =
                last_active_participant_data
                    .collectable_consumption_rewards
                    .safe_add(consumption_rewards)?;
        } else {
            player_data.collectable_consumption_rewards = player_data
                .collectable_consumption_rewards
                .safe_add(consumption_rewards)?;
        }
        msg!(
            "Player earned {} consumption rewards for spending {} tokens.",
            consumption_rewards,
            total_cost
        );
    }

    // Transfer the initial grand prize amount from game_vault to round_vault.
    transfer_from_token_vault_to_token_account(
        game,
        &game_vault,
        &round_vault,
        &token_program,
        grand_prizes_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    game.increment_event_nonce()?;

    // Emit an event recording the purchase
    emit!(TransferEvent {
        event_type: EventType::CandyTap,
        event_nonce: game.event_nonce,
        data: EventData::CandyTap {
            game: game.key(),
            round: current_round.key(),
            player: player.key(),
            last_active_participant,
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
