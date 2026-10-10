# E2E tests

Playwright tests against `docker/docker-compose.e2e.yml`. Uses **yarn**.

- `specs/pos_flow.spec.ts` — API flow, JSON snapshot per step (`snapshots/pos_flow/`)
- `specs/ui_flow.spec.ts` — real wasm UI in a browser (new POS UI: icon nav, goods tiles, QR pay screen with 5-min countdown)
- `specs/cloudflare_flow.spec.ts` — same API flow against the Cloudflare Worker under `docker/docker-compose.cloudflare-e2e.yml` (`wrangler dev` + local D1, port 18787). JSON snapshots per step to `snapshots/cloudflare_flow/`, captures (health response, worker-served UI screenshot) to `captures/cloudflare_flow/`. No PDF steps (pdf endpoints are non-wasm). See "Cloudflare worker e2e" below.
- `specs/phantom_extension.spec.ts` — real Chrome + real Phantom extension (headless, `channel: chromium` + `--headless=new`): runs the full onboarding (Create a New Wallet → Recovery Phrase Wallet → password + ToS), captures each step to `captures/phantom_extension/`, and asserts `window.phantom.solana` is injected into the POS UI page. Wallet profile persists in `e2e/.phantom/profile`; address saved to `e2e/.phantom/test-wallet-address.txt`. First run creates the wallet (~18s); later runs reuse it

Note: current Phantom versions no longer show the seed phrase during onboarding — backup is a later in-wallet task. `seed captured: 0 words` is expected.

## Run

```sh
make e2e                # hermetic: down -v, fresh stack + UI container, all non-cloudflare specs
make e2e-cloudflare     # hermetic: worker stack + UI container, cloudflare specs (runs ui-build first)
```

Both targets wipe state first (compose `down -v`) because subscriptions and
shop limits persist in the database. The stacks share surfpool port 8899 —
run them sequentially, never in parallel.

Manual setup (what the targets do):

```sh
cargo install trunk && rustup target add wasm32-unknown-unknown   # one-time
make ui-build                          # wasm bundle -> crates/ui/ui_wasm/dist
docker compose -f docker/docker-compose.e2e.yml up -d --build
docker run -d --name pos-ui-e2e -p 8081:80 \
  -v "$PWD/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro" nginx:alpine
cd e2e && yarn install && yarn e2e
docker compose -f docker/docker-compose.e2e.yml down -v
docker rm -f pos-ui-e2e
```

### Cloudflare worker e2e

```sh
make e2e-cloudflare            # ui-build + compose up + run specs/cloudflare_flow.spec.ts
make e2e-cloudflare-down       # teardown
```

Snapshots: `snapshots/cloudflare_flow/`, captures: `captures/cloudflare_flow/`
(`UPDATE_SNAPSHOTS=1` regenerates).

UI spec drives `http://localhost:8081/?api=<backend>` — the `?api=` param sets the backend URL (default `http://localhost:8080`).

`BACKEND` / `UI` / `POS_BACKEND_URL` / `POS_UI_URL` env vars override defaults. `UPDATE_SNAPSHOTS=1` regenerates API-step JSON snapshots.

### Wallet-auth specs

- All specs sign in through `/auth/nonce` → `/auth/verify` (ed25519 over the
  server message, base58). UI specs inject a mock Phantom; phantom_* specs
  drive the real extension in headless Chrome.
- Subscription: `POST /payments/orders` → on-chain USDC transfer on the
  private surfpool (`helpers/solana.ts`) → `POST /payments/verify`.
- `freshWallet()` (helpers/solana.ts) gives each run a throwaway identity;
  payments are still made by the shared `payer` keypair.
