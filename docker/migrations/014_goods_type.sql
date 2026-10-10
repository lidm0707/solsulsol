-- 014: goods product type (service / food / electronic / ...), dropdown in UI.
ALTER TABLE goods ADD COLUMN goods_type TEXT NOT NULL DEFAULT 'other';
