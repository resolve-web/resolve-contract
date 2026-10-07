#![no_std]

mod contract;
mod errors;
mod events;
mod payout;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use contract::ResolveContract;
pub use errors::Error;
pub use events::ClaimKind;
pub use types::{
    Market, MarketOutcome, MarketStatus, Outcome, Position, Side, MAX_DESCRIPTION_LEN,
    MAX_MARKET_DURATION_SECS, MAX_QUESTION_LEN, MAX_RESOLUTION_TIMEOUT_SECS,
    MIN_MARKET_DURATION_SECS, MIN_RESOLUTION_TIMEOUT_SECS,
};
