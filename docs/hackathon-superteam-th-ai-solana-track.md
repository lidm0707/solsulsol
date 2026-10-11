# Superteam Thailand — AI × Solana Track (Colosseum Crypto World's Fair)

Submission prep for **solana-pos**.

## Bounty Facts

| Item | Value |
|---|---|
| Track | AI × Solana Track (Thailand) — Colosseum Crypto World's Fair |
| Sponsor | Superteam Thailand |
| Total Prizes | 10,000 USDG (1st 3,000 · 2nd 2,000 · 3rd 1,500 · 4th 1,000 · 5th 500 · 8× 250 bonus) |
| Region | Thailand only |
| Winner announcement | October 27, 2026 |
| Time left | ~5 days from listing date |
| Submissions so far | 10 |

## Required Actions (Eligibility)

- [ ] Register on Colosseum: https://colosseum.com/worldsfair — set country = **Thailand**
- [ ] Submit project on official Colosseum platform
- [ ] Submit same project on Superteam Earn
- [ ] Working demo with meaningful Solana integration
- [ ] Documentation: product, architecture, key integrations
- [ ] Codebase access for evaluation

## Submission Narrative (8 required sections)

Drafted for solana-pos (Solana point-of-sale with SPL token + PromptPay QR payments).

### 1. Problem
Small merchants (cafés, street food, retail shops in Thailand) juggle cash, PromptPay, and now crypto-paying customers. Existing crypto POS tools are Western-centric, require manual address handling, and give no invoice/VAT workflow.

### 2. User
Thai small-merchant owners and cashiers; crypto-holding customers (tourists + locals) who want to pay with USDC/USDG/SPL tokens.

### 3. Product
A full point-of-sale web app (Rust workspace: axum backend, Postgres/sqlx,
Dioxus/WASM frontend — also deployable as a Cloudflare Worker on D1) with:
- Wallet-only auth (Phantom/Solfare sign-in) + $1/mo USDC subscription
- Shop, product, and barcode-scanner sales flow
- Billing with VAT (none/exclusive/inclusive), hold-bill, PDF receipts
- **Fiat pricing**: goods priced in THB or USDT; merchant-set swap rate
  (THB per $1) converts at QR time — PromptPay QRs always in THB, Solana
  QRs carry the USD-token amount
- Payments: Solana Pay (USDC/USDT/SPL) and Thai PromptPay QR (EMVCo)
- E2E-tested UI (Playwright, recorded demo flows)

### 4. AI
> ⚠️ **Status: not implemented yet** — the repo currently ships no AI code.
> The bounty requires an AI feature demoed live; pick ONE of these and build
> it before submission:
- AI sales assistant: natural-language checkout ("2 iced lattes, one has oat milk") → cart actions
- AI receipt/invoice assistant: ask questions about sales data ("yesterday's revenue by category")
- AI agent payments: expose the POS as an MCP tool / x402-payable endpoint so AI agents can complete purchases autonomously
- Smart reconciliation: match onchain txs to invoices

### 5. Solana Layer
- SPL token payments (USDC/USDT) directly to merchant wallet — no custodian
- Solana Pay / QR transfer requests for customer-initiated payment, verified
  on-chain (destination, mint, amount) before a sale is marked paid
- Fiat interop: THB PromptPay QRs derived from the same THB/USD pricing via
  the merchant's swap rate
- x402 protocol for machine-to-machine payments (planned, per bounty resources)
- Fast, near-zero-fee settlement suited to small-ticket POS transactions

### 6. Distribution
- Thailand merchant communities (Line OA groups, markets, cafés)
- Superteam TH ecosystem + Build Stations for pilots
- PromptPay interop as the onramp: merchants keep existing habit, add crypto as new tender
- Open-source / self-host for Shopify-style plugin distribution later

### 7. Future
- Agent commerce hub: every shop becomes addressable by AI agents via x402/MCP
- Inventory + onchain settlement accounting
- Multi-chain → Solana-first expansion across SEA

### 8. Team
Fill in: names, roles, prior work, Thailand base, why this team ships.

## Resources

- Superteam TH: https://linktr.ee/superteam_TH
- Solana docs: https://solana.com/docs
- Colosseum: https://colosseum.com/worldsfair
- x402: https://solana.com/x402
- Colosseum rules: https://colosseum.com/hackathon#faqs

## Demo Checklist

- [x] 2–3 min demo video — `demo/demo_human_flow.mp4` (re-recorded on the
      fiat-pricing UI; AI voiceover confirmed OK per bounty comments)
- [x] Live POS flow: scan → cart → VAT → QR (PromptPay THB + Solana
      USDC/USDT), covered by e2e specs (`qr_types`, `currency_flow`,
      `demo_human_flow`)
- [x] Show onchain tx confirmation in merchant view — subscription payment
      flow verifies a real USDC transfer on-chain (surfpool in e2e, devnet/
      mainnet via `USDC_MINT` config)
- [ ] AI feature demoed live — **missing**; see section 4, must be built
