use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
#[instruction(referrer: Pubkey)]
pub struct SetReferrer<'info> {
    /// The global game account. Verified by seeds and bump, no additional constraints needed here.
    #[account(mut,seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The player setting their referrer. Must sign the transaction.
    /// The chosen referrer cannot be the same as the player's own key (no self-referral).
    #[account(mut, constraint = referrer != player.key() @ ErrorCode::CannotReferSelf)]
    pub player: Signer<'info>,

    /// The player's data account, must currently have the default referrer (no referrer set yet).
    /// This ensures that a referrer is only set once.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        constraint = player_data.referrer == game.default_player @ ErrorCode::ReferrerAlreadySet
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The referrer's data account, to increment their referral count once this relationship is established.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, referrer.as_ref()],
        bump
    )]
    pub referrer_data: Box<Account<'info, PlayerData>>,
}

/// The `set_referrer` instruction allows a player to assign a referrer for the first time.
/// Once a referrer is set, it cannot be changed, ensuring stable referral relationships.
///
/// Steps:
/// 1. Verify that the player is not referring themselves.
/// 2. Check that the player's current referrer is the default value, ensuring they have not set a referrer before.
/// 3. Update the player's data to record the new referrer.
/// 4. Increment the referrer's referral count, acknowledging a successful referral.
/// 5. Emit a `SetReferrer` event to record this action on-chain.
pub fn set_referrer(ctx: Context<SetReferrer>, referrer: Pubkey) -> Result<()> {
    // Obtain current UNIX timestamp for event logging and logic checks.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let SetReferrer {
        game,
        player,
        player_data,
        referrer_data,
        ..
    } = ctx.accounts;

    // Update player's data to set the chosen referrer
    player_data.set_referrer(referrer)?;

    // Increment the referral count in the referrer's data account
    referrer_data.increment_referral_count()?;

    game.increment_event_nonce()?;

    // Emit an event that the player successfully set a new referrer
    emit!(TransferEvent {
        event_type: EventType::SetReferrer,
        event_nonce: game.event_nonce,
        data: EventData::SetReferrer {
            player: player.key(),
            referrer
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
