-- Shop VAT setting; invoices snapshot the rate/mode used at checkout.

ALTER TABLE shops
    ADD COLUMN IF NOT EXISTS vat_mode TEXT NOT NULL DEFAULT 'none',
    ADD COLUMN IF NOT EXISTS vat_rate NUMERIC(5, 4) NOT NULL DEFAULT 0;

ALTER TABLE invoices
    ADD COLUMN IF NOT EXISTS vat_mode TEXT NOT NULL DEFAULT 'none',
    ADD COLUMN IF NOT EXISTS vat_rate NUMERIC(5, 4) NOT NULL DEFAULT 0;
