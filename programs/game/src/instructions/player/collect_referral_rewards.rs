use crate::constants::{GAME_SEED, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `CollectReferralRewards` instruction allows players to claim referral rewards they have accumulated through inviting other participants.
/// Referral rewards incentivize community growth and user engagement, ensuring players benefit from their network-building efforts.
///
/// Steps:
/// 1. Ensure the player has pending referral rewards available to collect.
/// 2. Verify that the game's referral reward pool can cover the requested amount.
/// 3. Update the player's and game's record of distributed referral rewards.
/// 4. Mint corresponding voucher tokens to the player's voucher account and transfer the underlying assets from the game vault to the voucher vault.
/// 5. Emit a `CollectReferralReward` event to record the referral reward claim on-chain.
#[derive(Accounts)]
pub struct CollectReferralRewards<'info> {
    /// The global game account holding reward pools and distribution logic.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = game_vault
    )]
    pub game: Box<Account<'info, Game>>,

    /// The game vault token account from where the underlying tokens are sourced.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The player claiming referral rewards. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, tracking referral-related balances, pending and collected rewards.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = token_account
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The player's token account, holding the underlying tokens to be transferred.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The SPL token program, facilitating minting and transfer operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

pub fn collect_referral_rewards(ctx: Context<CollectReferralRewards>) -> Result<()> {
    // Obtain the current UNIX timestamp to record when the referral rewards were claimed
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let CollectReferralRewards {
        player,
        player_data,
        game,
        game_vault,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    // Ensure the player has referral rewards to collect
    let referral_rewards = player_data.collectable_referral_rewards;
    require!(referral_rewards > 0, ErrorCode::NoRewardsToCollect);

    // Update player's collected and pending referral rewards
    player_data.collected_referral_rewards = player_data
        .collected_referral_rewards
        .safe_add(referral_rewards)?;
    player_data.collectable_referral_rewards = 0;

    // Check that the game's referral reward pool has sufficient funds
    require!(
        game.referral_rewards_pool_balance >= referral_rewards,
        ErrorCode::InsufficientReferrerRewardBalance
    );
    game.referral_rewards_pool_balance = game
        .referral_rewards_pool_balance
        .safe_sub(referral_rewards)?;
    game.distributed_referral_rewards = game
        .distributed_referral_rewards
        .safe_add(referral_rewards)?;

    // Transfer the underlying tokens from the game vault to the voucher vault, backing the newly issued vouchers
    transfer_from_token_vault_to_token_account(
        game,
        &game_vault,
        &token_account,
        &token_program,
        referral_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    msg!("Referral rewards: {}", referral_rewards);

    game.increment_event_nonce()?;

    // Emit a transfer event to log the referral reward collection
    emit!(TransferEvent {
        event_type: EventType::CollectReferralReward,
        event_nonce: game.event_nonce,
        data: EventData::CollectReferralReward {
            game: game.key(),
            player: player.key(),
            referral_rewards
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
