use crate::constants::{
    CONSTRUCTION_POOL_SHARE, CONSUMPTION_POOL_SHARE, GAME_SEED, GRAND_PRIZES_POOL_SHARE,
    LAMPORTS_PER_ORE, LOTTERY_POOL_SHARE, PLAYER_DATA_SEED, REFERRAL_POOL_SHARE, TOKEN_MINT,
    VOUCHER_MINT_SEED, VOUCHER_SEED,
};
use crate::errors::ErrorCode;
use crate::events::{EventData, EventType, InitiatorType, TransferEvent};
use crate::state::*;
use crate::utils::{
    calculate_proportion, redeem_vouchers, timestamp_to_days, to_timestamp_u64,
    transfer_from_player_to_vault,
};
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;
use anchor_spl::token::{self, burn, Burn, Mint, Token, TokenAccount};
use solana_program::sysvar::clock::Clock;
use std::cmp::min;

/// The `Purchase` instruction enables players to buy ORE tokens within the current round, affecting various in-game pools and distributions.
/// Through this action, players potentially earn wages, access continuous purchase rewards, and contribute to multiple reward pools (bonus, lottery, construction, etc.).
///
/// Steps:
/// 1. Validate that the current round has started and handle edge cases if the round end conditions are met.
/// 2. Ensure the player has sufficient funds (vouchers + tokens) to cover the ORE purchase cost.
/// 3. Calculate proportional allocations (construction, bonus, lottery, developer, referral, and grand prize pools) from the purchase amount.
/// 4. Update round and game-level states, adjusting earnings_per_ore, sold_ores, participant lists, and round end time.
/// 5. Manage player state: update consecutive purchase days, settle pending construction rewards, and adjust ORE holdings and earnings_per_ore.
/// 6. If vouchers are used as payment, burn them and redeem underlying tokens. Also, transfer funds from player accounts to game and round vaults as required.
/// 7. Emit a `Purchase` event to record the transaction on-chain.
#[derive(Accounts)]
pub struct Purchase<'info> {
    /// The player making the purchase. Must sign the transaction.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The player's data account, linking to their token and voucher accounts, and indicating their current team.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player.key().as_ref()],
        bump,
        has_one = token_account,
        has_one = voucher_account,
        has_one = team,
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// The referrer's data account, tracking pending referral rewards due to them.
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED, player_data.referrer.as_ref()],
        bump
    )]
    pub referrer_data: Box<Account<'info, PlayerData>>,

    /// The global `Game` account, referencing current round, period, and main vault.
    #[account(
        mut,
        seeds = [GAME_SEED],
        bump,
        has_one = current_round,
        has_one = current_period,
        has_one = game_vault,
    )]
    pub game: Box<Account<'info, Game>>,

    /// The current round account, must be active (not ended), referencing its `round_vault`.
    #[account(
        mut,
        constraint = !current_round.is_over @ ErrorCode::RoundAlreadyEnded,
        has_one = round_vault,
    )]
    pub current_round: Box<Account<'info, Round>>,

    /// The current period account representing a leaderboard period.
    #[account(mut)]
    pub current_period: Box<Account<'info, Period>>,

    /// The team account the player belongs to, or the default team if none.
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,

    /// The global voucher account, managing voucher issuance authority and linking to `voucher_vault`.
    #[account(
        mut,
        seeds = [VOUCHER_SEED], bump,
        has_one = voucher_vault,
    )]
    pub voucher: Box<Account<'info, Voucher>>,

    /// The main game vault holding the platform's aggregated funds.
    #[account(mut)]
    pub game_vault: Box<Account<'info, TokenAccount>>,

    /// The round-specific vault holding tokens allocated for the current round.
    #[account(mut)]
    pub round_vault: Box<Account<'info, TokenAccount>>,

    /// The voucher vault account holding underlying assets backing voucher tokens.
    #[account(mut)]
    pub voucher_vault: Box<Account<'info, TokenAccount>>,

    /// The player's token account used to pay part of the purchase cost and receive rewards.
    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// The player's voucher account, storing voucher tokens that can be burned for payment.
    #[account(mut)]
    pub voucher_account: Box<Account<'info, TokenAccount>>,

    /// The voucher mint account used for issuing and burning voucher tokens.
    #[account(mut, seeds = [VOUCHER_MINT_SEED], bump)]
    pub voucher_mint: Box<Account<'info, Mint>>,

    /// The token mint account used for issuing and burning token tokens.
    #[account(mut, address = TOKEN_MINT)]
    pub token_mint: Box<Account<'info, Mint>>,

    /// The SPL Token program used for token operations like minting, burning, and transfers.
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

