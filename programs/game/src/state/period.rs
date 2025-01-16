use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

/// Number of top player winners recorded for each period.
const PLAYER_WINNERS_COUNT: usize = 10;

/// Number of top team winners recorded for each period.
const TEAM_WINNERS_COUNT: usize = 10;

#[account]
#[derive(Debug, Default, InitSpace)]
/// The `Period` account represents a leaderboard period in the game.
/// Each `Period` tracks a set duration (start and end times), reward allocations for top teams and players,
/// and maintains sorted lists of the top-performing teams and players. Rewards are distributed at the end of the period.
///
/// # Fields
/// - `period_number`: A unique sequential number identifying this period.
/// - `period_vault`: A token account holding resources allocated for the period.
/// - `team_reward_pool_balance`: The total token balance allocated for team rewards during this period.
/// - `individual_reward_pool_balance`: The total token balance allocated for individual player rewards.
/// - `start_time`: The UNIX timestamp marking when the period begins.
/// - `end_time`: The UNIX timestamp marking when the period ends.
/// - `top_player_list`: A vector of `TopPlayerAccount`, each representing a top player's performance (tracked by purchased ores).
/// - `top_team_list`: A vector of `TopTeamAccount`, each representing a top team's performance.
/// - `team_rewards`: The total amount of rewards dedicated to teams.
/// - `team_first_place_rewards`, `team_second_place_rewards`, `team_third_place_rewards`:
///   The share of `team_rewards` allocated to the top three teams, respectively.
/// - `individual_rewards`: The total amount of rewards dedicated to individual players.
/// - `is_distribution_completed`: A boolean flag indicating whether the rewards for this period have been distributed.
/// - `bump`: A PDA bump seed.
pub struct Period {
    pub period_number: u16,
    pub period_vault: Pubkey,

    pub team_reward_pool_balance: u64,
    pub individual_reward_pool_balance: u64,

    pub start_time: u64,
    pub end_time: u64,

    #[max_len(PLAYER_WINNERS_COUNT)]
    pub top_player_list: Vec<TopPlayerAccount>,

    #[max_len(TEAM_WINNERS_COUNT)]
    pub top_team_list: Vec<TopTeamAccount>,

    pub team_rewards: u64,
    pub team_first_place_rewards: u64,
    pub team_second_place_rewards: u64,
    pub team_third_place_rewards: u64,
    pub individual_rewards: u64,

    pub is_distribution_completed: bool,
    pub bump: u8,
}

/// Represents a top-performing player in the `Period`.
/// Each entry stores the player's public key and their total purchased ores,
/// which serve as a performance metric.
#[derive(Debug, InitSpace, AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct TopPlayerAccount {
    /// The public key of the player
    pub player: Pubkey,

    /// The total amount of purchased ores by this player during the period
    pub purchased_ores: u32,
}

/// Represents a top-performing team in the `Period`.
/// Each entry stores the team's public key and total purchased ores,
/// reflecting collective team performance.
#[derive(Debug, InitSpace, AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct TopTeamAccount {
    /// The public key of the team
    pub team: Pubkey,

    /// The total amount of purchased ores by this team during the period
    pub purchased_ores: u32,
}

impl Period {
    /// Initializes a new period with the given configuration parameters.
    /// This includes setting the start and end times, allocating reward pools, and initializing the top players and teams lists.
    ///
    /// # Arguments
    /// - `period_number`: A sequential identifier for the period.
    /// - `period_vault`: The public key of the token vault used for this period's rewards.
    /// - `start_time`: The UNIX timestamp marking when this period starts.
    /// - `leaderboard_duration`: How long this period will run, in seconds.
    /// - `team_rewards`: Total rewards allocated to teams this period.
    /// - `individual_rewards`: Total rewards allocated to individual players this period.
    /// - `default_player`: A default player key used to initialize the top player list.
    /// - `default_team`: A default team key used to initialize the top team list.
    /// - `bump`: The PDA bump seed.
    ///
    /// # Returns
    /// Returns `Ok(())` if successful, or an `InvalidTimestamp` error if the end time computation overflows.
    pub fn initialize(
        &mut self,
        period_number: u16,
        period_vault: Pubkey,
        start_time: u64,
        leaderboard_duration: u64,
        team_rewards: u64,
        individual_rewards: u64,
        default_player: Pubkey,
        default_team: Pubkey,
        bump: u8,
    ) -> Result<()> {
        // Compute the end time safely
        let end_time = start_time
            .checked_add(leaderboard_duration)
            .ok_or(ErrorCode::InvalidTimestamp)?;

        // Compute the distribution for first, second, and third place teams
        let team_first_place_rewards = team_rewards.safe_div(2)?;
        let team_second_place_rewards = team_first_place_rewards.safe_div(5)?.safe_mul(3)?;
        let team_third_place_rewards = team_rewards
            .safe_sub(team_first_place_rewards)?
            .safe_sub(team_second_place_rewards)?;

        *self = Period {
            period_number,
            period_vault,
            start_time,
            end_time,
            team_reward_pool_balance: team_rewards,
            individual_reward_pool_balance: individual_rewards,
            team_rewards,
            team_first_place_rewards,
            team_second_place_rewards,
            team_third_place_rewards,
            individual_rewards,
            top_player_list: vec![
                TopPlayerAccount {
                    player: default_player,
                    purchased_ores: 0,
                };
                PLAYER_WINNERS_COUNT
            ],
            top_team_list: vec![
                TopTeamAccount {
                    team: default_team,
                    purchased_ores: 0,
                };
                TEAM_WINNERS_COUNT
            ],
            bump,
            ..Default::default()
        };

        Ok(())
    }

