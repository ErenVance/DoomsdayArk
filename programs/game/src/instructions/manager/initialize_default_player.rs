use crate::constants::{
    DEFAULT_PLAYER, GAME_SEED, PLAYER_DATA_SEED, TOKEN_MINT, VOUCHER_MINT_SEED,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
pub struct InitializeDefaultPlayer<'info> {
    /// The authority (signer) who is authorized to initialize the default player.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The global game account, referencing token_mint and authority to ensure authorized access.
    #[account(mut,
        seeds = [GAME_SEED],
        bump,
        has_one = authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The default player public key (constant).
    /// CHECK: Not read or written to, only used for address verification.
    #[account(address = DEFAULT_PLAYER)]
    pub default_player: AccountInfo<'info>,

    /// The default player data account, to be created for storing player information.
    #[account(
        init,
        payer = authority,
        space = 8 + PlayerData::INIT_SPACE,
        seeds = [PLAYER_DATA_SEED, default_player.key().as_ref()],
        bump,
    )]
    pub default_player_data: Box<Account<'info, PlayerData>>,

    /// The token mint representing the in-game currency.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The voucher mint account for creating vouchers.
    /// Linked to VOUCHER_MINT_SEED for derivation.
    #[account(
        seeds = [VOUCHER_MINT_SEED],
        bump,
    )]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The default player's token account, created if needed to hold the in-game tokens.
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = default_player
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The default player's voucher account, created if needed to hold vouchers.
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = voucher_mint,
        associated_token::authority = default_player
    )]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The SPL token program enabling token operations.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The Associated Token program used for creating associated token accounts.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The System program for basic Solana operations.
    pub system_program: Program<'info, System>,
}

/// The `initialize_default_player` instruction sets up a default player and its associated data account.
/// This player often acts as a placeholder or default reference in the game logic, ensuring a baseline state.
///
/// Steps:
/// 1. Validate that the authority is authorized to initialize the default player.
/// 2. Create and initialize the `default_player_data` account, linking it to the default player's accounts.
/// 3. Assign the default team and use the `default_player` as both the player and referrer fields for initialization.
/// 4. Emit an `InitializeDefaultPlayer` event to log this initialization.
pub fn initialize_default_player(ctx: Context<InitializeDefaultPlayer>) -> Result<()> {
    // Obtain current UNIX timestamp for event logging.
    let timestamp = to_timestamp_u64(Clock::get()?.unix_timestamp)?;

    let InitializeDefaultPlayer {
        authority,
        default_player,
        default_player_data,
        token_account,
        voucher_account,
        game,
        ..
    } = ctx.accounts;

    // Initialize the default_player_data with the default player's pubkey as player and referrer.
    default_player_data.initialize(
        default_player.key(),
        default_player.key(),
        game.default_team,
        token_account.key(),
        voucher_account.key(),
    )?;

    game.increment_event_nonce()?;

    // Emit an event to record the initialization of the default player.
    emit!(TransferEvent {
        event_type: EventType::InitializeDefaultPlayer,
        event_nonce: game.event_nonce,
        data: EventData::InitializeDefaultPlayer {
            player: default_player.key(),
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}
