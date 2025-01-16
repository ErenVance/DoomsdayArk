use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

const MAX_TEAM_APPLICATIONS: usize = 3;

/// The `PlayerData` account maintains state for an individual player within the game.
/// It tracks the player's associated accounts, their team status, referral relationships,
/// participation in rounds and periods, and various types of rewards (referral, construction, grand prize, lottery, etc.).
/// It also records player-specific configuration such as auto-reinvestment settings, purchased ore quantities, and earned rewards.
///
/// # Fields
/// - `player`: The public key identifying this player.
/// - `token_account`: The player's main token account holding in-game currency.
/// - `voucher_account`: The player's voucher account representing staked or deposited tokens.
/// - `team`: The public key of the team the player currently belongs to. If this is the `default_team`, the player is effectively team-less.
/// - `team_applications`: A list of teams to which the player has applied but not yet joined.
///   Limited by `MAX_TEAM_APPLICATIONS` to prevent spam and complexity.
/// - `can_apply_to_team_timestamp`: A UNIX timestamp indicating when the player can next apply to a team.
///   Useful for cooldowns or preventing immediate re-application after leaving a team.
/// - `referrer`: The public key of the entity who referred this player, if any.
/// - `referral_count`: How many players this player has referred.
/// - `collectable_referral_rewards`: Accumulated referral rewards not yet collected.
/// - `collected_referral_rewards`: Total referral rewards already collected by this player.
/// - `current_round`, `current_period`: Identify which round and period the player is currently participating in, used for calculating round/period-specific earnings.
/// - `current_period_purchased_ores`: The amount of ores purchased by this player in the current period, used for leaderboard or reward calculations.
/// - `earnings_per_ore`: The player's current earnings rate per ore unit in the ongoing round.
/// - `collectable_construction_rewards`, `collected_construction_rewards`: Track construction-related rewards (e.g., rewards from building game infrastructure).
/// - `collected_grand_prizes`: Total grand prizes that the player has already claimed.
/// - `available_ores`: The amount of ore available to the player in the current round.
/// - `purchased_ores`: The player's total purchased ore quantity (a historical record, previously called "score").
/// - `is_auto_reinvesting`: Indicates whether earnings are automatically reinvested for compounding returns.
/// - `consecutive_purchased_days`: How many consecutive days the player has made a purchase, useful for streak-based rewards.
/// - `last_purchased_day`: The most recent day on which the player purchased ores, helping track consecutive purchase streaks.
/// - `last_collected_airdrop_reward_day`: The day on which the player last collected airdrop rewards, enforcing daily airdrop limits.
/// - `collected_airdrop_rewards`: How many airdrop rewards the player has accumulated so far.
/// - `randomness_provider`, `commit_slot`, `spin_symbols`, `result_multiplier`, `result_revealed`:
///   Fields tracking the player's lottery spin or randomness-based game interactions, including the randomness provider account and the outcome of a spin.
/// - `collectable_consumption_rewards`, `collected_consumption_rewards`: Track rewards based on player consumption or spending behavior in the game.
/// - `is_exited`: Indicates whether the player has exited the game, resetting round participation and disabling certain activities.
/// - `collected_exit_rewards`: Total exit rewards collected by the player.
/// - `collected_lottery_rewards`, `collected_individual_rewards`, `collected_team_rewards`: Tally various categories of collected rewards for accounting and analytics.
/// - `nonce`: A counter used for generating unique PDAs or other player-specific keys.
#[account]
#[derive(Debug, Default, InitSpace)]
pub struct PlayerData {
    // Basic account info
    pub player: Pubkey,
    pub token_account: Pubkey,
    pub voucher_account: Pubkey,
    pub nonce: u16,

    // Team related
    pub team: Pubkey,
    #[max_len(MAX_TEAM_APPLICATIONS)]
    pub team_applications: Vec<Pubkey>,
    pub can_apply_to_team_timestamp: u64,

    // Referral related
    pub referrer: Pubkey,
    pub referral_count: u16,
    pub collectable_referral_rewards: u64,
    pub collected_referral_rewards: u64,

    // Round & Period related
    pub current_round: Pubkey,
    pub current_period: Pubkey,
    pub current_period_purchased_ores: u32,
    pub is_exited: bool,

    pub earnings_per_ore: u64,
    pub collectable_construction_rewards: u64,

    // Ore related
    pub available_ores: u32,
    pub purchased_ores: u32,
    pub is_auto_reinvesting: bool,

