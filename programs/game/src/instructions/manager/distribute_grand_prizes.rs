use crate::constants::{GAME_SEED, PLAYER_DATA_SEED, ROUND_SEED, TOKEN_MINT};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{to_timestamp_u64, transfer_from_token_vault_to_token_account};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, burn, Burn, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;

#[derive(Accounts)]
#[instruction(index: u8, player: Pubkey)]
pub struct DistributeGrandPrizes<'info> {
    /// The authority executing the distribution of grand prizes. Must sign the transaction.
    #[account(mut)]
    pub bot_authority: Signer<'info>,

    /// The global game account, linked to round and ensuring authorized access.
    #[account(mut, seeds = [GAME_SEED], bump,
        has_one = bot_authority @ ErrorCode::AuthorityMismatch,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The current round account, which must be ended (is_over = true) and grand_prize_distribution not completed.
    /// Ensures grand_prize_distribution_index < 10, meaning 10 winners maximum.
    #[account(mut,
        constraint = round.is_over @ ErrorCode::RoundInProgress,
        has_one = round_vault,
    )]
    pub round: Box<Account<'info, Round>>,

    /// The player_data account for the recipient of the grand prize.
    /// Must match the `index` and `player` with the round's last_active_participant_list,
    /// ensuring this player is indeed one of the last 10 active participants.
    #[account(mut,
        seeds = [
            PLAYER_DATA_SEED,
            player.as_ref(),
        ],
        bump,
        has_one = token_account,
        constraint = index == round.grand_prize_distribution_index @ ErrorCode::InvalidGrandPrizeIndex,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The round vault token account holding the grand prize tokens.
    #[account(mut)]
    pub round_vault: Box<Account<'info, TokenAccount>>,

    /// The player's token account where grand prizes will be transferred.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The token mint representing the in-game currency.
    #[account(mut, address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The token program used for token transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// The `distribute_grand_prizes` instruction awards one of the last 10 active participants in the round with their portion of the grand prize.
/// This action is typically executed by an authorized entity after the round has ended, distributing prizes in sequence (index 0 to 9).
///
/// Steps:
/// 1. Ensure the round has ended and grand prize distribution is still ongoing (not all 10 winners distributed).
/// 2. Confirm that the `index` and `player` match the next expected winner in `round.last_active_participant_list`.
/// 3. Call `distribute_grand_prizes()` on `round` to determine the reward amount for this winner.
/// 4. Update the `player_data` to record the collected grand prizes.
/// 5. Transfer the grand prize amount from `round_vault` to the player's `token_account`.
/// 6. Emit a `DistributeGrandPrizes` event to record this distribution on-chain.

pub fn distribute_grand_prizes(
    ctx: Context<DistributeGrandPrizes>,
    index: u8,
    player: Pubkey,
) -> Result<()> {
    // Obtain the current UNIX timestamp for event logging and verification.
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    let DistributeGrandPrizes {
        bot_authority,
        game,
        round,
        round_vault,
        token_mint,
        token_account,
        token_program,
        player_data,
        ..
    } = ctx.accounts;

    require!(
        !round.is_grand_prize_distribution_completed,
        ErrorCode::GrandPrizeDistributionAlreadyCompleted,
    );

    require!(
        round
            .last_active_participant_list
            .get(index as usize)
            .ok_or(ErrorCode::InvalidGrandPrizeIndex)?
            == &player,
        ErrorCode::PlayerAddressMismatch,
    );

    // Determine the grand_prizes amount to be distributed from the round's logic.
    let grand_prizes = round.distribute_grand_prizes()?;

    if player_data.player == game.default_player {
        burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: token_mint.to_account_info(),
                    from: round_vault.to_account_info(),
                    authority: round.to_account_info(),
                },
                &[&[
                    ROUND_SEED,
                    round.round_number.to_le_bytes().as_ref(),
                    &[round.bump],
                ]],
            ),
            grand_prizes,
        )?;
    } else {
        game.distributed_grand_prizes = game.distributed_grand_prizes.safe_add(grand_prizes)?;

        // Update the player's data with the collected grand prizes.
        player_data.collect_grand_prizes(grand_prizes)?;

        // Transfer the grand prize tokens from the round vault to the player's token account.
        transfer_from_token_vault_to_token_account(
            round,
            &round_vault,
            &token_account,
            &token_program,
            grand_prizes,
            &[
                ROUND_SEED,
                round.round_number.to_le_bytes().as_ref(),
                &[round.bump],
            ],
        )?;
    }

    game.increment_event_nonce()?;

    // Emit a transfer event capturing this grand prize distribution.
    emit!(TransferEvent {
        event_type: EventType::DistributeGrandPrizes,
        event_nonce: game.event_nonce,
        data: EventData::DistributeGrandPrizes {
            round: round.key(),
            player,
            index,
            grand_prizes
        },
        initiator_type: InitiatorType::SYSTEM,
        initiator: bot_authority.key(),
        timestamp,
    });

    Ok(())
}
