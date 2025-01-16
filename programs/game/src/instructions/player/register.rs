use crate::constants::{GAME_SEED, PLAYER_DATA_SEED, TOKEN_MINT, VOUCHER_MINT_SEED, VOUCHER_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use crate::utils::transfer_from_token_vault_to_token_account;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, mint_to, Mint, MintTo, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

/// The `Register` instruction handles the onboarding process for a new player in the game ecosystem.
/// Upon registration, the player links their wallet, initializes their player data account,
/// sets a referrer, and may receive a registration reward if slots remain.
///
/// Steps:
/// 1. Validate the referrer is not the player themselves (no self-referral).
/// 2. Initialize a new `PlayerData` account, associating it with the player's `token_account` and `voucher_account`.
/// 3. Increment the referrer's referral count.
/// 4. If registration reward slots are still available, distribute the registration reward to the player's voucher account:
///    - Deduct from `registration_rewards_pool_balance` and update `distributed_registration_rewards`.
///    - Mint voucher tokens corresponding to the registration reward and transfer underlying tokens from the `game_vault` to `voucher_vault`.
/// 5. Emit a `Register` event to log the new player onboarding action.
#[derive(Accounts)]
#[instruction(referrer: Pubkey)]
pub struct Register<'info> {
    /// The player creating a new account, must sign the transaction.
    /// Ensures the `referrer` is not the same as the player (`CannotReferSelf`).
    #[account(
        mut,
        constraint = referrer != player.key() @ ErrorCode::CannotReferSelf
    )]
    pub player: Signer<'info>,

    /// The player's data account, initialized here to track player-specific information and link token/voucher accounts.
    #[account(
        init,
        payer = player,
        space = 8 + PlayerData::INIT_SPACE,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The referrer's data account, from which we increment the referral count upon a successful registration.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, referrer.as_ref()],
        bump
    )]
    pub referrer_data: Box<Account<'info, PlayerData>>,

    /// The global game account, governing rewards, rounds, and vault balances.
    #[account(
        mut,
        seeds = [GAME_SEED.as_ref()],
        bump,
        has_one = game_vault,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The token mint representing the in-game token currency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The voucher mint used for generating and distributing voucher tokens.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The player's associated token account for the in-game token. Created if it doesn't exist.
    #[account(
        associated_token::mint = token_mint,
        associated_token::authority = player
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The player's associated voucher account, created if needed.
    #[account(
        init_if_needed,
        payer = player,
        associated_token::mint = voucher_mint,
        associated_token::authority = player
    )]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The global voucher state, controlling voucher mint authority and linking to `voucher_vault`.
    #[account(
        mut,
        seeds = [VOUCHER_SEED], bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The voucher vault token account holding underlying assets backing the voucher tokens.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The main game vault token account holding tokens for rewards and distributions.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The SPL token program enabling minting, burning, and transfer operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The Associated Token program used to create associated token accounts for the player.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The System program for basic Solana operations like account creation.
    pub system_program: Program<'info, System>,
}

/// Executes the registration logic:
///
/// - `referrer`: The public key of the player who referred this new player.
///
pub fn register(ctx: Context<Register>, referrer: Pubkey) -> Result<()> {
    // Get current UNIX timestamp for event logging and logic
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let Register {
        player,
        player_data,
        referrer_data,
        game,
        // token_mint,
        voucher_mint,
        token_account,
        voucher,
        voucher_account,
        voucher_vault,
        game_vault,
        token_program,
        ..
    } = ctx.accounts;

    // Initialize the player's data, linking them to the default team and setting referrer
    player_data.initialize(
        player.key(),
        referrer,
        game.default_team,
        token_account.key(),
        voucher_account.key(),
    )?;

    // Increment the referrer's referral count
    referrer_data.increment_referral_count()?;

    // Check if registration rewards are still available and distribute if yes
    if game.registration_rewards_pool_balance >= game.registration_rewards {
        require!(
            game.registration_rewards <= game.registration_rewards_pool_balance,
            ErrorCode::InsufficientRegistrationRewardBalance
        );
        // Deduct from registration pool and update distributed amount
        game.registration_rewards_pool_balance = game
            .registration_rewards_pool_balance
            .safe_sub(game.registration_rewards)?;
        game.distributed_registration_rewards = game
            .distributed_registration_rewards
            .safe_add(game.registration_rewards)?;

        // Mint voucher tokens for the registration reward
        voucher.mint(game.registration_rewards)?;

        // Transfer the underlying tokens from the game vault to the voucher vault
        transfer_from_token_vault_to_token_account(
            game,
            game_vault,
            voucher_vault,
            token_program,
            game.registration_rewards,
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
            game.registration_rewards,
        )?;
    }

    game.increment_event_nonce()?;

    // Emit a Register event to record the player's onboarding
    emit!(TransferEvent {
        event_type: EventType::Register,
        event_nonce: game.event_nonce,
        data: EventData::Register {
            player: player.key(),
            referrer,
            voucher: voucher.key(),
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