    // Purchase tracking
    pub consecutive_purchased_days: u16,
    pub last_purchased_day: u32,

    // Airdrop related
    pub last_collected_airdrop_reward_day: u32,
    pub collected_airdrop_rewards: u64,

    // Randomness & Spin related
    pub randomness_provider: Pubkey,
    pub commit_slot: u64,
    pub spin_symbols: [u8; 3],
    pub result_multiplier: u16,
    pub result_revealed: bool,

    // Rewards related
    pub collected_construction_rewards: u64,
    pub collected_grand_prizes: u64,
    pub collectable_consumption_rewards: u64,
    pub collected_consumption_rewards: u64,
    pub collected_exit_rewards: u64,
    pub collected_lottery_rewards: u64,
    pub collected_individual_rewards: u64,
    pub collected_team_rewards: u64,
}

impl PlayerData {
    /// Initialize a player with default values and the provided accounts.
    ///
    /// # Arguments
    /// - `player`: The player's public key.
    /// - `referrer`: The public key of the referrer who introduced this player (may be `Pubkey::default()` if none).
    /// - `team`: The public key of the team the player initially joins (or a default team if none).
    /// - `token_account`: The player's main token account.
    /// - `voucher_account`: The player's voucher account.
    ///
    /// # Returns
    /// Returns `Ok(())` if initialization succeeds.
    pub fn initialize(
        &mut self,
        player: Pubkey,
        referrer: Pubkey,
        team: Pubkey,
        token_account: Pubkey,
        voucher_account: Pubkey,
    ) -> Result<()> {
        *self = PlayerData {
            player,
            referrer,
            team,
            token_account,
            voucher_account,
            team_applications: Vec::with_capacity(MAX_TEAM_APPLICATIONS),
            is_auto_reinvesting: false,
            is_exited: true,
            spin_symbols: [0; 3],
            result_revealed: true,
            nonce: 1,
            ..Default::default()
        };

        Ok(())
    }

    /// Increments the `nonce` to maintain unique derivations for player-related accounts.
    pub fn increment_nonce(&mut self) -> Result<()> {
        self.nonce = self.nonce.safe_add(1)?;
        Ok(())
    }

    /// Sets a new referrer for the player.
    ///
    /// # Arguments
    /// - `referrer`: The public key of the new referrer.
    pub fn set_referrer(&mut self, referrer: Pubkey) -> Result<()> {
        self.referrer = referrer;
        Ok(())
    }

    /// Increments the referral count by one, tracking how many new players this player has introduced.
    pub fn increment_referral_count(&mut self) -> Result<()> {
        self.referral_count = self.referral_count.safe_add(1)?;
        Ok(())
    }

    /// Checks if a given team is already in the player's team application list.
    pub fn is_team_application_list_contains(&self, team: Pubkey) -> bool {
        self.team_applications.contains(&team)
    }

    /// Checks if the team application list is currently full.
    pub fn is_team_application_list_full(&self) -> bool {
        self.team_applications.len() == MAX_TEAM_APPLICATIONS
    }

    /// Joins a given team, updating the player's current `team` field.
    pub fn join_team(&mut self, team: Pubkey) -> Result<()> {
        self.team = team;
        Ok(())
    }

    /// Leaves the current team and resets the `team` to `default_team`.
    /// Also updates `can_apply_to_team_timestamp` to prevent immediate reapplication.
    pub fn leave_team(
        &mut self,
        default_team: Pubkey,
        can_apply_to_team_timestamp: u64,
    ) -> Result<()> {
        require!(self.team != default_team, ErrorCode::PlayerIsNotInTeam);
        self.team = default_team;
        self.can_apply_to_team_timestamp = can_apply_to_team_timestamp;
        Ok(())
    }

    /// Applies to join a new team, adding it to the player's application list if space is available and not already present.
    pub fn apply_to_join_team(&mut self, team: Pubkey) -> Result<()> {
        require!(
            !self.is_team_application_list_full(),
            ErrorCode::PlayerTeamApplicationListFull
        );
        require!(
            !self.is_team_application_list_contains(team),
            ErrorCode::PlayerAlreadyAppliedToThisTeam
        );

        self.team_applications.push(team);
        Ok(())
    }

