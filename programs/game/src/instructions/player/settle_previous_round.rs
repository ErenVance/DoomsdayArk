use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
pub struct SettlePreviousRound<'info> {
    // The player who initiates the settlement of the previous round, must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    // The player's data account, storing round info, ORE holdings, rewards, etc.
    // Ensures player is in the current_round, not exited yet, and references player's token account.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()], bump,
        has_one = player,
        has_one = token_account,
        has_one = current_round,
        constraint = !player_data.is_exited @ ErrorCode::PlayerAlreadyExited,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    // The player's token account to which settled rewards will be transferred.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    // The global game account. No special constraints here besides referencing current_round and vaults if needed.
    #[account(mut, seeds = [GAME_SEED], bump, has_one = game_vault)]
    pub game: Box<Account<'info, Game>>,

    // The game's vault token account holding tokens allocated for the settled rewards.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    // The round account must be over to settle it. Ensures that the round vault is accessible.
    #[account(mut,
        constraint = current_round.is_over @ ErrorCode::RoundInProgress,
    )]
    pub current_round: Box<Account<'info, Round>>,

    // The SPL token program enabling token transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// The `settle_previous_round` instruction allows a player who participated in a now-concluded round to finalize their position:
/// - Settle construction rewards (based on player's available ORE and earnings_per_ore).
/// - Clear ORE from the player's holdings and reduce ORE from the round's available supply.
/// - Transfer the settled rewards from the round vault to the player's token account.
/// - Mark the player as exited from that round, enabling them to join new rounds or take other actions.
///
/// Steps:
/// 1. Verify the round has ended and the player is still associated with it.
/// 2. Settle pending construction rewards according to the final earnings_per_ore.
/// 3. Deduct the corresponding ORE from the round and the player's holdings, distributing the earned construction rewards.
/// 4. Transfer these rewards from the round vault to the player's token account.
/// 5. Emit a `SettlePreviousRound` event to record the completion of this settlement action.
pub fn settle_previous_round(ctx: Context<SettlePreviousRound>) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and logical checks.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for cleaner readability.
    let SettlePreviousRound {
        player,
        player_data,
        game,
        game_vault,
        current_round,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    // Settle any pending construction rewards based on current_round.earnings_per_ore.
    player_data.settle_collectable_construction_rewards(current_round.earnings_per_ore)?;

    let construction_rewards = player_data.collectable_construction_rewards;
    player_data.collectable_construction_rewards = player_data
        .collectable_construction_rewards
        .safe_sub(construction_rewards)?;
    let player_available_ores = player_data.available_ores;

    // Deduct construction rewards from the game's construction pool and update distributed metrics.
    game.construction_rewards_pool_balance = game
        .construction_rewards_pool_balance
        .safe_sub(construction_rewards)?;
    game.distributed_construction_rewards = game
        .distributed_construction_rewards
        .safe_add(construction_rewards)?;

    // Ensure the round has enough ORE to cover the player's holdings and reduce it accordingly.
    require!(
        current_round.available_ores >= player_available_ores,
        RoundError::InsufficientOres
    );
    current_round.available_ores = current_round
        .available_ores
        .safe_sub(player_available_ores)?;

    // Update the player's collected construction rewards and mark them as exited from the round.
    player_data.collected_construction_rewards = player_data
        .collected_construction_rewards
        .safe_add(construction_rewards)?;
    player_data.exit_round()?;

    // Transfer the settled construction rewards from the round vault to the player's token account.
    transfer_from_token_vault_to_token_account(
        game,
        &game_vault,
        &token_account,
        &token_program,
        construction_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    msg!("Construction rewards: {}", construction_rewards);

    game.increment_event_nonce()?;

    // Emit a `SettlePreviousRound` event to log the completion of this settlement action.
    emit!(TransferEvent {
        event_type: EventType::SettlePreviousRound,
        event_nonce: game.event_nonce,
        data: EventData::SettlePreviousRound {
            round: current_round.key(),
            player: player.key(),
            available_ores: player_available_ores,
            construction_rewards
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
