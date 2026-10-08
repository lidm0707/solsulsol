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
A full point-of-sale web app (Rust workspace: axum backend, Postgres/sqlx, Dioxus/WASM frontend) with:
- Shop, product, and barcode-scanner sales flow
- Billing with VAT, hold-bill, bulk operations
- PDF invoices + QR payment
- Payments: SPL token transfer and PromptPay QR
- E2E-tested UI (Playwright)

### 4. AI
Where AI creates leverage (pick/confirm before submission):
- AI sales assistant: natural-language checkout ("2 iced lattes, one has oat milk") → cart actions
- AI receipt/invoice assistant: ask questions about sales data ("yesterday's revenue by category")
- AI agent payments: expose the POS as an MCP tool / x402-payable endpoint so AI agents can complete purchases autonomously
- Smart reconciliation: match onchain txs to invoices

### 5. Solana Layer
- SPL token payments (USDC/USDG) directly to merchant wallet — no custodian
- Solana Pay / QR transfer requests for customer-initiated payment
- x402 protocol for machine-to-machine payments (per bounty resources)
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

- [ ] 2–3 min demo video (AI voiceover confirmed OK per bounty comments)
- [ ] Live POS flow: scan → cart → VAT → pay with SPL token on devnet/mainnet
- [ ] Show onchain tx confirmation in merchant view
- [ ] AI feature demoed live
