# solana-pos

Solana point-of-sale: Rust workspace with an axum backend (Postgres/sqlx),
Dioxus/WASM frontend, and Playwright e2e tests.

## Hackathon Submission

Entering the [AI × Solana Track](https://superteam.fun/earn/listing/ai-solana-track/)
— Superteam Thailand, Colosseum Crypto World's Fair ($10,000 total prizes in USDG).

**The Problem** — merchants at physical events and shops need fast, low-fee crypto
payments without dedicated POS hardware or complex onboarding.

**The User** — small merchants and street vendors accepting stablecoin payments.

**The Product** — a web-based point-of-sale that generates QR invoices for Solana
payments (see `qr-pay.png`, `sample-billing.pdf` for the invoice/receipt flow).

**The Solana Layer** — USDC/SPL stablecoin payments on Solana: near-instant
settlement, sub-cent fees, direct-to-merchant wallets.

**The AI Layer** — natural-language invoice creation and sales Q&A over the
merchant's transaction history (agents can also create/pay invoices as MCP tools).

**Distribution** — mobile-first web app, no install, merchant onboards with just a
Solana wallet.

## Stack

- `crates/backend` — axum app: domain → app services → ports → infra (http + sqlx)
- `crates/api` — shared request/response schema
- `crates/ui` — Dioxus WASM frontend (`ui_interface` ApiClient boundary)
- `docker/` — docker-compose + numbered Postgres migrations
- `e2e/` — Playwright suite

## Getting Started

```sh
make stack-up     # postgres + deps (docker)
make run          # backend
make ui-build     # trunk build of the WASM UI
make test         # all cargo tests
make e2e          # playwright suite (needs stack test compose)
```