    /// Accepts a team application and clears all team applications.
    pub fn accept_team_application(&mut self, team: Pubkey) -> Result<()> {
        require!(
            self.is_team_application_list_contains(team),
            ErrorCode::PlayerTeamApplicationNotFound
        );
        self.join_team(team)?;
        self.clear_team_applications()?;
        Ok(())
    }

    /// Rejects a team application by removing it from the application list.
    pub fn reject_team_application(&mut self, team: Pubkey) -> Result<()> {
        require!(
            self.is_team_application_list_contains(team),
            ErrorCode::PlayerTeamApplicationNotFound
        );
        self.cancel_team_application(team)?;
        Ok(())
    }

    /// Clears all team applications from the player's list, resetting it.
    fn clear_team_applications(&mut self) -> Result<()> {
        self.team_applications.clear();
        Ok(())
    }

    /// Removes a specific team from the application list.
    fn cancel_team_application(&mut self, team: Pubkey) -> Result<()> {
        self.team_applications.retain(|&x| x != team);
        Ok(())
    }

    /// Adds referral rewards to the player's pending referral rewards balance.
    pub fn add_collectable_referral_rewards(&mut self, referral_rewards: u64) -> Result<()> {
        self.collectable_referral_rewards = self
            .collectable_referral_rewards
            .safe_add(referral_rewards)?;
        Ok(())
    }

    /// Collects construction rewards, adding them to the total collected construction rewards.
    pub fn collect_construction_rewards(&mut self, construction_rewards: u64) -> Result<()> {
        self.collected_construction_rewards = self
            .collected_construction_rewards
            .safe_add(construction_rewards)?;
        Ok(())
    }

    /// Collects grand prizes, incrementing the total grand prizes the player has received.
    pub fn collect_grand_prizes(&mut self, grand_prizes: u64) -> Result<()> {
        self.collected_grand_prizes = self.collected_grand_prizes.safe_add(grand_prizes)?;
        Ok(())
    }

    /// Collects lottery rewards.
    pub fn collect_lottery_rewards(&mut self, lottery_rewards: u64) -> Result<()> {
        self.collected_lottery_rewards =
            self.collected_lottery_rewards.safe_add(lottery_rewards)?;
        Ok(())
    }

    /// Collects individual rewards accrued by the player.
    pub fn collect_individual_rewards(&mut self, individual_rewards: u64) -> Result<()> {
        self.collected_individual_rewards = self
            .collected_individual_rewards
            .safe_add(individual_rewards)?;
        Ok(())
    }

    /// Collects team rewards.
    pub fn collect_team_rewards(&mut self, team_rewards: u64) -> Result<()> {
        self.collected_team_rewards = self.collected_team_rewards.safe_add(team_rewards)?;
        Ok(())
    }

    /// Settles pending construction rewards based on changes in `earnings_per_ore`.
    /// This is used, for instance, when an updated earnings rate is applied after a round ends,
    /// enabling additional construction rewards to be calculated.
    pub fn settle_collectable_construction_rewards(
        &mut self,
        round_earnings_per_ore: u64,
    ) -> Result<()> {
        let delta_earnings_per_ore = round_earnings_per_ore.safe_sub(self.earnings_per_ore)?;
        let additional_rewards_fraction =
            delta_earnings_per_ore.safe_mul(self.available_ores as u64)?;
        self.earnings_per_ore = round_earnings_per_ore;
        self.collectable_construction_rewards = self
            .collectable_construction_rewards
            .safe_add(additional_rewards_fraction)?;
        Ok(())
    }

    /// Updates the randomness-related fields, resetting spin symbols and result state.
    pub fn update_randomness(
        &mut self,
        randomness_provider: Pubkey,
        commit_slot: u64,
    ) -> Result<()> {
        self.randomness_provider = randomness_provider;
        self.commit_slot = commit_slot;
        self.spin_symbols = [0; 3];
        self.result_multiplier = 0;
        self.result_revealed = false;
        Ok(())
    }

    /// Exits the current round, clearing round and period-specific data and resetting certain fields to their default states.
    pub fn exit_round(&mut self) -> Result<()> {
        self.earnings_per_ore = 0;
        self.available_ores = 0;
        self.is_auto_reinvesting = false;
        self.is_exited = true;
        Ok(())
    }

    /// Resets period-specific data without fully exiting the round,
    /// useful when a new period starts and previous period counts should be cleared.
    pub fn reset_period_data(&mut self) -> Result<()> {
        self.current_period_purchased_ores = 0;
        Ok(())
    }
}
