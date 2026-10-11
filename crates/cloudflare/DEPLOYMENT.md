# Deployment & Usage Guide (English)

How to run the POS worker (Cloudflare Workers + D1 + R2) on **devnet** for
testing, and what changes for mainnet. See also `README.md` in this crate.

## Prerequisites

```sh
rustup target add wasm32-unknown-unknown
npm install -g wrangler        # or use `npx wrangler ...`
wrangler login                 # opens browser, one time
```

## 1. Prepare wrangler.toml

Copy the committed template if you don't have a real config yet (the real
`wrangler.toml` is gitignored — never commit it):

```sh
cp wrangler.example.toml wrangler.toml
```

Then fill in:

| Field | How to get it |
|---|---|
| `database_id` | `wrangler d1 create my-db`, copy the printed id |
| `bucket_name` | `wrangler r2 bucket create <name>`, use the same name |
| `PAYMENT_RECIPIENT` | your wallet address (receives USDC payments) |
| `POS_DOMAIN` | the origin the UI is served from (must match at sign-in) |

Switch to devnet: comment out the three mainnet lines in `[vars]` and
uncomment the devnet ones (`SOLANA_NETWORK`, `SOLANA_RPC_URL`, `USDC_MINT`).

## 2. Create Cloudflare resources (once)

```sh
wrangler d1 create my-db                                  # put id in wrangler.toml
wrangler r2 bucket create my-bucket                       # match bucket_name
wrangler d1 execute my-db --remote --file schema.sql      # init remote D1
```

Re-run the `d1 execute` step after every `schema.sql` change — the worker has
no migration runner.

## 3. Deploy

```sh
wrangler deploy
```

Wrangler prints the worker URL (e.g. `https://worker.<subdomain>.workers.dev`).
The first build compiles the whole workspace to WASM and is slow; later
builds are incremental.

## 4. Verify

1. Open the worker URL in a browser.
2. Sign in with a Solana wallet (devnet).
3. Add goods, generate a payment QR, pay it, confirm the order flips to paid.

Devnet test funds:

- SOL for fees: `https://faucet.solana.com`
- devnet USDC (mint `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`): Circle
  devnet faucet or your own minted supply.

## Local development (no deploy)

```sh
wrangler d1 execute my-db --local --file schema.sql
wrangler dev --port 8787
```

`[vars]` apply to local mode too, so the same `wrangler.toml` serves both.
`wrangler dev` hot-reloads on Rust source changes. In Docker add
`--ip 0.0.0.0` (dev binds loopback).

## Going to mainnet later

1. In `[vars]`: restore the mainnet `SOLANA_NETWORK` / `SOLANA_RPC_URL` /
   `USDC_MINT` trio, set a real `POS_DOMAIN`, real `PAYMENT_RECIPIENT`.
2. Prefer a paid RPC provider over the public mainnet endpoint (rate limits).
3. Keep secrets out of `wrangler.toml` — use `wrangler secret put <NAME>`.
4. Redeploy; D1 data created on devnet does not carry over to a new database.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| Sign-in always fails | `POS_DOMAIN` does not match the origin the UI uses |
| Payment never confirms | `USDC_MINT` doesn't match the cluster, or payer lacks devnet USDC |
| `duplicate column name` on fresh D1 | someone added an `ALTER TABLE` instead of folding the column into the `CREATE` — see `schema.sql` conventions |
| Silent logs | `tracing` is a no-op in workers; use `worker::console_error!` / `console_log!` |
