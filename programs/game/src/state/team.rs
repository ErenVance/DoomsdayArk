use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

const MAX_APPLICATION_LIST_LENGTH: usize = 10;
const MAX_MEMBER_LIST_LENGTH: usize = 30;
const MAX_MANAGER_LIST_LENGTH: usize = 3;

#[account]
#[derive(Debug, Default, InitSpace)]
/// The `Team` account represents a group of players working as a team within the game.
/// It maintains the team's configuration, including captain, managers, members, and applications.
/// Additional fields track the team's overall performance (total purchased ores) and reward distributions.
///
/// # Fields
/// - `team_number`: A unique identifier for the team.
/// - `team_vault`: A public key referencing the team's token vault holding shared resources or rewards.
/// - `captain`: The public key of the team's captain, who leads the team.
/// - `manager_list`: A list of managers appointed by the captain. Managers have certain administrative privileges.
/// - `member_list`: The list of all members in the team, including the captain and managers.
/// - `application_list`: Pending player applications to join the team.
/// - `current_period`: The current competition period in which the team is participating.
/// - `purchased_ores`: The cumulative total of ores purchased by team members over the team's lifetime.
/// - `current_period_purchased_ores`: The total ores purchased by the team in the current period, useful for leaderboard standings.
/// - `distributable_team_rewards`: The amount of rewards currently available for the team to collect.
/// - `distributed_team_rewards`: The total amount of rewards the team has already claimed.
/// - `last_updated_timestamp`: The UNIX timestamp when the team's data was last updated, useful for time-based logic.
/// - `bump`: A PDA bump seed for the team account.
pub struct Team {
    pub team_number: u32,
    pub team_vault: Pubkey,
    pub captain: Pubkey,

    #[max_len(MAX_MANAGER_LIST_LENGTH)]
    pub manager_list: Vec<Pubkey>,

    #[max_len(MAX_MEMBER_LIST_LENGTH)]
    pub member_list: Vec<Pubkey>,

    #[max_len(MAX_APPLICATION_LIST_LENGTH)]
    pub application_list: Vec<Pubkey>,

    pub current_period: Pubkey,

    pub purchased_ores: u32,
    pub current_period_purchased_ores: u32,

    pub distributable_team_rewards: u64,
    pub distributed_team_rewards: u64,

    pub last_updated_timestamp: u64,

    pub bump: u8,
}

impl Team {
    /// Initializes the team with a given team number, vault, and captain.
    /// The captain is automatically added as the first member.
    ///
    /// # Arguments
    /// - `team_number`: Unique identifier for the team.
    /// - `team_vault`: The public key of the team's token vault.
    /// - `captain`: The public key of the team captain.
    /// - `timestamp`: The current UNIX timestamp, used for `last_updated_timestamp`.
    /// - `bump`: PDA bump seed.
    ///
    /// # Returns
    /// Returns `Ok(())` if successful.
    pub fn initialize(
        &mut self,
        team_number: u32,
        team_vault: Pubkey,
        captain: Pubkey,
        timestamp: u64,
        bump: u8,
    ) -> Result<()> {
        *self = Team {
            team_number,
            team_vault,
            captain,
            member_list: vec![captain], // The captain is the first and founding member
            manager_list: Vec::with_capacity(MAX_MANAGER_LIST_LENGTH),
            application_list: Vec::with_capacity(MAX_APPLICATION_LIST_LENGTH),
            last_updated_timestamp: timestamp,
            bump,
            ..Default::default()
        };

        Ok(())
    }

    /// Checks if a given player is either the captain or one of the managers.
    pub fn is_captain_or_manager(&self, player: Pubkey) -> bool {
        self.is_captain(player) || self.is_manager(player)
    }

    /// Checks if a given player is the team captain.
    pub fn is_captain(&self, player: Pubkey) -> bool {
        player == self.captain
    }

    /// Checks if the team member list is at full capacity.
    fn is_full(&self) -> bool {
        self.member_list.len() >= MAX_MEMBER_LIST_LENGTH
    }

    /// Checks if a given player is a member of the team.
    fn is_member(&self, player: Pubkey) -> bool {
        self.member_list.contains(&player)
    }

    /// Checks if the manager list has reached maximum capacity.
    fn is_manager_list_full(&self) -> bool {
        self.manager_list.len() == MAX_MANAGER_LIST_LENGTH
    }

