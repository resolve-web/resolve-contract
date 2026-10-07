# Resolve Contract

Binary YES/NO prediction markets on Stellar (Soroban).

This repository is the **source of truth** for financial state and protocol rules. Applications should treat on-chain balances and claim results as authoritative; indexers and UIs are caches and conveniences only.

## What Resolve is

Users deposit a SEP-41 token into YES or NO for a market. After the staking deadline, a designated resolver reports the outcome. Winners divide the full market pool proportional to their winning-side stake. If a market cannot be resolved in time, anyone may invalidate it so participants can recover deposits.

## Architecture context

```text
Wallet → resolve-app → resolve-sdk → Stellar RPC → Resolve Contract → Token (SEP-41)
                              ↑
                    resolve-indexer (events only; not settlement authority)
```

Sibling repositories:

| Repo | Role |
|------|------|
| [resolve-sdk](../resolve-sdk) | TypeScript client |
| [resolve-indexer](../resolve-indexer) | Event indexer + query API |
| [resolve-app](../resolve-app) | Reference web application |

## Market lifecycle

```text
create_market
      │
      ▼
   Open  ──(ledger time ≥ close_at)──►  Awaiting resolution
      │                                      │
      │                                      ├─ resolve(Yes|No|Invalid) [resolver]
      │                                      │
      │                                      └─ invalidate [anyone, after timeout]
      ▼                                      ▼
 stake() allowed only while               Resolved | Invalid
 status=Open AND now < close_at                 │
                                                ▼
                                             claim()
```

`Awaiting resolution` is not a stored status: status remains `Open` until `resolve` or `invalidate` finalizes the market.

## Participation rule

A single address **may stake on both YES and NO**. Sides are tracked independently. On a normal Yes/No resolution, only the winning side is paid; the losing side is forfeited. On invalid/refund settlement, both sides are returned.

## Payout mathematics

For a resolved market with a non-empty winning pool:

```text
payout = floor(user_winning_stake * (yes_pool + no_pool) / winning_pool)
```

- All amounts are `i128` integers (no floating point).
- Division **floors** toward zero for positive amounts.
- Residual **dust stays in the contract** and is not claimable. This guarantees `sum(successful claims) ≤ total pool`.
- Claim order does not change any user's payout (each claim is independent floor division).

### Worked example

YES pool = 1000, NO pool = 500, outcome = YES. Alice staked 250 YES:

```text
payout = floor(250 * 1500 / 1000) = 375
```

### Zero-sided markets

If the market resolves YES but `yes_pool == 0` (only NO was staked), or resolves NO with `no_pool == 0`, there are no winners. Settlement switches to **full refund** of every position's deposits. This avoids division by zero and undefined payouts.

### Invalid / unresolved markets

| Path | Who | When | Funds |
|------|-----|------|-------|
| `resolve(..., Invalid)` | Designated resolver | After `close_at` | Full refund of deposits |
| `invalidate(...)` | Any authorized caller | After `close_at + resolution_timeout` | Full refund of deposits |

`resolution_timeout` is configured per market at creation (minimum 1 hour, maximum 30 days). A market may close at most 120 days after creation. These limits fit the full lifecycle inside the contract's 180-day persistent-storage TTL window and prevent a missing resolver from trapping funds forever.

## Contract interface

| Function | Auth | Description |
|----------|------|-------------|
| `__constructor` | deploy | Initializes market id counter |
| `create_market(creator, resolver, question, description, token, close_at, resolution_timeout) -> u64` | `creator` | Creates market, returns id |
| `stake(user, market_id, side, amount)` | `user` | Transfers tokens in, updates pools/position |
| `resolve(market_id, outcome)` | `resolver` | Finalizes Yes/No/Invalid after close |
| `invalidate(caller, market_id)` | `caller` | Permissionless finalize after timeout |
| `claim(user, market_id) -> i128` | `user` | Pays payout or refund once |
| `get_market` / `get_position` / `get_claimable` / `next_market_id` | none | Views |

## Authorization model

- Creators authorize market creation.
- Stakers authorize deposits (and the nested token `transfer`).
- Only the market's `resolver` address can `resolve`.
- Anyone can `invalidate` after the resolution timeout (must authorize as `caller`).
- Claimants authorize withdrawals to themselves.
- The contract never moves escrowed tokens except via `stake` (in) and `claim` (out).

## Security assumptions

- Settlement token implements SEP-41 correctly and is not adversarial in ways that break escrow (fee-on-transfer, rebasing, blacklist mid-flight). Prefer standard SAC assets.
- Resolvers are trusted for timely honest outcomes; economic recourse for bad resolution is off-chain / social. Invalidation only covers *non*-resolution, not disputed wrong outcomes.
- Ledger timestamp is the clock; validators set it within protocol rules.
- Contract is **not upgradeable** in v0.1.0 (no admin upgrade path). Redeploy for changes.
- This code is **unaudited**. Do not use with mainnet funds at scale without an audit.

## Prerequisites

- Rust 1.84+ (tested with 1.98)
- `wasm32v1-none` target: `rustup target add wasm32v1-none`
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli) for WASM builds and deploy

## Build

```bash
cargo test --manifest-path contracts/resolve/Cargo.toml
stellar contract build
# WASM: target/wasm32v1-none/release/resolve.wasm
```

Without Stellar CLI, native tests still run via `cargo test`.

## Configuration

See `.env.example` for deploy/invoke helper variables (not required for unit tests).

## Deploy (testnet sketch)

```bash
stellar keys generate deployer --network testnet --fund
stellar contract build
stellar contract deploy \
  --wasm target/wasm32v1-none/release/resolve.wasm \
  --source-account deployer \
  --network testnet
```

Record the contract id and set it in resolve-sdk / resolve-app / resolve-indexer env files.

For a reproducible deployment that also records the WASM hash and shared configuration:

```powershell
./scripts/deploy-testnet.ps1 -SourceAccount deployer -SettlementTokenId C...
./scripts/verify-testnet.ps1
```

Commit `deployments/testnet.json` after verifying it. The manifest is consumed by the SDK and provides one reviewable source for app and indexer configuration.

## Testing

```bash
cargo test --manifest-path contracts/resolve/Cargo.toml
```

Coverage includes creation validation, staking, both-sides positions, deadlines, resolution, invalidation, zero-sided refunds, rounding dust, double-claim, and pool invariants.

## License

Apache-2.0 — see [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).
