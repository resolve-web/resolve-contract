use soroban_sdk::{contracttype, Address, String};

/// Maximum question length in bytes (UTF-8).
pub const MAX_QUESTION_LEN: u32 = 256;

/// Maximum description length in bytes (UTF-8).
pub const MAX_DESCRIPTION_LEN: u32 = 1024;

/// Minimum staking window: close_at must be at least this many seconds after creation.
pub const MIN_MARKET_DURATION_SECS: u64 = 60;

/// Maximum staking window (120 days). Together with the resolution window this
/// keeps the complete market lifecycle inside the network's maximum TTL.
pub const MAX_MARKET_DURATION_SECS: u64 = 120 * 24 * 60 * 60;

/// Maximum resolution timeout after close (30 days).
pub const MAX_RESOLUTION_TIMEOUT_SECS: u64 = 30 * 24 * 60 * 60;

/// Minimum resolution timeout after close (1 hour) so resolvers have a window
/// before permissionless invalidation is available.
pub const MIN_RESOLUTION_TIMEOUT_SECS: u64 = 3600;

/// Ledger TTL bump parameters (~30 day threshold, ~180 day extend-to).
pub const DAY_IN_LEDGERS: u32 = 17_280;
pub const INSTANCE_BUMP_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub const INSTANCE_BUMP_TO: u32 = 180 * DAY_IN_LEDGERS;
pub const PERSISTENT_BUMP_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub const PERSISTENT_BUMP_TO: u32 = 180 * DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Next market id counter (instance).
    NextMarketId,
    /// Market metadata and pools keyed by id (persistent).
    Market(u64),
    /// User position for (market_id, user) (persistent).
    Position(u64, Address),
}

/// On-chain market status. "Closed for staking" is derived from `close_at`
/// while status remains `Open`.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MarketStatus {
    Open,
    Resolved,
    Invalid,
}

/// Stake side. Resolve outcome reuses Yes/No; Invalid is only for resolution/invalidation.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Side {
    Yes,
    No,
}

/// Final market outcome. Prefer this over `Option` in storage — Soroban
/// `Option<custom enum>` encoding is unreliable across SDK versions.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MarketOutcome {
    /// Not yet finalized.
    Unset,
    Yes,
    No,
    Invalid,
}

impl MarketOutcome {
    pub fn from_resolve(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Yes => MarketOutcome::Yes,
            Outcome::No => MarketOutcome::No,
            Outcome::Invalid => MarketOutcome::Invalid,
        }
    }

    pub fn as_outcome(&self) -> Option<Outcome> {
        match self {
            MarketOutcome::Unset => None,
            MarketOutcome::Yes => Some(Outcome::Yes),
            MarketOutcome::No => Some(Outcome::No),
            MarketOutcome::Invalid => Some(Outcome::Invalid),
        }
    }
}

/// Argument / event outcome (never Unset).
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    Yes,
    No,
    /// Explicit invalid resolution by the designated resolver (before timeout).
    Invalid,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Market {
    pub id: u64,
    pub creator: Address,
    pub resolver: Address,
    pub question: String,
    pub description: String,
    /// SEP-41 token contract used for deposits and payouts.
    pub token: Address,
    pub created_at: u64,
    /// Unix timestamp after which staking is rejected.
    pub close_at: u64,
    /// Seconds after `close_at` before anyone may call `invalidate`.
    pub resolution_timeout: u64,
    pub yes_pool: i128,
    pub no_pool: i128,
    pub status: MarketStatus,
    /// `Unset` until resolved or invalidated.
    pub outcome: MarketOutcome,
    /// 0 means not finalized yet.
    pub finalized_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub yes_amount: i128,
    pub no_amount: i128,
    pub claimed: bool,
}

impl Position {
    pub fn empty() -> Self {
        Self {
            yes_amount: 0,
            no_amount: 0,
            claimed: false,
        }
    }

    pub fn total(&self) -> Result<i128, crate::errors::Error> {
        self.yes_amount
            .checked_add(self.no_amount)
            .ok_or(crate::errors::Error::Overflow)
    }

    pub fn is_empty(&self) -> bool {
        self.yes_amount == 0 && self.no_amount == 0
    }
}
