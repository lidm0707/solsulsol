-- 010: wallet-only accounts — drop legacy password auth artifacts.
ALTER TABLE users DROP COLUMN IF EXISTS password_hash;
