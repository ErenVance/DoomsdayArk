use crate::constants::{
    GAME_SEED, MIN_LOTTERY_REWARDS_POOL_BALANCE, ONCE_DRAW_LOTTERY_VOUCHER_COST, PLAYER_DATA_SEED,
    VOUCHER_MINT_SEED, VOUCHER_SEED,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{redeem_vouchers, to_timestamp_u64};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;
use switchboard_on_demand::accounts::RandomnessAccountData;

/// The `DrawLottery` instruction enables a player to participate in a lottery draw using their voucher tokens.
/// The lottery mechanism depends on external randomness data (via Switchboard) and updates the global lottery and developer pools accordingly.
///
/// Steps:
/// 1. Validate that the lottery pool has sufficient balance (`MIN_LOTTERY_REWARDS_POOL_BALANCE`).
/// 2. Ensure the player has revealed the previous lottery result before attempting another draw.
/// 3. Check that the player holds enough voucher tokens (`ONCE_DRAW_LOTTERY_VOUCHER_COST`).
/// 4. Fetch and verify randomness data, ensuring it originates from the expected slot.
/// 5. Deduct a portion of the cost as developer rewards and allocate the remainder to the lottery pool.
/// 6. Update the player's randomness-related data.
/// 7. Burn the player's voucher tokens and redeem them for underlying tokens.
/// 8. Emit a `DrawLottery` event to record the action on-chain.
#[derive(Accounts)]
pub struct DrawLottery<'info> {
    /// The player initiating the lottery draw. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, storing voucher account references, last revealed results, etc.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = voucher_account,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The player's voucher account from which voucher tokens will be burned to participate.
    #[account(mut)]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: The Switchboard randomness data account.
    /// Verified externally by the program logic to ensure proper seed_slot alignment.
    pub randomness_account_data: AccountInfo<'info>,

    /// The global game account, holding references to `game_vault` and associated economics.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = game_vault,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The main game vault account from which tokens are sourced.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The voucher state account managing voucher mint authority and supply.
    #[account(
        mut,
        seeds = [VOUCHER_SEED], bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The voucher vault token account holding the underlying assets backing voucher tokens.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The voucher mint account used to create or burn voucher tokens.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The SPL Token program used for minting, burning, and transferring tokens.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

pub fn draw_lottery(ctx: Context<DrawLottery>) -> Result<()> {
    // Retrieve the current cluster time for event logging
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let DrawLottery {
        player,
        player_data,
        voucher_account,
        randomness_account_data,
        game,
        game_vault,
        voucher,
        voucher_vault,
        voucher_mint,
        token_program,
        ..
    } = ctx.accounts;

    // Check that the lottery pool holds enough funds to justify a draw
    require!(
        game.lottery_rewards_pool_balance >= MIN_LOTTERY_REWARDS_POOL_BALANCE,
        ErrorCode::LotteryPoolIsEmpty
    );

    // Ensure the player has revealed the previous lottery result
    require!(
        player_data.result_revealed,
        ErrorCode::BeforeThisLotteryNeedToRevealLastResult
    );

    let voucher_cost = ONCE_DRAW_LOTTERY_VOUCHER_COST;

    // Ensure the player has sufficient vouchers to pay the lottery cost
    require!(
        voucher_account.amount >= voucher_cost,
        ErrorCode::InsufficientFundsToPayFee
    );

    // Parse the randomness account data from Switchboard
    let randomness_data = RandomnessAccountData::parse(randomness_account_data.data.borrow())
        .map_err(|_| ErrorCode::RandomnessNotResolved)?;

    let current_slot = clock.slot;

    // Verify that the randomness seed is from the immediately preceding slot
    if randomness_data.seed_slot != current_slot - 1 {
        return Err(ErrorCode::RandomnessAlreadyRevealed.into());
    }

    // Update global game accounts with new balances
    game.lottery_rewards_pool_balance = game.lottery_rewards_pool_balance.safe_add(voucher_cost)?;

    // Update the player's randomness provider and seed slot info
    player_data.update_randomness(randomness_account_data.key(), randomness_data.seed_slot)?;

    // Burn the voucher tokens from the player's voucher account
    voucher.burn(voucher_cost)?;
    let cpi_accounts = Burn {
        mint: voucher_mint.to_account_info(),
        from: voucher_account.to_account_info(),
        authority: player.to_account_info(),
    };
    let cpi_context = CpiContext::new(token_program.to_account_info(), cpi_accounts);
    token::burn(cpi_context, voucher_cost)?;

    // Redeem the burned vouchers by transferring underlying tokens from voucher_vault to game_vault
    redeem_vouchers(
        voucher,
        voucher_vault,
        game_vault,
        token_program,
        voucher_cost,
        &[VOUCHER_SEED, &[ctx.bumps.voucher]],
    )?;

    game.increment_event_nonce()?;

    // Emit an event capturing the lottery draw action
    emit!(TransferEvent {
        event_type: EventType::DrawLottery,
        event_nonce: game.event_nonce,
        data: EventData::DrawLottery {
            game: game.key(),
            player: player.key(),
            randomness_provider: randomness_account_data.key(),
            bet_amount: voucher_cost,
            voucher: voucher.key(),
        },
        initiator_type: InitiatorType::LOTTERY,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
