-- D1 (SQLite) schema for the worker: equivalent to docker/migrations 001..007
-- with Postgres-specific syntax translated (TIMESTAMPTZ/now() -> TEXT,
-- ALTER COLUMN TYPE folded in, partial index kept as-is).

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'employee',
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS shops (
    id       TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    name     TEXT NOT NULL,
    vat_mode TEXT NOT NULL DEFAULT 'none',
    vat_rate NUMERIC(6, 4) NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS shop_members (
    shop_id TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role    TEXT NOT NULL DEFAULT 'employee',
    PRIMARY KEY (shop_id, user_id)
);

CREATE TABLE IF NOT EXISTS products (
    id      TEXT PRIMARY KEY,
    shop_id TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    name    TEXT NOT NULL,
    price   NUMERIC(20, 4) NOT NULL CHECK (price > 0),
    cost    NUMERIC(20, 4) NOT NULL DEFAULT 0,
    units   BIGINT NOT NULL DEFAULT 0,
    barcode TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS products_shop_barcode_idx
    ON products (shop_id, barcode)
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
    product_id TEXT NOT NULL REFERENCES products(id),
    name       TEXT NOT NULL,
    price      NUMERIC(20, 4) NOT NULL CHECK (price > 0),
    cost       NUMERIC(20, 4) NOT NULL DEFAULT 0,
    qty        INTEGER NOT NULL CHECK (qty > 0),
    PRIMARY KEY (invoice_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_members_user ON shop_members (user_id);
CREATE INDEX IF NOT EXISTS idx_products_shop ON products (shop_id);
CREATE INDEX IF NOT EXISTS idx_destinations_shop ON destinations (shop_id);
CREATE INDEX IF NOT EXISTS idx_invites_shop ON invites (shop_id);
CREATE INDEX IF NOT EXISTS idx_invoices_shop ON invoices (shop_id);
CREATE INDEX IF NOT EXISTS idx_lines_invoice ON invoice_lines (invoice_id);
