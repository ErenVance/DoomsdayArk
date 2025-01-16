use crate::constants::{GAME_SEED, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::Game;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
pub struct CollectDeveloperRewards<'info> {
    /// The authority account that must sign the transaction.
    /// This is typically a designated administrator who can collect developer rewards.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The global game account referencing the main vault and token mint.
    /// Must have the same authority and ensure that authority is authorized (not unauthorized access).
    #[account(mut,
        seeds = [GAME_SEED], bump,
        has_one = game_vault,
        has_one = authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The game vault token account holding tokens for developer rewards and other pools.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The token mint representing the in-game currency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The authority's token account where developer rewards will be transferred.
    /// Created if needed, ensuring the authority can receive tokens directly.
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The SPL token program, enabling token transfers and related operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program used to create associated token accounts.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program for Solana account creation and basic operations.
    pub system_program: Program<'info, System>,
}

/// The `collect_developer_rewards` instruction allows the authorized entity to withdraw accumulated developer rewards from the game vault.
/// Developer rewards are funds set aside for maintenance, operation costs, or other developer incentives.
///
/// Steps:
/// 1. Ensure that the authority matches the game's designated authority.
/// 2. Retrieve the total `developer_rewards_pool_balance` from the game account.
/// 3. If `developer_rewards` > 0, transfer these tokens from the `game_vault` to the authority's token account.
/// 4. Update the `developer_rewards_pool_balance` and `distributed_developer_rewards` to reflect the payout.
/// 5. Emit a `CollectDeveloperRewards` event to record the transaction on-chain.
pub fn collect_developer_rewards(ctx: Context<CollectDeveloperRewards>) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and timing records.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to accounts for clarity.
    let CollectDeveloperRewards {
        authority,
        game,
        game_vault,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    // Determine how many developer rewards are available.
    let developer_rewards = game.developer_rewards_pool_balance;

    // Check if the game has enough balance to cover these developer rewards.
    require!(
        game.developer_rewards_pool_balance >= developer_rewards,
        ErrorCode::InsufficientDeveloperRewardBalance
    );

    // Deduct the developer rewards from the `developer_rewards_pool_balance` and update distribution record.
    game.developer_rewards_pool_balance = game
        .developer_rewards_pool_balance
        .safe_sub(developer_rewards)?;
    game.distributed_developer_rewards = game
        .distributed_developer_rewards
        .safe_add(developer_rewards)?;

    // Transfer the developer rewards from the game vault to the authority's token account.
    transfer_from_token_vault_to_token_account(
        game,
        &game_vault,
        &token_account,
        &token_program,
        developer_rewards,
        &[GAME_SEED, &[ctx.bumps.game]],
    )?;

    game.increment_event_nonce()?;

    // Emit an event logging the developer reward collection.
    emit!(TransferEvent {
        event_type: EventType::CollectDeveloperRewards,
        event_nonce: game.event_nonce,
        data: EventData::CollectDeveloperRewards {
            game: game.key(),
            developer_rewards,
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}
