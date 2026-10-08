-- Goods stock: unit cost and on-hand units.

ALTER TABLE products
    ADD COLUMN IF NOT EXISTS cost  NUMERIC(20, 4) NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS units BIGINT NOT NULL DEFAULT 0;
