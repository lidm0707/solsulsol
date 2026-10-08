# Cloudflare Workers + D1 for the POS backend

How to run the backend as a Cloudflare Worker with D1 as the database.
This replaces the axum + sqlx/Postgres stack; the existing crates stay reusable
(see "What ports where" below).

## Why this shape

- `crates/api` (`pos-api`): DTOs, route types, error mapping — reusable as-is,
  compiles for `wasm32-unknown-unknown`.
- `crates/backend` ports (`src/port/`): the seams. On Workers you implement
  `UserRepo`, `ShopRepo`, `ShopMemberRepo`, `ProductRepo`, `DestinationRepo`,
  `InviteRepo`, `InvoiceRepo`, plus `encryption`/`qr` adapters over D1/JS APIs.
- Do **not** try to port sqlx/Postgres to Workers. D1 is SQLite-flavored, so SQL
  text differs (no `UUID` type — store `TEXT`, no `now()` — use
  `datetime('now')`).

## 1. Scaffold the worker crate

```sh
cargo new crates/cloudflare --lib
cd crates/cloudflare
cargo add worker --features d1
rustup target add wasm32-unknown-unknown
```

`crates/cloudflare/Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
worker = { version = "0.4", features = ["d1"] }
pos-api = { path = "../api" }
```

## 2. wrangler config

`crates/cloudflare/wrangler.toml`:

```toml
name = "pos-api"
main = "src/lib.rs"
compatibility_date = "2025-01-01"

[[d1_databases]]
binding = "DB"
database_name = "pos"
database_id = "<from step 3>"

[vars]
POS_ENV = "production"

[dev]
port = 8787
```

Create the database and export its id into the config:

```sh
npx wrangler d1 create pos
npx wrangler secret put POS_JWT_SECRET   # never in wrangler.toml
```

## 3. D1 schema + migrations

`crates/cloudflare/migrations/0001_init.sql` — same tables as the Postgres
`migrations`, with SQLite types:

```sql
CREATE TABLE users (
  id TEXT PRIMARY KEY,            -- uuid string
  email TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE shops (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL REFERENCES users(id),
  name TEXT NOT NULL
);

CREATE TABLE shop_members (
  shop_id TEXT NOT NULL REFERENCES shops(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  role TEXT NOT NULL CHECK (role IN ('owner','manager','cashier')),
  PRIMARY KEY (shop_id, user_id)
);

CREATE TABLE products (
  id TEXT PRIMARY KEY,
  shop_id TEXT NOT NULL REFERENCES shops(id),
  name TEXT NOT NULL,
  price_minor INTEGER NOT NULL,
  units INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE destinations (
  id TEXT PRIMARY KEY,
  shop_id TEXT NOT NULL REFERENCES shops(id),
  label TEXT NOT NULL,
  wallet TEXT NOT NULL
);

CREATE TABLE invites (
  id TEXT PRIMARY KEY,
  shop_id TEXT NOT NULL REFERENCES shops(id),
  email TEXT NOT NULL,
  role TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE invoices (
  id TEXT PRIMARY KEY,
  shop_id TEXT NOT NULL REFERENCES shops(id),
  items_json TEXT NOT NULL,       -- D1 has no arrays; store JSON
  total_minor INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'open',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Apply:

```sh
npx wrangler d1 migrations apply pos --remote   # prod
npx wrangler d1 migrations apply pos --local    # local dev
```

## 4. Router + repo impl

Minimal worker entry (`src/lib.rs`) using workers-rs; real routes mirror the
`pos-api` route definitions:

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

A repo example — `worker/src/d1_repos.rs`, implementing
`crate::backend::port::database::UserRepo` (adjust paths to your workspace):

```rust
use pos_backend::domain::User;
use pos_backend::port::database::UserRepo;
use worker::D1Database;

pub struct D1UserRepo {
    db: D1Database,
}

impl D1UserRepo {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait(?Send)]
impl UserRepo for D1UserRepo {
    async fn insert(&self, user: &User) -> Result<(), String> {
        self.db
            .prepare("INSERT INTO users (id, email, password_hash) VALUES (?1, ?2, ?3)")
            .bind(&[user.id.to_string().into(), user.email.as_str().into(), user.password_hash.as_str().into()])
            .map_err(|e| e.to_string())?
            .run()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, String> {
        let row = self
            .db
            .prepare("SELECT id, email, password_hash FROM users WHERE email = ?1")
            .bind(&[email.into()])
            .map_err(|e| e.to_string())?
            .first::<UserRow>(None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.map(UserRow::into_domain))
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<User>, String> {
        self.find_by_email(&id.to_string()).await // or query by id; placeholder
    }
}

#[derive(serde::Deserialize)]
struct UserRow {
    id: String,
    email: String,
    password_hash: String,
}

impl UserRow {
    fn into_domain(self) -> User {
        User {
            id: uuid::Uuid::parse_str(&self.id).expect("uuid"),
            email: self.email,
            password_hash: self.password_hash,
        }
    }
}
```

Gotchas:

- workers-rs futures are not `Send`, so trait impls need
  `#[async_trait::async_trait(?Send)]` while the port traits declare
  `Send + Sync` — you may need a `?Send` variant of the port traits, or
  `cfg(feature = "worker")` on the trait definitions.
- D1 `bind` takes `Vec<JsValue>`-like values; convert uuids with
  `.to_string()` and money as `i64` minor units.
- Batch related writes with `db.batch([...])` — D1 statements are cheap but
  round-trips are not.

## 5. Local dev / deploy

```sh
npx wrangler dev                     # local D1 + worker on :8787
npx wrangler d1 execute pos --local --command "SELECT count(*) FROM users"
npx wrangler deploy                  # prod
```

## 6. Front-end wiring (Pages UI → Worker API)

- Deploy the worker to `api.yourdomain.com` (add a route in wrangler.toml or a
  Custom Domain in the dashboard).
- Build the dioxus UI with `POS_API_BASE=https://api.yourdomain.com`, or serve
  the UI from the same zone and use a relative `/api` path with a zone-level
  route so requests stay same-origin.
- Enable CORS on the worker only if you must keep separate origins:

```rust
Response::empty()
    .map(|mut r| { r.headers_mut().set("Access-Control-Allow-Origin", "https://app.yourdomain.com")?; Ok(r) })
```

## 7. What replaces what

| Postgres stack            | Workers stack                    |
|---------------------------|----------------------------------|
| axum router               | workers-rs `Router`              |
| sqlx PgPool               | `env.d1("DB")`                   |
| uuid columns              | `TEXT` uuid strings              |
| `timestamptz now()`       | `TEXT datetime('now')`           |
| arrays / jsonb            | `TEXT` with JSON                 |
| bcrypt/argon2 (`tokio`)   | WebCrypto PBKDF2 (port `encryption`) |
| QR crate (`qrcode`)       | pre-render at build time, or a JS QR lib; port `qr` |
| PDF invoice               | client-side (UI) or external service |
| `POS_JWT_SECRET` env      | `wrangler secret put POS_JWT_SECRET` |

## Checklist

- [ ] `wrangler d1 create pos`, id pasted into `wrangler.toml`
- [ ] migrations applied `--local` and `--remote`
- [ ] `POS_JWT_SECRET` stored via `wrangler secret put`
- [ ] port traits compile with `?Send` variant for wasm
- [ ] invoice PDF path decided (client-side render recommended)
