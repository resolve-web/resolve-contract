use soroban_sdk::{contract, contractimpl, token::TokenClient, Address, Env, String};

use crate::errors::Error;
use crate::events::{ClaimKind, Claimed, MarketCreated, MarketInvalidated, MarketResolved, Staked};
use crate::payout::{claimable_amount, settlement_for, Settlement};
use crate::storage::{
    bump_instance, get_market, get_position, next_market_id, save_market, save_position,
    set_next_market_id,
};
use crate::types::{
    Market, MarketOutcome, MarketStatus, Outcome, Position, Side, MAX_DESCRIPTION_LEN,
    MAX_MARKET_DURATION_SECS, MAX_QUESTION_LEN, MAX_RESOLUTION_TIMEOUT_SECS,
    MIN_MARKET_DURATION_SECS, MIN_RESOLUTION_TIMEOUT_SECS,
};

#[contract]
pub struct ResolveContract;

#[contractimpl]
impl ResolveContract {
    /// Deploy-time constructor. No admin — market creation is permissionless.
    pub fn __constructor(env: Env) {
        set_next_market_id(&env, 1);
        bump_instance(&env);
    }

    /// Create a binary prediction market.
    ///
    /// - `creator` must authorize.
    /// - `close_at` must be at least `MIN_MARKET_DURATION_SECS` after now.
    /// - `resolution_timeout` must be in
    ///   `[MIN_RESOLUTION_TIMEOUT_SECS, MAX_RESOLUTION_TIMEOUT_SECS]`.
    /// - A wallet may later stake on both YES and NO.
    pub fn create_market(
        env: Env,
        creator: Address,
        resolver: Address,
        question: String,
        description: String,
        token: Address,
        close_at: u64,
        resolution_timeout: u64,
    ) -> Result<u64, Error> {
        creator.require_auth();
        bump_instance(&env);

        if question.is_empty() || question.len() > MAX_QUESTION_LEN {
            return Err(Error::InvalidQuestion);
        }
        if description.len() > MAX_DESCRIPTION_LEN {
            return Err(Error::InvalidMarketConfig);
        }

        let now = env.ledger().timestamp();
        if close_at < now.saturating_add(MIN_MARKET_DURATION_SECS) {
            return Err(Error::InvalidMarketConfig);
        }
        if close_at > now.saturating_add(MAX_MARKET_DURATION_SECS) {
            return Err(Error::InvalidMarketConfig);
        }
        if !(MIN_RESOLUTION_TIMEOUT_SECS..=MAX_RESOLUTION_TIMEOUT_SECS)
            .contains(&resolution_timeout)
        {
            return Err(Error::InvalidMarketConfig);
        }

        let id = next_market_id(&env);
        let next = id.checked_add(1).ok_or(Error::Overflow)?;
        set_next_market_id(&env, next);

        let market = Market {
            id,
            creator: creator.clone(),
            resolver: resolver.clone(),
            question,
            description,
            token: token.clone(),
            created_at: now,
            close_at,
            resolution_timeout,
            yes_pool: 0,
            no_pool: 0,
            status: MarketStatus::Open,
            outcome: MarketOutcome::Unset,
            finalized_at: 0,
        };
        save_market(&env, &market);

        MarketCreated {
            market_id: id,
            creator,
            resolver,
            token,
            close_at,
            resolution_timeout,
        }
        .publish(&env);

        Ok(id)
    }

    /// Stake `amount` of the market's settlement token on `side`.
    ///
    /// Transfers tokens from `user` into this contract. Users may stake on both
    /// sides; each side is tracked independently.
    pub fn stake(
        env: Env,
        user: Address,
        market_id: u64,
        side: Side,
        amount: i128,
    ) -> Result<(), Error> {
        user.require_auth();
        bump_instance(&env);

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut market = get_market(&env, market_id)?;
        ensure_open_for_staking(&env, &market)?;

        match side {
            Side::Yes => {
                market.yes_pool = market.yes_pool.checked_add(amount).ok_or(Error::Overflow)?;
            }
            Side::No => {
                market.no_pool = market.no_pool.checked_add(amount).ok_or(Error::Overflow)?;
            }
        }

        let mut position = get_position(&env, market_id, &user);
        if position.claimed {
            return Err(Error::AlreadyClaimed);
        }
        match side {
            Side::Yes => {
                position.yes_amount = position
                    .yes_amount
                    .checked_add(amount)
                    .ok_or(Error::Overflow)?;
            }
            Side::No => {
                position.no_amount = position
                    .no_amount
                    .checked_add(amount)
                    .ok_or(Error::Overflow)?;
            }
        }

        // Effects before interaction with the token contract.
        save_market(&env, &market);
        save_position(&env, market_id, &user, &position);

        let token = TokenClient::new(&env, &market.token);
        token.transfer(&user, env.current_contract_address(), &amount);

        Staked {
            market_id,
            user,
            side,
            amount,
            yes_pool: market.yes_pool,
            no_pool: market.no_pool,
        }
        .publish(&env);

        Ok(())
    }

