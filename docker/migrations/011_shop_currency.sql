-- Shop display/settlement currency: 'thb' (default) or 'usdt'.
ALTER TABLE shops ADD COLUMN IF NOT EXISTS currency TEXT NOT NULL DEFAULT 'thb';
