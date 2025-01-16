use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
pub struct SetIsAutoReinvesting<'info> {
    /// The player enabling auto-reinvest. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, tracking whether auto-reinvest is enabled.
    #[account(mut, seeds = [PLAYER_DATA_SEED, player.key().as_ref()], bump)]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The global game account, referencing the current round.
    #[account(mut,seeds = [GAME_SEED], bump, has_one = current_round)]
    pub game: Box<Account<'info, Game>>,

    /// The current round account, ensuring it is ongoing (not ended).
    #[account(mut,
        constraint = !current_round.is_over @ ErrorCode::RoundAlreadyEnded,
    )]
    pub current_round: Box<Account<'info, Round>>,
}

/// The `set_is_auto_reinvesting` instruction allows a player to enable automatic reinvestment of their earnings.
/// With auto-reinvest enabled, the player's accumulated rewards will be automatically converted into ORE without requiring manual intervention.
///
/// Steps:
/// 1. Ensure the current round is active (not ended).
/// 2. Check that the player does not already have auto-reinvest enabled.
/// 3. Enable auto-reinvest by updating the player_data field.
/// 4. Increment the count of auto-reinvesting players in the current round.
/// 5. Emit a `SetIsAutoReinvesting` event to record this action on-chain.
pub fn set_is_auto_reinvesting(ctx: Context<SetIsAutoReinvesting>) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and logic checks.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let SetIsAutoReinvesting {
        game,
        player,
        player_data,
        current_round,
        ..
    } = ctx.accounts;

    // Player must not have auto-reinvest enabled already
    require!(
        !player_data.is_auto_reinvesting,
        ErrorCode::AutoReinvestAlreadyEnabled
    );

    // Enable auto-reinvest for the player
    player_data.is_auto_reinvesting = true;

    // Increment the auto-reinvesting players count in the current round
    current_round.auto_reinvesting_players = current_round.auto_reinvesting_players.safe_add(1)?;

    game.increment_event_nonce()?;

    // Emit an event to log that the player enabled auto-reinvest
    emit!(TransferEvent {
        event_type: EventType::SetIsAutoReinvesting,
        event_nonce: game.event_nonce,
        data: EventData::SetIsAutoReinvesting {
            player: player.key(),
            round: current_round.key(),
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
