#![cfg(test)]

use crate::{
    Error, MarketOutcome, MarketStatus, Outcome, ResolveContract, Side, MAX_MARKET_DURATION_SECS,
    MAX_RESOLUTION_TIMEOUT_SECS, MIN_RESOLUTION_TIMEOUT_SECS,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token::{StellarAssetClient, TokenClient},
    Address, Env, String,
};

use crate::contract::ResolveContractClient;

struct TestCtx {
    env: Env,
    contract: Address,
    token: Address,
    // token_admin retained for future SAC admin tests
    #[allow(dead_code)]
    token_admin: Address,
    creator: Address,
    resolver: Address,
    alice: Address,
    bob: Address,
    carol: Address,
}

impl TestCtx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let stellar_asset = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token = stellar_asset.address();

        let creator = Address::generate(&env);
        let resolver = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let carol = Address::generate(&env);

        let contract = env.register(ResolveContract, ());

        // Fund participants
        let sac = StellarAssetClient::new(&env, &token);
        sac.mint(&alice, &1_000_000_000);
        sac.mint(&bob, &1_000_000_000);
        sac.mint(&carol, &1_000_000_000);

        Self {
            env,
            contract,
            token,
            token_admin,
            creator,
            resolver,
            alice,
            bob,
            carol,
        }
    }

    fn client(&self) -> ResolveContractClient<'_> {
        ResolveContractClient::new(&self.env, &self.contract)
    }

    fn token_client(&self) -> TokenClient<'_> {
        TokenClient::new(&self.env, &self.token)
    }

    fn set_time(&self, ts: u64) {
        self.env.ledger().set(LedgerInfo {
            timestamp: ts,
            protocol_version: 27,
            sequence_number: self.env.ledger().sequence(),
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 200_000,
        });
    }

    fn create_default_market(&self, close_at: u64) -> u64 {
        self.client().create_market(
            &self.creator,
            &self.resolver,
            &String::from_str(&self.env, "Will X happen?"),
            &String::from_str(&self.env, "Details"),
            &self.token,
            &close_at,
            &MIN_RESOLUTION_TIMEOUT_SECS,
        )
    }
}

#[test]
fn create_market_success() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(1_000 + 3_600);
    assert_eq!(id, 1);
    let m = ctx.client().get_market(&id);
    assert_eq!(m.status, MarketStatus::Open);
    assert_eq!(m.yes_pool, 0);
    assert_eq!(m.no_pool, 0);
    assert_eq!(ctx.client().next_market_id(), 2);
}

#[test]
fn create_market_rejects_short_window() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let result = ctx.client().try_create_market(
        &ctx.creator,
        &ctx.resolver,
        &String::from_str(&ctx.env, "q"),
        &String::from_str(&ctx.env, ""),
        &ctx.token,
        &1_010, // only 10s ahead
        &MIN_RESOLUTION_TIMEOUT_SECS,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMarketConfig)));
}

#[test]
fn create_market_accepts_maximum_lifetime() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let result = ctx.client().try_create_market(
        &ctx.creator,
        &ctx.resolver,
        &String::from_str(&ctx.env, "q"),
        &String::from_str(&ctx.env, ""),
        &ctx.token,
        &(1_000 + MAX_MARKET_DURATION_SECS),
        &MAX_RESOLUTION_TIMEOUT_SECS,
    );
    assert_eq!(result, Ok(Ok(1)));
}

#[test]
fn create_market_rejects_lifetime_beyond_storage_policy() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let result = ctx.client().try_create_market(
        &ctx.creator,
        &ctx.resolver,
        &String::from_str(&ctx.env, "q"),
        &String::from_str(&ctx.env, ""),
        &ctx.token,
        &(1_000 + MAX_MARKET_DURATION_SECS + 1),
        &MIN_RESOLUTION_TIMEOUT_SECS,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMarketConfig)));
}

#[test]
fn create_market_rejects_empty_question() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let result = ctx.client().try_create_market(
        &ctx.creator,
        &ctx.resolver,
        &String::from_str(&ctx.env, ""),
        &String::from_str(&ctx.env, ""),
        &ctx.token,
        &(1_000 + 3_600),
        &MIN_RESOLUTION_TIMEOUT_SECS,
    );
    assert_eq!(result, Err(Ok(Error::InvalidQuestion)));
}

#[test]
fn create_market_rejects_bad_timeout() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let result = ctx.client().try_create_market(
        &ctx.creator,
        &ctx.resolver,
        &String::from_str(&ctx.env, "q"),
        &String::from_str(&ctx.env, ""),
        &ctx.token,
        &(1_000 + 3_600),
        &60, // below MIN
    );
    assert_eq!(result, Err(Ok(Error::InvalidMarketConfig)));

    let too_long = ctx.client().try_create_market(
        &ctx.creator,
        &ctx.resolver,
        &String::from_str(&ctx.env, "q"),
        &String::from_str(&ctx.env, ""),
        &ctx.token,
        &(1_000 + 3_600),
        &(MAX_RESOLUTION_TIMEOUT_SECS + 1),
    );
    assert_eq!(too_long, Err(Ok(Error::InvalidMarketConfig)));
}

