# Running the API + DB on Cloudflare: options and gotchas

Decision guide for hosting the POS API and its database on Cloudflare.
For the full Workers + D1 port walkthrough see `workers-d1.md`; for deploying
the existing axum stack see `deploy-cloudflare.md`.

## Can axum run on Workers?

No. Axum needs Tokio (threads, timers, real sockets); Workers runs Rust in V8
isolates via `workers-rs` with no Tokio and no `std::net`. Options:

| Option | Fit |
|---|---|
| `workers-rs` + D1 | native edge API, best free-tier fit — see `workers-d1.md` |
| Hyperdrive | keep external Postgres, pooled from Workers |
| Workers Containers | run axum unchanged in a container (supported path for axum) |
| Tunnel to VM | run current docker stack, expose via Cloudflare — see `deploy-cloudflare.md` |

Reusable across all options: `crates/api` (DTOs, route types) and the domain
crates compile for `wasm32-unknown-unknown` as-is.

## Workers + D1 (default)

```sh
npx wrangler d1 create pos
cargo add worker --features d1
```

`wrangler.toml`:

```toml
[[d1_databases]]
binding = "DB"
database_name = "pos"
database_id = "<id>"

[build]
command = "cargo install worker-build && worker-build --release"
```

Minimal router:

```rust
use worker::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let db = env.d1("DB")?;
    Router::new()
        .get_async("/health", |_req, _ctx| async { Response::ok("ok") })
        .run(req, env)
        .await
}
```

Schema notes: SQLite types — `TEXT` for uuids, `TEXT datetime('now')` for
timestamps, JSON text for arrays. Full schema in `workers-d1.md` §3.

## Workers + Hyperdrive (keep Postgres)

If the stack must stay on Postgres, bind it instead of migrating to D1:

```sh
npx wrangler hyperdrive create pos-pg \
  --connection-string="postgres://user:pass@host/db"
```

```toml
[[hyperdrive]]
binding = "PG"
id = "<id from create>"
```

Caveat: `sqlx` does not work on Workers (requires Tokio + real TCP). Connect
via Workers TCP sockets (`connect()`) with a wasm-compatible Postgres client,
or accept a TypeScript worker for these routes. If Postgres features (jsonb,
extensions, locking) are not required, D1 is the simpler path.

## Cargo / wasm gotchas

- No Tokio: avoid crates needing `TcpListener`, threads, `tokio::time`.
- workers-rs futures are `!Send` — port trait impls need
  `#[async_trait::async_trait(?Send)]`.
- 10 MB gzipped worker limit: `opt-level = "z"`, `strip = true`, lean deps.
- Batch D1 writes with `db.batch([...])`; round-trips dominate latency.

## Deploy

```sh
npx wrangler dev      # local worker + local D1
npx wrangler deploy
npx wrangler secret put POS_JWT_SECRET
```
