use crate::constants::{
    EXCHANGE_COLLATERAL_RATE, GAME_SEED, PLAYER_DATA_SEED, VOUCHER_MINT_SEED, VOUCHER_SEED,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{calculate_proportion, to_timestamp_u64, transfer_from_player_to_vault};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, mint_to, Mint, MintTo, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `CollateralExchange` instruction allows a player to convert their tokens (FGC/FGV) into vouchers at a predefined exchange rate.
/// This process integrates seamlessly with the voucher minting system, ensuring the player's assets are properly secured and represented.
/// By performing this exchange, the player obtains vouchers proportional to their input tokens, fueling their ability to participate in further ecosystem activities.
#[derive(Accounts)]
pub struct CollateralExchange<'info> {
    /// The player initiating the collateral exchange. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The game account.
    #[account(mut, seeds = [GAME_SEED], bump)]
    pub game: Box<Account<'info, Game>>,

    /// The player's data account, ensuring we have a record of the player's token/voucher accounts.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = token_account,
        has_one = voucher_account,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The player's token account from which tokens are deducted for the exchange.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The player's voucher account where newly minted vouchers will be deposited.
    #[account(mut)]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The global voucher state account, tracking mint authority and total issuance.
    #[account(
        mut,
        seeds = [VOUCHER_SEED],
        bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The voucher vault account holding the underlying tokens supporting the voucher supply.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The voucher mint account, used to mint voucher tokens into the player's voucher account.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The SPL Token program used for token operations such as `mint_to`.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// Processes the collateral exchange logic:
///
/// Steps:
/// 1. Verify that the player holds sufficient tokens in `token_account`.
/// 2. Calculate the number of vouchers to mint based on `EXCHANGE_COLLATERAL_RATE`.
/// 3. Mint the corresponding voucher tokens to the player's `voucher_account`.
/// 4. Transfer the exchanged tokens from the player's `token_account` to the `voucher_vault`.
/// 5. Emit a `CollateralExchange` event to record the operation on-chain.
pub fn collateral_exchange(
    ctx: Context<CollateralExchange>,
    exchange_token_amount: u64,
) -> Result<()> {
    // Retrieve the current UNIX timestamp to log the event timing
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to key accounts for clarity
    let CollateralExchange {
        game,
        player,
        voucher,
        voucher_account,
        voucher_mint,
        voucher_vault,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    // Ensure the player has enough tokens to perform the exchange
    require!(
        token_account.amount >= exchange_token_amount,
        ErrorCode::InsufficientFundsToPayFee
    );

    // Calculate how many vouchers will be minted based on the provided exchange rate
    let voucher_amount = calculate_proportion(exchange_token_amount, EXCHANGE_COLLATERAL_RATE)?;

    // Update voucher state to reflect newly minted vouchers
    voucher.mint(exchange_token_amount)?;

    // Transfer the exchanged tokens from the player's token account to the voucher vault
    transfer_from_player_to_vault(
        player,
        token_account,
        voucher_vault,
        token_program,
        exchange_token_amount,
    )?;

    // Mint voucher tokens into the player's voucher account
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
        voucher_amount,
    )?;

    msg!(
        "Collateral exchange: {} tokens in exchange for {} vouchers.",
        exchange_token_amount,
        voucher_amount
    );

    game.increment_event_nonce()?;

    // Emit an event to note that collateral exchange took place
    emit!(TransferEvent {
        event_type: EventType::CollateralExchange,
        event_nonce: game.event_nonce,
        data: EventData::CollateralExchange {
            player: player.key(),
            voucher: voucher.key(),
            exchange_token_amount,
            voucher_amount,
        },
        initiator_type: InitiatorType::VOUCHER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
