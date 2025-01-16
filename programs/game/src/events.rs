use anchor_lang::prelude::*;

#[event]
/// Represents a generic event emitted by the program, capturing various types of actions, their contexts, and origins.
/// This event standardizes how different game-related operations are logged on-chain.
pub struct TransferEvent {
    /// The category of the event, indicating what kind of action occurred.
    pub event_type: EventType,
    /// The nonce of the event, used to ensure unique event IDs.
    pub event_nonce: u32,
    /// Detailed data associated with the event, including involved accounts, amounts, and other parameters.
    pub data: EventData,
    /// Describes the type of the entity (system, player, team, stake pool) that initiated the event.
    pub initiator_type: InitiatorType,
    /// The public key of the entity (e.g., player or team) that triggered the event.
    pub initiator: Pubkey,
    /// A UNIX timestamp (in seconds) marking when the event took place.
    pub timestamp: u64,
}

/// Enumerates the different payload structures associated with each event.
/// Each variant corresponds to a particular action or state change within the game.
/// These payloads provide context and parameters needed to understand the recorded event.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum EventData {
    /// Emitted when the player's earnings are automatically reinvested into the game.
    AutoReinvest {
        game: Pubkey,
        round: Pubkey,
        period: Pubkey,
        player: Pubkey,
        team: Pubkey,
        purchased_ores: u32,
    },
    /// Emitted when developer rewards are collected from the game.
    CollectDeveloperRewards {
        game: Pubkey,
        developer_rewards: u64,
    },
    /// Emitted upon creating a new competition period.
    CreatePeriod { game: Pubkey, period: Pubkey },
    /// Emitted upon creating a new game round.
    CreateRound { game: Pubkey, round: Pubkey },
    /// Emitted when grand prizes are distributed at the end of a round or a special event.
    DistributeGrandPrizes {
        round: Pubkey,
        player: Pubkey,
        index: u8,
        grand_prizes: u64,
    },
    /// Emitted when leaderboard rewards are distributed, indicating which teams and players won.
    DistributeLeaderboardRewards {
        period: Pubkey,
        team_first: Pubkey,
        team_first_place_rewards: u64,
        team_second: Pubkey,
        team_second_place_rewards: u64,
        team_third: Pubkey,
        team_third_place_rewards: u64,
        player_leaderboard_winner: Pubkey,
        individual_rewards: u64,
    },
    /// Emitted when a default player entity is initialized.
    InitializeDefaultPlayer { player: Pubkey },
    /// Emitted when a default team entity is initialized with a given team number.
    InitializeDefaultTeam { team: Pubkey, team_number: u32 },
    /// Emitted when a stake pool is initialized, setting up a structure for staked tokens and rewards.
    InitializeStakeTokenPool { stake_pool: Pubkey },
    /// Emitted when a stake pool is initialized, setting up a structure for staked tokens and rewards.
    InitializeStakeVoucherPool { stake_pool: Pubkey, voucher: Pubkey },
    /// Emitted when a swap is initialized, setting up a structure for swapping tokens.
    InitializeVault {
        vault: Pubkey,
        token_mint: Pubkey,
        token_vault: Pubkey,
        token_amount: u64,
    },
    /// Emitted when a voucher (tokenized representation of deposited resources) is initialized.
    InitializeVoucher { voucher: Pubkey },
    /// Emitted when the game is initialized, marking the start of its operational context.
    Initialize { game: Pubkey },
    /// Emitted when a player cancels the auto-reinvest setting.
    CancelIsAutoReinvesting { player: Pubkey, round: Pubkey },
    /// Emitted when a player taps the candy machine.
    CandyTap {
        game: Pubkey,
        round: Pubkey,
        player: Pubkey,
        last_active_participant: Pubkey,
    },
    /// Emitted when collateral tokens are exchanged for another token or voucher.
    CollateralExchange {
        player: Pubkey,
        voucher: Pubkey,
        exchange_token_amount: u64,
        voucher_amount: u64,
    },
    /// Emitted when a player collects airdrop rewards allocated to them.
    CollectAirdropReward {
        game: Pubkey,
        player: Pubkey,
        airdrop_rewards: u64,
        voucher: Pubkey,
    },
    /// Emitted when a player collects rewards gained from consumption or spending activities.
    CollectConsumptionRewards {
        game: Pubkey,
        player: Pubkey,
        consumption_rewards: u64,
        voucher: Pubkey,
    },
    /// Emitted when a player collects referral rewards for inviting new participants.
    CollectReferralReward {
        game: Pubkey,
        player: Pubkey,
        referral_rewards: u64,
    },
    /// Emitted when the lottery is drawn, indicating the involved player, randomness provider, and bet amount.
    DrawLottery {
        game: Pubkey,
        player: Pubkey,
        randomness_provider: Pubkey,
        bet_amount: u64,
        voucher: Pubkey,
    },
    /// Emitted when a player exits the game or round, possibly collecting accrued rewards.
    Exit {
        game: Pubkey,
        player: Pubkey,
        round: Pubkey,
        team: Pubkey,
        available_ores: u32,
    },
    /// Emitted when a purchase occurs, logging details like the buyer, round, period, and any referral or team info.
    Purchase {
        game: Pubkey,
        player: Pubkey,
        round: Pubkey,
        period: Pubkey,
        referrer: Pubkey,
        team: Pubkey,
        purchased_ores: u32,
        voucher: Pubkey,
    },
    /// Emitted when a round ends, including information like the final call count and last call slot.
    RoundEnd {
        round: Pubkey,
        period: Pubkey,
        call_count: u8,
        last_call_slot: u64,
    },
    /// Emitted when a player registers for the game, optionally associated with a referrer.
    Register {
        player: Pubkey,
        referrer: Pubkey,
        voucher: Pubkey,
    },
    /// Emitted when a player reinvests their earned rewards back into the game.
    Reinvest {
        game: Pubkey,
        player: Pubkey,
        team: Pubkey,
        round: Pubkey,
        period: Pubkey,
        purchased_ores: u32,
    },
    /// Emitted after revealing the lottery result, providing the drawn symbols, multiplier, and earned lottery rewards.
    RevealDrawLotteryResult {
        game: Pubkey,
        player: Pubkey,
        symbols: [u8; 3],
        multiplier: u16,
        lottery_rewards: u64,
    },
    /// Emitted when auto-reinvesting is enabled for a player.
    SetIsAutoReinvesting { player: Pubkey, round: Pubkey },
    /// Emitted when a player's referrer is set or updated.
    SetReferrer { player: Pubkey, referrer: Pubkey },
    /// Emitted when the previous round is settled, distributing final rewards and clearing state.
    SettlePreviousRound {
        round: Pubkey,
        player: Pubkey,
        available_ores: u32,
        construction_rewards: u64,
    },
    /// Emitted when a player requests an early unstake of their staked tokens.
    RequestEarlyUnstake {
        player: Pubkey,
        stake_order: Pubkey,
        voucher: Pubkey,
        voucher_rewards: u64,
    },
    /// Emitted when a player stakes tokens, indicating the amount and associated stake order.
    Stake {
        player: Pubkey,
        stake_amount: u64,
        stake_order: Pubkey,
        stake_pool: Pubkey,
        annual_rate: u8,
        lock_duration: u64,
        token_rewards: u64,
        voucher_rewards: u64,
    },
    /// Emitted when a player unstakes tokens, finalizing the release of staked assets back to the player.
    Unstake {
        player: Pubkey,
        stake_order: Pubkey,
        stake_amount: u64,
        token_rewards: u64,
        voucher_rewards: u64,
        stake_pool: Pubkey,
    },
    Deposit {
        player: Pubkey,
        vault: Pubkey,
        token_amount: u64,
    },
    /// Emitted when a team application is accepted.
    AcceptTeamApplication { team: Pubkey, applicant: Pubkey },
    /// Emitted when a player applies to join a team.
    ApplyToJoinTeam { team: Pubkey, player: Pubkey },
    /// Emitted when a new team is created.
    CreateTeam { team: Pubkey, player: Pubkey },
    /// Emitted when team-level rewards are distributed to a specific member.
    DistributeTeamRewards {
        team: Pubkey,
        member: Pubkey,
        team_rewards: u64,
    },
    /// Emitted when a member is granted managerial privileges within a team.
    GrantManagerPrivileges { team: Pubkey, member: Pubkey },
    /// Emitted when a member voluntarily leaves a team.
    LeaveTeam { player: Pubkey, team: Pubkey },
    /// Emitted when a team application is rejected.
    RejectTeamApplication { team: Pubkey, applicant: Pubkey },
    /// Emitted when a member is forcibly removed from a team.
    RemoveMemberFromTeam { team: Pubkey, member: Pubkey },
    /// Emitted when a member's manager privileges are revoked.
    RevokeManagerPrivileges { team: Pubkey, manager: Pubkey },
    /// Emitted when the team captaincy is transferred to another member.
    TransferTeamCaptaincy {
        team: Pubkey,
        captain: Pubkey,
        new_captain: Pubkey,
    },
}

