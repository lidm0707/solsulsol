-- Shop currency default becomes USD/USDT ('usdt'); new shops price in USD
-- unless the merchant picks THB in settings.
ALTER TABLE shops ALTER COLUMN currency SET DEFAULT 'usdt';