    /// Checks if a given player is one of the team's managers.
    pub fn is_manager(&self, player: Pubkey) -> bool {
        self.manager_list.contains(&player)
    }

    /// Checks if the application list for joining the team is full.
    fn is_application_list_full(&self) -> bool {
        self.application_list.len() == MAX_APPLICATION_LIST_LENGTH
    }

    /// Checks if a given player is already in the team's application list.
    fn is_application_list_contains(&self, player: Pubkey) -> bool {
        self.application_list.contains(&player)
    }

    /// Allows a player to apply to join the team if there is space and they are not already a member or applicant.
    pub fn apply_to_join_team(&mut self, player: Pubkey) -> Result<()> {
        require!(!self.is_full(), ErrorCode::TeamFull);
        require!(!self.is_member(player), ErrorCode::AlreadyMember);
        require!(
            !self.is_application_list_full(),
            ErrorCode::TeamApplicationListFull
        );
        require!(
            !self.is_application_list_contains(player),
            ErrorCode::TeamApplicationAlreadyExists
        );
        self.application_list.push(player);
        Ok(())
    }

    /// Accepts a player's team application, adding them to the team's member list and removing them from the application list.
    pub fn accept_team_application(&mut self, applicant: Pubkey) -> Result<()> {
        require!(!self.is_full(), ErrorCode::TeamFull);
        require!(!self.is_member(applicant), ErrorCode::AlreadyMember);
        require!(
            self.is_application_list_contains(applicant),
            ErrorCode::TeamApplicationNotFound
        );
        self.application_list.retain(|&x| x != applicant);
        self.member_list.push(applicant);
        Ok(())
    }

    /// Rejects a player's team application and removes them from the application list.
    pub fn reject_team_application(&mut self, applicant: Pubkey) -> Result<()> {
        require!(
            self.is_application_list_contains(applicant),
            ErrorCode::TeamApplicationNotFound
        );
        self.application_list.retain(|&x| x != applicant);
        Ok(())
    }

    /// Transfers captaincy to another team member. The new captain is removed from the manager list if they are a manager.
    pub fn transfer_captaincy(&mut self, new_captain: Pubkey) -> Result<()> {
        require!(self.is_member(new_captain), ErrorCode::NotATeamMember);
        require!(!self.is_captain(new_captain), ErrorCode::AlreadyMember);
        self.manager_list.retain(|&x| x != new_captain);
        self.captain = new_captain;
        Ok(())
    }

    /// Grants manager privileges to an existing team member, if there's space in the manager list.
    pub fn grant_manager_privileges(&mut self, member: Pubkey) -> Result<()> {
        require!(self.is_member(member), ErrorCode::NotATeamMember);
        require!(!self.is_manager_list_full(), ErrorCode::TeamManagerListFull);
        require!(!self.is_manager(member), ErrorCode::TeamAlreadyManager);
        self.manager_list.push(member);
        Ok(())
    }

    /// Revokes manager privileges from a given manager.
    pub fn revoke_manager_privileges(&mut self, manager: Pubkey) -> Result<()> {
        require!(self.is_manager(manager), ErrorCode::ManagerNotFound);
        self.manager_list.retain(|&x| x != manager);
        Ok(())
    }

    /// Removes a member from the team, ensuring the captain cannot remove themselves.
    pub fn remove_member(&mut self, player: Pubkey) -> Result<()> {
        require!(self.is_member(player), ErrorCode::TeamMemberNotFound);
        require!(!self.is_captain(player), ErrorCode::TeamCaptainCannotLeave);
        self.member_list.retain(|&x| x != player);
        self.manager_list.retain(|&x| x != player);
        Ok(())
    }

    /// Updates the current period for the team and resets period-based ore counts if the period changes.
    pub fn update_current_period(&mut self, current_period_pubkey: Pubkey) {
        if self.current_period != current_period_pubkey {
            self.current_period = current_period_pubkey;
            self.current_period_purchased_ores = 0;
        }
    }

    /// Distributes a specified amount of team rewards if enough are available.
    pub fn distribute_team_rewards(&mut self, reward_amount: u64) -> Result<()> {
        require!(
            self.distributable_team_rewards >= reward_amount,
            ErrorCode::InsufficientTeamRewardBalance
        );
        self.distributable_team_rewards =
            self.distributable_team_rewards.safe_sub(reward_amount)?;
        self.distributed_team_rewards = self.distributed_team_rewards.safe_add(reward_amount)?;
        Ok(())
    }
}
