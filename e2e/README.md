# E2E tests

Playwright tests against `docker/docker-compose.test.yml`. Uses **yarn**.

- `specs/pos_flow.spec.ts` — API flow, JSON snapshot per step (`snapshots/pos_flow/`)
- `specs/ui_flow.spec.ts` — real wasm UI in a browser (new POS UI: icon nav, goods tiles, QR pay screen with 5-min countdown)
- `specs/phantom_extension.spec.ts` — real Chrome + real Phantom extension (headless, `channel: chromium` + `--headless=new`): runs the full onboarding (Create a New Wallet → Recovery Phrase Wallet → password + ToS), captures each step to `captures/phantom_extension/`, and asserts `window.phantom.solana` is injected into the POS UI page. Wallet profile persists in `e2e/.phantom/profile`; address saved to `e2e/.phantom/test-wallet-address.txt`. First run creates the wallet (~18s); later runs reuse it

Note: current Phantom versions no longer show the seed phrase during onboarding — backup is a later in-wallet task. `seed captured: 0 words` is expected.

## Run

```sh
cargo install trunk && rustup target add wasm32-unknown-unknown   # one-time
make ui-build                          # wasm bundle -> crates/ui/ui_wasm/dist
docker compose -f docker/docker-compose.test.yml up -d --build
docker run -d --name pos-ui-e2e -p 8081:80 \
  -v "$PWD/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro" nginx:alpine
cd e2e && yarn install && yarn e2e
docker compose -f docker/docker-compose.test.yml down -v
docker rm -f pos-ui-e2e
```

UI spec drives `http://localhost:8081/?api=<backend>` — the `?api=` param sets the backend URL (default `http://localhost:8080`).

`BACKEND` / `UI` / `POS_BACKEND_URL` / `POS_UI_URL` env vars override defaults. `UPDATE_SNAPSHOTS=1` regenerates API-step JSON snapshots.
