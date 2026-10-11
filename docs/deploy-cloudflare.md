# Deploying the POS stack with Cloudflare

Four paths, pick per environment:

1. [Cloudflare Pages — UI only](#1-cloudflare-pages--ui-only)
2. [Cloudflare Worker — API + UI in one deploy](#2-cloudflare-worker--api-ui-in-one-deploy)
3. [Link the Pages UI to the Worker API](#3-link-the-pages-ui-to-the-worker-api)
4. [Submission — Superteam TH Solana track](#4-submission--superteam-th-solana-track)

---

## 1. Cloudflare Pages — UI only

The Dioxus/WASM UI is a static bundle; ship it to Pages and point it at a
hosted API (Worker, see §2, or the axum backend behind a Tunnel).

### Build

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
cd crates/ui/ui_wasm
trunk build --release --dist dist
```

### Pages project

- Build command: `trunk build --release --dist dist`
- Build output directory: `crates/ui/ui_wasm/dist`
- Root directory: `crates/ui/ui_wasm` (or repo root if you wire trunk in CI)

The UI picks its API base at runtime from the `?api=` query parameter and falls
back to `/api` (`DEFAULT_BASE` in `ui_interface_app.rs`). No rebuild is needed
to change backends — see §3.

Cross-origin note: the axum backend enables `CorsLayer::permissive()`, so a
Pages-hosted UI can call a remote API directly. If you prefer same-origin,
proxy `/api/*` with a Pages Function (`[[path]].ts` calling
`env.BACKEND.fetch(...)` via a service binding).

### Custom domain (Pages)

Pages project → **Custom domains** → add e.g. `pay.yourdomain.com`. Cloudflare
provisions the DNS + cert automatically for zones on the same account.

---

## 2. Cloudflare Worker — API + UI in one deploy

`crates/cloudflare` (workers-rs + D1 + R2) serves the API **and** the UI:
the `[assets]` binding in `wrangler.example.toml` points at
`../ui/ui_wasm/dist`, and `/api/*` routes to the Rust handler (prefix stripped
so the shared axum router keeps its paths). One `wrangler deploy` ships the
whole app.

### First-time setup

```sh
rustup target add wasm32-unknown-unknown
npm install -g wrangler          # or use npx wrangler
wrangler login

cp crates/cloudflare/wrangler.example.toml crates/cloudflare/wrangler.toml
wrangler d1 create my-db         # put the id into wrangler.toml
wrangler r2 bucket create your-bucket-name
```

Build the UI first so the assets directory exists:

```sh
cd crates/ui/ui_wasm && trunk build --release --dist dist
```

### Configure `wrangler.toml` vars

| Var | Meaning | Default |
|---|---|---|
| `SOLANA_NETWORK` | cluster name | `mainnet-beta` |
| `SOLANA_RPC_URL` | JSON-RPC endpoint | `https://api.<network>.solana.com` |
| `USDC_MINT` | SPL mint accepted for payment | mainnet USDC (`EPjFW…t1v`) |
| `PAYMENT_RECIPIENT` | merchant wallet that receives USDC | **must set** |
| `POS_DOMAIN` | domain used in payment/nonce URLs | `localhost` |
| `NONCE_TTL_SECS` / `GRACE_DAYS` / `RECOVERY_DAYS` | auth + subscription tuning | see example |

Secrets never go in `wrangler.toml`:

```sh
wrangler secret put POS_JWT_SECRET
```

### Init remote D1 and deploy

```sh
cd crates/cloudflare
wrangler d1 execute my-db --remote --file schema.sql
wrangler deploy
```

Workers dev URL: `https://worker.<your-subdomain>.workers.dev`. `wrangler dev`
runs the same thing locally on `:8787` (init local D1 first with
`wrangler d1 execute my-db --local --file schema.sql`).

### Custom domain (Worker)

Worker → **Settings → Domains & Routes** → **Custom domain**, e.g.
`api.yourdomain.com` (or `pos.yourdomain.com` if it also serves the UI).
Requirements: zone on the same Cloudflare account; first deployment must exist
before attaching the domain. `workers.dev` can be disabled per-worker once the
custom domain is live.

---

## 3. Link the Pages UI to the Worker API

The UI resolves its API base as: `?api=<url>` query param → else `/api`
(same-origin). Three linking options, simplest first:

### a. Same origin via `/api` (recommended with the Worker assets deploy)

If the UI is served by the Worker itself (§2), the default `/api` base already
hits the API — nothing to configure. Same-origin, no CORS, no config.

### b. Cross-origin via `?api=` query param

Deploy UI on Pages (`pos-ui.pages.dev`), API on a Worker custom domain. Share
links like:

```
https://pos-ui.pages.dev/?api=https://api.yourdomain.com
```

The axum backend and the worker both send permissive CORS, so browser calls
work across origins. Wallet sign-in and Solana Pay QR need an https API base —
a custom domain on the worker (not `*.workers.dev` if you embed it in payment
URLs via `POS_DOMAIN`) is the clean setup.

### c. Same origin via Pages Functions proxy

Pages project file `functions/api/[[path]].ts`:

```ts
export const onRequest: PagesFunction<{ BACKEND: Fetcher }> = async (ctx) => {
  const url = new URL(ctx.request.url);
  url.pathname = url.pathname.replace(/^\/api/, "");
  return ctx.env.BACKEND.fetch(new Request(url, ctx.request));
};
```

With a service binding in the Pages project (`wrangler.toml` of the Pages
project or dashboard → Functions → Service bindings):

```toml
[[services]]
binding = "BACKEND"
service = "worker"          # the deployed worker name
environment = "production"
```

Now the Pages origin can call `/api/...` same-origin and the default `DEFAULT_BASE`
works with no query param.

---

## 4. Submission — Superteam TH Solana track

Full submission prep lives in
[`docs/hackathon-superteam-th-ai-solana-track.md`](hackathon-superteam-th-ai-solana-track.md)
(register on Colosseum with country = Thailand, submit there + Superteam Earn,
8-section narrative already drafted).

What to demo, in order:

1. **Wallet sign-in** — connect Phantom/Solflare, server verifies the
   `signMessage` ed25519 signature against a nonce (`/auth/nonce` →
   `/auth/verify`).
2. **Sell flow** — scan barcode / pick goods → cart with VAT → checkout.
3. **Solana Pay QR** — customer pays USDC to the merchant destination
   (`solana:<recipient>?amount=<amount>&spl-token=<mint>` QR, "open in wallet"
   link). On devnet this is a faucet-funded wallet; keep `SOLANA_NETWORK=devnet`
   and the devnet USDC mint in `wrangler.toml` for the demo.
4. **Onchain confirmation** — the worker verifies the transfer via
   `getTransaction` on RPC (`RpcPaymentVerifier`) and marks the payment order
   paid; show the paid state + signature in the merchant view.
5. **PromptPay fallback** — THB QR for non-crypto customers.

Submission links:

- Colosseum World's Fair: https://colosseum.com/worldsfair
- Superteam TH: https://linktr.ee/superteam_TH
- x402: https://solana.com/x402

Demo checklist (video, live tx confirmation, AI feature) is at the bottom of
the track doc.

---

## Self-hosted alternative (axum backend + Tunnel)

The backend needs Postgres + long-lived HTTP, so a Cloudflare Tunnel in front
of a VM/container is the pragmatic path:

1. Run the stack: `docker compose -f docker/docker-compose.yml up -d`.
2. `cloudflared tunnel create pos-backend && cloudflared tunnel route dns
   pos-backend api.yourdomain.com`.
3. `~/.cloudflared/config.yml` ingress: `api.yourdomain.com` →
   `http://localhost:8080`.
4. Set `POS_JWT_SECRET` to a strong random value before exposing.

### Checklist

- [ ] `POS_JWT_SECRET` set from a secret manager, not the default dev value
- [ ] `PAYMENT_RECIPIENT` set to a wallet you control (devnet for demos)
- [ ] Postgres backups enabled (managed Neon/Supabase, or `pg_dump` cron)
- [ ] Preview deployments point at a staging backend, not prod
