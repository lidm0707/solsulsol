# pos-worker (Cloudflare Worker)

POS API built with [workers-rs](https://github.com/cloudflare/workers-rs) (Rust → WASM)
running on Cloudflare Workers, backed by **D1** (Cloudflare SQLite).

Reuses the shared workspace crates: `pos-app` (services), `pos-api` (schema),
`pos-http`, `pos-qr-code`, `pos-promptpay`, `pos-barcode`.

## Layout

```
src/lib.rs       worker entry, router setup
src/infra/       D1-backed repo adapters (the only Cloudflare-specific layer)
schema.sql       D1 schema
wrangler.toml    real config (do not commit secrets; never read local .env)
wrangler.example.toml    committed template
```

## Prerequisites

- Rust with the WASM target:
  ```
  rustup target add wasm32-unknown-unknown
  ```
- [wrangler](https://developers.cloudflare.com/workers/wrangler/) (npm):
  ```
  npm install -g wrangler
  ```
  (or use `npx wrangler ...`)

## Local dev (no Cloudflare account needed)

Local mode runs the Worker on `workerd`/miniflare on your machine with a local D1.

```sh
# 1. init the local D1 database
wrangler d1 execute my-db --local --file schema.sql

# 2. build + run
wrangler dev --port 8787
```

`wrangler dev` hot-reloads: it re-runs the `[build]` command (`worker-build`)
on Rust source changes.

First build is slow (compiles the whole workspace to WASM); subsequent builds
are incremental.

## Deploy

```sh
wrangler login
wrangler d1 create my-db        # once; put the id in wrangler.toml
wrangler d1 execute my-db --remote --file schema.sql
wrangler deploy
```

`wrangler.example.toml` is the template — copy it and fill in real ids for a
fresh environment. Keep secrets in `wrangler secret put <NAME>`, never in
`wrangler.toml`.

## Configuration conventions

- Config for local dev goes through `wrangler.toml` `[vars]` / secrets — no
  `.env` files here.
- New D1 migration? `schema.sql` is the full schema for fresh installs; append
  idempotent statements rather than editing existing ones.

## Pitfalls (found via e2e, see `.plans/16`)

- `tracing` is **silent** in the workers runtime — use `worker::console_error!`
  / `console_log!` for anything that must show under `wrangler dev`.
- `std::env` **panics on wasm32**. Shared crates must `#[cfg(not(target_arch =
  "wasm32"))]` any env access (e.g. `POS_SOLANA_CLUSTER` in pos-app).
- SQLite `NUMERIC` affinity returns decimals as JS numbers; D1 row structs
  deserialize decimals with the `num_str` helper (string-or-number), never
  plain `String`.
- Every pg migration in `docker/migrations/` must be mirrored into `schema.sql`
  the same day — the worker has no migration runner.
- `wrangler dev` binds loopback; in Docker pass `--ip 0.0.0.0`.
- The wasm build needs `pkg-config`, `libssl-dev`, `clang` (see
  `docker/Dockerfile.cloudflare-worker`).

## Notes

- This crate is not part of a normal Docker flow — it targets the Workers
  runtime, not a Linux binary. See `.plans/` for the docker/self-host
  discussion.
- `crate-type = ["cdylib"]` only: it has no `main`; don't try to `cargo run` it.
- Finish checklist (workspace-wide) still applies:
  `cargo check --workspace && cargo clippy --workspace --all-targets`
