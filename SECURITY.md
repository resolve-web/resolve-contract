# Security Policy

## Status

Resolve contracts are **unaudited** open-source software. Treat them as experimental financial infrastructure.

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/resolve-web/resolve-contract/security/advisories/new) with:

- Affected repository and commit/tag
- Description of the issue and impact (funds at risk, permanent lock, auth bypass)
- Proof of concept if available (private)

Please do **not** open a public GitHub issue for fund-affecting bugs until coordinated disclosure is complete.

We aim to acknowledge reports within 72 hours.

## Scope

In scope: `contracts/resolve` payout math, authorization, state transitions, token accounting, event integrity assumptions.

Out of scope: issues solely in third-party wallets, RPC providers, or adversarial custom tokens that violate SEP-41 expectations (documented in README).
