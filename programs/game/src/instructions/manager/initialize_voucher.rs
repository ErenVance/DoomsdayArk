use crate::constants::{GAME_SEED, TOKEN_MINT, VOUCHER_MINT_SEED, VOUCHER_SEED};
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::to_timestamp_u64;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::metadata::{
    self, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3, Metadata,
};
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

const VOUCHER_METADATA_URI: &str = "https://www.thedoomsdayark.com/meta/av/metadata.json";
const VOUCHER_NAME: &str = "Ark Voucher";
const VOUCHER_SYMBOL: &str = "AV";

/// The `InitializeVoucher` instruction sets up the voucher mint and associated metadata for the Fire Game Voucher (FGV).
/// This voucher can be used within the game to represent and distribute certain in-game assets or rewards.
///
/// # Steps
/// 1. Create and initialize the `voucher` account.
/// 2. Create the `voucher_mint` (with `VOUCHER_MINT_SEED`) and set `voucher` as its authority.
/// 3. Create the `voucher_vault` associated token account for holding tokens related to the voucher.
/// 4. Use `create_voucher_token_metadata` to associate metadata with the `voucher_mint`.
/// 5. Emit an `InitializeVoucher` event to record the initialization on-chain.
#[derive(Accounts)]
pub struct InitializeVoucher<'info> {
    /// The global game account, ensuring the authority and token_mint constraints.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = authority,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The authority (signer) authorized to initialize the voucher.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The voucher account to be initialized, responsible for managing voucher minting.
    #[account(
        init,
        payer = authority,
        space = 8 + Voucher::INIT_SPACE,
        seeds = [VOUCHER_SEED],
        bump,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The main token mint account.
    #[account(address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The voucher token mint account.
    /// Created with `VOUCHER_MINT_SEED` and `voucher` as authority.
    #[account(
        init,
        payer = authority,
        seeds = [VOUCHER_MINT_SEED],
        bump,
        mint::decimals = 6,
        mint::authority = voucher,
    )]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The voucher token vault associated token account, holding tokens backing the voucher.
    #[account(
        init,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = voucher
    )]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The token metadata account, checked in CPI calls.
    /// CHECK: Validated via CPI to token metadata program.
    #[account(mut)]
    pub token_metadata: UncheckedAccount<'info>,

    /// The token metadata program used to create and manage metadata for the voucher_mint.
    pub token_metadata_program: Program<'info, Metadata>,

    /// The SPL token program for token minting, burning, and transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// The associated token program for creating token accounts.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// The system program required for account creations.
    pub system_program: Program<'info, System>,

    /// Rent sysvar to fetch rent exemption data.
    pub rent: Sysvar<'info, Rent>,
}

/// Executes the `InitializeVoucher` instruction:
///
/// - Creates and initializes the voucher account, voucher mint, and voucher vault.
/// - Calls `create_voucher_token_metadata` to attach metadata (name, symbol, URI) to the voucher_mint.
/// - Emits `InitializeVoucher` event to record the initialization on-chain.
pub fn initialize_voucher(ctx: Context<InitializeVoucher>) -> Result<()> {
    // Obtain current UNIX timestamp for event logging.
    let timestamp = to_timestamp_u64(Clock::get()?.unix_timestamp)?;

    // Create metadata for the voucher token
    create_voucher_token_metadata(&ctx)?;

    let InitializeVoucher {
        game,
        voucher,
        voucher_mint,
        voucher_vault,
        authority,
        ..
    } = ctx.accounts;

    // Initialize the voucher with mint and vault
    voucher.initialize(voucher_mint.key(), voucher_vault.key())?;

    game.increment_event_nonce()?;

    // Emit initialization event
    emit!(TransferEvent {
        event_type: EventType::InitializeVoucher,
        event_nonce: game.event_nonce,
        data: EventData::InitializeVoucher {
            voucher: voucher.key()
        },
        initiator_type: InitiatorType::VOUCHER,
        initiator: authority.key(),
        timestamp,
    });

    Ok(())
}

/// Creates the metadata for the voucher token using the token metadata program.
/// Associates VOUCHER_NAME, VOUCHER_SYMBOL, and VOUCHER_METADATA_URI with the `voucher_mint`.
fn create_voucher_token_metadata(ctx: &Context<InitializeVoucher>) -> Result<()> {
    let InitializeVoucher {
        token_metadata_program,
        voucher_mint,
        token_metadata,
        voucher,
        authority,
        rent,
        system_program,
        ..
    } = &ctx.accounts;

    metadata::create_metadata_accounts_v3(
        CpiContext::new_with_signer(
            token_metadata_program.to_account_info(),
            CreateMetadataAccountsV3 {
                metadata: token_metadata.to_account_info(),
                mint: voucher_mint.to_account_info(),
                mint_authority: voucher.to_account_info(),
                update_authority: voucher.to_account_info(),
                payer: authority.to_account_info(),
                rent: rent.to_account_info(),
                system_program: system_program.to_account_info(),
            },
            &[&[VOUCHER_SEED, &[ctx.bumps.voucher]]],
        ),
        DataV2 {
            uri: VOUCHER_METADATA_URI.to_string(),
            name: VOUCHER_NAME.to_string(),
            symbol: VOUCHER_SYMBOL.to_string(),
            creators: None,
            seller_fee_basis_points: 0,
            collection: None,
            uses: None,
        },
        false,
        true,
        None,
    )
}
