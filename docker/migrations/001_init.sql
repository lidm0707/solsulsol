-- POS schema v1: users, shops, members, products, destinations, invites, invoices.

CREATE TABLE IF NOT EXISTS users (
    id            UUID PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'employee',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS shops (
    id       UUID PRIMARY KEY,
    owner_id UUID NOT NULL REFERENCES users(id),
    name     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS shop_members (
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role    TEXT NOT NULL DEFAULT 'employee',
    PRIMARY KEY (shop_id, user_id)
);

CREATE TABLE IF NOT EXISTS products (
    id      UUID PRIMARY KEY,
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    name    TEXT NOT NULL,
    price   NUMERIC(20, 4) NOT NULL CHECK (price > 0)
);

CREATE TABLE IF NOT EXISTS destinations (
    id      UUID PRIMARY KEY,
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    chain   TEXT NOT NULL,
    address TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS invites (
    id         UUID PRIMARY KEY,
    shop_id    UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    email      TEXT NOT NULL,
    role       TEXT NOT NULL DEFAULT 'employee',
    status     TEXT NOT NULL DEFAULT 'pending',
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS invoices (
    id             UUID PRIMARY KEY,
    shop_id        UUID NOT NULL REFERENCES shops(id),
    status         TEXT NOT NULL DEFAULT 'pending',
    destination_id UUID NOT NULL REFERENCES destinations(id)
);

CREATE TABLE IF NOT EXISTS invoice_lines (
    invoice_id UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES products(id),
    name       TEXT NOT NULL,
    price      NUMERIC(20, 4) NOT NULL CHECK (price > 0),
    qty        BIGINT NOT NULL CHECK (qty > 0),
    PRIMARY KEY (invoice_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_members_user ON shop_members (user_id);
CREATE INDEX IF NOT EXISTS idx_products_shop ON products (shop_id);
CREATE INDEX IF NOT EXISTS idx_destinations_shop ON destinations (shop_id);
CREATE INDEX IF NOT EXISTS idx_invites_shop ON invites (shop_id);
CREATE INDEX IF NOT EXISTS idx_invoices_shop ON invoices (shop_id);
CREATE INDEX IF NOT EXISTS idx_lines_invoice ON invoice_lines (invoice_id);