    /// Resolve a closed market. Only the designated resolver may call.
    ///
    /// `outcome` may be Yes, No, or Invalid (resolver-declared invalid).
    pub fn resolve(env: Env, market_id: u64, outcome: Outcome) -> Result<(), Error> {
        bump_instance(&env);
        let mut market = get_market(&env, market_id)?;

        if market.status != MarketStatus::Open {
            return Err(Error::AlreadyResolved);
        }

        market.resolver.require_auth();

        let now = env.ledger().timestamp();
        if now < market.close_at {
            return Err(Error::MarketStillOpen);
        }

        match outcome {
            Outcome::Yes | Outcome::No | Outcome::Invalid => {}
        }

        if outcome == Outcome::Invalid {
            market.status = MarketStatus::Invalid;
            market.outcome = MarketOutcome::Invalid;
        } else {
            market.status = MarketStatus::Resolved;
            market.outcome = MarketOutcome::from_resolve(outcome);
        }
        market.finalized_at = now;
        save_market(&env, &market);

        MarketResolved {
            market_id,
            resolver: market.resolver.clone(),
            outcome,
        }
        .publish(&env);

        Ok(())
    }

    /// Permissionless invalidation after `close_at + resolution_timeout`.
    /// Prevents unresolved markets from trapping funds forever.
    pub fn invalidate(env: Env, caller: Address, market_id: u64) -> Result<(), Error> {
        caller.require_auth();
        bump_instance(&env);

        let mut market = get_market(&env, market_id)?;
        if market.status != MarketStatus::Open {
            return Err(Error::AlreadyResolved);
        }

        let now = env.ledger().timestamp();
        if now < market.close_at {
            return Err(Error::DeadlineNotReached);
        }
        let invalidate_at = market
            .close_at
            .checked_add(market.resolution_timeout)
            .ok_or(Error::Overflow)?;
        if now < invalidate_at {
            return Err(Error::ResolutionTimeoutNotReached);
        }

        market.status = MarketStatus::Invalid;
        market.outcome = MarketOutcome::Invalid;
        market.finalized_at = now;
        save_market(&env, &market);

        MarketInvalidated {
            market_id,
            caller: caller.clone(),
        }
        .publish(&env);

        Ok(())
    }

    /// Claim payout (winner) or refund (invalid / zero-sided settlement).
    ///
    /// One claim per (market, user). Losing stakes on a resolved market with a
    /// non-empty winning pool receive nothing and still mark the position claimed
    /// only when a winning/refund amount is paid — losers with only losing side
    /// get `NotWinner`. Users with both sides who won on one side receive the
    /// winning payout only (losing side is forfeited).
    pub fn claim(env: Env, user: Address, market_id: u64) -> Result<i128, Error> {
        user.require_auth();
        bump_instance(&env);

        let market = get_market(&env, market_id)?;
        let mut position = get_position(&env, market_id, &user);

        let amount = claimable_amount(&env, &market, &position)?;
        let kind = match settlement_for(&market)? {
            Settlement::Refund => ClaimKind::Refund,
            Settlement::Payout { .. } => ClaimKind::Payout,
        };

        position.claimed = true;
        save_position(&env, market_id, &user, &position);

        let token = TokenClient::new(&env, &market.token);
        token.transfer(&env.current_contract_address(), &user, &amount);

        Claimed {
            market_id,
            user: user.clone(),
            amount,
            kind,
        }
        .publish(&env);

        Ok(amount)
    }

    // --- Views ---

    pub fn get_market(env: Env, market_id: u64) -> Result<Market, Error> {
        get_market(&env, market_id)
    }

    pub fn get_position(env: Env, market_id: u64, user: Address) -> Position {
        get_position(&env, market_id, &user)
    }

    /// Returns claimable amount if the market is finalized and the position is eligible.
    pub fn get_claimable(env: Env, market_id: u64, user: Address) -> Result<i128, Error> {
        let market = get_market(&env, market_id)?;
        let position = get_position(&env, market_id, &user);
        claimable_amount(&env, &market, &position)
    }

    pub fn next_market_id(env: Env) -> u64 {
        next_market_id(&env)
    }
}

fn ensure_open_for_staking(env: &Env, market: &Market) -> Result<(), Error> {
    if market.status != MarketStatus::Open {
        return Err(Error::MarketClosed);
    }
    if env.ledger().timestamp() >= market.close_at {
        return Err(Error::MarketClosed);
    }
    Ok(())
}
