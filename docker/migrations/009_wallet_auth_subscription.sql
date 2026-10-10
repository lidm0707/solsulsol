-- 009: Solana wallet auth + USDC subscriptions.
-- Wallet-only accounts: email becomes optional; password_hash is unused
-- (kept as NULL-able for schema history).

ALTER TABLE users ADD COLUMN IF NOT EXISTS wallet TEXT UNIQUE;
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;

CREATE TABLE IF NOT EXISTS auth_nonces (
    id         UUID PRIMARY KEY,
    wallet     TEXT NOT NULL,
    nonce      TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_nonces_wallet ON auth_nonces(wallet);

CREATE TABLE IF NOT EXISTS subscriptions (
    id          UUID PRIMARY KEY,
    user_id     UUID NOT NULL UNIQUE REFERENCES users(id),
    status      TEXT NOT NULL DEFAULT 'active',
    started_at  TIMESTAMPTZ NOT NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    grace_until TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_subscriptions_expires ON subscriptions(expires_at);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions(status);

CREATE TABLE IF NOT EXISTS payment_orders (
    id         UUID PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id),
    amount     NUMERIC(20, 6) NOT NULL CHECK (amount > 0),
    mint       TEXT NOT NULL,
    recipient  TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'pending',
    signature  TEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    paid_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_orders_user ON payment_orders(user_id);

CREATE TABLE IF NOT EXISTS storage_files (
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    key     UUID NOT NULL,
    bytes   BIGINT NOT NULL CHECK (bytes >= 0),
    PRIMARY KEY (shop_id, key)
);

ALTER TABLE shops ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_shops_owner ON shops(owner_id);