/// Classifies event types into a known set of categories, mirroring variants of `EventData`.
/// This enum helps in quickly identifying the nature of the event without parsing the entire payload.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum EventType {
    AutoReinvest,
    CollectDeveloperRewards,
    CreatePeriod,
    CreateRound,
    DistributeGrandPrizes,
    DistributeLeaderboardRewards,
    InitializeDefaultPlayer,
    InitializeDefaultTeam,
    InitializeStakeTokenPool,
    InitializeStakeVoucherPool,
    InitializeVault,
    InitializeVoucher,
    Initialize,
    CancelIsAutoReinvesting,
    CandyTap,
    CollateralExchange,
    CollectAirdropReward,
    CollectConsumptionRewards,
    CollectReferralReward,
    DrawLottery,
    Exit,
    Purchase,
    RoundEnd,
    Register,
    Reinvest,
    RevealDrawLotteryResult,
    SetIsAutoReinvesting,
    SetReferrer,
    SettlePreviousRound,
    RequestEarlyUnstake,
    Stake,
    Unstake,
    Deposit,
    AcceptTeamApplication,
    ApplyToJoinTeam,
    CreateTeam,
    DistributeTeamRewards,
    GrantManagerPrivileges,
    LeaveTeam,
    RejectTeamApplication,
    RemoveMemberFromTeam,
    RevokeManagerPrivileges,
    TransferTeamCaptaincy,
}

/// Identifies the nature of the entity initiating the event.
/// This helps contextualize actions as being triggered by the system, a player, a team, or a stake pool.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum InitiatorType {
    /// Indicates that the system or program logic initiated the event.
    SYSTEM,
    /// Indicates that a player (end-user) initiated the event.
    PLAYER,
    /// Indicates that a team (team entity) triggered the event.
    TEAM,
    /// Indicates that a lottery-related entity (lottery pool) triggered the event.
    LOTTERY,
    /// Indicates that a voucher-related entity (voucher) triggered the event.
    VOUCHER,
    /// Indicates that a staking-related entity (stake pool) initiated the event.
    STAKE,
    /// Indicates that a deposit-related entity (deposit) initiated the event.
    DEPOSIT,
}
