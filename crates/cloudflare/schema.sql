-- D1 (SQLite) schema for the worker: equivalent to docker/migrations 001..007
-- with Postgres-specific syntax translated (TIMESTAMPTZ/now() -> TEXT,
-- ALTER COLUMN TYPE folded in, partial index kept as-is).

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    email         TEXT UNIQUE,
    role          TEXT NOT NULL DEFAULT 'employee',
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    wallet        TEXT
);
-- 010 note: D1 is wallet-only; legacy email/password columns from the old 001
-- block above require a full table rebuild in SQLite, so fresh databases are
-- created wallet-first instead (see migration 010 parity note at the end).

CREATE TABLE IF NOT EXISTS shops (
    id             TEXT PRIMARY KEY,
    owner_id       TEXT NOT NULL REFERENCES users(id),
    name           TEXT NOT NULL,
    vat_mode       TEXT NOT NULL DEFAULT 'none',
    vat_rate       NUMERIC(6, 4) NOT NULL DEFAULT 0,
    fx_thb_per_sol TEXT NOT NULL DEFAULT '0',
    currency       TEXT NOT NULL DEFAULT 'thb',
    deleted_at     TEXT
);

CREATE TABLE IF NOT EXISTS shop_members (
    shop_id TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role    TEXT NOT NULL DEFAULT 'employee',
    PRIMARY KEY (shop_id, user_id)
);

CREATE TABLE IF NOT EXISTS goods (
    id      TEXT PRIMARY KEY,
    shop_id TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    name    TEXT NOT NULL,
    price   NUMERIC(20, 4) NOT NULL CHECK (price > 0),
    cost    NUMERIC(20, 4) NOT NULL DEFAULT 0,
    units   BIGINT NOT NULL DEFAULT 0,
    barcode TEXT,
    icon    TEXT NOT NULL DEFAULT 'package',
    color   TEXT NOT NULL DEFAULT 'c01',
    goods_type TEXT NOT NULL DEFAULT 'other'
);

CREATE UNIQUE INDEX IF NOT EXISTS goods_shop_barcode_idx
    ON goods (shop_id, barcode)
    WHERE barcode IS NOT NULL;

CREATE TABLE IF NOT EXISTS destinations (
    id      TEXT PRIMARY KEY,
    shop_id TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    chain   TEXT NOT NULL,
    address TEXT NOT NULL,
    token   TEXT NOT NULL DEFAULT 'sol'
);

CREATE TABLE IF NOT EXISTS invites (
    id         TEXT PRIMARY KEY,
    shop_id    TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    email      TEXT NOT NULL,
    role       TEXT NOT NULL DEFAULT 'employee',
    status     TEXT NOT NULL DEFAULT 'pending',
    expires_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS invoices (
    id             TEXT PRIMARY KEY,
    shop_id        TEXT NOT NULL REFERENCES shops(id),
    status         TEXT NOT NULL DEFAULT 'pending',
    destination_id TEXT NOT NULL REFERENCES destinations(id),
    vat_mode       TEXT NOT NULL DEFAULT 'none',
    vat_rate       NUMERIC(6, 4) NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS invoice_lines (
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    goods_id   TEXT NOT NULL REFERENCES goods(id),
    name       TEXT NOT NULL,
    price      NUMERIC(20, 4) NOT NULL CHECK (price > 0),
    cost       NUMERIC(20, 4) NOT NULL DEFAULT 0,
    qty        INTEGER NOT NULL CHECK (qty > 0),
    PRIMARY KEY (invoice_id, goods_id)
);

CREATE INDEX IF NOT EXISTS idx_members_user ON shop_members (user_id);
CREATE INDEX IF NOT EXISTS idx_goods_shop ON goods (shop_id);
CREATE INDEX IF NOT EXISTS idx_destinations_shop ON destinations (shop_id);
CREATE INDEX IF NOT EXISTS idx_invites_shop ON invites (shop_id);
CREATE INDEX IF NOT EXISTS idx_invoices_shop ON invoices (shop_id);
CREATE INDEX IF NOT EXISTS idx_lines_invoice ON invoice_lines (invoice_id);

-- 008: SOL->THB rate for PromptPay QR amounts (column folded into the
-- CREATE TABLE above for idempotency)

-- 009: wallet auth + subscriptions + payment orders + storage accounting
-- (wallet column folded into the users CREATE TABLE above for idempotency)
CREATE UNIQUE INDEX IF NOT EXISTS users_wallet_idx ON users (wallet) WHERE wallet IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_shops_owner ON shops (owner_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_shops_deleted ON shops (deleted_at);

CREATE TABLE IF NOT EXISTS auth_nonces (
    id         TEXT PRIMARY KEY,
    wallet     TEXT NOT NULL,
    nonce      TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    used_at    TEXT
);
CREATE INDEX IF NOT EXISTS idx_nonces_wallet ON auth_nonces (wallet);

CREATE TABLE IF NOT EXISTS subscriptions (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL UNIQUE REFERENCES users(id),
    status      TEXT NOT NULL DEFAULT 'active',
    started_at  TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    grace_until TEXT
);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions (status, expires_at);

CREATE TABLE IF NOT EXISTS payment_orders (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    amount     TEXT NOT NULL,
    mint       TEXT NOT NULL,
    recipient  TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'pending',
    signature  TEXT,
    created_at TEXT NOT NULL,
    paid_at    TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS payment_orders_signature_idx
    ON payment_orders (signature) WHERE signature IS NOT NULL;

CREATE TABLE IF NOT EXISTS storage_files (
    shop_id TEXT NOT NULL,
    key     TEXT NOT NULL,
    bytes   INTEGER NOT NULL,
    PRIMARY KEY (shop_id, key)
);

CREATE TABLE IF NOT EXISTS storage_usage (
    shop_id TEXT PRIMARY KEY,
    bytes   INTEGER NOT NULL DEFAULT 0
);

-- 012: products renamed to goods — table/columns folded into the CREATE
-- statements above for idempotency (012_goods_rename.sql in Postgres).

-- 013: goods icon + pastel color — columns folded into the goods CREATE
-- statement above for idempotency (013_goods_icon_color.sql in Postgres).

-- 014: goods product type — column folded into the goods CREATE statement
-- above for idempotency (014_goods_type.sql in Postgres).

-- 010: wallet-only accounts — legacy password artifacts dropped in Postgres
-- (docker/migrations/010_drop_password.sql); in D1 the users table is created
-- wallet-first at the top of this file.

-- 011: shop display/settlement currency ('thb' default or 'usdt'); column
-- folded into the shops CREATE TABLE above for idempotency.
-- (the ALTER TABLE ADD COLUMN was removed — it broke fresh-DB init with
-- "duplicate column name: currency" since the CREATE above already has it)
