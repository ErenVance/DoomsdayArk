use crate::constants::{
    GAME_SEED, LAMPORTS_PER_TOKEN, PLAYER_DATA_SEED, VOUCHER_MINT_SEED, VOUCHER_SEED,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{
    timestamp_to_days, to_timestamp_u64, transfer_from_token_vault_to_token_account,
};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, mint_to, Mint, MintTo, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `CollectAirdropRewards` instruction allows players to claim their daily airdrop reward,
/// provided they meet certain criteria such as having made a purchase on the current day and not having claimed already.
/// This mechanism encourages regular participation and continuous engagement.
///
/// Steps:
/// 1. Verify that the player has not already collected airdrop rewards today and has made a purchase on the current day.
/// 2. Determine the airdrop reward amount based on the player's consecutive purchase streak.
/// 3. Ensure that the game's daily airdrop cap and pool balances can cover this reward.
/// 4. Mint the corresponding voucher tokens and transfer their underlying assets from the game vault to the player's voucher account.
/// 5. Emit a `CollectAirdropReward` event to record the action on-chain.
#[derive(Accounts)]
pub struct CollectAirdropRewards<'info> {
    /// The global `Game` account managing rewards, daily caps, and the current day's state.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = game_vault
    )]
    pub game: Box<Account<'info, Game>>,

    /// The global `Voucher` account, managing voucher mint authority and overall distribution.
    #[account(
        mut,
        seeds = [VOUCHER_SEED],
        bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The voucher mint account used to mint vouchers to players.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The voucher vault token account holding assets that back voucher issuance.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The game vault token account holding the game's assets, from which airdrop rewards originate.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The player claiming the airdrop rewards. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, tracking consecutive purchase days, last collected day, and voucher account association.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = voucher_account
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The player's voucher token account, where newly minted vouchers are deposited.
    #[account(mut)]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The SPL token program used for token-related instructions (minting, transferring).
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

pub fn collect_airdrop_rewards(ctx: Context<CollectAirdropRewards>) -> Result<()> {
    // Retrieve the current UNIX timestamp for event logging and day calculations
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let CollectAirdropRewards {
        player,
        player_data,
        game,
        game_vault,
        voucher,
        voucher_vault,
        voucher_mint,
        voucher_account,
        token_program,
        ..
    } = ctx.accounts;

    // Convert current timestamp to a day index
    let current_day = timestamp_to_days(timestamp)?;

    // Ensure the player has not already claimed airdrop rewards today
    require!(
        player_data.last_collected_airdrop_reward_day != current_day,
        ErrorCode::AirdropRewardsAlreadyCollected
    );

    // Ensure the player made a purchase today, making them eligible for the airdrop
    require!(
        player_data.last_purchased_day == current_day,
        ErrorCode::AirdropRewardsNotAvailable
    );

    // Determine airdrop rewards based on consecutive purchase streak
    let airdrop_rewards = match player_data.consecutive_purchased_days {
        1 => 100 * LAMPORTS_PER_TOKEN,
        2 => 200 * LAMPORTS_PER_TOKEN,
        3 => 300 * LAMPORTS_PER_TOKEN,
        4 => 400 * LAMPORTS_PER_TOKEN,
        5 => 500 * LAMPORTS_PER_TOKEN,
        _ => 1000 * LAMPORTS_PER_TOKEN,
    };

    // Update the player's collected rewards and last collected day
    player_data.collected_airdrop_rewards = player_data
        .collected_airdrop_rewards
        .safe_add(airdrop_rewards)?;
    player_data.last_collected_airdrop_reward_day = current_day;

    // If a new day has started, reset the game's current day and daily distributed amount
    if current_day > game.current_day {
        game.current_day = current_day;
        game.current_day_distributed_airdrop_rewards = 0;
    }

    // Ensure we do not exceed the daily airdrop cap
    let new_daily_total = game
        .current_day_distributed_airdrop_rewards
        .safe_add(airdrop_rewards)?;
    require!(
        game.current_day_cap_airdrop_rewards >= new_daily_total,
        ErrorCode::ExceedsDailyAirdropCap
    );
    game.current_day_distributed_airdrop_rewards = new_daily_total;

    // Ensure there are enough tokens in the airdrop pool
    require!(
        game.airdrop_rewards_pool_balance >= airdrop_rewards,
        ErrorCode::InsufficientAirdropRewardBalance
    );
    game.airdrop_rewards_pool_balance = game
        .airdrop_rewards_pool_balance
        .safe_sub(airdrop_rewards)?;
    game.distributed_airdrop_rewards =
        game.distributed_airdrop_rewards.safe_add(airdrop_rewards)?;

    // Mint vouchers corresponding to the airdrop rewards
    voucher.mint(airdrop_rewards)?;

    // Transfer the underlying tokens from the game vault to the voucher vault, backing the newly minted vouchers
    transfer_from_token_vault_to_token_account(
        game,
        game_vault,
        voucher_vault,
        token_program,
        airdrop_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    // Mint vouchers into the player's voucher account
    mint_to(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            MintTo {
                mint: voucher_mint.to_account_info(),
                to: voucher_account.to_account_info(),
                authority: voucher.to_account_info(),
            },
            &[&[VOUCHER_SEED, &[ctx.bumps.voucher]]],
        ),
        airdrop_rewards,
    )?;

    msg!("Airdrop rewards: {}", airdrop_rewards);

    game.increment_event_nonce()?;

    // Emit an event recording the airdrop claim action
    emit!(TransferEvent {
        event_type: EventType::CollectAirdropReward,
        event_nonce: game.event_nonce,
        data: EventData::CollectAirdropReward {
            game: game.key(),
            player: player.key(),
            airdrop_rewards,
            voucher: voucher.key(),
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
