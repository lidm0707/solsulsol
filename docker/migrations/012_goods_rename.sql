-- 012: rename products → goods (table, column, indexes) to match domain naming.

ALTER TABLE products RENAME TO goods;
ALTER INDEX products_shop_barcode_idx RENAME TO goods_shop_barcode_idx;
ALTER INDEX idx_products_shop RENAME TO idx_goods_shop;

ALTER TABLE invoice_lines RENAME COLUMN product_id TO goods_id;
