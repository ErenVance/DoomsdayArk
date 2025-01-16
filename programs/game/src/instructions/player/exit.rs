use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;
use std::cmp::min;

/// The `Exit` instruction allows a player to voluntarily exit the current round before it concludes.
/// By exiting early, the player claims accumulated construction rewards, bonus rewards, and exit rewards.
/// However, the player forfeits all held ORE (cleared upon exit), losing future earnings and grand prize eligibility.
///
/// Steps:
/// 1. Verify the current round is ongoing and the player is participating in it.
/// 2. Check that the player has ORE to justify an exit (no ORE means no need to exit).
/// 3. Settle any pending construction rewards based on the round's current earnings rate.
/// 4. Calculate and distribute construction rewards, bonus rewards, and exit rewards from the respective pools.
/// 5. Deduct the player's ORE from the round's available ORE and update the round's end time if necessary.
/// 6. Mark the player as exited, reset their round-related data, and transfer all due rewards to the player's token account.
/// 7. Emit an `Exit` event to log the action on-chain.
#[derive(Accounts)]
pub struct Exit<'info> {
    /// The global game account referencing the current round and main vault.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = game_vault,
        has_one = current_round,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The main game vault holding tokens used for various rewards and pools.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The current round account, ensuring the round is active (not ended).
    #[account(
        mut,
        constraint = !current_round.is_over @ ErrorCode::RoundAlreadyEnded,
    )]
    pub current_round: Box<Account<'info, Round>>,

    /// The player exiting the construction. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, referencing their token account and ensuring proper association.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = token_account,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The player's token account to which rewards will be transferred.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The SPL token program used for transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

pub fn exit(ctx: Context<Exit>) -> Result<()> {
    // Obtain the current UNIX timestamp to confirm round timing and event logging.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let Exit {
        game,
        current_round,
        player,
        player_data,
        game_vault,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    // Ensure the current round has started
    require!(
        current_round.start_time <= timestamp,
        ErrorCode::RoundNotStarted
    );

    // The player must be part of this ongoing round and not have a pending settlement from a previous round.
    require!(
        player_data.current_round == current_round.key(),
        ErrorCode::NeedToSettlePreviousRound
    );

    // Check that the player has not already exited
    require!(!player_data.is_exited, ErrorCode::PlayerAlreadyExited);

    // The player must hold ORE to make exiting meaningful
    require!(
        player_data.available_ores > 0,
        ErrorCode::DoNotNeedToExitWithoutOre
    );

    // Settle any pending construction rewards based on the round's current earnings per ORE
    player_data.settle_collectable_construction_rewards(current_round.earnings_per_ore)?;

    let construction_rewards = player_data.collectable_construction_rewards;
    player_data.collectable_construction_rewards = player_data
        .collectable_construction_rewards
        .safe_sub(construction_rewards)?;
    let bonus_rewards = construction_rewards; // The bonus equals the construction rewards
    let available_ores = player_data.available_ores;

    // Deduct construction rewards from the game's construction pool and update distribution
    game.construction_rewards_pool_balance = game
        .construction_rewards_pool_balance
        .safe_sub(construction_rewards)?;
    game.distributed_construction_rewards = game
        .distributed_construction_rewards
        .safe_add(construction_rewards)?;

    // Deduct bonus rewards from the game's bonus pool and record distribution
    game.bonus_rewards_pool_balance = game.bonus_rewards_pool_balance.safe_sub(bonus_rewards)?;
    game.distributed_bonus_rewards = game.distributed_bonus_rewards.safe_add(bonus_rewards)?;

    // Add collected rewards to the player's tally
    player_data.collected_construction_rewards = player_data
        .collected_construction_rewards
        .safe_add(construction_rewards)?
        .safe_add(bonus_rewards)?;

    // Calculate exit rewards based on elapsed time since last collection and ensure no exceedance of pool balance
    let elapsed_time = timestamp.safe_sub(current_round.last_collected_exit_reward_timestamp)?;
    let potential_exit_rewards = game.exit_rewards_per_second.safe_mul(elapsed_time)?;
    let exit_rewards = min(potential_exit_rewards, game.exit_rewards_pool_balance);

    // Update player's collected exit rewards and mark new timestamp
    player_data.collected_exit_rewards =
        player_data.collected_exit_rewards.safe_add(exit_rewards)?;
    current_round.last_collected_exit_reward_timestamp = timestamp;

    // Deduct exit rewards from the game's exit pool and record them as distributed
    game.exit_rewards_pool_balance = game.exit_rewards_pool_balance.safe_sub(exit_rewards)?;
    game.distributed_exit_rewards = game.distributed_exit_rewards.safe_add(exit_rewards)?;

    // Remove the player's ORE from the round's available supply
    require!(
        current_round.available_ores >= available_ores,
        RoundError::InsufficientOres
    );
    current_round.available_ores = current_round.available_ores.safe_sub(available_ores)?;

    // Update the round's end time based on this exit action
    current_round.update_end_time(timestamp)?;

    // Mark the player as exited and reset their round state
    player_data.exit_round()?;

    // Transfer the player's rewards (bonus + exit rewards) from the game vault to player's token account
    transfer_from_token_vault_to_token_account(
        game,
        game_vault,
        token_account,
        token_program,
        construction_rewards
            .safe_add(bonus_rewards)?
            .safe_add(exit_rewards)?,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    msg!("Construction rewards: {}", construction_rewards);
    msg!("Bonus rewards: {}", bonus_rewards);
    if exit_rewards > 0 {
        msg!("Exit rewards: {}", exit_rewards);
    }

    game.increment_event_nonce()?;

    // Emit an event logging the player's exit, including how much ORE they held at exit
    emit!(TransferEvent {
        event_type: EventType::Exit,
        event_nonce: game.event_nonce,
        data: EventData::Exit {
            game: game.key(),
            round: current_round.key(),
            player: player.key(),
            team: player_data.team,
            available_ores,
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
