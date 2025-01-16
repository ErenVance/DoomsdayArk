use crate::constants::{GAME_SEED, ONCE_DRAW_LOTTERY_VOUCHER_COST, PLAYER_DATA_SEED};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{
    calculate_multiplier, get_symbol_id, to_timestamp_u64,
    transfer_from_token_vault_to_token_account,
};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;
use switchboard_on_demand::accounts::RandomnessAccountData;

#[derive(Accounts)]
pub struct RevealDrawLotteryResult<'info> {
    /// The global game account, referencing main vaults and configurations.
    #[account(mut, seeds = [GAME_SEED], bump, has_one = game_vault)]
    pub game: Box<Account<'info, Game>>,

    /// The player revealing the lottery result, must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, linked to their randomness provider and token account.
    /// It stores info about the committed random slot, spin symbols, and result multipliers.
    #[account(mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = randomness_provider,
        has_one = token_account,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The randomness provider account (Switchboard). It's an UncheckedAccount because validation occurs at runtime.
    /// CHECK: Validated at runtime via RandomnessAccountData parsing.
    pub randomness_provider: UncheckedAccount<'info>,

    /// The main game vault holding tokens for rewards and payouts.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The player's token account where lottery rewards will be deposited if they win.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The SPL token program enabling token transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// The `reveal_draw_lottery_result` instruction finalizes a previously initiated lottery draw by revealing the outcome.
/// It uses the Switchboard randomness data to determine the final symbols and multiplier. If the player wins, it distributes lottery rewards accordingly.
///
/// Steps:
/// 1. Fetch the randomness data from the `randomness_provider` and ensure it matches the committed slot in `player_data`.
/// 2. Confirm that the randomness is resolved and fresh (not expired or invalid).
/// 3. Derive symbol IDs from the random values and calculate a multiplier to determine lottery rewards.
/// 4. If the player wins (multiplier > 0), deduct the corresponding rewards from the lottery pool and transfer them to the player's token account.
/// 5. Update `player_data` with the revealed symbols, multiplier, and collected lottery rewards if any.
/// 6. Emit a `RevealDrawLotteryResult` event to log the outcome on-chain.

pub fn reveal_draw_lottery_result(ctx: Context<RevealDrawLotteryResult>) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and logical checks.
    let clock: Clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references to the accounts for clarity.
    let RevealDrawLotteryResult {
        game,
        player,
        player_data,
        randomness_provider,
        game_vault,
        token_account,
        token_program,
        ..
    } = ctx.accounts;

    // Parse the randomness data from the Switchboard randomness account.
    let randomness_data = RandomnessAccountData::parse(randomness_provider.data.borrow())
        .map_err(|_| ErrorCode::InvalidRandomnessAccount)?;

    // Ensure randomness is resolved (seed_slot != 0 means we have valid randomness).
    require!(
        randomness_data.seed_slot != 0 && player_data.result_revealed == false,
        ErrorCode::RandomnessNotResolved
    );

    // Check that the retrieved randomness matches the player's committed random slot.
    require!(
        randomness_data.seed_slot == player_data.commit_slot,
        ErrorCode::RandomnessExpired
    );

    // Obtain the revealed random value from Switchboard.
    let revealed_random_value = randomness_data
        .get_value(&clock)
        .map_err(|_| ErrorCode::RandomnessNotResolved)?;

    // Derive symbol IDs from the random values for the lottery outcome.
    let symbol1_id = get_symbol_id(revealed_random_value[0]);
    let symbol2_id = get_symbol_id(revealed_random_value[1]);
    let symbol3_id = get_symbol_id(revealed_random_value[2]);

    let symbols = [symbol1_id, symbol2_id, symbol3_id];

    // Calculate the multiplier for the player's winnings based on the revealed symbols.
    let multiplier = calculate_multiplier(symbols);

    // Update player's spin symbols, multiplier, and result revealed flag.
    player_data.spin_symbols = symbols;
    player_data.result_multiplier = multiplier;
    player_data.result_revealed = true;
    player_data.commit_slot = 0;

    // Calculate lottery rewards if the player wins.
    let lottery_rewards = ONCE_DRAW_LOTTERY_VOUCHER_COST.safe_mul(multiplier as u64)?;

    // If multiplier > 0, player wins and receives lottery rewards.
    if multiplier > 0 {
        // Deduct lottery rewards from the game's lottery pool.
        game.lottery_rewards_pool_balance = game
            .lottery_rewards_pool_balance
            .safe_sub(lottery_rewards)?;
        game.distributed_lottery_rewards =
            game.distributed_lottery_rewards.safe_add(lottery_rewards)?;

        // Update player's collected lottery rewards tally.
        player_data.collected_lottery_rewards = player_data
            .collected_lottery_rewards
            .safe_add(lottery_rewards)?;

        // Transfer the winning tokens to the player's token account.
        transfer_from_token_vault_to_token_account(
            game,
            &game_vault,
            &token_account,
            &token_program,
            lottery_rewards,
            &[GAME_SEED, &[ctx.bumps.game]],
        )?;
    }

    msg!(
        "Revealed draw lottery result: {}, {} tokens",
        multiplier,
        lottery_rewards
    );

    game.increment_event_nonce()?;

    // Emit the event capturing the revealed draw lottery result,
    // including the symbols, multiplier, and any awarded lottery rewards.
    emit!(TransferEvent {
        event_type: EventType::RevealDrawLotteryResult,
        event_nonce: game.event_nonce,
        data: EventData::RevealDrawLotteryResult {
            game: game.key(),
            player: player.key(),
            symbols,
            multiplier,
            lottery_rewards,
        },
        initiator_type: InitiatorType::LOTTERY,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}
