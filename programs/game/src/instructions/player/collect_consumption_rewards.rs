use crate::constants::{GAME_SEED, PLAYER_DATA_SEED, VOUCHER_MINT_SEED, VOUCHER_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, mint_to, Mint, MintTo, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `CollectConsumptionRewards` instruction enables a player to claim their accumulated consumption rewards.
/// These rewards generally stem from the player's spending behavior within the platform's ecosystem and are stored as pending rewards until collected.
///
/// Steps:
/// 1. Verify that the player has pending consumption rewards available to collect.
/// 2. Check that the `Game` account's consumption reward pool balance can cover the requested amount.
/// 3. Update the player's and game's state, adjusting pool balances and distributed totals.
/// 4. Mint voucher tokens to the player's voucher account, backed by transferring the corresponding assets from the `game_vault` to the `voucher_vault`.
/// 5. Emit a `CollectConsumptionRewards` event to record the reward claim on-chain.
#[derive(Accounts)]
pub struct CollectConsumptionRewards<'info> {
    /// The player who is collecting their consumption rewards. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, which tracks pending and collected consumption rewards, as well as voucher associations.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = voucher_account
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The player's voucher token account where newly minted vouchers representing the claimed rewards will be deposited.
    #[account(mut)]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The global `Game` account, maintaining consumption reward pools and other economic parameters.
    #[account(
        mut,
        seeds = [GAME_SEED.as_ref()],
        bump,
        has_one = game_vault,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The global voucher state, controlling voucher minting authority.
    #[account(
        mut,
        seeds = [VOUCHER_SEED],
        bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The voucher mint account used to create new voucher tokens.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The voucher vault holding the underlying assets that back the voucher tokens.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The game vault token account from which the underlying assets are transferred to the voucher vault.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The SPL token program used for token operations (minting, transferring).
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

pub fn collect_consumption_rewards(ctx: Context<CollectConsumptionRewards>) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging
    let timestamp = to_timestamp_u64(Clock::get()?.unix_timestamp)?;

    // Extract references for clarity
    let CollectConsumptionRewards {
        player,
        player_data,
        game,
        voucher_mint,
        voucher,
        voucher_account,
        voucher_vault,
        game_vault,
        token_program,
        ..
    } = ctx.accounts;

    // Check if the player has pending consumption rewards
    let consumption_rewards = player_data.collectable_consumption_rewards;
    require!(consumption_rewards > 0, ErrorCode::NoRewardsToCollect);

    // Update player's collected and pending reward records
    player_data.collected_consumption_rewards = player_data
        .collected_consumption_rewards
        .safe_add(consumption_rewards)?;
    player_data.collectable_consumption_rewards = 0;

    // Ensure the game has sufficient consumption rewards in its pool
    require!(
        consumption_rewards <= game.consumption_rewards_pool_balance,
        ErrorCode::InsufficientConsumptionRewardBalance
    );
    game.consumption_rewards_pool_balance = game
        .consumption_rewards_pool_balance
        .safe_sub(consumption_rewards)?;
    game.distributed_consumption_rewards = game
        .distributed_consumption_rewards
        .safe_add(consumption_rewards)?;

    // Mint voucher tokens corresponding to the claimed consumption rewards
    voucher.mint(consumption_rewards)?;

    // Transfer the underlying assets from the game vault to the voucher vault
    transfer_from_token_vault_to_token_account(
        game,
        game_vault,
        voucher_vault,
        token_program,
        consumption_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

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
        consumption_rewards,
    )?;

    msg!("Consumption rewards: {}", consumption_rewards);

    game.increment_event_nonce()?;

    // Emit an event to record the reward collection
    emit!(TransferEvent {
        event_type: EventType::CollectConsumptionRewards,
        event_nonce: game.event_nonce,
        data: EventData::CollectConsumptionRewards {
            game: game.key(),
            player: player.key(),
            consumption_rewards,
            voucher: voucher.key(),
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
