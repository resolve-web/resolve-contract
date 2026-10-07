use crate::errors::Error;
use crate::types::{Market, MarketOutcome, MarketStatus, Outcome, Position};

/// Settlement mode for a finalized market.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Settlement {
    /// Winning side divides the full pool. Floor division; dust stays in contract.
    Payout { outcome: Outcome },
    /// Each participant recovers exactly their deposited amounts.
    Refund,
}

/// Determine settlement rules for a finalized market.
///
/// Rules:
/// - `Invalid` status or `MarketOutcome::Invalid` → full refund of deposits.
/// - Resolved YES with `yes_pool == 0` (only NO staked, or empty) → refund (no winners).
/// - Resolved NO with `no_pool == 0` → refund.
/// - Otherwise → proportional payout to the winning side.
pub fn settlement_for(market: &Market) -> Result<Settlement, Error> {
    match market.status {
        MarketStatus::Open => Err(Error::MarketNotResolved),
        MarketStatus::Invalid => Ok(Settlement::Refund),
        MarketStatus::Resolved => {
            let outcome = market
                .outcome
                .as_outcome()
                .ok_or(Error::MarketNotResolved)?;
            match outcome {
                Outcome::Invalid => Ok(Settlement::Refund),
                Outcome::Yes => {
                    if market.yes_pool == 0 {
                        Ok(Settlement::Refund)
                    } else {
                        Ok(Settlement::Payout {
                            outcome: Outcome::Yes,
                        })
                    }
                }
                Outcome::No => {
                    if market.no_pool == 0 {
                        Ok(Settlement::Refund)
                    } else {
                        Ok(Settlement::Payout {
                            outcome: Outcome::No,
                        })
                    }
                }
            }
        }
    }
}

/// Compute claimable amount for a position under the market's settlement rules.
///
/// Payout formula (integer floor division):
/// ```text
/// payout = user_winning_stake * (yes_pool + no_pool) / winning_pool
/// ```
/// Dust from flooring remains in the contract and is not claimable by anyone.
/// Refunds return `yes_amount + no_amount` exactly.
pub fn claimable_amount(market: &Market, position: &Position) -> Result<i128, Error> {
    if position.claimed {
        return Err(Error::AlreadyClaimed);
    }
    if position.is_empty() {
        return Err(Error::NoPosition);
    }

    match settlement_for(market)? {
        Settlement::Refund => position.total(),
        Settlement::Payout { outcome } => {
            let (user_win, win_pool) = match outcome {
                Outcome::Yes => (position.yes_amount, market.yes_pool),
                Outcome::No => (position.no_amount, market.no_pool),
                Outcome::Invalid => return Err(Error::InvalidOutcome),
            };

            if user_win == 0 {
                return Err(Error::NotWinner);
            }

            let total_pool = market
                .yes_pool
                .checked_add(market.no_pool)
                .ok_or(Error::Overflow)?;

            // Floor: (user_win * total_pool) / win_pool
            let numerator = user_win.checked_mul(total_pool).ok_or(Error::Overflow)?;
            Ok(numerator / win_pool)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MarketStatus;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn market_fixture(
        env: &Env,
        yes: i128,
        no: i128,
        status: MarketStatus,
        outcome: MarketOutcome,
    ) -> Market {
        Market {
            id: 1,
            creator: Address::generate(env),
            resolver: Address::generate(env),
            question: String::from_str(env, "q"),
            description: String::from_str(env, "d"),
            token: Address::generate(env),
            created_at: 0,
            close_at: 100,
            resolution_timeout: 3600,
            yes_pool: yes,
            no_pool: no,
            status,
            outcome,
            finalized_at: 200,
        }
    }

    #[test]
    fn payout_yes_proportional() {
        let env = Env::default();
        let m = market_fixture(&env, 1000, 500, MarketStatus::Resolved, MarketOutcome::Yes);
        let p = Position {
            yes_amount: 250,
            no_amount: 0,
            claimed: false,
        };
        // 250 * 1500 / 1000 = 375
        assert_eq!(claimable_amount(&m, &p).unwrap(), 375);
    }

    #[test]
    fn payout_floors_dust() {
        let env = Env::default();
        let m = market_fixture(&env, 3, 1, MarketStatus::Resolved, MarketOutcome::Yes);
        let p = Position {
            yes_amount: 1,
            no_amount: 0,
            claimed: false,
        };
        // 1 * 4 / 3 = 1 (floor); residual 1 unit of dust across three winners possible
        assert_eq!(claimable_amount(&m, &p).unwrap(), 1);
    }

    #[test]
    fn refund_on_invalid() {
        let env = Env::default();
        let m = market_fixture(&env, 100, 50, MarketStatus::Invalid, MarketOutcome::Invalid);
        let p = Position {
            yes_amount: 40,
            no_amount: 10,
            claimed: false,
        };
        assert_eq!(claimable_amount(&m, &p).unwrap(), 50);
    }

    #[test]
    fn zero_sided_yes_win_refunds() {
        let env = Env::default();
        let m = market_fixture(&env, 0, 1000, MarketStatus::Resolved, MarketOutcome::Yes);
        assert_eq!(settlement_for(&m).unwrap(), Settlement::Refund);
    }
}