    /// Checks if the current time falls within the period's active duration.
    ///
    /// # Arguments
    /// - `current_time`: A UNIX timestamp representing the current time.
    ///
    /// # Returns
    /// `true` if `current_time` is between `start_time` and `end_time` (exclusive of end_time), otherwise `false`.
    pub fn is_ongoing(&self, current_time: u64) -> bool {
        current_time >= self.start_time && current_time < self.end_time
    }

    /// Updates or inserts a player's record in the top player list based on purchased ores.
    /// If the player already exists, their ores count is updated; otherwise, a new entry is added.
    /// After updating, the list is re-sorted to maintain the ordering by purchased ores in descending order.
    ///
    /// # Arguments
    /// - `player`: The public key of the player.
    /// - `purchased_ores`: The updated purchased ore count for this player.
    pub fn update_top_player(&mut self, player: Pubkey, purchased_ores: u32) -> Result<()> {
        if let Some(existing_player) = self.top_player_list.iter_mut().find(|p| p.player == player)
        {
            existing_player.purchased_ores = purchased_ores;
        } else {
            self.top_player_list.push(TopPlayerAccount {
                player,
                purchased_ores,
            });
        }

        self.top_player_list
            .sort_by(|a, b| b.purchased_ores.cmp(&a.purchased_ores));

        if self.top_player_list.len() > PLAYER_WINNERS_COUNT {
            self.top_player_list.truncate(PLAYER_WINNERS_COUNT);
        }

        Ok(())
    }

    /// Similar to `update_top_player`, updates or inserts a team record based on purchased ores.
    /// After updating or inserting, the list is sorted to keep top teams in descending order of performance.
    ///
    /// # Arguments
    /// - `team`: The public key of the team.
    /// - `purchased_ores`: The updated purchased ore count for this team.
    pub fn update_top_team_list(&mut self, team: Pubkey, purchased_ores: u32) -> Result<()> {
        if let Some(existing_team) = self.top_team_list.iter_mut().find(|s| s.team == team) {
            existing_team.purchased_ores = purchased_ores;
        } else {
            self.top_team_list.push(TopTeamAccount {
                team,
                purchased_ores,
            });
        }

        self.top_team_list
            .sort_by(|a, b| b.purchased_ores.cmp(&a.purchased_ores));

        if self.top_team_list.len() > TEAM_WINNERS_COUNT {
            self.top_team_list.truncate(TEAM_WINNERS_COUNT);
        }

        Ok(())
    }

    /// Checks if the period has ended.
    ///
    /// # Arguments
    /// - `current_time`: A UNIX timestamp representing the current time.
    ///
    /// # Returns
    /// `true` if `current_time` is greater than or equal to `end_time`, otherwise `false`.
    pub fn is_ended(&self, current_time: u64) -> bool {
        current_time >= self.end_time
    }

    /// Marks this period's rewards distribution as completed.
    /// Fails if distribution was already marked as completed, ensuring that rewards cannot be granted twice.
    pub fn mark_distribution_completed(&mut self) -> Result<()> {
        require!(
            !self.is_distribution_completed,
            ErrorCode::AlreadyDistributed
        );
        self.is_distribution_completed = true;
        Ok(())
    }
}