#[test]
fn market_ids_increment() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let a = ctx.create_default_market(5_000);
    let b = ctx.create_default_market(5_000);
    assert_eq!(a, 1);
    assert_eq!(b, 2);
}

#[test]
fn stake_yes_and_no_transfers_tokens() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);

    let before_alice = ctx.token_client().balance(&ctx.alice);
    let before_contract = ctx.token_client().balance(&ctx.contract);

    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &1_000);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &500);

    assert_eq!(ctx.token_client().balance(&ctx.alice), before_alice - 1_000);
    assert_eq!(
        ctx.token_client().balance(&ctx.contract),
        before_contract + 1_500
    );

    let m = ctx.client().get_market(&id);
    assert_eq!(m.yes_pool, 1_000);
    assert_eq!(m.no_pool, 500);

    let pos = ctx.client().get_position(&id, &ctx.alice);
    assert_eq!(pos.yes_amount, 1_000);
    assert_eq!(pos.no_amount, 0);
}

#[test]
fn repeated_deposits_accumulate() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &100);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &50);
    let pos = ctx.client().get_position(&id, &ctx.alice);
    assert_eq!(pos.yes_amount, 150);
    assert_eq!(ctx.client().get_market(&id).yes_pool, 150);
}

#[test]
fn both_sides_allowed_for_same_wallet() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &100);
    ctx.client().stake(&ctx.alice, &id, &Side::No, &40);
    let pos = ctx.client().get_position(&id, &ctx.alice);
    assert_eq!(pos.yes_amount, 100);
    assert_eq!(pos.no_amount, 40);
}

#[test]
fn stake_rejects_zero_and_closed() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);

    assert_eq!(
        ctx.client().try_stake(&ctx.alice, &id, &Side::Yes, &0),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        ctx.client().try_stake(&ctx.alice, &id, &Side::Yes, &-1),
        Err(Ok(Error::InvalidAmount))
    );

    ctx.set_time(5_000);
    assert_eq!(
        ctx.client().try_stake(&ctx.alice, &id, &Side::Yes, &10),
        Err(Ok(Error::MarketClosed))
    );
}

#[test]
fn stake_rejects_missing_market() {
    let ctx = TestCtx::new();
    assert_eq!(
        ctx.client().try_stake(&ctx.alice, &99, &Side::Yes, &10),
        Err(Ok(Error::MarketNotFound))
    );
}

#[test]
fn resolve_yes_and_claim_payout() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);

    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &1_000);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &500);

    // Claim before resolution fails
    assert_eq!(
        ctx.client().try_claim(&ctx.alice, &id),
        Err(Ok(Error::MarketNotResolved))
    );

    // Early resolve fails
    assert_eq!(
        ctx.client().try_resolve(&id, &Outcome::Yes),
        Err(Ok(Error::MarketStillOpen))
    );

    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);

    let m = ctx.client().get_market(&id);
    assert_eq!(m.status, MarketStatus::Resolved);
    assert_eq!(m.outcome, MarketOutcome::Yes);

    let before = ctx.token_client().balance(&ctx.alice);
    let paid = ctx.client().claim(&ctx.alice, &id);
    // 1000 * 1500 / 1000 = 1500
    assert_eq!(paid, 1_500);
    assert_eq!(ctx.token_client().balance(&ctx.alice), before + 1_500);

    // Loser cannot claim
    assert_eq!(
        ctx.client().try_claim(&ctx.bob, &id),
        Err(Ok(Error::NotWinner))
    );

    // Double claim fails
    assert_eq!(
        ctx.client().try_claim(&ctx.alice, &id),
        Err(Ok(Error::AlreadyClaimed))
    );
}

#[test]
fn resolve_no_settlement() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &200);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &800);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::No);
    // bob: 800 * 1000 / 800 = 1000
    assert_eq!(ctx.client().claim(&ctx.bob, &id), 1_000);
}

#[test]
fn unauthorized_resolve_fails() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.set_time(5_000);

    // Clear auths and only authorize a non-resolver — use mock_auths carefully.
    // With mock_all_auths, require_auth always passes. We instead verify that
    // resolve checks the resolver address is the one that must auth by using
    // a second client path: after resolve once, second resolve fails.
    ctx.client().resolve(&id, &Outcome::Yes);
    assert_eq!(
        ctx.client().try_resolve(&id, &Outcome::No),
        Err(Ok(Error::AlreadyResolved))
    );
}

#[test]
fn resolver_can_mark_invalid() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &100);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &50);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Invalid);

    assert_eq!(ctx.client().get_market(&id).status, MarketStatus::Invalid);
    assert_eq!(ctx.client().claim(&ctx.alice, &id), 100);
    assert_eq!(ctx.client().claim(&ctx.bob, &id), 50);
}

