use anchor_lang::prelude::*;

/// Internal modules and utilities
#[doc(hidden)]
pub mod constants;
#[doc(hidden)]
pub mod errors;
pub mod events;
#[doc(hidden)]
pub mod instructions;
pub mod state;
#[doc(hidden)]
pub mod utils;

use instructions::*;

declare_id!("HCMBs4McFkMXzrCi9xbgSejtok3q8qD2WHZbbHwGxWLy");

#[program]
mod game {
    use super::*;

    /// Automatically reinvests a player's earnings back into the staking or
    /// game pool to compound returns.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `player`: The public key of the player whose earnings are to be reinvested.
    pub fn auto_reinvest(ctx: Context<AutoReinvest>, player: Pubkey) -> Result<()> {
        instructions::auto_reinvest::auto_reinvest(ctx, player)
    }

    /// Collects accumulated developer rewards from the contract's reward pool.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn collect_developer_rewards(ctx: Context<CollectDeveloperRewards>) -> Result<()> {
        instructions::collect_developer_rewards::collect_developer_rewards(ctx)
    }

    /// Distributes grand prizes to a specified player at the end of a round or
    /// promotional period.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `index`: An identifier to select which prize to distribute.
    /// - `player`: The public key of the prize recipient.
    pub fn distribute_grand_prizes(
        ctx: Context<DistributeGrandPrizes>,
        index: u8,
        player: Pubkey,
    ) -> Result<()> {
        instructions::distribute_grand_prizes::distribute_grand_prizes(ctx, index, player)
    }

    /// Distributes rewards to the top-ranking players on the leaderboard.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `player_leaderboard_winner`: The public key of the winner who topped the leaderboard.
    pub fn distribute_leaderboard_rewards(
        ctx: Context<DistributeLeaderboardRewards>,
        player_leaderboard_winner: Pubkey,
    ) -> Result<()> {
        instructions::distribute_leaderboard_rewards::distribute_leaderboard_rewards(
            ctx,
            player_leaderboard_winner,
        )
    }

    /// Initializes a default player account, preparing it for participation in the game.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn initialize_default_player(ctx: Context<InitializeDefaultPlayer>) -> Result<()> {
        instructions::initialize_default_player::initialize_default_player(ctx)
    }

    /// Initializes a default team, enabling group-based participation and rewards.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn initialize_default_team(ctx: Context<InitializeDefaultTeam>) -> Result<()> {
        instructions::initialize_default_team::initialize_default_team(ctx)
    }

    pub fn initialize_vault(
        ctx: Context<InitializeVault>,
        token_mint: Pubkey,
        token_amount: u64,
    ) -> Result<()> {
        instructions::initialize_vault::initialize_vault(ctx, token_mint, token_amount)
    }

    /// Creates a new competition period, specifying start time, leaderboard duration, and reward allocations.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `start_time`: The UNIX timestamp when the period begins.
    /// - `leaderboard_duration`: The duration of the leaderboard phase in seconds.
    /// - `team_rewards`: The total reward amount allocated for teams.
    /// - `individual_rewards`: The total reward amount allocated for individual players.
    pub fn create_period(
        ctx: Context<CreatePeriod>,
        start_time: u64,
        leaderboard_duration: u64,
        team_rewards: u64,
        individual_rewards: u64,
    ) -> Result<()> {
        instructions::create_period::create_period(
            ctx,
            start_time,
            leaderboard_duration,
            team_rewards,
            individual_rewards,
        )
    }

    /// Creates a new round, specifying start time, duration, and the initial grand prize pool balance.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `start_time`: The UNIX timestamp marking the beginning of the round.
    /// - `countdown_duration`: The length of the round in seconds.
    /// - `initial_grand_prize_pool_balance`: The initial amount of tokens allocated to the grand prize pool.
    pub fn create_round(
        ctx: Context<CreateRound>,
        start_time: u64,
        countdown_duration: u64,
        initial_grand_prize_pool_balance: u64,
    ) -> Result<()> {
        instructions::create_round::create_round(
            ctx,
            start_time,
            countdown_duration,
            initial_grand_prize_pool_balance,
        )
    }

    /// Initializes a stake token pool, enabling tokenized representation of pool deposits.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn initialize_stake_token_pool(
        ctx: Context<InitializeStakeTokenPool>,
        token_rewards: u64,
    ) -> Result<()> {
        instructions::manager::initialize_stake_token_pool::initialize_stake_token_pool(
            ctx,
            token_rewards,
        )
    }

    /// Initializes a stake voucher pool, enabling tokenized representation of pool deposits.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn initialize_stake_voucher_pool(
        ctx: Context<InitializeStakeVoucherPool>,
        voucher_rewards: u64,
    ) -> Result<()> {
        instructions::manager::initialize_stake_voucher_pool::initialize_stake_voucher_pool(
            ctx,
            voucher_rewards,
        )
    }

    /// Initializes a voucher account, allowing for tokenized representation of pool deposits.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn initialize_voucher(ctx: Context<InitializeVoucher>) -> Result<()> {
        instructions::initialize_voucher::initialize_voucher(ctx)
    }

    /// Performs initial setup for the program, allocating necessary state and configuration.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `bot_authority`: The public key of the bot authority.
    pub fn initialize(
        ctx: Context<Initialize>,
        bot_authority: Pubkey,
        round_rewards: u64,
        period_rewards: u64,
        registration_rewards: u64,
        airdrop_rewards: u64,
        exit_rewards: u64,
        lottery_rewards: u64,
        consumption_rewards: u64,
        sugar_rush_rewards: u64,
    ) -> Result<()> {
        instructions::initialize::initialize(
            ctx,
            bot_authority,
            round_rewards,
            period_rewards,
            registration_rewards,
            airdrop_rewards,
            exit_rewards,
            lottery_rewards,
            consumption_rewards,
            sugar_rush_rewards,
        )
    }

    /// Stakes a specified amount of tokens into the pool to earn ongoing rewards.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `amount`: The amount of tokens to stake.
    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        instructions::stake::stake(ctx, amount)
    }

    /// Requests an early unstake of previously staked tokens before the lock-up period ends, possibly incurring penalties.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `order_number`: The identifier of the stake order to be released early.
    pub fn request_early_unstake(
        ctx: Context<RequestEarlyUnstake>,
        order_number: u16,
    ) -> Result<()> {
        instructions::stake::request_early_unstake::request_early_unstake(ctx, order_number)
    }

    /// Unstakes tokens that have reached their required staking period and can now be withdrawn without penalty.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `order_number`: The identifier of the fully matured stake order to be withdrawn.
    pub fn unstake(ctx: Context<Unstake>, order_number: u16) -> Result<()> {
        instructions::unstake::unstake(ctx, order_number)
    }

    /// Cancels the auto-reinvest setting for a player, stopping automatic compounding of earnings.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn cancel_is_auto_reinvesting(ctx: Context<CancelIsAutoReinvesting>) -> Result<()> {
        instructions::cancel_is_auto_reinvesting::cancel_is_auto_reinvesting(ctx)
    }

    /// Candy tap is a function that allows players to tap into the game's rewards pool.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn candy_tap(ctx: Context<CandyTap>, last_active_participant: Pubkey) -> Result<()> {
        instructions::candy_tap::candy_tap(ctx, last_active_participant)
    }

    /// Collects any available airdrop rewards for the player.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn collect_airdrop_rewards(ctx: Context<CollectAirdropRewards>) -> Result<()> {
        instructions::collect_airdrop_rewards::collect_airdrop_rewards(ctx)
    }

    /// Collects rewards earned through player consumption or spending activities.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn collect_consumption_rewards(ctx: Context<CollectConsumptionRewards>) -> Result<()> {
        instructions::collect_consumption_rewards::collect_consumption_rewards(ctx)
    }

    /// Exchanges collateral tokens into the corresponding in-game currency or resource.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `amount`: The amount of collateral to be exchanged.
    pub fn collateral_exchange(ctx: Context<CollateralExchange>, amount: u64) -> Result<()> {
        instructions::collateral_exchange::collateral_exchange(ctx, amount)
    }

    /// Collects referral rewards earned by inviting new participants to the platform.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn collect_referral_rewards(ctx: Context<CollectReferralRewards>) -> Result<()> {
        instructions::collect_referral_rewards::collect_referral_rewards(ctx)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::deposit(ctx, amount)
    }

    /// Conducts a lottery draw to determine winners from a pool of participants.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn draw_lottery(ctx: Context<DrawLottery>) -> Result<()> {
        instructions::draw_lottery::draw_lottery(ctx)
    }

    /// Exits from the current game or round, potentially collecting any accrued exit rewards.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn exit(ctx: Context<Exit>) -> Result<()> {
        instructions::exit::exit(ctx)
    }

    /// Registers a new player into the game, optionally associating them with a referrer.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `referrer`: The public key of the referrer (if any).
    pub fn register(ctx: Context<Register>, referrer: Pubkey) -> Result<()> {
        instructions::register::register(ctx, referrer)
    }

    /// Purchases a specified quantity of in-game assets or lottery entries.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `purchase_quantity`: The number of units or tickets to purchase.
    pub fn purchase(ctx: Context<Purchase>, purchase_quantity: u32) -> Result<()> {
        instructions::purchase::purchase(ctx, purchase_quantity)
    }

    /// Reinvests a player's claims or accrued rewards back into the game environment.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn reinvest(ctx: Context<Reinvest>) -> Result<()> {
        instructions::reinvest::reinvest(ctx)
    }

    /// Reveals the outcome of the previously drawn lottery, finalizing the results on-chain.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn reveal_draw_lottery_result(ctx: Context<RevealDrawLotteryResult>) -> Result<()> {
        instructions::reveal_draw_lottery_result::reveal_draw_lottery_result(ctx)
    }

    /// Enables automatic reinvestment for a player, compounding their returns without manual intervention.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn set_is_auto_reinvesting(ctx: Context<SetIsAutoReinvesting>) -> Result<()> {
        instructions::set_is_auto_reinvesting::set_is_auto_reinvesting(ctx)
    }

    /// Assigns a referrer to a player, establishing referral relationships.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `referrer`: The public key of the referrer.
    pub fn set_referrer(ctx: Context<SetReferrer>, referrer: Pubkey) -> Result<()> {
        instructions::set_referrer::set_referrer(ctx, referrer)
    }

    /// Settles the previous round, finalizing and distributing any outstanding rewards.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn settle_previous_round(ctx: Context<SettlePreviousRound>) -> Result<()> {
        instructions::settle_previous_round::settle_previous_round(ctx)
    }

    /// Accepts a player's application to join a team, officially adding them to the team.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `applicant`: The public key of the player requesting to join.
    pub fn accept_team_application(
        ctx: Context<AcceptTeamApplication>,
        applicant: Pubkey,
    ) -> Result<()> {
        instructions::accept_team_application::accept_team_application(ctx, applicant)
    }

    /// Allows a player to apply to join an existing team, pending acceptance by the team management.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn apply_to_join_team(ctx: Context<ApplyToJoinTeam>) -> Result<()> {
        instructions::apply_to_join_team::apply_to_join_team(ctx)
    }

    /// Creates a new team, enabling a group of players to form a team with collective goals and rewards.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn create_team(ctx: Context<CreateTeam>) -> Result<()> {
        instructions::create_team::create_team(ctx)
    }

    /// Distributes team-level rewards to a specific team member.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `member`: The public key of the team member receiving rewards.
    /// - `reward_amount`: The amount of rewards to distribute.
    pub fn distribute_team_rewards(
        ctx: Context<DistributeTeamRewards>,
        member: Pubkey,
        reward_amount: u64,
    ) -> Result<()> {
        instructions::distribute_team_rewards::distribute_team_rewards(ctx, member, reward_amount)
    }

    /// Grants manager-level privileges within the team to a specific member, allowing them to manage membership and rewards.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `member`: The public key of the member to be granted manager privileges.
    pub fn grant_manager_privileges(
        ctx: Context<GrantManagerPrivileges>,
        member: Pubkey,
    ) -> Result<()> {
        instructions::grant_manager_privileges::grant_manager_privileges(ctx, member)
    }

    /// Allows a member to voluntarily leave a team.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    pub fn leave_team(ctx: Context<LeaveTeam>) -> Result<()> {
        instructions::leave_team::leave_team(ctx)
    }

    /// Rejects a team application from a particular applicant.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `applicant`: The public key of the applicant to reject.
    pub fn reject_team_application(
        ctx: Context<RejectTeamApplication>,
        applicant: Pubkey,
    ) -> Result<()> {
        instructions::reject_team_application::reject_team_application(ctx, applicant)
    }

    /// Removes a member from a team, revoking their participation and any associated privileges.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `member_to_remove`: The public key of the member to be removed.
    pub fn remove_member_from_team(
        ctx: Context<RemoveMemberFromTeam>,
        member_to_remove: Pubkey,
    ) -> Result<()> {
        instructions::remove_member_from_team::remove_member_from_team(ctx, member_to_remove)
    }

    /// Revokes previously granted manager privileges from a specific team member.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `manager`: The public key of the member whose manager privileges will be revoked.
    pub fn revoke_manager_privileges(
        ctx: Context<RevokeManagerPrivileges>,
        manager: Pubkey,
    ) -> Result<()> {
        instructions::revoke_manager_privileges::revoke_manager_privileges(ctx, manager)
    }

    /// Transfers the role of team captain to another member.
    ///
    /// # Parameters
    /// - `ctx`: Execution context.
    /// - `member`: The public key of the member to become the new captain.
    pub fn transfer_team_captaincy(
        ctx: Context<TransferTeamCaptaincy>,
        member: Pubkey,
    ) -> Result<()> {
        instructions::transfer_team_captaincy::transfer_team_captaincy(ctx, member)
    }
}
