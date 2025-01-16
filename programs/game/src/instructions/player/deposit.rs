use crate::constants::{GAME_SEED, TOKEN_MINT, VAULT_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{redeem_vouchers, to_timestamp_u64};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, burn, Burn, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `Stake` instruction allows a player to stake tokens (shards) into the staking pool, create a stake order,
/// and receive voucher tokens representing their staked assets. It involves transferring tokens from the player's
/// account to a newly created stake order vault, then into the pool vault. Additionally, the player receives minted
/// vouchers to confirm their stake, and rewards are allocated based on the configured APR.
#[derive(Accounts)]
pub struct Deposit<'info> {
    /// The player initiating the stake, must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The global game account.
    #[account(mut,
        seeds = [GAME_SEED], bump,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The player's token account, from which staked tokens will be deducted.
    #[account(mut,
        associated_token::mint = token_mint,
        associated_token::authority = player
    )]
    pub token_0_account: Box<Account<'info, TokenAccount>>,

    /// The player's token account, from which staked tokens will be deducted.
    #[account(init_if_needed,
        payer = player,
        associated_token::mint = token_1_mint,
        associated_token::authority = player
    )]
    pub token_1_account: Box<Account<'info, TokenAccount>>,

    /// The deposit account, which holds the deposit information.
    #[account(mut, seeds = [VAULT_SEED], bump, has_one = token_mint, has_one = token_vault)]
    pub vault: Box<Account<'info, Vault>>,

    /// The token mint for the deposit token.
    #[account(mut)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The token mint for the deposit token.
    #[account(mut, address = TOKEN_MINT)]
    pub token_1_mint: Box<Account<'info, Mint>>,

    /// The token vault for the deposit token.
    #[account(mut)]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    /// The SPL token program, used for token operations like minting and transferring.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program, used for creating associated token accounts (like stake_order_vault).
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program, required for certain account creation operations.
    pub system_program: Program<'info, System>,
}

/// Executes the staking logic:
/// 1. Validates the input `shards_amount`.
/// 2. Converts `shards_amount` into `stake_amount` using predefined constants (`ONE_MILLION` and `LAMPORTS_PER_TOKEN`).
/// 3. Ensures the player has sufficient tokens.
/// 4. Creates a stake order and allocates reward tokens from the pool.
/// 5. Transfers the staked tokens from the player's token account to the `stake_order_vault`,
///    then from `stake_order_vault` to the `stake_pool_token_vault`.
/// 6. Mints voucher tokens to the player's voucher account and moves corresponding tokens to the `voucher_vault`.
/// 7. Emits a `TransferEvent` logging the stake operation.
pub fn deposit(ctx: Context<Deposit>, token_amount: u64) -> Result<()> {
    // Fetch the current UNIX timestamp for record keeping
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to accounts for easier manipulation
    let Deposit {
        game,
        player,
        token_0_account,
        token_1_account,
        vault,
        token_mint,
        token_vault,
        token_program,
        ..
    } = ctx.accounts;

    // Validate that the player is staking a positive amount
    require!(
        token_0_account.amount >= token_amount,
        ErrorCode::InvalidAmount
    );

    require!(vault.token_amount >= token_amount, ErrorCode::InvalidAmount);

    vault.deposit(token_amount)?;

    burn(
        CpiContext::new(
            token_program.to_account_info(),
            Burn {
                mint: token_mint.to_account_info(),
                from: token_0_account.to_account_info(),
                authority: player.to_account_info(),
            },
        ),
        token_amount,
    )?;

    // Redeem underlying tokens corresponding to the burned vouchers,
    // transferring them from the voucher_vault to the game_vault.
    redeem_vouchers(
        vault,
        token_vault,
        token_1_account,
        token_program,
        token_amount,
        &[VAULT_SEED, &[ctx.bumps.vault]],
    )?;

    game.increment_event_nonce()?;

    // Emit an event to record the staking action on-chain
    emit!(TransferEvent {
        event_type: EventType::Deposit,
        event_nonce: game.event_nonce,
        data: EventData::Deposit {
            player: player.key(),
            vault: vault.key(),
            token_amount,
        },
        initiator_type: InitiatorType::DEPOSIT,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