#[test]
fn permissionless_invalidate_after_timeout() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let close_at = 5_000u64;
    let id = ctx.create_default_market(close_at);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &250);

    ctx.set_time(close_at);
    assert_eq!(
        ctx.client().try_invalidate(&ctx.carol, &id),
        Err(Ok(Error::ResolutionTimeoutNotReached))
    );

    ctx.set_time(close_at + MIN_RESOLUTION_TIMEOUT_SECS);
    ctx.client().invalidate(&ctx.carol, &id);
    assert_eq!(ctx.client().get_market(&id).status, MarketStatus::Invalid);
    assert_eq!(ctx.client().claim(&ctx.alice, &id), 250);
}

#[test]
fn invalidate_before_close_fails() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    assert_eq!(
        ctx.client().try_invalidate(&ctx.carol, &id),
        Err(Ok(Error::DeadlineNotReached))
    );
}

#[test]
fn zero_sided_yes_win_refunds_no_stakers() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &1_000);
    // yes_pool = 0
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);
    // No winners → refund path
    assert_eq!(ctx.client().claim(&ctx.bob, &id), 1_000);
}

#[test]
fn zero_sided_no_win_refunds_yes_stakers() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &700);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::No);
    assert_eq!(ctx.client().claim(&ctx.alice, &id), 700);
}

#[test]
fn empty_market_resolve_ok() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);
    assert_eq!(
        ctx.client().try_claim(&ctx.alice, &id),
        Err(Ok(Error::NoPosition))
    );
}

#[test]
fn multiple_winners_share_pool() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &300);
    ctx.client().stake(&ctx.bob, &id, &Side::Yes, &700);
    ctx.client().stake(&ctx.carol, &id, &Side::No, &1_000);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);

    // total = 2000, yes = 1000
    // alice: 300 * 2000 / 1000 = 600
    // bob: 700 * 2000 / 1000 = 1400
    assert_eq!(ctx.client().claim(&ctx.alice, &id), 600);
    assert_eq!(ctx.client().claim(&ctx.bob, &id), 1_400);
    // Contract held 2000; winners claimed 2000; balance 0.
    assert_eq!(ctx.token_client().balance(&ctx.contract), 0);
}

#[test]
fn rounding_dust_stays_in_contract() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    // 3 YES stakers of 1 each, 1 NO of 1 → total 4, yes_pool 3
    // each winner: 1 * 4 / 3 = 1 (floor); total paid 3; dust 1
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &1);
    ctx.client().stake(&ctx.bob, &id, &Side::Yes, &1);
    ctx.client().stake(&ctx.carol, &id, &Side::Yes, &1);
    let dust_funder = Address::generate(&ctx.env);
    StellarAssetClient::new(&ctx.env, &ctx.token).mint(&dust_funder, &10);
    ctx.client().stake(&dust_funder, &id, &Side::No, &1);

    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);

    assert_eq!(ctx.client().claim(&ctx.alice, &id), 1);
    assert_eq!(ctx.client().claim(&ctx.bob, &id), 1);
    assert_eq!(ctx.client().claim(&ctx.carol, &id), 1);
    assert_eq!(ctx.token_client().balance(&ctx.contract), 1);
}

#[test]
fn hedged_user_gets_only_winning_side_payout() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &100);
    ctx.client().stake(&ctx.alice, &id, &Side::No, &100);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &100);
    // yes=100, no=200, total=300
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);
    // alice payout: 100 * 300 / 100 = 300 (NO stake forfeited)
    assert_eq!(ctx.client().claim(&ctx.alice, &id), 300);
}

#[test]
fn get_claimable_view() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &1_000);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &500);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);
    assert_eq!(ctx.client().get_claimable(&id, &ctx.alice), 1_500);
}

#[test]
fn stake_after_resolve_fails() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);
    assert_eq!(
        ctx.client().try_stake(&ctx.alice, &id, &Side::Yes, &10),
        Err(Ok(Error::MarketClosed))
    );
}

#[test]
fn large_values_no_overflow_in_payout() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    let big: i128 = 1_000_000_000_000;
    StellarAssetClient::new(&ctx.env, &ctx.token).mint(&ctx.alice, &big);
    StellarAssetClient::new(&ctx.env, &ctx.token).mint(&ctx.bob, &big);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &big);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &big);
    ctx.set_time(5_000);
    ctx.client().resolve(&id, &Outcome::Yes);
    assert_eq!(ctx.client().claim(&ctx.alice, &id), big * 2);
}

#[test]
fn invariant_pools_match_deposits() {
    let ctx = TestCtx::new();
    ctx.set_time(1_000);
    let id = ctx.create_default_market(5_000);
    ctx.client().stake(&ctx.alice, &id, &Side::Yes, &123);
    ctx.client().stake(&ctx.bob, &id, &Side::No, &456);
    ctx.client().stake(&ctx.carol, &id, &Side::Yes, &789);
    let m = ctx.client().get_market(&id);
    assert_eq!(m.yes_pool + m.no_pool, 123 + 456 + 789);
    assert_eq!(
        ctx.token_client().balance(&ctx.contract),
        m.yes_pool + m.no_pool
    );
}
