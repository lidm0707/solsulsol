-- Invoice lines snapshot unit cost at checkout for profit reporting.

ALTER TABLE invoice_lines
    ADD COLUMN IF NOT EXISTS cost NUMERIC(20, 4) NOT NULL DEFAULT 0;
