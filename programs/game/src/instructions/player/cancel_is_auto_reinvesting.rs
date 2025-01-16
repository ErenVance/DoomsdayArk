use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use solana_program::sysvar::clock::Clock;

/// The `CancelIsAutoReinvesting` instruction allows a player to disable the automatic reinvestment feature.
/// This feature, when enabled, automatically reinvests accrued wages or earnings into ORE without manual interaction.
/// By canceling this setting, the player regains full manual control over their investment actions.
///
/// Steps:
/// 1. Ensure the player is currently set to auto-reinvest (otherwise, there's nothing to cancel).
/// 2. Update the player's data account to disable `is_auto_reinvesting`.
/// 3. Decrement the `auto_reinvesting_players` count in the current round, maintaining accurate round-level statistics.
/// 4. Emit a `CancelIsAutoReinvesting` event to log this action on-chain.
#[derive(Accounts)]
pub struct CancelIsAutoReinvesting<'info> {
    /// The player requesting to cancel auto-reinvestment. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, tracking personal state such as `is_auto_reinvesting`.
    /// Verified by `PLAYER_DATA_SEED` and `bump` for correct derivation.
    #[account(mut, seeds = [PLAYER_DATA_SEED, player.key().as_ref()], bump)]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The global game account, providing references to the current round and other configurations.
    #[account(mut, seeds = [GAME_SEED], bump, has_one = current_round)]
    pub game: Box<Account<'info, Game>>,

    /// The current round account.
    /// Ensures the round is still ongoing (`!current_round.is_over`) to allow changes in player behavior.
    #[account(
        mut,
        constraint = !current_round.is_over @ ErrorCode::RoundAlreadyEnded,
    )]
    pub current_round: Box<Account<'info, Round>>,
}

pub fn cancel_is_auto_reinvesting(ctx: Context<CancelIsAutoReinvesting>) -> Result<()> {
    // Retrieve the current UNIX timestamp for event logging
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to the relevant accounts
    let CancelIsAutoReinvesting {
        game,
        player,
        player_data,
        current_round,
        ..
    } = ctx.accounts;

    // Validate that auto-reinvestment is currently enabled
    require!(
        player_data.is_auto_reinvesting,
        ErrorCode::AutoReinvestNotEnabled
    );

    // Disable auto-reinvestment for this player
    player_data.is_auto_reinvesting = false;

    // Adjust the count of auto-reinvesting players at the round level
    require!(
        current_round.auto_reinvesting_players > 0,
        ErrorCode::InsufficientAutoReinvestPlayers
    );
    current_round.auto_reinvesting_players = current_round.auto_reinvesting_players.safe_sub(1)?;

    game.increment_event_nonce()?;

    // Emit an event to record the cancellation action
    emit!(TransferEvent {
        event_type: EventType::CancelIsAutoReinvesting,
        event_nonce: game.event_nonce,
        data: EventData::CancelIsAutoReinvesting {
            player: player.key(),
            round: current_round.key(),
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
