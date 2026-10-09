# Stellar Wave maintainer readiness

Resolve is a four-repository open-source Stellar project. Apply the repositories from the `resolve-web` GitHub organization to the Stellar Wave Program in this order:

1. `resolve-contract` — protocol and authoritative financial state
2. `resolve-sdk` — typed transaction and query integration
3. `resolve-indexer` — event discovery API
4. `resolve-app` — reference wallet application

## Verifiable project state

- Public Stellar testnet contract: `CD3YJNAYKVKT72DYPVS644OPNVNW6673TUIQWGXXA4VQD7536ARWB6MZ`
- Native testnet XLM asset contract: `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`
- Reproducible deployment manifest and WASM SHA-256: `deployments/testnet.json`
- Completed create → stake → resolve → claim evidence: `deployments/testnet-e2e.json`
- Contract CI: formatting, clippy with warnings denied, and 33 tests
- SDK CI: typecheck, dual ESM/CJS package build, and 44 tests
- Indexer CI: Node 22 build and 19 tests
- App CI: deterministic install, production Next.js build, typecheck, 15 tests, and a production dependency audit

Each repository is public, Apache-2.0 licensed, has contribution and security guidance, uses protected `main` with required CI checks, and has an actionable issue backlog. Financial behavior is documented explicitly, including resolver trust, invalidation, payout flooring, zero-sided refunds, storage TTL constraints, and the unaudited status.

## Public services

- App: `https://resolveit-app.vercel.app`
- Indexer health: `https://resolve-indexer.onrender.com/health`
- Indexer readiness: `https://resolve-indexer.onrender.com/ready`

Before submitting, verify all three URLs return successfully and complete the wallet flow in `resolve-app/docs/DEMO.md`.

## Drips onboarding

1. Sign in to the Drips Wave app with the GitHub account that administers `resolve-web`.
2. Open **Maintainers → Orgs and Repos** and install the Drips Wave GitHub App for `resolve-web`.
3. Sync the four public repositories and apply them to the Stellar Wave Program.
4. After approval, select well-scoped open issues and assign complexity in the Wave dashboard.
5. Review applications quickly, assign one contributor per issue, and require PRs to link their issue.

Repository approval is decided by the Stellar Wave organizers and cannot be guaranteed. If a repository is declined, use the in-app appeal after making substantive improvements and observing the applicable cooldown.