/// Handles the `Purchase` logic, applying cost calculations, distribution of funds to various pools,
/// updating leaderboards and player states, and managing the round lifecycle if conditions warrant ending the round.
pub fn purchase(ctx: Context<Purchase>, purchased_ores: u32) -> Result<()> {
    // Obtain current Solana time for logic and event logging
    let clock = Clock::get()?;
    let timestamp = to_timestamp_u64(clock.unix_timestamp)?;

    // Extract references for clarity
    let Purchase {
        player,
        player_data,
        token_account,
        voucher_account,
        game,
        game_vault,
        current_round,
        round_vault,
        current_period,
        team,
        voucher,
        voucher_vault,
        voucher_mint,
        referrer_data,
        token_mint,
        token_program,
        ..
    } = ctx.accounts;

    // Validate that the current round is active (has started)
    require!(
        current_round.start_time <= timestamp,
        ErrorCode::RoundNotStarted
    );

    game.increment_event_nonce()?;

    // If the round end_time has passed and no ORE are purchased, handle round end scenario
    if current_round.end_time <= timestamp && purchased_ores == 0 {
        handle_round_end(current_round, current_period, clock.slot, timestamp)?;

        emit!(TransferEvent {
            event_type: EventType::RoundEnd,
            event_nonce: game.event_nonce,
            data: EventData::RoundEnd {
                round: current_round.key(),
                period: current_period.key(),
                call_count: current_round.call_count,
                last_call_slot: current_round.last_call_slot,
            },
            initiator_type: InitiatorType::SYSTEM,
            initiator: player.key(),
            timestamp,
        });
        return Ok(());
    }

    // Ensure a positive ORE purchase quantity
    require!(
        purchased_ores > 0,
        ErrorCode::PurchaseQuantityMustGreaterThanZero
    );

    // The player must have settled previous rounds or must already be in this current round
    require!(
        player_data.is_exited || player_data.current_round == current_round.key(),
        ErrorCode::NeedToSettlePreviousRound
    );

    // Calculate total cost in lamports for the requested ORE quantity
    let total_cost = LAMPORTS_PER_ORE.safe_mul(purchased_ores as u64)?;

    // Determine player's available voucher and token balances
    let voucher_balance: u64 = voucher_account.amount;
    let token_balance: u64 = token_account.amount;

    // Decide how much cost is covered by vouchers vs tokens
    let voucher_cost = min(voucher_balance, total_cost);
    let token_cost = total_cost.safe_sub(voucher_cost)?;

    // Check if total funds (vouchers + tokens) cover the total_cost
    let player_balance = token_balance.safe_add(voucher_balance)?;
    require!(
        player_balance >= total_cost,
        ErrorCode::InsufficientFundsToPayFee
    );

    let current_ores = current_round.available_ores;

    // Calculate proportional rewards for various pools
    let construction_rewards = calculate_proportion(total_cost, CONSTRUCTION_POOL_SHARE)?;
    let bonus_rewards = construction_rewards;
    let lottery_rewards = calculate_proportion(total_cost, LOTTERY_POOL_SHARE)?;
    let referral_rewards = calculate_proportion(total_cost, REFERRAL_POOL_SHARE)?;
    let grand_prizes_rewards = calculate_proportion(total_cost, GRAND_PRIZES_POOL_SHARE)?;
    let consumption_rewards = calculate_proportion(token_cost, CONSUMPTION_POOL_SHARE)?;
    let developer_rewards = calculate_proportion(token_cost, CONSUMPTION_POOL_SHARE)?;

    let current_round_key = current_round.key();
    let current_period_key = current_period.key();
    let current_day = timestamp_to_days(timestamp)?;

    // Update the player to reflect they are now in the current round and period
    player_data.current_round = current_round_key;
    if player_data.current_period != current_period_key {
        player_data.current_period_purchased_ores = 0;
    }
    player_data.current_period = current_period_key;
    // Update consecutive purchase days if needed
    if player_data.last_purchased_day != current_day {
        if player_data.last_purchased_day + 1 == current_day {
            player_data.consecutive_purchased_days =
                player_data.consecutive_purchased_days.safe_add(1)?;
        } else {
            player_data.consecutive_purchased_days = 1;
        }
    }
    player_data.last_purchased_day = current_day;
    // Mark player as not exited since they are making a new purchase
    player_data.is_exited = false;
    // Update team to reflect they are now in the current period
    if team.current_period != current_period_key {
        team.current_period_purchased_ores = 0;
    }
    team.current_period = current_period_key;

    // Update game-level pools
    if current_ores > 0 {
        game.construction_rewards_pool_balance = game
            .construction_rewards_pool_balance
            .safe_add(construction_rewards)?;
        game.bonus_rewards_pool_balance =
            game.bonus_rewards_pool_balance.safe_add(bonus_rewards)?;
    } else {
        current_round.grand_prize_pool_balance = current_round
            .grand_prize_pool_balance
            .safe_add(construction_rewards)?
            .safe_add(bonus_rewards)?;
    }
    game.lottery_rewards_pool_balance = game
        .lottery_rewards_pool_balance
        .safe_add(lottery_rewards)?;
    if player_data.referrer != game.default_player {
        game.referral_rewards_pool_balance = game
            .referral_rewards_pool_balance
            .safe_add(referral_rewards)?;
    }

    // Update round-level pools
    current_round.grand_prize_pool_balance = current_round
        .grand_prize_pool_balance
        .safe_add(grand_prizes_rewards)?;

    if current_ores > 0 {
        // Update earnings_per_ore in the round
        let available_ores = current_round.available_ores.max(1);
        let earnings_per_ore_increment = construction_rewards.safe_div(available_ores as u64)?;
        current_round.earnings_per_ore = current_round
            .earnings_per_ore
            .safe_add(earnings_per_ore_increment)?;
    }

    // Update round state: sold ORE, participant list, end time
    current_round.available_ores = current_round.available_ores.safe_add(purchased_ores)?;
    current_round.sold_ores = current_round.sold_ores.safe_add(purchased_ores)?;
    current_round.update_last_active_participant_list(player.key())?;
    current_round.update_end_time(timestamp)?;

    // Settle any pending construction rewards before adding newly purchased ORE
    player_data.settle_collectable_construction_rewards(current_round.earnings_per_ore)?;

    // Update player ORE holdings and earnings rate
    player_data.available_ores = player_data.available_ores.safe_add(purchased_ores)?;
    player_data.purchased_ores = player_data.purchased_ores.safe_add(purchased_ores)?;

    if player_data.referrer != game.default_player {
        // Add referral rewards to the referrer's pending rewards
        referrer_data.collectable_referral_rewards = referrer_data
            .collectable_referral_rewards
            .safe_add(referral_rewards)?;
    }

    // If the player is part of a team, update team ORE and period data
    team.purchased_ores = team.purchased_ores.safe_add(purchased_ores)?;
    team.last_updated_timestamp = timestamp;

    // If the current period is ongoing, update leaderboards
    if current_period.is_ongoing(timestamp) {
        player_data.current_period_purchased_ores = player_data
            .current_period_purchased_ores
            .safe_add(purchased_ores)?;
        current_period
            .update_top_player(player.key(), player_data.current_period_purchased_ores)?;

        team.current_period_purchased_ores = team
            .current_period_purchased_ores
            .safe_add(purchased_ores)?;
        if player_data.team != game.default_team {
            current_period.update_top_team_list(team.key(), team.current_period_purchased_ores)?;
        }
    }

    // If mining pool balance is enough, add developer rewards
    if developer_rewards > 0 && game.consumption_rewards_pool_balance >= developer_rewards {
        game.consumption_rewards_pool_balance = game
            .consumption_rewards_pool_balance
            .safe_sub(developer_rewards)?;
        game.distributable_consumption_rewards = game
            .distributable_consumption_rewards
            .safe_sub(developer_rewards)?;
        game.developer_rewards_pool_balance = game
            .developer_rewards_pool_balance
            .safe_add(developer_rewards)?;
        msg!(
            "Developer consumption pool increased by {}.",
            developer_rewards
        );
    }

    // If tokens are used (token_cost > 0), add consumption rewards
    if consumption_rewards > 0 && game.distributable_consumption_rewards >= consumption_rewards {
        game.distributable_consumption_rewards = game
            .distributable_consumption_rewards
            .safe_sub(consumption_rewards)?;
        player_data.collectable_consumption_rewards = player_data
            .collectable_consumption_rewards
            .safe_add(consumption_rewards)?;
        msg!(
            "Player earned {} consumption rewards for spending {} tokens.",
            consumption_rewards,
            token_cost
        );
    }

    // If vouchers are used to pay (voucher_cost > 0), burn them and redeem underlying tokens
    if voucher_cost > 0 {
        voucher.burn(voucher_cost)?;

        burn(
            CpiContext::new(
                token_program.to_account_info(),
                Burn {
                    mint: voucher_mint.to_account_info(),
                    from: voucher_account.to_account_info(),
                    authority: player.to_account_info(),
                },
            ),
            voucher_cost,
        )?;

        redeem_vouchers(
            voucher,
            voucher_vault,
            token_account,
            token_program,
            voucher_cost,
            &[VOUCHER_SEED, &[ctx.bumps.voucher]],
        )?;

        msg!(
            "Burned {} vouchers from the player's account.",
            voucher_cost
        );
    }

    let mut transfer_to_game_vault_amount = lottery_rewards.safe_add(referral_rewards)?;

    let mut transfer_to_round_vault_amount = grand_prizes_rewards;

    if current_ores > 0 {
        transfer_to_game_vault_amount = transfer_to_game_vault_amount
            .safe_add(construction_rewards)?
            .safe_add(bonus_rewards)?;
    } else {
        transfer_to_round_vault_amount = transfer_to_round_vault_amount
            .safe_add(construction_rewards)?
            .safe_add(bonus_rewards)?;
    }

    // Transfer various pools' cost allocations to the appropriate vaults
    // Bonus, lottery, developer, and referral rewards go to the game_vault
    transfer_from_player_to_vault(
        player,
        token_account,
        game_vault,
        token_program,
        transfer_to_game_vault_amount,
    )?;

    if player_data.referrer == game.default_player {
        burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: token_mint.to_account_info(),
                    from: game_vault.to_account_info(),
                    authority: game.to_account_info(),
                },
                &[&[GAME_SEED, &[ctx.bumps.game]]],
            ),
            referral_rewards,
        )?;
    }

    // Construction and grand prize rewards go to the round_vault
    transfer_from_player_to_vault(
        player,
        token_account,
        round_vault,
        token_program,
        transfer_to_round_vault_amount,
    )?;

    // Emit an event recording the purchase
    emit!(TransferEvent {
        event_type: EventType::Purchase,
        event_nonce: game.event_nonce,
        data: EventData::Purchase {
            game: game.key(),
            round: current_round.key(),
            period: current_period.key(),
            player: player.key(),
            referrer: player_data.referrer,
            team: team.key(),
            purchased_ores,
            voucher: voucher.key(),
        },
        initiator_type: InitiatorType::PLAYER,
        initiator: player.key(),
        timestamp,
    });

    Ok(())
}

/// Handle round-end conditions if no ORE is purchased and the end_time has passed.
/// This function checks certain Solana slot conditions and adjusts the round state accordingly.
fn handle_round_end(
    current_round: &mut Round,
    current_period: &mut Period,
    current_slot: u64,
    timestamp: u64,
) -> Result<()> {
    if current_slot < current_round.last_call_slot.safe_add(150)? {
        msg!("Call count must be after 150 slots");
        msg!(
            "Current round last call slot: {}",
            current_round.last_call_slot
        );
        msg!("Current slot: {}", current_slot);
        return Ok(());
    }

    current_round.last_call_slot = current_slot;
    current_round.call_count += 1;

    // After a specific number of calls (e.g., 10), mark the round as over.
    if current_round.call_count >= 10 {
        current_round.is_over = true;
        // If the current period is ongoing, end it now.
        // If the period hasn't started (start_time > timestamp), adjust period times.
        if current_period.is_ongoing(timestamp) {
            current_period.end_time = timestamp;
        } else if current_period.start_time > timestamp {
            current_period.start_time = timestamp;
            current_period.end_time = timestamp;
        }
    }

    Ok(())
}
