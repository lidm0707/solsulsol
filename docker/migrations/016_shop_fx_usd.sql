-- Swap-rate semantics fix: the rate is THB per USD/USDT (goods are priced in
-- the shop currency, thb|usdt) — not THB per SOL as the old name implied.
ALTER TABLE shops RENAME COLUMN fx_thb_per_sol TO fx_thb_per_usd;
