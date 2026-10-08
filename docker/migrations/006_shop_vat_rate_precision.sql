-- VAT rate max is 100 → NUMERIC(5,4) caps at 9.9999; widen to hold 100.0000.
ALTER TABLE shops ALTER COLUMN vat_rate TYPE NUMERIC(6, 4);
ALTER TABLE invoices ALTER COLUMN vat_rate TYPE NUMERIC(6, 4);
