ALTER TABLE shops ALTER COLUMN fx_thb_per_usd SET DEFAULT 33;
UPDATE shops SET fx_thb_per_usd = 33 WHERE fx_thb_per_usd = 0;
