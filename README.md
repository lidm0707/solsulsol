# solana-pos

Solana point-of-sale: Rust workspace with an axum backend (Postgres/sqlx),
Dioxus/WASM frontend, and Playwright e2e tests.

## Hackathon Submission

Entering the [AI × Solana Track](https://superteam.fun/earn/listing/ai-solana-track/)
— Superteam Thailand, Colosseum Crypto World's Fair ($10,000 total prizes in USDG).

**The Problem** — merchants at physical events and shops need fast, low-fee crypto
payments without dedicated POS hardware or complex onboarding.

**The User** — small merchants and street vendors accepting stablecoin payments.

**The Product** — a web-based point-of-sale for Thai merchants: goods with
barcodes, VAT, PDF receipts, and QR payments in **Solana stablecoins (USDC /
USDT) or Thai PromptPay**. Prices are in the shop's currency (THB or USD) and
generated QRs convert automatically via a merchant-set swap rate (see
[`manual/`](manual/) for the full user walkthrough, `qr-pay.png` and
`sample-billing.pdf` for the invoice/receipt flow).

**The Solana Layer** — USDC/SPL stablecoin payments on Solana: near-instant
settlement, sub-cent fees, direct-to-merchant wallets, wallet-only onboarding
(sign-in + $1/mo subscription paid as an on-chain USDC transfer, verified
on-chain).

**The AI Layer (roadmap)** — drafted for this track, not shipped yet: natural-
language checkout and sales Q&A over the merchant's history, with the POS
exposed to agents as MCP / x402 tools (see
[`docs/hackathon-superteam-th-ai-solana-track.md`](docs/hackathon-superteam-th-ai-solana-track.md)).

**Distribution** — mobile-first web app, no install, merchant onboards with just a
Solana wallet.

## Stack

- `crates/backend` — axum app: domain → app services → ports → infra (http + sqlx)
- `crates/cloudflare` — the same POS on Cloudflare Workers + D1 (reuses the
  shared core layers, only `src/infra/` is Cloudflare-specific)
- `crates/api` — shared request/response schema
- `crates/ui` — Dioxus WASM frontend (`ui_interface` ApiClient boundary)
- `crates/promptpay` / `qr_code` / `barcode` / `sol_wallet_adaptor` — QR
  encoders, scanners, and the on-chain verification client
- `docker/` — docker-compose + numbered Postgres migrations
- `e2e/` — Playwright suite; `demo/` — landing page, demo videos, manual
- [`manual/`](manual/) — illustrated user manual (one screenshot per step)

## Getting Started

```sh
make stack-up     # postgres + deps (docker)
make run          # backend
make ui-build     # trunk build of the WASM UI
make test         # all cargo tests
make e2e          # playwright suite (needs stack test compose)
```
