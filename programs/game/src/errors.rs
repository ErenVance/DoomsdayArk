use anchor_lang::prelude::*;

/// Represents the set of possible errors emitted by the program.
/// Each variant corresponds to a specific failure scenario, enabling more precise debugging and error handling.
#[error_code]
#[derive(PartialEq)]
pub enum ErrorCode {
    //-------------------------------------------------------------------------
    // Authorization and Access Errors
    //-------------------------------------------------------------------------
    /// Emitted when the provided authority does not match the expected authority.
    #[msg("Authority mismatch: the provided authority does not match the expected authority for this operation.")]
    AuthorityMismatch,

    /// Emitted when the caller lacks the required authorization level for the requested action.
    #[msg("The caller is not authorized to perform this action.")]
    NotAuthorized,

    //-------------------------------------------------------------------------
    // Round and Game State Errors
    //-------------------------------------------------------------------------
    /// Emitted when an operation is attempted on a round that has already concluded.
    #[msg("Round has already ended.")]
    RoundAlreadyEnded,

    /// Emitted when the round is still ongoing and has not ended yet. Please wait until the round concludes.
    #[msg(
        "The round is still ongoing and has not ended yet. Please wait until the round concludes."
    )]
    RoundInProgress,

    /// Emitted when an action requires the round to have started, but it has not begun yet.
    #[msg("Round has not started yet.")]
    RoundNotStarted,

    /// Emitted when a previous round must be settled before proceeding with the current action.
    #[msg("The previous round must be settled before proceeding with this action.")]
    NeedToSettlePreviousRound,

    /// Emitted when a player attempts an action after they have already exited the game or round.
    #[msg("Player has already exited the game.")]
    PlayerAlreadyExited,

    /// Emitted when an action requiring auto-reinvest functionality is invoked but the player has not enabled it.
    #[msg("Auto-reinvest is not enabled.")]
    AutoReinvestNotEnabled,

    //-------------------------------------------------------------------------
    // Randomness and Draw Errors
    //-------------------------------------------------------------------------
    /// Emitted when the provided randomness account is not valid for the required operation.
    #[msg("Invalid randomness account provided for this operation.")]
    InvalidRandomnessAccount,

    /// Emitted when the provided randomness value has passed its validity window and can no longer be used.
    #[msg("The provided randomness has expired.")]
    RandomnessExpired,

    /// Emitted when randomness was already revealed and cannot be revealed again.
    #[msg("The randomness has already been revealed.")]
    RandomnessAlreadyRevealed,

    /// Emitted when the randomness value has not yet been resolved and an action depends on its resolution.
    #[msg("The randomness has not yet been resolved.")]
    RandomnessNotResolved,

    //-------------------------------------------------------------------------
    // Resource and Balance Errors
    //-------------------------------------------------------------------------
    /// Emitted when an action requires a higher token or resource balance than is currently available.
    #[msg("Insufficient balance.")]
    InsufficientBalance,

    /// Emitted when the caller lacks enough funds to cover the associated fee for the requested action.
    #[msg("Insufficient funds to cover the associated fee for this action. Please ensure your account has enough balance.")]
    InsufficientFundsToPayFee,

    //-------------------------------------------------------------------------
    // Input Validation Errors
    //-------------------------------------------------------------------------
    /// Emitted when an input amount is invalid, such as being zero, negative, or exceeding limits.
    #[msg("Invalid amount.")]
    InvalidAmount,

    /// Emitted when a player or entity does not have the necessary funds for the requested transaction or operation.
    #[msg("Insufficient funds.")]
    InsufficientFunds,

    /// Emitted when a provided timestamp is outside the acceptable range or format.
    #[msg("Invalid timestamp.")]
    InvalidTimestamp,

    //-------------------------------------------------------------------------
    // Time-related Errors
    //-------------------------------------------------------------------------
    /// Emitted when a timestamp must be convertible from `i64` to `u64` but fails due to overflow or invalidity.
    #[msg("The provided timestamp could not be converted from i64 to u64.")]
    InvalidTimestampConversion,

    /// Emitted when the remaining rewards in a stake pool are insufficient to cover the rewards for a new order.
    #[msg("Insufficient remaining token rewards in the stake pool.")]
    InsufficientRemainingTokenRewards,

    /// Emitted when the remaining rewards in a stake pool are insufficient to cover the rewards for a new order.
    #[msg("Insufficient remaining voucher rewards in the stake pool.")]
    InsufficientRemainingVoucherRewards,

    //-------------------------------------------------------------------------
    // Reinvest Errors
    //-------------------------------------------------------------------------
    /// Emitted when the user's salary is insufficient to purchase any boxes.
    #[msg("Insufficient salary to purchase any boxes.")]
    InsufficientSalaryToPurchaseBoxes,

    /// Emitted when the user's salary is insufficient to perform an auto-reinvest operation.
    #[msg("Insufficient salary to auto-reinvest.")]
    InsufficientSalaryToAutoReinvest,

    /// Emitted when there are not enough rewards to reinvest.
    #[msg("Not enough rewards to reinvest.")]
    ReinvestNotEnoughRewards,

    //-------------------------------------------------------------------------
    // Developer Rewards Errors
    //-------------------------------------------------------------------------
    /// Emitted when there are no developer rewards available to collect.
    #[msg("There are no developer rewards available to collect.")]
    NoDeveloperRewardsAvailable,

    //-------------------------------------------------------------------------
    // Grand Prize Distribution Errors
    //-------------------------------------------------------------------------
    /// Emitted when all grand prize distributions have already been completed.
    #[msg("Grand prize distribution has already been completed and cannot be performed again.")]
    GrandPrizeDistributionAlreadyCompleted,

    /// Emitted when the specified grand prize distribution index is invalid.
    #[msg("The specified grand prize distribution index is invalid.")]
    InvalidGrandPrizeIndex,

    /// Emitted when the player address does not match the expected address for the given grand prize distribution index.
    #[msg(
        "The player address does not match the expected address for this grand prize distribution."
    )]
    PlayerAddressMismatch,

    //-------------------------------------------------------------------------
    // Cancel Auto Reinvesting Errors
    //-------------------------------------------------------------------------
    /// Emitted if the round's `auto_reinvesting_players` count is zero, indicating an inconsistency.
    #[msg("Insufficient number of players eligible for auto-reinvest.")]
    InsufficientAutoReinvestPlayers,

    //-------------------------------------------------------------------------
    // Candy Tap Errors
    //-------------------------------------------------------------------------
    /// Emitted if the last active participant is not the first participant in the list.
    #[msg("Last active participant is not the first participant in the list.")]
    WrongLastActiveParticipant,

    /// Emitted if the player has no ORE available to tap.
    #[msg("No ORE available to tap.")]
    NoOresAvailable,

    //-------------------------------------------------------------------------
    // Draw Lottery Errors
    //-------------------------------------------------------------------------
    /// Emitted if the player tries to draw another lottery before revealing the last result.
    #[msg("You need to reveal the last result before participating in this lottery.")]
    BeforeThisLotteryNeedToRevealLastResult,

    /// Emitted if the lottery pool is found empty or insufficient for a draw.
    #[msg("Lottery pool is empty.")]
    LotteryPoolIsEmpty,

    //-------------------------------------------------------------------------
    // Exit Errors
    //-------------------------------------------------------------------------
    /// Emitted if the player attempts to exit without holding any ORE, making the exit pointless.
    #[msg("Cannot exit without holding any ORE.")]
    DoNotNeedToExitWithoutOre,

    //-------------------------------------------------------------------------
    // Purchase Errors
    //-------------------------------------------------------------------------
    /// Emitted if the player attempts to purchase zero ORE, which is nonsensical.
    #[msg("Purchase quantity must be greater than 0.")]
    PurchaseQuantityMustGreaterThanZero,

    //-------------------------------------------------------------------------
    // Set Auto Reinvesting Errors
    //-------------------------------------------------------------------------
    /// Emitted if the player tries to enable auto-reinvest when it's already enabled.
    #[msg("Auto-reinvest is already enabled.")]
    AutoReinvestAlreadyEnabled,

    //-------------------------------------------------------------------------
    // Apply to Join Team Errors
    //-------------------------------------------------------------------------
    /// Emitted when the player attempts to apply to a team before their cooldown has expired.
    #[msg("Team join is on cooldown. Please try again later.")]
    TeamJoinCooldown,

    /// Emitted if the player has already applied to join the team.
    #[msg("Already applied to join the team.")]
    TeamApplicationAlreadyExists,

    //-------------------------------------------------------------------------
    // Grant Manager Privileges Errors
    //-------------------------------------------------------------------------
    /// Emitted if a captain tries to grant manager privileges to themselves.
    #[msg("Team cannot grant privileges to self.")]
    TeamCannotGrantSelf,

    /// Emitted if the target player is not a member of the team.
    #[msg("The target player is not a member of the team.")]
    NotATeamMember,

    /// Emitted if the player is already a member of the team.
    #[msg("Already a team member.")]
    AlreadyMember,

    //-------------------------------------------------------------------------
    // Remove Member From Team Errors
    //-------------------------------------------------------------------------
    /// Emitted if the caller attempts to remove themselves, which is not allowed.
    #[msg("Cannot remove yourself from the team.")]
    CannotRemoveSelf,

    /// Emitted if a manager attempts to remove another manager, a privilege reserved only for the captain.
    #[msg("Only the captain can remove a manager.")]
    RemoveManagerMustBeCaptain,

    //-------------------------------------------------------------------------
    // Revoke Manager Privileges Errors
    //-------------------------------------------------------------------------
    /// Emitted if the specified manager is not found in the manager list.
    #[msg("Manager not found.")]
    ManagerNotFound,

    //-------------------------------------------------------------------------
    // Transfer Team Captaincy Errors
    //-------------------------------------------------------------------------
    /// Emitted if the captain tries to transfer the role to themselves, which is nonsensical.
    #[msg("Cannot transfer captaincy to yourself.")]
    CantTransferToSelf,

    //-------------------------------------------------------------------------
    // Game Errors
    //-------------------------------------------------------------------------
    /// Emitted when the developer reward pool lacks enough funds to distribute expected rewards.
    #[msg("Insufficient balance in developer reward pool.")]
    InsufficientDeveloperRewardBalance,

    /// Emitted when the referrer reward pool does not have enough tokens to pay referral incentives.
    #[msg("Insufficient balance in referrer reward pool.")]
    InsufficientReferrerRewardBalance,

    /// Emitted when the team reward pool cannot cover the requested team rewards.
    #[msg("Insufficient balance in team reward pool.")]
    InsufficientTeamRewardBalance,

    /// Emitted when the registration reward pool is depleted and no more registration rewards can be granted.
    #[msg("Insufficient balance in registration reward pool.")]
    InsufficientRegistrationRewardBalance,

    /// Emitted when the airdrop reward pool is empty or insufficient for distributing airdrop rewards.
    #[msg("Insufficient balance in airdrop reward pool.")]
    InsufficientAirdropRewardBalance,

    /// Emitted when the consumption reward pool does not have the required funds.
    #[msg("Insufficient balance in consumption reward pool.")]
    InsufficientConsumptionRewardBalance,

    /// Emitted when attempting to distribute airdrop rewards that exceed the daily allocated cap, helping maintain
    /// controlled token emissions and economic balance.
    #[msg("Exceeds daily airdrop rewards cap.")]
    ExceedsDailyAirdropCap,

    //-------------------------------------------------------------------------
    // Period Errors
    //-------------------------------------------------------------------------
    /// Emitted when rewards have already been distributed and a second attempt is made.
    #[msg("Rewards have already been distributed.")]
    AlreadyDistributed,

    //-------------------------------------------------------------------------
    // Player Data Errors
    //-------------------------------------------------------------------------
    /// Emitted when a player attempts to refer themselves.
    #[msg("A player cannot refer themselves.")]
    CannotReferSelf,

    /// Emitted when the referrer for this player has already been set.
    #[msg("The referrer for this player has already been set.")]
    ReferrerAlreadySet,

    /// Emitted when there are no rewards available to collect.
    #[msg("No rewards available to collect.")]
    NoRewardsToCollect,

    /// Emitted when the player has already applied to join a team.
    #[msg("The player has already applied to join this team.")]
    PlayerAlreadyAppliedToThisTeam,

    /// Emitted when the player's team application list is full.
    #[msg("The player's team application list is full.")]
    PlayerTeamApplicationListFull,

    /// Emitted when no matching team application is found for the player.
    #[msg("No matching team application found for this player.")]
    PlayerTeamApplicationNotFound,

    /// Emitted when the player is not currently in a team.
    #[msg("The player is not currently in a team.")]
    PlayerIsNotInTeam,

    /// Emitted when the player has already collected today's airdrop rewards.
    #[msg("The player has already collected today's airdrop rewards.")]
    AirdropRewardsAlreadyCollected,

    /// Emitted when no airdrop rewards are available for the player to collect.
    #[msg("No airdrop rewards are available for the player to collect.")]
    AirdropRewardsNotAvailable,

    /// Emitted when the earnings per ore value did not increase as expected.
    #[msg("The earnings per ore value did not increase as expected.")]
    EarningsPerOreIsNotIncreased,

    //-------------------------------------------------------------------------
    // Stake Order Errors
    //-------------------------------------------------------------------------
    /// Emitted when the stake order is not found.
    #[msg("Stake order not found.")]
    StakeOrderNotFound,

    /// Emitted when the voucher balance is insufficient.
    #[msg("Insufficient voucher balance.")]
    InsufficientVoucherBalance,

    /// Emitted when the stake order has insufficient balance.
    #[msg("Insufficient stake order balance.")]
    StakeOrderInsufficientBalance,

    /// Emitted when the stake order cannot be unstaked yet.
    #[msg("Stake order cannot be unstaked yet.")]
    StakeOrderCannotUnstake,

    /// Emitted when an early unlock has already been requested.
    #[msg("Early unlock already requested.")]
    EarlyUnlockAlreadyRequested,

    /// Emitted when the stake order is already completed.
    #[msg("Stake order is already completed.")]
    StakeOrderAlreadyCompleted,

    /// Emitted when the stake order is already early unstaked.
    #[msg("Stake order is already early unstaked.")]
    StakeOrderAlreadyEarlyUnstaked,

    //-------------------------------------------------------------------------
    // Team Errors
    //-------------------------------------------------------------------------
    /// Emitted when the team is full.
    #[msg("Team is full.")]
    TeamFull,

    /// Emitted when the team application list is full.
    #[msg("Team application list is full.")]
    TeamApplicationListFull,

    /// Emitted when the team application is not found.
    #[msg("Team application not found.")]
    TeamApplicationNotFound,

    /// Emitted when the manager list is full.
    #[msg("Manager list is full.")]
    TeamManagerListFull,

    /// Emitted when the player is already a manager.
    #[msg("Player is already a manager.")]
    TeamAlreadyManager,

    /// Emitted when the team member is not found.
    #[msg("Team member not found.")]
    TeamMemberNotFound,

    /// Emitted when the captain cannot leave the team.
    #[msg("Captain cannot leave the team.")]
    TeamCaptainCannotLeave,
}
